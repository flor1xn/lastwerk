//! Befehl `refine`: ein lokales Sprachmodell passt einen Regelplan an und erklärt ihn
//! (SPEC 5.4, 9, Task T15).
//!
//! Ablauf: Plan lesen und gegen Schema und harte Regeln prüfen, Anbieter anlegen (Adresse
//! geprüft, sonst Exit 2), den exakten Eingang anzeigen und eine Bestätigung verlangen (F24;
//! `--yes` für Skripte, ohne Terminal und ohne `--yes` Exit 2), dann `refine_plan`. Bei
//! `Applied` wird ein neuer Plan mit neuer ID als Entwurf gespeichert und neben die Eingabe
//! geschrieben, bei `Fallback` bleibt alles wie es war und der Grund steht auf der Ausgabe
//! (Exit 0: der Regelplan ist weiterhin gültig).
//!
//! Die Erklärung des Modells ist Anzeigetext. Sie wird als Text ausgegeben, nie gerendert;
//! Steuerzeichen (zum Beispiel Escape-Folgen für das Terminal) ersetzt die Ausgabe durch `?`,
//! der Plan behält den Text unverändert.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use lastwerk_coach::{
    FallbackReason, LlmClient, Provider, RefineConfig, RefineInput, RefineOutcome, refine_plan,
    system_prompt,
};
use lastwerk_core::plan::{Hardness, Plan};
use lastwerk_store::{PlanStatus, Store};

use crate::error::CliError;
use crate::planning;

/// Welcher Anbieter (Argument `--provider`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderChoice {
    Ollama,
    OpenAiCompatible,
}

/// Eingaben von `lastwerk refine`.
#[derive(Debug, Clone)]
pub struct RefineRequest {
    pub plan: PathBuf,
    pub provider: ProviderChoice,
    pub model: String,
    pub base_url: Option<String>,
    /// Ausdrückliche Bestätigung für einen Host im lokalen Netz.
    pub allow_lan: bool,
    pub out: Option<PathBuf>,
    /// Keine Rückfrage (für Skripte).
    pub yes: bool,
    pub timeout_s: u32,
}

/// Legt den Anbieter an. Eine nicht erlaubte Adresse ist ein Eingabefehler (Exit 2).
pub fn provider_for(req: &RefineRequest) -> Result<Provider, CliError> {
    make_provider(
        req.provider,
        req.base_url.as_deref(),
        &req.model,
        req.allow_lan,
    )
}

/// Legt einen Anbieter aus den Angaben der Kommandozeile an (auch für `chat`). Eine nicht
/// erlaubte Adresse ist ein Eingabefehler (Exit 2).
pub fn make_provider(
    choice: ProviderChoice,
    base_url: Option<&str>,
    model: &str,
    allow_lan: bool,
) -> Result<Provider, CliError> {
    let provider = match choice {
        ProviderChoice::Ollama => Provider::ollama(base_url, model, allow_lan),
        ProviderChoice::OpenAiCompatible => {
            let Some(url) = base_url else {
                return Err(CliError::Input(
                    "Für openai-compatible gibt es keine Standardadresse. Bitte --base-url \
                     angeben, zum Beispiel http://127.0.0.1:1234."
                        .into(),
                ));
            };
            Provider::openai_compatible(url, model, allow_lan)
        }
    };
    provider.map_err(|e| provider_error(&e))
}

/// Meldung zu einem abgelehnten Anbieter (Exit 2).
pub fn provider_error(e: &lastwerk_coach::ProviderError) -> CliError {
    use lastwerk_coach::ProviderError as E;
    CliError::Input(match e {
        E::HostNotAllowed {
            host,
            lan_confirmation_missing: true,
        } => format!(
            "Der Host {host} liegt im lokalen Netz. Lastwerk ruft ihn nur nach ausdrücklicher \
                 Bestätigung auf: bitte --allow-lan angeben."
        ),
        E::HostNotAllowed { host, .. } => format!(
            "Der Host {host} ist nicht erlaubt. Lastwerk ruft nur Loopback (127.0.0.1, ::1, \
                 localhost) oder, mit --allow-lan, Adressen im lokalen Netz auf, nie einen \
                 öffentlichen Host und keinen Cloud-Dienst."
        ),
        E::TlsNotSupported => "https wird in dieser Version nicht unterstützt. Für lokale \
                Server genügt http."
            .into(),
        E::SchemeNotAllowed(s) => {
            format!("Das Schema {s:?} ist nicht erlaubt, erwartet wird http.")
        }
        E::InvalidUrl(what) => format!("Die Adresse ist ungültig ({what})."),
        E::InvalidModel => "Der Modellname ist leer, zu lang oder enthält Steuerzeichen.".into(),
    })
}

pub(crate) fn io_error(e: &std::io::Error) -> CliError {
    CliError::Internal(format!("Ausgabefehler: {}", e.kind()))
}

/// Ersetzt Steuerzeichen außer Zeilenumbruch und Tabulator durch `?`.
pub fn plain_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                '?'
            } else {
                c
            }
        })
        .collect()
}

/// Standardpfad der Ausgabe: neben der Eingabe, `<name>.refined.json`.
pub fn default_out_path(plan: &Path) -> PathBuf {
    let stem = plan
        .file_stem()
        .map_or_else(|| "plan".to_string(), |s| s.to_string_lossy().into_owned());
    plan.with_file_name(format!("{stem}.refined.json"))
}

fn fallback_text(reason: &FallbackReason, timeout_s: u32) -> String {
    match reason {
        FallbackReason::BaseInvalid => {
            "Der Plan verletzt selbst harte Regeln, es gibt nichts zu verfeinern.".into()
        }
        FallbackReason::Timeout => {
            format!("Das Modell hat nicht innerhalb von {timeout_s} Sekunden geantwortet.")
        }
        FallbackReason::Transport(e) => format!(
            "Der Modellserver war nicht erreichbar oder antwortete unbrauchbar ({}).",
            e.code()
        ),
        FallbackReason::InvalidOutput(issues) => {
            let list: Vec<String> = issues
                .iter()
                .map(|i| match i.date {
                    Some(d) => format!("{} ({d})", i.code),
                    None => i.code.clone(),
                })
                .collect();
            format!(
                "Auch nach den erlaubten Korrekturen war die Antwort unbrauchbar: {}.",
                list.join(", ")
            )
        }
    }
}

/// Zeigt den Eingang an und holt die Bestätigung. `Ok(true)` heißt: senden.
fn confirm(
    req: &RefineRequest,
    provider: &Provider,
    base: &Plan,
    input_json: &str,
    reply: &mut dyn BufRead,
    out: &mut dyn Write,
) -> Result<bool, CliError> {
    writeln!(
        out,
        "Dieser Text geht an das lokale Modell ({}, {}, Modell {}):\n\n--- Anweisung (fest) ---\n{}\n\
         --- Eingang (JSON) ---\n{}\n---",
        provider.name(),
        provider.base_url(),
        provider.model(),
        system_prompt(base.locale).trim_end(),
        input_json
    )
    .map_err(|e| io_error(&e))?;
    if req.yes {
        return Ok(true);
    }
    write!(out, "Senden? [j/N] ").map_err(|e| io_error(&e))?;
    out.flush().map_err(|e| io_error(&e))?;
    let mut line = String::new();
    reply.read_line(&mut line).map_err(|e| {
        CliError::Input(format!("Die Eingabe lässt sich nicht lesen: {}", e.kind()))
    })?;
    Ok(matches!(
        line.trim().to_lowercase().as_str(),
        "j" | "ja" | "y" | "yes"
    ))
}

/// `goal.id` des aktiven Ziels, wenn es zum Ziel des Plans passt.
fn matching_goal_id(store: &Store, plan: &Plan) -> Result<Option<i64>, CliError> {
    Ok(store.active_goal()?.and_then(|g| {
        (g.goal.kind == plan.goal.kind && g.goal.event_date == plan.goal.event_date).then_some(g.id)
    }))
}

/// Führt `refine` mit einem fertigen Anbieter und Client aus. `reply` liefert die Antwort auf
/// die Rückfrage, `interactive` sagt, ob dort ein Terminal hängt. `new_plan_id` und `now`
/// bekommt der neue Plan, weil `refine_plan` weder Zufall noch Uhr kennt.
#[allow(clippy::too_many_arguments)]
pub fn run_refine(
    store: &mut Store,
    req: &RefineRequest,
    provider: &Provider,
    client: &dyn LlmClient,
    new_plan_id: String,
    now: DateTime<Utc>,
    reply: &mut dyn BufRead,
    interactive: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    if !req.yes && !interactive {
        return Err(CliError::Input(
            "Ohne Terminal ist keine Bestätigung möglich. Mit --yes wird der Eingang ohne \
             Rückfrage gesendet."
                .into(),
        ));
    }
    let base = planning::read_plan(&req.plan)?;
    let (rules, catalog) = planning::load_rules()?;
    let mut notes = Vec::new();
    let ctx = planning::validation_context(store, &base, &rules, catalog, &mut notes)?;
    for note in &notes {
        writeln!(err, "{note}").map_err(|e| io_error(&e))?;
    }
    // Ein Plan mit harter Verletzung geht gar nicht erst zum Modell.
    let hard = lastwerk_core::plan::validate_plan(&base, &ctx, &rules)
        .iter()
        .filter(|v| v.hardness == Hardness::Hard)
        .count();
    if hard > 0 {
        return Err(CliError::Validation(format!(
            "Der Plan verletzt {hard} harte Regeln. Bitte zuerst `lastwerk validate` ausführen."
        )));
    }

    let input = RefineInput::build(&base, &ctx, &rules);
    let input_json = input.to_json();
    if !confirm(req, provider, &base, &input_json, reply, out)? {
        writeln!(out, "Abgebrochen. Es wurde nichts gesendet.").map_err(|e| io_error(&e))?;
        return Ok(());
    }

    let cfg = RefineConfig {
        timeout_s: req.timeout_s,
        ..RefineConfig::default()
    };
    match refine_plan(&base, &ctx, &rules, client, &cfg) {
        RefineOutcome::Applied {
            mut plan,
            explanation,
            attempts,
        } => {
            plan.plan_id = new_plan_id;
            plan.created_at = now;
            let json = planning::plan_json(&plan)?;
            let out_path = req
                .out
                .clone()
                .unwrap_or_else(|| default_out_path(&req.plan));
            std::fs::write(&out_path, json).map_err(|e| {
                CliError::Input(format!(
                    "Die Ausgabedatei lässt sich nicht schreiben: {}",
                    e.kind()
                ))
            })?;
            let goal_id = matching_goal_id(store, &plan)?;
            store.save_plan_for_goal(&plan, PlanStatus::Draft, goal_id)?;
            writeln!(
                out,
                "Plan {} vom Modell angepasst: {} Aufrufe, {} Korrekturen, {} Warnungen. Als Entwurf \
                 gespeichert und als Datei geschrieben.\nErklärung des Modells (reiner Text):\n{}",
                plan.plan_id,
                attempts,
                attempts.saturating_sub(1),
                plan.warnings.len(),
                plain_text(&explanation)
            )
            .map_err(|e| io_error(&e))
        }
        RefineOutcome::Fallback {
            reason, attempts, ..
        } => writeln!(
            out,
            "Der Regelplan bleibt. Das Modell hat keinen gültigen Vorschlag geliefert: {} \
             (Grund: {}, {} Aufrufe, {} Korrekturen). Es wurde nichts geschrieben.",
            fallback_text(&reason, req.timeout_s),
            reason.code(),
            attempts,
            attempts.saturating_sub(1)
        )
        .map_err(|e| io_error(&e)),
    }
}
