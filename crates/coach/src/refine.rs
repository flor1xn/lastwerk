//! `refine_plan`: Ein lokales Modell passt den Regelplan an und erklärt ihn (SPEC 5.4, 9.4).
//!
//! Ablauf je Antwort des Modells:
//!
//! 1. Gültiges JSON nach `plan-edit.schema.json` (`edit::parse_reply`).
//! 2. Jede `template_id` steht im Katalog und passt zur Sportart des Slots, die Dauer liegt im
//!    Fenster der Vorlage (`edit::apply_edits`).
//! 3. Die Änderungen sind auf dem Basisplan angewandt, `core` hat die Lasten neu gerechnet.
//! 4. `validate_plan` meldet keine harte Verletzung.
//! 5. Weiche Verletzungen werden als Warnungen angehängt, nicht unterdrückt.
//!
//! Bei einem Fehler in 1 bis 4 geht die Liste der Fehler als Codes in einen Korrekturaufruf
//! (höchstens zwei, `MAX_CORRECTIONS` lässt sich nicht überschreiten). Danach, bei
//! Zeitüberschreitung oder Netzfehler bleibt der Regelplan: `RefineOutcome::Fallback` nennt
//! den Grund. Der Regelplan kommt dabei unverändert zurück.

use std::collections::BTreeSet;

use lastwerk_core::plan::{
    Hardness, Locale, Plan, PlanContext, PlanSource, PlanWarning, Rules, validate_plan,
    violations_to_warnings,
};
use serde_json::json;

use crate::client::{LlmClient, LlmError, LlmRequest, Message, Role};
use crate::edit::{self, Issue, hard_issues, parse_reply};
use crate::input::RefineInput;

/// Obergrenze der Korrekturaufrufe (SPEC 9.4). `RefineConfig::max_corrections` wird darauf
/// begrenzt.
pub const MAX_CORRECTIONS: u8 = 2;

/// Zeitlimit je Aufruf in Sekunden, wenn nichts angegeben ist (Faustregel für Rechner ohne
/// eigene Grafikkarte, O13 offen).
pub const DEFAULT_TIMEOUT_S: u32 = 120;

/// Längster Teil einer früheren Antwort, der in einen Korrekturaufruf zurückgeht (Konvention).
const MAX_ECHO_CHARS: usize = 4000;

const SYSTEM_DE: &str = include_str!("../prompts/system.de.txt");
const SYSTEM_EN: &str = include_str!("../prompts/system.en.txt");
const CORRECTION_DE: &str = include_str!("../prompts/correction.de.txt");
const CORRECTION_EN: &str = include_str!("../prompts/correction.en.txt");

/// Einstellungen eines Laufs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefineConfig {
    /// Korrekturaufrufe nach dem ersten Aufruf, höchstens [`MAX_CORRECTIONS`].
    pub max_corrections: u8,
    /// Zeitlimit je Aufruf in Sekunden.
    pub timeout_s: u32,
}

impl Default for RefineConfig {
    fn default() -> Self {
        Self {
            max_corrections: MAX_CORRECTIONS,
            timeout_s: DEFAULT_TIMEOUT_S,
        }
    }
}

/// Warum der Regelplan bleibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackReason {
    /// Der Basisplan verletzt selbst eine harte Regel; es gibt nichts zu verfeinern.
    BaseInvalid,
    /// Zeitlimit bei einem Aufruf.
    Timeout,
    /// Server nicht erreichbar, Status außerhalb von 2xx oder Antwort in unbekannter Form.
    Transport(LlmError),
    /// Auch der letzte erlaubte Korrekturaufruf lieferte eine unbrauchbare Antwort. Die Fehler
    /// der letzten Antwort als Codes.
    InvalidOutput(Vec<Issue>),
}

impl FallbackReason {
    /// Stabiler Code für Zähler und Protokolle.
    pub fn code(&self) -> &'static str {
        match self {
            Self::BaseInvalid => "base_invalid",
            Self::Timeout => "timeout",
            Self::Transport(_) => "transport",
            Self::InvalidOutput(_) => "invalid_output",
        }
    }
}

/// Ergebnis (SPEC 5.4). `attempts` zählt die Aufrufe an das Modell einschließlich der
/// Korrekturaufrufe, höchstens `1 + MAX_CORRECTIONS`.
#[derive(Debug, Clone, PartialEq)]
pub enum RefineOutcome {
    /// Neuer Plan mit `source = llm`. `plan_id` und `created_at` sind die des Basisplans; der
    /// Aufrufer vergibt neue, wenn er den Plan neben dem Regelplan speichert.
    Applied {
        plan: Plan,
        explanation: String,
        attempts: u8,
    },
    /// Der Regelplan, unverändert.
    Fallback {
        plan: Plan,
        reason: FallbackReason,
        attempts: u8,
    },
}

/// Feste Systemanweisung in der Sprache des Plans.
pub fn system_prompt(locale: Locale) -> &'static str {
    match locale {
        Locale::De => SYSTEM_DE,
        Locale::En => SYSTEM_EN,
    }
}

fn correction_preface(locale: Locale) -> &'static str {
    match locale {
        Locale::De => CORRECTION_DE,
        Locale::En => CORRECTION_EN,
    }
}

/// Die Nachrichten des ersten Aufrufs: feste Systemanweisung und der Eingang als JSON.
pub fn initial_messages(input: &RefineInput, locale: Locale) -> Vec<Message> {
    vec![
        Message::new(Role::System, system_prompt(locale)),
        Message::new(Role::User, input.to_json()),
    ]
}

/// Nachricht eines Korrekturaufrufs: fester Satz und die Fehler als Codes (JSON).
fn correction_message(locale: Locale, issues: &[Issue]) -> Message {
    correction_with(correction_preface(locale), issues)
}

/// Fester Satz `preface` und die Fehler als Codes (JSON). Auch für das Gespräch (`chat`).
pub(crate) fn correction_with(preface: &str, issues: &[Issue]) -> Message {
    let list = json!({ "violations": issues });
    Message::new(Role::User, format!("{}\n{}", preface.trim_end(), list))
}

/// Prüft eine Antwort in den Schritten 1 bis 5. `Ok` ist der neue Plan mit Erklärung.
fn evaluate(
    content: &str,
    base: &Plan,
    base_notes: &[PlanWarning],
    ctx: &PlanContext,
    rules: &Rules,
) -> Result<(Plan, String), Vec<Issue>> {
    let reply = parse_reply(content)?;
    let plan = check_edits(
        &reply.edits,
        &reply.explanation,
        base,
        base_notes,
        ctx,
        rules,
    )?;
    Ok((plan, reply.explanation))
}

/// Schritte 2 bis 5 für geparste Änderungen: anwenden (Katalog, Sportart, Dauer, Slots),
/// `validate_plan`, harte Verletzungen als Fehler, weiche als Warnungen. `Ok` ist der neue Plan
/// mit `source = llm` und `explanation`. Auch für Vorschläge im Gespräch (`chat`).
pub(crate) fn check_edits(
    edits: &[edit::Edit],
    explanation: &str,
    base: &Plan,
    base_notes: &[PlanWarning],
    ctx: &PlanContext,
    rules: &Rules,
) -> Result<Plan, Vec<Issue>> {
    let (mut plan, issues) = edit::apply_edits(base, edits, &ctx.catalog, rules);
    if !issues.is_empty() {
        return Err(issues);
    }
    let violations = validate_plan(&plan, ctx, rules);
    let hard = hard_issues(&violations);
    if !hard.is_empty() {
        return Err(hard);
    }
    let soft: Vec<_> = violations
        .iter()
        .filter(|v| v.hardness == Hardness::Soft)
        .cloned()
        .collect();
    let mut warnings: Vec<PlanWarning> = base_notes.to_vec();
    warnings.extend(violations_to_warnings(&soft));
    warnings.sort();
    warnings.dedup();
    plan.warnings = warnings;
    plan.source = PlanSource::Llm;
    plan.explanation = Some(explanation.to_string());
    Ok(plan)
}

/// Hinweise des Basisplans, die nicht aus einer Regelprüfung stammen (zum Beispiel R12 als
/// Info oder eine Ruhewoche nach gescheitertem Aufbau). Sie bleiben im neuen Plan. Alles andere
/// leitet `validate_plan` neu ab.
pub(crate) fn base_notes(base: &Plan, ctx: &PlanContext, rules: &Rules) -> Vec<PlanWarning> {
    let derived: BTreeSet<PlanWarning> = violations_to_warnings(&validate_plan(base, ctx, rules))
        .into_iter()
        .collect();
    base.warnings
        .iter()
        .filter(|w| !derived.contains(w))
        .cloned()
        .collect()
}

/// Lässt das Modell `base` anpassen (SPEC 9.4). Gibt entweder den geprüften neuen Plan oder
/// den unveränderten Regelplan mit Grund zurück. Panikfrei für beliebige Antworten.
pub fn refine_plan(
    base: &Plan,
    ctx: &PlanContext,
    rules: &Rules,
    client: &dyn LlmClient,
    cfg: &RefineConfig,
) -> RefineOutcome {
    let fallback = |reason, attempts| RefineOutcome::Fallback {
        plan: base.clone(),
        reason,
        attempts,
    };
    if validate_plan(base, ctx, rules)
        .iter()
        .any(|v| v.hardness == Hardness::Hard)
    {
        return fallback(FallbackReason::BaseInvalid, 0);
    }
    let notes = base_notes(base, ctx, rules);
    let input = RefineInput::build(base, ctx, rules);
    let mut messages = initial_messages(&input, base.locale);
    let schema = edit::edit_schema();
    let max_corrections = cfg.max_corrections.min(MAX_CORRECTIONS);
    let mut attempts: u8 = 0;
    loop {
        attempts += 1;
        let request = LlmRequest {
            messages: messages.clone(),
            schema: schema.clone(),
            timeout_s: cfg.timeout_s,
        };
        let response = match client.complete(&request) {
            Ok(r) => r,
            Err(LlmError::Timeout) => return fallback(FallbackReason::Timeout, attempts),
            Err(e) => return fallback(FallbackReason::Transport(e), attempts),
        };
        let issues = match evaluate(&response.content, base, &notes, ctx, rules) {
            Ok((plan, explanation)) => {
                return RefineOutcome::Applied {
                    plan,
                    explanation,
                    attempts,
                };
            }
            Err(issues) => issues,
        };
        if attempts > max_corrections {
            return fallback(FallbackReason::InvalidOutput(issues), attempts);
        }
        messages.push(Message::new(
            Role::Assistant,
            edit::truncate_chars(&response.content, MAX_ECHO_CHARS),
        ));
        messages.push(correction_message(base.locale, &issues));
    }
}
