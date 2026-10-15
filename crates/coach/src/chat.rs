//! Gespräch mit dem Coach (SPEC 9.7): eine Nachricht des Nutzers, eine geprüfte Antwort.
//!
//! Der Eingang ist der Kontext aus 9.3 ([`RefineInput`]) plus Kennzahlen als Code mit gerundeter
//! Zahl ([`MetricValue`]). Dazu kommen der Verlauf ([`ChatHistory`], nur Text des Nutzers und
//! Antworten des Modells) und die neue Nachricht. Der eigene Text des Nutzers ist der einzige
//! Freitext, der zum Modell geht (Entscheidung vom 15. Oktober 2026, SPEC 9.7); Zielbezeichnung,
//! Pfade, Namen und Notizen aus Dateien kennt keiner der Typen hier.
//!
//! Die Antwort ist JSON nach `schema/plan-chat.schema.json`: `reply` (Anzeigetext) und
//! optional `proposal`. Prüfung wie in [`crate::refine`]:
//!
//! - `plan_edit`: Schema, Katalog, Anwenden, `validate_plan`. Harte Verletzungen gehen als Codes
//!   in einen Korrekturaufruf (höchstens zwei).
//! - `planning_change`: nur die Grenzen (Schema und eine zweite Prüfung der Typen). Den neuen
//!   Plan erzeugt die App mit dem Regelplaner und prüft ihn dort.
//!
//! Bleibt der Vorschlag nach den Korrekturen ungültig, kommt die letzte lesbare Antwort ohne
//! Vorschlag zurück, mit den Codes der Fehler. Zeitüberschreitung und Netzfehler sind Fehler
//! ohne Korrekturaufruf. `reply` ist Anzeigetext und bleibt unverändert (SPEC 9.5): kein HTML,
//! kein Markdown, die Oberfläche setzt ihn als Text.

use std::collections::VecDeque;

use lastwerk_core::plan::{
    DayAvailability, Hardness, Locale, Plan, PlanContext, PlanSport, PlanWarning, Rules,
    validate_plan,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::client::{LlmClient, LlmError, LlmRequest, Message, Role};
use crate::edit::{self, Edit, Issue, PlanEdit, code, schema_issues, truncate_chars};
use crate::input::RefineInput;
use crate::refine::{MAX_CORRECTIONS, RefineConfig, base_notes, check_edits, correction_with};

/// Inhalt von `schema/plan-chat.schema.json`.
pub const CHAT_SCHEMA: &str = include_str!("../../../schema/plan-chat.schema.json");

/// Längste Antwort in Zeichen (Schema `maxLength`, SPEC 9.7). Längere Antworten werden gekürzt,
/// nicht abgelehnt (Konvention wie bei der Erklärung in 7.2).
pub const MAX_REPLY_CHARS: usize = 1200;

/// Höchstzahl der Nachrichten im Verlauf (SPEC 9.7). Ältere fallen weg.
pub const MAX_HISTORY_MESSAGES: usize = 12;

/// Längste Nachricht in Zeichen, im Verlauf und als neue Nachricht (SPEC 9.7).
pub const MAX_MESSAGE_CHARS: usize = 1000;

/// Höchstzahl der Kennzahlen im Eingang (Konvention). Das Dashboard (T25) hat weit weniger;
/// die Grenze hält den Eingang klein, falls ein Aufrufer Reihen statt Werte übergibt.
pub const MAX_METRICS: usize = 40;

/// Längster Code einer Kennzahl in Zeichen (Konvention).
pub const MAX_METRIC_CODE_CHARS: usize = 40;

/// Längster Teil einer früheren Antwort, der in einen Korrekturaufruf zurückgeht (Konvention,
/// wie in `refine`).
const MAX_ECHO_CHARS: usize = 4000;

const CHAT_DE: &str = include_str!("../prompts/chat.de.txt");
const CHAT_EN: &str = include_str!("../prompts/chat.en.txt");
const CHAT_CORRECTION_DE: &str = include_str!("../prompts/chat_correction.de.txt");
const CHAT_CORRECTION_EN: &str = include_str!("../prompts/chat_correction.en.txt");

/// Fehler vor oder während eines Gesprächsschritts.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChatError {
    #[error("the message is empty")]
    EmptyMessage,
    #[error("the message is longer than {max} characters")]
    MessageTooLong { max: usize },
    #[error("a metric has an invalid code, a value that is not finite or appears twice")]
    InvalidMetric,
    #[error("more than {max} metrics")]
    TooManyMetrics { max: usize },
    #[error("the model did not answer in time")]
    Timeout { calls: u32 },
    #[error("the model server is not reachable: {error}")]
    Transport { error: LlmError, calls: u32 },
    /// Auch nach den erlaubten Korrekturen keine lesbare Antwort (kein JSON, kein `reply`).
    /// Die Fehler der letzten Antwort als Codes.
    #[error("the answer of the model was unusable")]
    InvalidOutput { issues: Vec<Issue>, calls: u32 },
}

impl ChatError {
    /// Stabiler Fehlercode für Oberfläche und Protokolle.
    pub fn code(&self) -> &'static str {
        match self {
            Self::EmptyMessage => "empty_message",
            Self::MessageTooLong { .. } => "message_too_long",
            Self::InvalidMetric => "invalid_metric",
            Self::TooManyMetrics { .. } => "too_many_metrics",
            Self::Timeout { .. } => "timeout",
            Self::Transport { .. } => "transport",
            Self::InvalidOutput { .. } => "invalid_output",
        }
    }
}

/// Eine Kennzahl des Dashboards (T25) als fester Code mit einer Zahl, auf eine Nachkommastelle
/// gerundet (SPEC 9.7). Nur über [`MetricValue::new`] zu bekommen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetricValue {
    code: String,
    value: f64,
}

/// Erlaubter Code: `[a-z0-9_]{1,40}`. So kann kein Freitext als Code in den Eingang gelangen.
fn is_metric_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_METRIC_CODE_CHARS
        && code
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl MetricValue {
    /// Prüft den Code und rundet den Wert auf eine Nachkommastelle. `None` bei unzulässigem
    /// Code oder nicht endlichem Wert.
    pub fn new(code: &str, value: f64) -> Option<Self> {
        if !is_metric_code(code) || !value.is_finite() {
            return None;
        }
        // `+ 0.0` macht aus -0,0 eine 0,0, damit im Eingang keine "-0.0" steht.
        let rounded = (value * 10.0).round() / 10.0 + 0.0;
        Some(Self {
            code: code.to_string(),
            value: rounded,
        })
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

/// Eingang eines Gesprächsschritts: der Kontext aus 9.3 und die Kennzahlen. Hält zusätzlich
/// Basisplan, Kontext und Regeln für die Prüfung der Vorschläge; diese gehen nicht in den
/// JSON-Text.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatInput {
    #[serde(flatten)]
    context: RefineInput,
    metrics: Vec<MetricValue>,
    #[serde(skip)]
    base: Plan,
    #[serde(skip)]
    ctx: PlanContext,
    #[serde(skip)]
    rules: Rules,
}

impl ChatInput {
    /// Baut den Eingang aus Basisplan, Kontext, Regeln und Kennzahlen. Mehr als
    /// [`MAX_METRICS`] Kennzahlen oder ein Code, der doppelt vorkommt, sind Fehler.
    pub fn build(
        base: &Plan,
        ctx: &PlanContext,
        rules: &Rules,
        metrics: Vec<MetricValue>,
    ) -> Result<Self, ChatError> {
        if metrics.len() > MAX_METRICS {
            return Err(ChatError::TooManyMetrics { max: MAX_METRICS });
        }
        for (i, m) in metrics.iter().enumerate() {
            if metrics[..i].iter().any(|o| o.code == m.code) {
                return Err(ChatError::InvalidMetric);
            }
        }
        Ok(Self {
            context: RefineInput::build(base, ctx, rules),
            metrics,
            base: base.clone(),
            ctx: ctx.clone(),
            rules: rules.clone(),
        })
    }

    /// Der Kontext aus 9.3.
    pub fn context(&self) -> &RefineInput {
        &self.context
    }

    pub fn metrics(&self) -> &[MetricValue] {
        &self.metrics
    }

    /// Der Plan, auf den sich `plan_edit` bezieht.
    pub fn base(&self) -> &Plan {
        &self.base
    }

    /// Der Eingang als JSON-Text, genau so, wie er zum Modell geht (kompakt, eine Zeile).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ChatInput hat nur serialisierbare Felder")
    }
}

/// Wer eine Nachricht im Verlauf geschrieben hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
}

/// Eine Nachricht im Verlauf: Text des Nutzers oder `reply` des Modells, nie ein Vorschlag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
}

/// Verlauf des Gesprächs (SPEC 9.7): höchstens [`MAX_HISTORY_MESSAGES`] Nachrichten, je
/// höchstens [`MAX_MESSAGE_CHARS`] Zeichen, die ältesten fallen weg. Lebt nur im Speicher des
/// Aufrufers; `coach` speichert nichts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatHistory {
    messages: VecDeque<ChatMessage>,
}

impl ChatHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hängt eine Nachricht an. Steuerzeichen außer Zeilenumbruch und Tabulator werden zu
    /// Leerzeichen, der Text wird auf [`MAX_MESSAGE_CHARS`] gekürzt. Leere Texte entfallen.
    pub fn push(&mut self, role: ChatRole, text: &str) {
        let text = truncate_chars(&clean_text(text), MAX_MESSAGE_CHARS);
        if text.is_empty() {
            return;
        }
        self.messages.push_back(ChatMessage { role, text });
        while self.messages.len() > MAX_HISTORY_MESSAGES {
            self.messages.pop_front();
        }
    }

    /// Hängt die Nachricht des Nutzers und die Antwort des Modells aus `outcome` an.
    pub fn record(&mut self, user_text: &str, outcome: &ChatOutcome) {
        self.push(ChatRole::User, user_text);
        self.push(ChatRole::Assistant, &outcome.reply);
    }

    pub fn messages(&self) -> impl Iterator<Item = &ChatMessage> {
        self.messages.iter()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

/// Neue Eingaben für den Regelplaner. Nur die Felder, die sich ändern sollen; mindestens eines
/// ist gesetzt. Die Grenzen sind geprüft (Verfügbarkeit 0 bis 1440 Minuten je Tag, Sportarten
/// nur Ausdauer und ohne Doppel, Kraft 0 bis 2, Wochen 1 bis 26).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningChange {
    /// Montag bis Sonntag.
    pub availability: Option<[DayAvailability; 7]>,
    pub sports: Option<Vec<PlanSport>>,
    pub strength_per_week: Option<u8>,
    pub weeks: Option<u32>,
}

/// Größte Tagesverfügbarkeit in Minuten (ein Tag hat 1440; dieselbe Grenze wie in `store`).
pub const MAX_DAY_MINUTES: u16 = 1440;

impl PlanningChange {
    /// Zweite Prüfung der Grenzen nach dem Schema, damit die Typen auch dann stimmen, wenn das
    /// Schema einmal weiter wird. Fehler tragen Pfad und Stichwort wie Schemafehler.
    fn bound_issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        if self.availability.is_none()
            && self.sports.is_none()
            && self.strength_per_week.is_none()
            && self.weeks.is_none()
        {
            issues.push(Issue::schema("/proposal", "minProperties"));
        }
        if let Some(days) = &self.availability {
            for (i, d) in days.iter().enumerate() {
                if d.max_minutes > MAX_DAY_MINUTES {
                    issues.push(Issue::schema(
                        &format!("/proposal/availability/{i}/max_minutes"),
                        "maximum",
                    ));
                }
            }
        }
        if let Some(sports) = &self.sports {
            if sports.is_empty() {
                issues.push(Issue::schema("/proposal/sports", "minItems"));
            }
            for (i, s) in sports.iter().enumerate() {
                if !s.is_endurance() {
                    issues.push(Issue::schema(&format!("/proposal/sports/{i}"), "enum"));
                } else if sports[..i].contains(s) {
                    issues.push(Issue::schema("/proposal/sports", "uniqueItems"));
                }
            }
        }
        if self.strength_per_week.is_some_and(|n| n > 2) {
            issues.push(Issue::schema("/proposal/strength_per_week", "maximum"));
        }
        if let Some(w) = self.weeks
            && !(1..=26).contains(&w)
        {
            let keyword = if w < 1 { "minimum" } else { "maximum" };
            issues.push(Issue::schema("/proposal/weeks", keyword));
        }
        issues
    }
}

/// Ein geprüfter Vorschlag. Er ändert nichts, bis der Nutzer ihn übernimmt (SPEC 9.7).
#[derive(Debug, Clone, PartialEq)]
pub enum CheckedProposal {
    /// Änderungen am Basisplan, angewandt und gegen alle harten Regeln geprüft. `plan` hat
    /// `source = llm`, neu gerechnete Lasten, die weichen Verletzungen als Warnungen und
    /// `plan_id` und `created_at` des Basisplans (der Aufrufer vergibt neue).
    PlanEdit {
        edits: Vec<Edit>,
        explanation: String,
        plan: Plan,
    },
    /// Neue Eingaben für den Regelplaner. Die App erzeugt daraus den Plan und prüft ihn.
    PlanningChange(PlanningChange),
}

/// Ergebnis eines Gesprächsschritts.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatOutcome {
    /// Anzeigetext des Modells, unverändert bis auf die Kürzung auf [`MAX_REPLY_CHARS`].
    pub reply: String,
    pub proposal: Option<CheckedProposal>,
    /// Codes der Fehler, wenn das Modell einen Vorschlag machte, der auch nach den Korrekturen
    /// ungültig blieb (zum Beispiel `R03`, `SCHEMA`). Dann ist `proposal` leer.
    pub rejected_codes: Vec<String>,
    /// Aufrufe an das Modell einschließlich der Korrekturen, höchstens `1 + MAX_CORRECTIONS`.
    pub calls: u32,
}

/// Feste Systemanweisung des Gesprächs in der Sprache `locale`. Der Eingang folgt direkt
/// dahinter (siehe [`chat_messages`]).
pub fn chat_system_prompt(locale: Locale) -> &'static str {
    match locale {
        Locale::De => CHAT_DE,
        Locale::En => CHAT_EN,
    }
}

fn chat_correction_preface(locale: Locale) -> &'static str {
    match locale {
        Locale::De => CHAT_CORRECTION_DE,
        Locale::En => CHAT_CORRECTION_EN,
    }
}

/// Das Schema der Antwort als JSON-Wert.
pub fn chat_schema() -> Value {
    serde_json::from_str(CHAT_SCHEMA).expect("plan-chat.schema.json ist gültiges JSON")
}

/// Ersetzt Steuerzeichen außer Zeilenumbruch und Tabulator durch Leerzeichen und entfernt
/// Leerraum am Rand.
fn clean_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Prüft die neue Nachricht des Nutzers: nicht leer, höchstens [`MAX_MESSAGE_CHARS`] Zeichen.
/// Eine zu lange Nachricht wird nicht gekürzt, damit nicht stillschweigend etwas fehlt.
pub fn prepare_user_text(text: &str) -> Result<String, ChatError> {
    let text = clean_text(text);
    if text.is_empty() {
        return Err(ChatError::EmptyMessage);
    }
    if text.chars().count() > MAX_MESSAGE_CHARS {
        return Err(ChatError::MessageTooLong {
            max: MAX_MESSAGE_CHARS,
        });
    }
    Ok(text)
}

/// Die Nachrichten des ersten Aufrufs, genau so, wie sie zum Modell gehen (F24): feste
/// Anweisung mit dem Eingang als JSON, der Verlauf (Antworten des Modells im Antwortformat, nur
/// `reply`) und die neue Nachricht. `user_text` muss durch [`prepare_user_text`] gegangen sein.
pub fn chat_messages(
    input: &ChatInput,
    history: &ChatHistory,
    user_text: &str,
    locale: Locale,
) -> Vec<Message> {
    let mut messages = vec![Message::new(
        Role::System,
        format!(
            "{}\n{}",
            chat_system_prompt(locale).trim_end(),
            input.to_json()
        ),
    )];
    for m in history.messages() {
        messages.push(match m.role {
            ChatRole::User => Message::new(Role::User, m.text.clone()),
            ChatRole::Assistant => Message::new(
                Role::Assistant,
                json!({ "schema_version": 1, "reply": m.text }).to_string(),
            ),
        });
    }
    messages.push(Message::new(Role::User, user_text));
    messages
}

/// Vorschlag nach dem Schema, vor der Prüfung gegen Katalog und Regeln.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RawProposal {
    PlanEdit {
        edit: PlanEdit,
    },
    PlanningChange {
        #[serde(default)]
        availability: Option<Vec<DayAvailability>>,
        #[serde(default)]
        sports: Option<Vec<PlanSport>>,
        #[serde(default)]
        strength_per_week: Option<u8>,
        #[serde(default)]
        weeks: Option<u32>,
    },
}

/// Urteil über eine Antwort.
enum Verdict {
    Accepted {
        reply: String,
        proposal: Option<CheckedProposal>,
    },
    Rejected {
        /// Die lesbare Antwort, wenn nur der Vorschlag ungültig war.
        reply: Option<String>,
        issues: Vec<Issue>,
        /// `false`, wenn kein Korrekturaufruf helfen kann (Basisplan selbst ungültig).
        correctable: bool,
    },
}

fn validator(schema: &Value) -> jsonschema::Validator {
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(schema)
        .expect("plan-chat.schema.json ist ein gültiges Schema")
}

/// Kürzt den Text an `pointer` auf `max` Zeichen, falls er länger ist.
fn cut(value: &mut Value, pointer: &str, max: usize) {
    if let Some(Value::String(text)) = value.pointer_mut(pointer)
        && text.chars().nth(max).is_some()
    {
        *text = truncate_chars(text, max);
    }
}

/// Fehler im Vorschlag mit Pfad im gewählten Zweig. Die Prüfung über `oneOf` meldet nur
/// "/proposal: oneOf"; für den Korrekturaufruf ist der genaue Pfad nützlicher.
fn proposal_issues(schema: &Value, proposal: &Value, fallback: Vec<Issue>) -> Vec<Issue> {
    let branch = match proposal.get("kind").and_then(Value::as_str) {
        Some("plan_edit") => 0,
        Some("planning_change") => 1,
        _ => return fallback,
    };
    let sub = &schema["properties"]["proposal"]["oneOf"][branch];
    let issues = schema_issues(&validator(sub), proposal, "/proposal");
    if issues.is_empty() { fallback } else { issues }
}

fn evaluate(content: &str, input: &ChatInput, notes: &[PlanWarning], base_ok: bool) -> Verdict {
    let rejected = |reply, issues| Verdict::Rejected {
        reply,
        issues,
        correctable: true,
    };
    let Ok(mut value) = serde_json::from_str::<Value>(content.trim()) else {
        return rejected(None, vec![Issue::new(code::NOT_JSON)]);
    };
    // Zu lange Texte sind reine Anzeige und kein Grund für einen Korrekturaufruf (wie 7.2).
    cut(&mut value, "/reply", MAX_REPLY_CHARS);
    cut(
        &mut value,
        "/proposal/edit/explanation",
        edit::MAX_EXPLANATION_CHARS,
    );
    let schema = chat_schema();
    let all = schema_issues(&validator(&schema), &value, "");
    let (in_proposal, envelope): (Vec<Issue>, Vec<Issue>) = all.into_iter().partition(|i| {
        i.path
            .as_deref()
            .is_some_and(|p| p.starts_with("/proposal"))
    });
    if !envelope.is_empty() {
        let mut issues = envelope;
        issues.extend(in_proposal);
        return rejected(None, issues);
    }
    let reply = value["reply"].as_str().unwrap_or_default().to_string();
    let Some(proposal) = value.get("proposal") else {
        return Verdict::Accepted {
            reply,
            proposal: None,
        };
    };
    if !in_proposal.is_empty() {
        return rejected(Some(reply), proposal_issues(&schema, proposal, in_proposal));
    }
    let raw: RawProposal = match serde_json::from_value(proposal.clone()) {
        Ok(raw) => raw,
        // Das Schema ließ den Vorschlag durch, die Typen nicht (zum Beispiel ein Datum, das es
        // nicht gibt).
        Err(_) => return rejected(Some(reply), vec![Issue::schema("/proposal", "type")]),
    };
    match raw {
        RawProposal::PlanEdit { edit } => {
            if !base_ok {
                return Verdict::Rejected {
                    reply: Some(reply),
                    issues: vec![Issue::base_plan_invalid()],
                    correctable: false,
                };
            }
            match check_edits(
                &edit.edits,
                &edit.explanation,
                &input.base,
                notes,
                &input.ctx,
                &input.rules,
            ) {
                Ok(plan) => Verdict::Accepted {
                    reply,
                    proposal: Some(CheckedProposal::PlanEdit {
                        edits: edit.edits,
                        explanation: edit.explanation,
                        plan,
                    }),
                },
                Err(issues) => rejected(Some(reply), issues),
            }
        }
        RawProposal::PlanningChange {
            availability,
            sports,
            strength_per_week,
            weeks,
        } => {
            let availability = match availability.map(<[DayAvailability; 7]>::try_from) {
                None => None,
                Some(Ok(days)) => Some(days),
                Some(Err(_)) => {
                    return rejected(
                        Some(reply),
                        vec![Issue::schema("/proposal/availability", "minItems")],
                    );
                }
            };
            let change = PlanningChange {
                availability,
                sports,
                strength_per_week,
                weeks,
            };
            let issues = change.bound_issues();
            if issues.is_empty() {
                Verdict::Accepted {
                    reply,
                    proposal: Some(CheckedProposal::PlanningChange(change)),
                }
            } else {
                rejected(Some(reply), issues)
            }
        }
    }
}

/// Codes der Fehler ohne Doppel, in der Reihenfolge der Fehler.
fn codes(issues: &[Issue]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in issues {
        if !out.contains(&i.code) {
            out.push(i.code.clone());
        }
    }
    out
}

/// Ein Schritt des Gesprächs (SPEC 9.7): sendet Eingang, Verlauf und die neue Nachricht,
/// prüft die Antwort und korrigiert höchstens [`MAX_CORRECTIONS`] Mal. `cfg` wie bei
/// `refine_plan` (Korrekturen, Zeitlimit je Aufruf). `history` bleibt unverändert; der
/// Aufrufer hängt die Runde mit [`ChatHistory::record`] an. Panikfrei für beliebige Antworten.
pub fn chat_turn(
    client: &dyn LlmClient,
    input: &ChatInput,
    history: &ChatHistory,
    user_text: &str,
    locale: Locale,
    cfg: &RefineConfig,
) -> Result<ChatOutcome, ChatError> {
    let user_text = prepare_user_text(user_text)?;
    let base_ok = !validate_plan(&input.base, &input.ctx, &input.rules)
        .iter()
        .any(|v| v.hardness == Hardness::Hard);
    let notes = base_notes(&input.base, &input.ctx, &input.rules);
    let mut messages = chat_messages(input, history, &user_text, locale);
    let schema = chat_schema();
    let max_corrections = u32::from(cfg.max_corrections.min(MAX_CORRECTIONS));
    let mut calls: u32 = 0;
    loop {
        calls += 1;
        let request = LlmRequest {
            messages: messages.clone(),
            schema: schema.clone(),
            timeout_s: cfg.timeout_s,
        };
        let response = match client.complete(&request) {
            Ok(r) => r,
            Err(LlmError::Timeout) => return Err(ChatError::Timeout { calls }),
            Err(error) => return Err(ChatError::Transport { error, calls }),
        };
        let (reply, issues, correctable) = match evaluate(&response.content, input, &notes, base_ok)
        {
            Verdict::Accepted { reply, proposal } => {
                return Ok(ChatOutcome {
                    reply,
                    proposal,
                    rejected_codes: Vec::new(),
                    calls,
                });
            }
            Verdict::Rejected {
                reply,
                issues,
                correctable,
            } => (reply, issues, correctable),
        };
        if calls > max_corrections || !correctable {
            return match reply {
                Some(reply) => Ok(ChatOutcome {
                    reply,
                    proposal: None,
                    rejected_codes: codes(&issues),
                    calls,
                }),
                None => Err(ChatError::InvalidOutput { issues, calls }),
            };
        }
        messages.push(Message::new(
            Role::Assistant,
            truncate_chars(&response.content, MAX_ECHO_CHARS),
        ));
        messages.push(correction_with(chat_correction_preface(locale), &issues));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_the_documented_limits() {
        let s = chat_schema();
        assert_eq!(s["properties"]["reply"]["maxLength"], MAX_REPLY_CHARS);
        let change = &s["properties"]["proposal"]["oneOf"][1]["properties"];
        assert_eq!(change["availability"]["minItems"], 7);
        assert_eq!(change["availability"]["maxItems"], 7);
        assert_eq!(
            change["availability"]["items"]["properties"]["max_minutes"]["maximum"],
            MAX_DAY_MINUTES
        );
        assert_eq!(change["strength_per_week"]["maximum"], 2);
        assert_eq!(change["weeks"]["minimum"], 1);
        assert_eq!(change["weeks"]["maximum"], 26);
    }

    #[test]
    fn every_object_in_the_schema_forbids_additional_properties() {
        fn walk(v: &Value, path: &str) {
            match v {
                Value::Object(map) => {
                    if map.get("type").and_then(Value::as_str) == Some("object") {
                        assert_eq!(
                            map.get("additionalProperties"),
                            Some(&json!(false)),
                            "{path}"
                        );
                    }
                    for (k, x) in map {
                        walk(x, &format!("{path}/{k}"));
                    }
                }
                Value::Array(items) => {
                    for (i, x) in items.iter().enumerate() {
                        walk(x, &format!("{path}/{i}"));
                    }
                }
                _ => {}
            }
        }
        walk(&chat_schema(), "");
    }

    #[test]
    fn the_plan_edit_part_equals_plan_edit_schema() {
        let mut edit = edit::edit_schema();
        let map = edit.as_object_mut().unwrap();
        map.remove("$schema");
        map.remove("$id");
        map.remove("title");
        assert_eq!(
            chat_schema()["properties"]["proposal"]["oneOf"][0]["properties"]["edit"],
            edit
        );
    }

    #[test]
    fn metric_values_are_rounded_to_one_decimal() {
        // 3,1417 * 10 = 31,417, gerundet 31, also 3,1. 2,25 * 10 = 22,5, kaufmännisch 23,
        // also 2,3. -0,04 * 10 = -0,4, gerundet -0, ausgegeben als 0.
        assert_eq!(
            MetricValue::new("ramp_ctl_7d", 3.1417).unwrap().value(),
            3.1
        );
        assert_eq!(MetricValue::new("x", 2.25).unwrap().value(), 2.3);
        let zero = MetricValue::new("x", -0.04).unwrap();
        assert_eq!(
            serde_json::to_string(&zero).unwrap(),
            r#"{"code":"x","value":0.0}"#
        );
        assert_eq!(
            MetricValue::new("share_z12_4w", 81.0).unwrap().value(),
            81.0
        );
    }

    #[test]
    fn metric_codes_are_restricted() {
        for bad in [
            "",
            "Ramp",
            "ramp-ctl",
            "ramp ctl",
            "ramp.ctl",
            "rämp",
            "MARKER",
            &"a".repeat(41),
        ] {
            assert!(MetricValue::new(bad, 1.0).is_none(), "{bad:?}");
        }
        assert!(MetricValue::new(&"a".repeat(40), 1.0).is_some());
        assert!(MetricValue::new("x", f64::NAN).is_none());
        assert!(MetricValue::new("x", f64::INFINITY).is_none());
    }

    #[test]
    fn user_text_is_checked_not_cut() {
        assert_eq!(prepare_user_text("  "), Err(ChatError::EmptyMessage));
        assert_eq!(
            prepare_user_text("\u{1b}\u{7}"),
            Err(ChatError::EmptyMessage)
        );
        assert_eq!(
            prepare_user_text(&"ä".repeat(MAX_MESSAGE_CHARS + 1)),
            Err(ChatError::MessageTooLong {
                max: MAX_MESSAGE_CHARS
            })
        );
        assert_eq!(
            prepare_user_text(&"ä".repeat(MAX_MESSAGE_CHARS)).unwrap(),
            "ä".repeat(MAX_MESSAGE_CHARS)
        );
        assert_eq!(
            prepare_user_text(" Dienstag\u{1b}[31m keine Zeit\n").unwrap(),
            "Dienstag [31m keine Zeit"
        );
    }
}
