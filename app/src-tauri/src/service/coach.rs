//! Gespräch mit dem lokalen Coach (T26 Teil B, SPEC 9.7).
//!
//! Die Schicht verbindet `lastwerk-coach` (Eingang, Verlauf, Prüfung der Antwort) mit der
//! Datenbank und dem Regelplaner. Sie rechnet nichts selbst: Kennzahlen kommen aus dem
//! Dashboard (T25), Pläne aus `lastwerk-cli`, Vorher und Nachher aus den Plänen. Das Modell
//! kann nur einen Vorschlag machen; er ändert nichts, bis der Nutzer ihn übernimmt (`apply`).
//!
//! Zustand: Verlauf und offene Vorschläge leben im Speicher der App ([`CoachState`]), nie in
//! der Datenbank. Der Zustand überlebt keinen Neustart. In der Datenbank stehen nur die
//! Einstellungen `coach.provider`, `coach.base_url` und `coach.model` (kein Schlüsselfeld,
//! SPEC 9.2). Die Adresse ist Loopback, ein Host im lokalen Netz gäbe es nur nach ausdrücklicher
//! Bestätigung (SPEC 9.2), die die Oberfläche noch nicht bietet.
//!
//! Alle Funktionen nehmen den Client und die Zeit als Parameter und sind ohne Netzwerk
//! testbar; `chat_with_provider` verbindet sie mit dem echten Client.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc};
use lastwerk_cli::planning::{self, DEFAULT_STRENGTH_PER_WEEK, PlanRequest};
use lastwerk_coach::chat::prepare_user_text;
use lastwerk_coach::edit::Edit;
use lastwerk_coach::{
    BaseUrl, ChatError, ChatFocus, ChatHistory, ChatInput, CheckedProposal, DEFAULT_TIMEOUT_S,
    LlmClient, LlmError, LlmRequest, LlmResponse, MetricValue, OLLAMA_DEFAULT_URL, PlanningChange,
    Provider, RefineConfig, Role, chat_messages, chat_turn, ollama_models, pick_model,
};
use lastwerk_core::plan::{Locale, Plan, PlanSport, PlanWeek, Slot};
use lastwerk_store::{PlanStatus, Store};
use serde::Serialize;

use crate::dto::{AvailabilityDay, ChatFocusDto, Dashboard, PlanDetail};
use crate::error::{AppError, Result};
use crate::service::{KEY_LOCALE, dashboard, plans};

/// Schlüssel in `setting` (SPEC 9.2). Kein Schlüssel für einen Zugangsschlüssel: Es gibt keinen.
pub const KEY_PROVIDER: &str = "coach.provider";
pub const KEY_BASE_URL: &str = "coach.base_url";
pub const KEY_MODEL: &str = "coach.model";

/// Zeitlimit für die Abfrage der Modellliste in Sekunden (Konvention: Eine lokale Liste
/// antwortet sofort. Wer nach zwei Sekunden nichts hört, hat keinen Server, und die Oberfläche
/// soll nicht auf einen Fehler warten).
pub const STATUS_TIMEOUT_S: u32 = 2;

/// Höchstzahl offener Vorschläge im Speicher (Konvention). Ältere fallen weg; ein Vorschlag, den
/// der Nutzer nach vier weiteren Antworten noch will, ist ohnehin veraltet.
pub const MAX_PENDING: usize = 4;

const PROVIDER_OLLAMA: &str = "ollama";
const PROVIDER_OPENAI: &str = "openai_compatible";

// --- Einstellungen und Status --------------------------------------------------------

/// Gespeicherte Einstellungen des Coachs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachConfig {
    pub openai_compatible: bool,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

pub fn read_config(store: &Store) -> Result<CoachConfig> {
    Ok(CoachConfig {
        // Unbekannte Werte gelten als Ollama: Das ist der Standard und lokal.
        openai_compatible: store.get_setting(KEY_PROVIDER)?.as_deref() == Some(PROVIDER_OPENAI),
        base_url: non_empty(store.get_setting(KEY_BASE_URL)?),
        model: non_empty(store.get_setting(KEY_MODEL)?),
    })
}

/// Was die Oberfläche über den Coach wissen muss, um ihn anzubieten oder die Einrichtung zu
/// erklären.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CoachStatus {
    /// Ein Modell ist erreichbar und gewählt.
    pub available: bool,
    /// `ollama` oder `openai_compatible`.
    pub provider: String,
    /// Die Adresse des Modellservers (Loopback), für den Hinweis zur Einrichtung.
    pub base_url: String,
    /// Das Modell, mit dem ein Gespräch liefe: das gespeicherte, wenn es vorhanden ist, sonst
    /// die Wahl von `pick_model`.
    pub model: Option<String>,
    /// Vorhandene Modelle (nur Ollama), in der Reihenfolge des Servers.
    pub models: Vec<String>,
    /// Warum nicht verfügbar: `no_server`, `no_model`, `bad_url` oder `not_configured`.
    pub reason: Option<String>,
}

struct Resolved {
    status: CoachStatus,
    provider: Option<Provider>,
}

fn unavailable(cfg: &CoachConfig, base_url: String, reason: &str, models: Vec<String>) -> Resolved {
    Resolved {
        status: CoachStatus {
            available: false,
            provider: provider_name(cfg).to_owned(),
            base_url,
            model: cfg.model.clone(),
            models,
            reason: Some(reason.to_owned()),
        },
        provider: None,
    }
}

fn provider_name(cfg: &CoachConfig) -> &'static str {
    if cfg.openai_compatible {
        PROVIDER_OPENAI
    } else {
        PROVIDER_OLLAMA
    }
}

/// Ermittelt Status und Anbieter. `probe` fragt die Modelle eines Ollama-Servers ab.
fn resolve_with(
    cfg: &CoachConfig,
    probe: &dyn Fn(&BaseUrl) -> std::result::Result<Vec<String>, LlmError>,
) -> Resolved {
    let shown = |url: &Option<String>| url.clone().unwrap_or_else(|| OLLAMA_DEFAULT_URL.to_owned());
    if cfg.openai_compatible {
        // Ein OpenAI-kompatibler Server hat keine Standardadresse und keine Modellliste, die
        // wir abfragen (SPEC 9.2): ohne Adresse und Modell ist er nicht eingerichtet.
        let (Some(url), Some(model)) = (&cfg.base_url, &cfg.model) else {
            return unavailable(
                cfg,
                cfg.base_url.clone().unwrap_or_default(),
                "not_configured",
                Vec::new(),
            );
        };
        return match Provider::openai_compatible(url, model, false) {
            Ok(provider) => Resolved {
                status: CoachStatus {
                    available: true,
                    provider: PROVIDER_OPENAI.to_owned(),
                    base_url: provider.base_url().as_text(),
                    model: Some(model.clone()),
                    models: Vec::new(),
                    reason: None,
                },
                provider: Some(provider),
            },
            Err(_) => unavailable(cfg, url.clone(), "bad_url", Vec::new()),
        };
    }
    let url_text = shown(&cfg.base_url);
    let Ok(url) = BaseUrl::parse(&url_text, false) else {
        return unavailable(cfg, url_text, "bad_url", Vec::new());
    };
    let Ok(models) = probe(&url) else {
        return unavailable(cfg, url.as_text(), "no_server", Vec::new());
    };
    // Das gespeicherte Modell gilt nur, wenn es der Server hat (es kann gelöscht worden sein).
    let model = cfg
        .model
        .clone()
        .filter(|m| models.contains(m))
        .or_else(|| pick_model(&models));
    let Some(model) = model else {
        return unavailable(cfg, url.as_text(), "no_model", models);
    };
    match Provider::ollama(Some(&url.as_text()), &model, false) {
        Ok(provider) => Resolved {
            status: CoachStatus {
                available: true,
                provider: PROVIDER_OLLAMA.to_owned(),
                base_url: url.as_text(),
                model: Some(model),
                models,
                reason: None,
            },
            provider: Some(provider),
        },
        // Ein Modellname, den die Prüfung ablehnt (Steuerzeichen), ist kein brauchbares Modell.
        Err(_) => unavailable(cfg, url.as_text(), "no_model", models),
    }
}

fn resolve(cfg: &CoachConfig) -> Resolved {
    resolve_with(cfg, &|url| ollama_models(url, STATUS_TIMEOUT_S))
}

/// Status des Coachs. Fragt bei Ollama die Modelle ab (Loopback, kurzes Zeitlimit).
pub fn status(store: &Store) -> Result<CoachStatus> {
    Ok(resolve(&read_config(store)?).status)
}

/// Merkt das Modell für das Gespräch. Leer heißt: das Programm wählt eines (`pick_model`).
pub fn set_model(store: &mut Store, model: &str) -> Result<CoachStatus> {
    let model = model.trim();
    if !model.is_empty() {
        Provider::ollama(None, model, false).map_err(|_| AppError::new("invalid_value"))?;
    }
    store.set_setting(KEY_MODEL, model)?;
    status(store)
}

// --- Zustand im Speicher -------------------------------------------------------------

#[derive(Debug, Clone)]
enum Stored {
    /// Der geprüfte Plan des Modells, `plan_id` und `created_at` sind noch die des Basisplans.
    Edit {
        plan: Box<Plan>,
    },
    Change(PlanningChange),
}

#[derive(Debug, Clone)]
struct Pending {
    id: String,
    /// Der Plan, auf den sich der Vorschlag bezieht. Hat er sich geändert, ist der Vorschlag
    /// veraltet (`proposal_stale`).
    base_plan_id: String,
    stored: Stored,
}

#[derive(Debug, Default)]
struct Inner {
    history: ChatHistory,
    pending: VecDeque<Pending>,
}

#[derive(Debug, Default)]
struct Shared {
    inner: Mutex<Inner>,
    /// Ein Gespräch läuft gerade (höchstens eines gleichzeitig).
    busy: AtomicBool,
    /// Gesetzt von `reset`: Weitere Aufrufe an das Modell unterbleiben.
    cancel: AtomicBool,
    /// Zählt bei jedem `reset` hoch. Eine Antwort, die nach einem Zurücksetzen eintrifft,
    /// wird verworfen.
    epoch: AtomicU64,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        // Ein Panic in einem anderen Befehl darf das Gespräch nicht dauerhaft sperren.
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Gesprächszustand der App, billig zu kopieren (für `spawn_blocking`).
#[derive(Debug, Clone, Default)]
pub struct CoachState(Arc<Shared>);

/// Hält die Sperre für ein laufendes Gespräch und gibt sie beim Verlassen frei.
#[derive(Debug)]
pub struct BusyGuard(Arc<Shared>);

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}

impl CoachState {
    /// Beginnt ein Gespräch. `None`, wenn schon eines läuft (`busy`).
    pub fn begin(&self) -> Option<BusyGuard> {
        self.0
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()?;
        self.0.cancel.store(false, Ordering::SeqCst);
        Some(BusyGuard(Arc::clone(&self.0)))
    }

    /// Bricht ein laufendes Gespräch ab (SPEC 9.5: Abbruch jederzeit): Es folgt kein weiterer
    /// Aufruf an das Modell, und die Antwort eines Aufrufs, der gerade läuft, wird verworfen.
    /// Ein laufender Aufruf selbst endet erst mit seinem Zeitlimit, die Sperre (`busy`) bleibt
    /// bis dahin. Der Verlauf bleibt erhalten.
    pub fn cancel(&self) {
        self.0.epoch.fetch_add(1, Ordering::SeqCst);
        self.0.cancel.store(true, Ordering::SeqCst);
    }

    /// Beginnt von vorn: bricht ab und leert Verlauf und offene Vorschläge.
    pub fn reset(&self) {
        self.cancel();
        let mut inner = self.0.lock();
        inner.history.clear();
        inner.pending.clear();
    }

    /// Zahl der Nachrichten im Verlauf (für Tests).
    pub fn history_len(&self) -> usize {
        self.0.lock().history.len()
    }

    /// Zahl der offenen Vorschläge (für Tests).
    pub fn pending_len(&self) -> usize {
        self.0.lock().pending.len()
    }
}

/// Reicht Aufrufe nur durch, solange das Gespräch nicht abgebrochen ist.
struct Cancelable<'a> {
    inner: &'a dyn LlmClient,
    cancel: &'a AtomicBool,
}

impl LlmClient for Cancelable<'_> {
    fn complete(&self, req: &LlmRequest) -> std::result::Result<LlmResponse, LlmError> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(LlmError::Transport("cancelled".into()));
        }
        self.inner.complete(req)
    }
}

// --- Antworten an die Oberfläche -----------------------------------------------------

/// Zahlen einer Woche, von `core` gerechnet (Minuten und Last stehen im Plan).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeekFigures {
    pub minutes: u32,
    /// Harte Einheiten in der Woche.
    pub hard: u32,
    /// Geplante Last der Woche, auf eine Stelle gerundet. `None`, wenn der Plan keine nennt.
    pub load: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeekComparison {
    pub week_start: String,
    /// `None`, wenn die Woche im Plan davor nicht vorkommt.
    pub before: Option<WeekFigures>,
    /// `None`, wenn die Woche im Plan danach entfällt.
    pub after: Option<WeekFigures>,
}

/// Eine Änderung eines `plan_edit` (SPEC 7.2) in der Form, die die Oberfläche anzeigt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EditLine {
    /// `replace`, `move` oder `remove`.
    pub op: &'static str,
    pub date: String,
    /// `am` oder `pm`.
    pub slot: &'static str,
    pub template_id: Option<String>,
    pub duration_min: Option<u32>,
    pub to_date: Option<String>,
    pub to_slot: Option<&'static str>,
}

/// Neue Eingaben für den Regelplaner aus einem `planning_change`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangeLine {
    pub availability: Option<Vec<AvailabilityDay>>,
    pub sports: Option<Vec<PlanSport>>,
    pub strength_per_week: Option<u8>,
    pub weeks: Option<u32>,
    /// Montag, ab dem der neue Plan beginnt.
    pub week_start: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProposalSummary {
    pub weeks: Vec<WeekComparison>,
    pub edits: Vec<EditLine>,
    pub change: Option<ChangeLine>,
    /// Warnungen im neuen Plan (weiche Verletzungen, SPEC 8.4).
    pub warnings: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProposalPreview {
    pub id: String,
    /// `plan_edit` oder `planning_change`.
    pub kind: &'static str,
    /// Erklärung des Modells zu einem `plan_edit`, Anzeigetext.
    pub explanation: Option<String>,
    pub summary: ProposalSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoachReply {
    /// Antwort des Modells, Anzeigetext (SPEC 9.5).
    pub reply: String,
    pub proposal: Option<ProposalPreview>,
    /// Regelcodes, wenn das Modell einen Vorschlag machte, der die Prüfung nicht bestand.
    pub rejected_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderInfo {
    pub name: String,
    pub base_url: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PreviewMessage {
    pub role: &'static str,
    pub content: String,
}

/// Der exakte Text, der bei der nächsten Nachricht zum Modell ginge (F24).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PromptPreview {
    pub provider: ProviderInfo,
    pub messages: Vec<PreviewMessage>,
}

// --- Kennzahlen aus dem Dashboard ----------------------------------------------------

/// Kennzahlen für den Eingang, aus dem Dashboard (T25), auf eine Nachkommastelle gerundet
/// (`MetricValue`). Nur Zahlen mit festem Code, kein Text, keine Rohwerte der Erholung (die
/// stecken im Zeichen `recovery_signal` des Kontexts, SPEC 9.3). Fehlende Werte fehlen in der
/// Liste, sie werden nie zu 0.
pub fn chat_metrics(d: &Dashboard) -> Vec<MetricValue> {
    let mut out = Vec::new();
    let mut add = |code: &str, value: Option<f64>| {
        if let Some(m) = value.and_then(|v| MetricValue::new(code, v)) {
            out.push(m);
        }
    };
    // Stand heute, im Unterschied zu `fitness` im Kontext (Tag vor dem Planbeginn).
    add("ctl_now", d.form.as_ref().map(|f| f.ctl));
    add("atl_now", d.form.as_ref().map(|f| f.atl));
    add("tsb_now", d.form.as_ref().map(|f| f.tsb));
    add("ramp_ctl_7d", d.ramp.current);
    add("ramp_limit_ctl_7d", Some(d.ramp.limit));
    add("easy_share_28d", d.distribution.window_easy_percent);
    add("easy_share_min", Some(d.distribution.min_easy_percent));
    add("adherence_last_week", d.adherence.ratio_percent);
    add("ef_28d", d.efficiency.current);
    add("ef_change_28d_pct", d.efficiency.change_percent);
    out
}

// --- Vorbereitung --------------------------------------------------------------------

fn chat_error(e: &ChatError, cancelled: bool) -> AppError {
    AppError::new(match e {
        ChatError::EmptyMessage => "empty_message",
        ChatError::MessageTooLong { .. } => "message_too_long",
        ChatError::InvalidMetric | ChatError::TooManyMetrics { .. } => "internal",
        ChatError::Timeout { .. } => "coach_timeout",
        ChatError::Transport { .. } if cancelled => "coach_cancelled",
        ChatError::Transport { .. } => "coach_unreachable",
        ChatError::InvalidOutput { .. } => "coach_invalid_output",
    })
}

/// Prüft den Fokus der Oberfläche (SPEC 9.7). Nur bekannte Codes kommen durch, alles andere
/// ist `invalid_value` (vorhandener Code für ungültige Argumente): So kann über den Fokus kein Freitext zum Modell gelangen.
pub fn parse_focus(focus: Option<&ChatFocusDto>) -> Result<Option<ChatFocus>> {
    focus
        .map(|f| {
            ChatFocus::from_codes(&f.view, f.topic.as_deref())
                .ok_or_else(|| AppError::new("invalid_value"))
        })
        .transpose()
}

/// Der Plan, auf den sich das Gespräch bezieht: der aktive, sonst der neueste Entwurf.
fn base_plan(store: &Store) -> Result<Plan> {
    if let Some(plan) = store.active_plan()? {
        return Ok(plan);
    }
    for s in store.list_plans()? {
        if s.status == PlanStatus::Draft
            && let Some(plan) = store.load_plan(&s.id)?
        {
            return Ok(plan);
        }
    }
    Err(AppError::new("no_plan"))
}

fn ui_locale(store: &Store) -> Result<Locale> {
    Ok(match store.get_setting(KEY_LOCALE)?.as_deref() {
        Some("en") => Locale::En,
        _ => Locale::De,
    })
}

struct Prepared {
    input: ChatInput,
    base: Plan,
    locale: Locale,
}

fn prepare(store: &Store, today: NaiveDate, focus: Option<ChatFocus>) -> Result<Prepared> {
    let base = base_plan(store)?;
    let (rules, catalog) = planning::load_rules().map_err(|_| AppError::new("internal"))?;
    let mut notes = Vec::new();
    let ctx = planning::validation_context(store, &base, &rules, catalog, &mut notes)
        .map_err(|_| AppError::new("plan_failed"))?;
    // Ohne Kennzahlen bleibt das Gespräch möglich; ein Fehler im Dashboard ist dann eine
    // Lücke im Eingang und kein Grund, den Coach zu sperren.
    let metrics = dashboard::dashboard(store, &today.to_string())
        .map(|d| chat_metrics(&d))
        .unwrap_or_default();
    let input =
        ChatInput::build(&base, &ctx, &rules, metrics, focus).map_err(|e| chat_error(&e, false))?;
    Ok(Prepared {
        input,
        base,
        locale: ui_locale(store)?,
    })
}

/// Der exakte Eingang der nächsten Nachricht (F24). `message` darf leer sein: Dann zeigt die
/// Vorschau Anweisung, Eingang und bisherigen Verlauf, die Nachricht fehlt noch. `focus` wie
/// bei [`chat`].
pub fn preview(
    store: &Store,
    state: &CoachState,
    message: &str,
    focus: Option<&ChatFocusDto>,
    today: NaiveDate,
) -> Result<PromptPreview> {
    let focus = parse_focus(focus)?;
    let prepared = prepare(store, today, focus)?;
    let history = state.0.lock().history.clone();
    let text = message.trim();
    let mut messages = if text.is_empty() {
        let mut m = chat_messages(&prepared.input, &history, "-", prepared.locale);
        m.pop();
        m
    } else {
        let text = prepare_user_text(text).map_err(|e| chat_error(&e, false))?;
        chat_messages(&prepared.input, &history, &text, prepared.locale)
    };
    let resolved = resolve(&read_config(store)?);
    let status = resolved.status;
    Ok(PromptPreview {
        provider: ProviderInfo {
            name: status.provider,
            base_url: status.base_url,
            model: status.model,
        },
        messages: messages
            .drain(..)
            .map(|m| PreviewMessage {
                role: match m.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                content: m.content,
            })
            .collect(),
    })
}

// --- Vorher und Nachher --------------------------------------------------------------

fn round1(v: f32) -> f64 {
    (f64::from(v) * 10.0).round() / 10.0
}

fn figures(w: &PlanWeek) -> WeekFigures {
    WeekFigures {
        minutes: w.total_minutes(),
        hard: u32::try_from(
            w.days
                .iter()
                .flat_map(|d| d.sessions.iter())
                .filter(|s| s.hard)
                .count(),
        )
        .unwrap_or(u32::MAX),
        load: w.planned_load.map(round1),
    }
}

/// Vorher und Nachher je Woche, nach dem Beginn der Woche gepaart.
pub fn compare_plans(before: &Plan, after: &Plan) -> Vec<WeekComparison> {
    let mut starts: Vec<NaiveDate> = before
        .weeks
        .iter()
        .chain(&after.weeks)
        .map(|w| w.week_start)
        .collect();
    starts.sort_unstable();
    starts.dedup();
    starts
        .into_iter()
        .map(|s| WeekComparison {
            week_start: s.to_string(),
            before: before.weeks.iter().find(|w| w.week_start == s).map(figures),
            after: after.weeks.iter().find(|w| w.week_start == s).map(figures),
        })
        .collect()
}

fn slot_name(s: Slot) -> &'static str {
    match s {
        Slot::Am => "am",
        Slot::Pm => "pm",
    }
}

fn edit_line(e: &Edit) -> EditLine {
    match e {
        Edit::Replace {
            date,
            slot,
            template_id,
            duration_min,
        } => EditLine {
            op: "replace",
            date: date.to_string(),
            slot: slot_name(*slot),
            template_id: Some(template_id.clone()),
            duration_min: Some(*duration_min),
            to_date: None,
            to_slot: None,
        },
        Edit::Move {
            from_date,
            from_slot,
            to_date,
            to_slot,
        } => EditLine {
            op: "move",
            date: from_date.to_string(),
            slot: slot_name(*from_slot),
            template_id: None,
            duration_min: None,
            to_date: Some(to_date.to_string()),
            to_slot: Some(slot_name(*to_slot)),
        },
        Edit::Remove { date, slot } => EditLine {
            op: "remove",
            date: date.to_string(),
            slot: slot_name(*slot),
            template_id: None,
            duration_min: None,
            to_date: None,
            to_slot: None,
        },
    }
}

fn availability_dtos(days: &[lastwerk_core::plan::DayAvailability; 7]) -> Vec<AvailabilityDay> {
    (0u8..)
        .zip(days)
        .map(|(weekday, d)| AvailabilityDay {
            weekday,
            max_minutes: d.max_minutes,
            allow_hard: d.allow_hard,
            fixed_rest: d.fixed_rest,
        })
        .collect()
}

// --- Eingaben des Regelplaners aus einem planning_change -----------------------------

/// Ausdauersportarten des Plans in der Reihenfolge ihres ersten Auftretens.
fn plan_sports(plan: &Plan) -> Vec<PlanSport> {
    let mut out = Vec::new();
    for s in plan
        .weeks
        .iter()
        .flat_map(|w| w.days.iter())
        .flat_map(|d| d.sessions.iter())
    {
        if s.sport.is_endurance() && !out.contains(&s.sport) {
            out.push(s.sport);
        }
    }
    out
}

/// Krafteinheiten je Woche im Plan (die meisten in einer Woche), höchstens 2 (R7).
fn plan_strength(plan: &Plan) -> u8 {
    let most = plan
        .weeks
        .iter()
        .map(|w| {
            w.days
                .iter()
                .flat_map(|d| d.sessions.iter())
                .filter(|s| s.sport == PlanSport::Strength)
                .count()
        })
        .max()
        .unwrap_or(DEFAULT_STRENGTH_PER_WEEK.into());
    u8::try_from(most.min(2)).unwrap_or(2)
}

/// Anfrage an den Regelplaner. Felder, die der Vorschlag nicht nennt, bleiben wie im
/// Basisplan. Der neue Plan ersetzt den laufenden und beginnt deshalb am Montag der heutigen
/// Woche (ein Plan, der erst in der Zukunft beginnt, behält seinen Beginn). Vergangene Tage
/// dieser Woche tragen damit geplante Einheiten, die in der Planerfüllung als offen zählen.
fn change_request(
    base: &Plan,
    change: &PlanningChange,
    today: NaiveDate,
    locale: Locale,
) -> PlanRequest {
    let this_monday = today - TimeDelta::days(i64::from(today.weekday().num_days_from_monday()));
    let week_start = match base.weeks.first() {
        Some(first) if first.week_start > this_monday => first.week_start,
        _ => this_monday,
    };
    let weeks = change.weeks.or_else(|| {
        let last = base.weeks.last()?.week_start;
        let n = (last - week_start).num_days().div_euclid(7) + 1;
        (n >= 1).then(|| u32::try_from(n.min(i64::from(plans::MAX_WEEKS))).unwrap_or(1))
    });
    PlanRequest {
        week_start,
        goal_date: None,
        goal_sport: None,
        weeks,
        locale: if base.weeks.is_empty() {
            locale
        } else {
            base.locale
        },
        sports: change.sports.clone().unwrap_or_else(|| plan_sports(base)),
        strength_per_week: change
            .strength_per_week
            .unwrap_or_else(|| plan_strength(base)),
    }
}

// --- Gespräch ------------------------------------------------------------------------

/// Kurzlebig (ein Wert je Antwort), Boxen brächte nichts.
#[allow(clippy::large_enum_variant)]
enum Built {
    Ready(ProposalPreview, Pending),
    Rejected(&'static str),
}

fn build_proposal(
    proposal: CheckedProposal,
    prepared: &Prepared,
    store: &Store,
    today: NaiveDate,
    now: DateTime<Utc>,
    id: String,
) -> Built {
    let base_plan_id = prepared.base.plan_id.clone();
    match proposal {
        CheckedProposal::PlanEdit {
            edits,
            explanation,
            plan,
        } => {
            let summary = ProposalSummary {
                weeks: compare_plans(&prepared.base, &plan),
                edits: edits.iter().map(edit_line).collect(),
                change: None,
                warnings: u32::try_from(plan.warnings.len()).unwrap_or(u32::MAX),
            };
            Built::Ready(
                ProposalPreview {
                    id: id.clone(),
                    kind: "plan_edit",
                    explanation: Some(explanation),
                    summary,
                },
                Pending {
                    id,
                    base_plan_id,
                    stored: Stored::Edit {
                        plan: Box::new(plan),
                    },
                },
            )
        }
        CheckedProposal::PlanningChange(change) => {
            let req = change_request(&prepared.base, &change, today, prepared.locale);
            // Erzeugen ohne zu speichern: Der Regelplaner zeigt, was aus den neuen Eingaben
            // würde. Scheitert er, ist der Vorschlag nicht umsetzbar (SPEC 9.7, 3).
            let Ok(generated) =
                planning::generate_with(store, &req, change.availability, id.clone(), now)
            else {
                return Built::Rejected("PLAN_FAILED");
            };
            let summary = ProposalSummary {
                weeks: compare_plans(&prepared.base, &generated.plan),
                edits: Vec::new(),
                change: Some(ChangeLine {
                    availability: change.availability.as_ref().map(availability_dtos),
                    sports: change.sports.clone(),
                    strength_per_week: change.strength_per_week,
                    weeks: change.weeks,
                    week_start: req.week_start.to_string(),
                }),
                warnings: u32::try_from(generated.plan.warnings.len()).unwrap_or(u32::MAX),
            };
            Built::Ready(
                ProposalPreview {
                    id: id.clone(),
                    kind: "planning_change",
                    explanation: None,
                    summary,
                },
                Pending {
                    id,
                    base_plan_id,
                    stored: Stored::Change(change),
                },
            )
        }
    }
}

/// Ein Gesprächsschritt mit einem fertigen Client (SPEC 9.7). `new_id` vergibt die Nummer
/// eines Vorschlags. Der Verlauf wächst nur, wenn das Modell geantwortet hat. `focus` ist die
/// Ansicht der Oberfläche als Code ([`parse_focus`]); er gilt nur für diesen Schritt und steht
/// nicht im Verlauf.
#[allow(clippy::too_many_arguments)]
pub fn chat(
    store: &Store,
    state: &CoachState,
    client: &dyn LlmClient,
    message: &str,
    focus: Option<&ChatFocusDto>,
    today: NaiveDate,
    now: DateTime<Utc>,
    new_id: &dyn Fn() -> String,
) -> Result<CoachReply> {
    let shared = &state.0;
    let text = prepare_user_text(message).map_err(|e| chat_error(&e, false))?;
    let focus = parse_focus(focus)?;
    let prepared = prepare(store, today, focus)?;
    let history = shared.lock().history.clone();
    let epoch = shared.epoch.load(Ordering::SeqCst);
    let guarded = Cancelable {
        inner: client,
        cancel: &shared.cancel,
    };
    let cfg = RefineConfig {
        timeout_s: DEFAULT_TIMEOUT_S,
        ..RefineConfig::default()
    };
    let outcome = chat_turn(
        &guarded,
        &prepared.input,
        &history,
        &text,
        prepared.locale,
        &cfg,
    )
    .map_err(|e| chat_error(&e, shared.cancel.load(Ordering::SeqCst)))?;
    if shared.epoch.load(Ordering::SeqCst) != epoch {
        return Err(AppError::new("coach_cancelled"));
    }

    let mut rejected_codes = outcome.rejected_codes.clone();
    let mut preview = None;
    let mut pending = None;
    if let Some(p) = outcome.proposal.clone() {
        match build_proposal(p, &prepared, store, today, now, new_id()) {
            Built::Ready(pr, pe) => {
                preview = Some(pr);
                pending = Some(pe);
            }
            Built::Rejected(code) => rejected_codes.push(code.to_owned()),
        }
    }

    let mut inner = shared.lock();
    // Zwischen Prüfung und Eintrag kann ein Zurücksetzen liegen.
    if shared.epoch.load(Ordering::SeqCst) != epoch {
        return Err(AppError::new("coach_cancelled"));
    }
    inner.history.record(&text, &outcome);
    if let Some(p) = pending {
        inner.pending.push_back(p);
        while inner.pending.len() > MAX_PENDING {
            inner.pending.pop_front();
        }
    }
    Ok(CoachReply {
        reply: outcome.reply,
        proposal: preview,
        rejected_codes,
    })
}

/// Wie [`chat`], aber mit dem eingestellten Anbieter und dem echten Client. Ohne erreichbares
/// Modell `coach_unavailable`.
pub fn chat_with_provider(
    store: &Store,
    state: &CoachState,
    message: &str,
    focus: Option<&ChatFocusDto>,
    today: NaiveDate,
    now: DateTime<Utc>,
    new_id: &dyn Fn() -> String,
) -> Result<CoachReply> {
    // Erst Nachricht und Fokus prüfen: Beides braucht keine Abfrage des Servers.
    prepare_user_text(message).map_err(|e| chat_error(&e, false))?;
    parse_focus(focus)?;
    let provider = resolve(&read_config(store)?)
        .provider
        .ok_or_else(|| AppError::new("coach_unavailable"))?;
    let client = provider.client();
    chat(store, state, &*client, message, focus, today, now, new_id)
}

// --- Übernehmen ----------------------------------------------------------------------

/// Übernimmt einen Vorschlag (SPEC 9.7, 4): `plan_edit` speichert den geprüften Plan mit neuer
/// ID als aktiven Plan, `planning_change` schreibt die Verfügbarkeit, erzeugt mit dem
/// Regelplaner einen neuen Plan (geprüft gegen Schema und harte Regeln) und aktiviert ihn. Der
/// bisherige aktive Plan wird archiviert, alle übrigen offenen Vorschläge verfallen, weil sich
/// der Basisplan geändert hat.
pub fn apply(
    store: &mut Store,
    state: &CoachState,
    proposal_id: &str,
    today: NaiveDate,
    now: DateTime<Utc>,
    new_plan_id: String,
) -> Result<PlanDetail> {
    let pending = state
        .0
        .lock()
        .pending
        .iter()
        .find(|p| p.id == proposal_id)
        .cloned()
        .ok_or_else(|| AppError::new("not_found"))?;
    let base = base_plan(store)?;
    if base.plan_id != pending.base_plan_id {
        // Ein veralteter Vorschlag wird nie mehr gültig.
        state.0.lock().pending.retain(|p| p.id != proposal_id);
        return Err(AppError::new("proposal_stale"));
    }
    let saved_id = match pending.stored {
        Stored::Edit { plan } => {
            let mut plan = *plan;
            plan.plan_id = new_plan_id;
            plan.created_at = now;
            planning::plan_json(&plan).map_err(|_| AppError::new("plan_invalid"))?;
            let goal_id = store.active_goal()?.and_then(|g| {
                (g.goal.kind == plan.goal.kind && g.goal.event_date == plan.goal.event_date)
                    .then_some(g.id)
            });
            store.save_plan_for_goal(&plan, PlanStatus::Active, goal_id)?;
            plan.plan_id
        }
        Stored::Change(change) => {
            let locale = ui_locale(store)?;
            let req = change_request(&base, &change, today, locale);
            let generated =
                planning::generate_with(store, &req, change.availability, new_plan_id, now)
                    .map_err(|_| AppError::new("plan_failed"))?;
            planning::plan_json(&generated.plan).map_err(|_| AppError::new("plan_invalid"))?;
            if let Some(days) = &change.availability {
                plans::set_availability(store, &availability_dtos(days))?;
            }
            store.save_plan_for_goal(&generated.plan, PlanStatus::Active, generated.goal_id)?;
            generated.plan.plan_id
        }
    };
    state.0.lock().pending.clear();
    plans::get_plan(store, &saved_id)
}
