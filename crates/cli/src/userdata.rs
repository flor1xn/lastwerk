//! Befehle `backup`, `restore`, `goal` und `availability` (SPEC 5.4, 6.1, 6.3).
//!
//! `backup` schreibt die Nutzerdaten (Schwellenwerte, Ziele, Verfügbarkeit, Pläne, manuelle
//! Einheiten, Einstellungen) als JSON mit `backup_version` und `exported_at`. Die Datei kann
//! persönliche Angaben enthalten (Zielbezeichnungen, Pläne), sie gehört nicht ins Repository.
//! Meldungen nennen nie den Pfad der Datei.

use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Utc};
use lastwerk_core::plan::{GoalKind, PlanSport, Priority};
use lastwerk_store::{NewGoal, Store, UserDataExport, plan_sport_str};

use crate::error::CliError;

fn output_error(e: &std::io::Error) -> CliError {
    CliError::Internal(format!("Ausgabefehler: {}", e.kind()))
}

/// Zusammenfassung der Tabellen eines Backups.
fn counts(e: &UserDataExport) -> String {
    format!(
        "{} Schwellenwerte, {} Ziele, {} Verfügbarkeiten, {} Pläne, {} manuelle Einheiten, \
         {} Erholungstage, {} Einstellungen",
        e.thresholds.len(),
        e.goals.len(),
        e.availability.len(),
        e.plans.len(),
        e.manual_sessions.len(),
        e.recovery_days.len(),
        e.settings.len()
    )
}

/// `lastwerk backup`: schreibt die Nutzerdaten nach `out_path`.
pub fn backup(
    store: &Store,
    out_path: &Path,
    now: DateTime<Utc>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let export = store.export_user_data()?.with_exported_at(now);
    let mut text = export.to_json()?;
    text.push('\n');
    std::fs::write(out_path, text).map_err(|e| {
        CliError::Input(format!(
            "Die Sicherungsdatei lässt sich nicht schreiben: {}",
            e.kind()
        ))
    })?;
    writeln!(out, "Sicherung geschrieben: {}.", counts(&export)).map_err(|e| output_error(&e))
}

/// `lastwerk restore`: spielt eine Sicherung ein. Die Nutzertabellen der Datenbank werden
/// vollständig ersetzt (SPEC 6.3), der Cache bleibt unberührt.
pub fn restore(store: &mut Store, in_path: &Path, out: &mut dyn Write) -> Result<(), CliError> {
    let text = std::fs::read_to_string(in_path).map_err(|e| {
        CliError::Input(format!(
            "Die Sicherungsdatei lässt sich nicht lesen: {}",
            e.kind()
        ))
    })?;
    let export = UserDataExport::from_json(&text)
        .map_err(|_| CliError::Input("Die Sicherungsdatei ist keine gültige Sicherung.".into()))?;
    store.import_user_data(&export)?;
    writeln!(
        out,
        "Sicherung eingespielt: {}.\nDen Cache mit `lastwerk rebuild --dir <ordner>` neu \
         aufbauen, damit Aktivitäten und Kennzahlen wieder vorliegen.",
        counts(&export)
    )
    .map_err(|e| output_error(&e))
}

/// Auftrag von `goal set`.
#[derive(Debug)]
pub struct GoalRequest {
    pub date: Option<chrono::NaiveDate>,
    pub sport: Option<PlanSport>,
    pub priority: Option<Priority>,
    pub label: Option<String>,
}

/// `lastwerk goal set`: mit Datum ein Termin, ohne Datum ein fortlaufendes Ziel. Das neue Ziel
/// ist das aktive.
pub fn goal_set(store: &mut Store, req: &GoalRequest, out: &mut dyn Write) -> Result<(), CliError> {
    let kind = if req.date.is_some() {
        GoalKind::Event
    } else {
        GoalKind::Continuous
    };
    let id = store.set_goal(&NewGoal {
        kind,
        sport: req.sport,
        event_date: req.date,
        label: req.label.clone(),
        priority: req.priority,
    })?;
    writeln!(out, "Ziel {id} gesetzt und aktiv.").map_err(|e| output_error(&e))
}

fn priority_label(p: Priority) -> &'static str {
    match p {
        Priority::A => "A",
        Priority::B => "B",
        Priority::C => "C",
    }
}

/// `lastwerk goal list`.
pub fn goal_list(store: &Store, out: &mut dyn Write) -> Result<(), CliError> {
    let goals = store.list_goals()?;
    if goals.is_empty() {
        return writeln!(out, "Keine Ziele gesetzt (`lastwerk goal set`).")
            .map_err(|e| output_error(&e));
    }
    for g in goals {
        let kind = match g.goal.kind {
            GoalKind::Event => "Termin",
            GoalKind::Continuous => "fortlaufend",
        };
        writeln!(
            out,
            "{:>3}  {}  {:<11}  Datum: {}  Sportart: {}  Priorität: {}  Bezeichnung: {}",
            g.id,
            if g.active { "aktiv  " } else { "inaktiv" },
            kind,
            g.goal
                .event_date
                .map_or_else(|| "-".to_string(), |d| d.to_string()),
            g.goal.sport.map_or("-", plan_sport_str),
            g.goal.priority.map_or("-", priority_label),
            g.goal.label.as_deref().unwrap_or("-")
        )
        .map_err(|e| output_error(&e))?;
    }
    Ok(())
}

/// Auftrag von `availability set`.
#[derive(Debug)]
pub struct AvailabilityRequest {
    /// 0 = Montag bis 6 = Sonntag.
    pub weekday: u8,
    pub max_minutes: u16,
    pub allow_hard: bool,
    pub fixed_rest: bool,
}

const WEEKDAYS: [&str; 7] = [
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
];

/// `lastwerk availability set`.
pub fn availability_set(
    store: &mut Store,
    req: &AvailabilityRequest,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    store.set_availability(req.weekday, req.max_minutes, req.allow_hard, req.fixed_rest)?;
    writeln!(
        out,
        "Verfügbarkeit für {} gesetzt.",
        WEEKDAYS[usize::from(req.weekday)]
    )
    .map_err(|e| output_error(&e))
}

/// `lastwerk availability list`.
pub fn availability_list(store: &Store, out: &mut dyn Write) -> Result<(), CliError> {
    let rows = store.list_availability()?;
    if rows.is_empty() {
        return writeln!(
            out,
            "Keine Verfügbarkeit gesetzt. Der Planer nimmt für jeden Tag {} Minuten und \
             erlaubt harte Einheiten (`lastwerk availability set`).",
            crate::planning::DEFAULT_DAY.max_minutes
        )
        .map_err(|e| output_error(&e));
    }
    for r in rows {
        writeln!(
            out,
            "{:<10}  {:>3} Minuten  harte Einheiten: {}  fester Ruhetag: {}",
            WEEKDAYS[usize::from(r.weekday)],
            r.max_minutes,
            if r.allow_hard { "ja" } else { "nein" },
            if r.fixed_rest { "ja" } else { "nein" }
        )
        .map_err(|e| output_error(&e))?;
    }
    Ok(())
}
