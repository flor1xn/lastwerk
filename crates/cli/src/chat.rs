//! Befehl `chat`: ein Gesprächsschritt mit dem lokalen Coach (SPEC 9.7, T26 Teil A), gedacht
//! für Versuche mit einem echten Modell. Es gibt keinen Verlauf über den Aufruf hinaus, und
//! nichts wird gespeichert: Ein Vorschlag wird nur angezeigt, übernehmen kann ihn erst die App.
//!
//! Ablauf: Anbieter anlegen (Adresse geprüft, sonst Exit 2; ohne `--model` fragt der Befehl bei
//! Ollama die vorhandenen Modelle ab und wählt eines mit `pick_model`), Plan lesen (Datei oder
//! der aktive Plan der Datenbank), Kontext aus der Datenbank bilden, Nachricht prüfen, mit
//! `--show-input` genau den Text anzeigen, der zum Modell geht, dann `chat_turn`.
//!
//! Kennzahlen des Dashboards (T25) gibt es noch nicht; der Eingang trägt eine leere Liste
//! `metrics`. Die Antwort ist Anzeigetext und wird wie bei `refine` ohne Steuerzeichen als Text
//! ausgegeben. Antwortet das Modell nicht oder unbrauchbar, steht der Grund auf der Ausgabe und
//! der Exit-Code bleibt 0 (wie bei `refine`: es hat sich nichts geändert).

use std::io::Write;
use std::path::PathBuf;

use lastwerk_coach::chat::prepare_user_text;
use lastwerk_coach::edit::Edit;
use lastwerk_coach::{
    BaseUrl, ChatError, ChatHistory, ChatInput, CheckedProposal, LlmClient, OLLAMA_DEFAULT_URL,
    PlanningChange, Provider, RefineConfig, chat_messages, chat_turn, ollama_models, pick_model,
};
use lastwerk_core::plan::{Plan, PlanSport, Slot};
use lastwerk_store::Store;

use crate::error::CliError;
use crate::planning;
use crate::refine::{ProviderChoice, io_error, make_provider, plain_text, provider_error};

/// Zeitlimit für die Abfrage der Modellliste in Sekunden (Konvention: eine lokale Liste
/// antwortet sofort, länger wartet niemand auf einen Fehler).
pub const DISCOVER_TIMEOUT_S: u32 = 10;

/// Eingaben von `lastwerk chat`.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// Plan als Datei. Ohne Angabe gilt der aktive Plan der Datenbank.
    pub plan: Option<PathBuf>,
    pub message: String,
    pub provider: ProviderChoice,
    /// Ohne Angabe wählt der Befehl bei Ollama ein vorhandenes Modell.
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub allow_lan: bool,
    /// Zeigt vor dem Senden genau den Text, der zum Modell geht (F24).
    pub show_input: bool,
    pub timeout_s: u32,
}

/// Legt den Anbieter an. Ohne `--model` fragt der Befehl Ollama nach den vorhandenen Modellen;
/// die Adresse ist dabei schon geprüft. Fehler sind Eingabefehler (Exit 2).
pub fn resolve_provider(req: &ChatRequest) -> Result<Provider, CliError> {
    let model = match (&req.model, req.provider) {
        (Some(model), _) => model.clone(),
        (None, ProviderChoice::Ollama) => {
            let url = BaseUrl::parse(
                req.base_url.as_deref().unwrap_or(OLLAMA_DEFAULT_URL),
                req.allow_lan,
            )
            .map_err(|e| provider_error(&e))?;
            let models = ollama_models(&url, DISCOVER_TIMEOUT_S).map_err(|e| {
                CliError::Input(format!(
                    "Die Modelle von Ollama lassen sich nicht abfragen ({}). Läuft Ollama? \
                     Sonst bitte --model angeben.",
                    e.code()
                ))
            })?;
            pick_model(&models).ok_or_else(|| {
                CliError::Input(
                    "Ollama hat kein Modell, das antworten kann. Bitte eines laden \
                     (`ollama pull <modell>`) oder --model angeben."
                        .into(),
                )
            })?
        }
        (None, ProviderChoice::OpenAiCompatible) => {
            return Err(CliError::Input(
                "Für openai-compatible bitte --model angeben.".into(),
            ));
        }
    };
    make_provider(req.provider, req.base_url.as_deref(), &model, req.allow_lan)
}

fn slot_name(slot: Slot) -> &'static str {
    match slot {
        Slot::Am => "am",
        Slot::Pm => "pm",
    }
}

fn sport_name(sport: PlanSport) -> &'static str {
    match sport {
        PlanSport::Cycling => "cycling",
        PlanSport::Running => "running",
        PlanSport::Swimming => "swimming",
        PlanSport::Strength => "strength",
        PlanSport::Rest => "rest",
    }
}

fn describe_edit(edit: &Edit) -> String {
    match edit {
        Edit::Replace {
            date,
            slot,
            template_id,
            duration_min,
        } => format!(
            "ersetzen {date} {} durch {template_id}, {duration_min} min",
            slot_name(*slot)
        ),
        Edit::Move {
            from_date,
            from_slot,
            to_date,
            to_slot,
        } => format!(
            "verschieben {from_date} {} nach {to_date} {}",
            slot_name(*from_slot),
            slot_name(*to_slot)
        ),
        Edit::Remove { date, slot } => format!("streichen {date} {}", slot_name(*slot)),
    }
}

/// Vorher und Nachher je Woche: Minuten, harte Einheiten, geplante Last (von `core` gerechnet).
fn compare_weeks(before: &Plan, after: &Plan) -> Vec<String> {
    let hard = |w: &lastwerk_core::plan::PlanWeek| {
        w.days
            .iter()
            .flat_map(|d| d.sessions.iter())
            .filter(|s| s.hard)
            .count()
    };
    let load = |w: &lastwerk_core::plan::PlanWeek| {
        w.planned_load
            .map_or_else(|| "-".to_string(), |l| format!("{l:.1}"))
    };
    before
        .weeks
        .iter()
        .zip(&after.weeks)
        .map(|(b, a)| {
            format!(
                "  Woche ab {}: {} -> {} min, hart {} -> {}, Last {} -> {}",
                b.week_start,
                b.total_minutes(),
                a.total_minutes(),
                hard(b),
                hard(a),
                load(b),
                load(a)
            )
        })
        .collect()
}

fn describe_change(change: &PlanningChange) -> Vec<String> {
    const DAYS: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];
    let mut lines = Vec::new();
    if let Some(days) = &change.availability {
        let list: Vec<String> = days
            .iter()
            .zip(DAYS)
            .map(|(d, name)| {
                if d.fixed_rest {
                    format!("{name} Ruhetag")
                } else {
                    format!(
                        "{name} {} min{}",
                        d.max_minutes,
                        if d.allow_hard { "" } else { " locker" }
                    )
                }
            })
            .collect();
        lines.push(format!("  Verfügbarkeit: {}", list.join(", ")));
    }
    if let Some(sports) = &change.sports {
        let names: Vec<&str> = sports.iter().map(|s| sport_name(*s)).collect();
        lines.push(format!("  Sportarten: {}", names.join(", ")));
    }
    if let Some(n) = change.strength_per_week {
        lines.push(format!("  Kraft je Woche: {n}"));
    }
    if let Some(n) = change.weeks {
        lines.push(format!("  Wochen: {n}"));
    }
    lines
}

fn failure_text(e: &ChatError, timeout_s: u32) -> String {
    match e {
        ChatError::Timeout { .. } => {
            format!("Das Modell hat nicht innerhalb von {timeout_s} Sekunden geantwortet.")
        }
        ChatError::Transport { error, .. } => format!(
            "Der Modellserver war nicht erreichbar oder antwortete unbrauchbar ({}).",
            error.code()
        ),
        ChatError::InvalidOutput { issues, .. } => {
            let list: Vec<&str> = issues.iter().map(|i| i.code.as_str()).collect();
            format!(
                "Auch nach den erlaubten Korrekturen war die Antwort nicht lesbar: {}.",
                list.join(", ")
            )
        }
        other => format!("Fehler ({}).", other.code()),
    }
}

fn calls_of(e: &ChatError) -> u32 {
    match e {
        ChatError::Timeout { calls }
        | ChatError::Transport { calls, .. }
        | ChatError::InvalidOutput { calls, .. } => *calls,
        _ => 0,
    }
}

/// Prüft die Nachricht (nicht leer, höchstens 1000 Zeichen). Läuft vor allem anderen, damit
/// bei einer unbrauchbaren Nachricht auch keine Modellliste abgefragt wird (Exit 2).
pub fn check_message(req: &ChatRequest) -> Result<String, CliError> {
    prepare_user_text(&req.message).map_err(|e| {
        CliError::Input(match e {
            ChatError::MessageTooLong { max } => {
                format!("Die Nachricht ist länger als {max} Zeichen.")
            }
            _ => "Die Nachricht ist leer.".into(),
        })
    })
}

/// Führt einen Gesprächsschritt mit einem fertigen Anbieter und Client aus.
pub fn run_chat(
    store: &Store,
    req: &ChatRequest,
    provider: &Provider,
    client: &dyn LlmClient,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let text = check_message(req)?;
    let plan = match &req.plan {
        Some(path) => planning::read_plan(path)?,
        None => store.active_plan()?.ok_or_else(|| {
            CliError::Input(
                "Es gibt keinen aktiven Plan. Bitte --plan angeben oder einen Plan mit \
                 `lastwerk plan-status` aktivieren."
                    .into(),
            )
        })?,
    };
    let (rules, catalog) = planning::load_rules()?;
    let mut notes = Vec::new();
    let ctx = planning::validation_context(store, &plan, &rules, catalog, &mut notes)?;
    for note in &notes {
        writeln!(err, "{note}").map_err(|e| io_error(&e))?;
    }
    let input = ChatInput::build(&plan, &ctx, &rules, Vec::new())
        .map_err(|e| CliError::Internal(format!("Eingang ({})", e.code())))?;
    let history = ChatHistory::new();
    if req.show_input {
        writeln!(
            out,
            "Dieser Text geht an das lokale Modell ({}, {}, Modell {}):",
            provider.name(),
            provider.base_url(),
            provider.model()
        )
        .map_err(|e| io_error(&e))?;
        for m in chat_messages(&input, &history, &text, plan.locale) {
            writeln!(out, "--- {} ---\n{}", m.role.as_str(), m.content)
                .map_err(|e| io_error(&e))?;
        }
        writeln!(out, "---").map_err(|e| io_error(&e))?;
    }
    let cfg = RefineConfig {
        timeout_s: req.timeout_s,
        ..RefineConfig::default()
    };
    let outcome = match chat_turn(client, &input, &history, &text, plan.locale, &cfg) {
        Ok(outcome) => outcome,
        Err(e) => {
            return writeln!(
                out,
                "Keine Antwort des Coachs: {} (Grund: {}, {} Aufrufe).",
                failure_text(&e, req.timeout_s),
                e.code(),
                calls_of(&e)
            )
            .map_err(|e| io_error(&e));
        }
    };
    let mut lines = vec![
        format!(
            "Antwort des Coachs ({} Aufrufe, reiner Text):",
            outcome.calls
        ),
        plain_text(&outcome.reply),
    ];
    match &outcome.proposal {
        None if outcome.rejected_codes.is_empty() => {}
        None => lines.push(format!(
            "Der Vorschlag des Modells wurde verworfen: {}.",
            outcome.rejected_codes.join(", ")
        )),
        Some(CheckedProposal::PlanEdit {
            edits, plan: new, ..
        }) => {
            lines.push(format!(
                "Vorschlag (Änderungen am Plan, geprüft, {} Warnungen):",
                new.warnings.len()
            ));
            lines.extend(edits.iter().map(|e| format!("  {}", describe_edit(e))));
            lines.extend(compare_weeks(&plan, new));
        }
        Some(CheckedProposal::PlanningChange(change)) => {
            lines.push(
                "Vorschlag (neue Eingaben für den Regelplaner, Grenzen geprüft; den Plan \
                 erzeugt und prüft die App):"
                    .into(),
            );
            lines.extend(describe_change(change));
        }
    }
    if outcome.proposal.is_some() {
        lines.push("Nichts wurde übernommen oder gespeichert.".into());
    }
    writeln!(out, "{}", lines.join("\n")).map_err(|e| io_error(&e))
}
