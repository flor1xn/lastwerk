//! Befehle `plans` und `plan-status` (SPEC 5.4): gespeicherte Pläne auflisten und ihren Status
//! setzen.
//!
//! Die Statuslogik liegt im Speicher (`Store::set_plan_status`), dieselbe Methode nutzt die
//! Desktop-App. Wer einen Plan aktiv setzt, archiviert damit den bisherigen aktiven Plan (es gibt
//! höchstens einen). Ohne diesen Befehl käme, wer nur die Kommandozeile nutzt, nie zu einem
//! aktiven Plan, und `get_current_plan` im MCP-Server bliebe ein Fehler.

use std::io::Write;

use lastwerk_store::{PlanStatus, Store};

use crate::error::CliError;

fn output_error(e: &std::io::Error) -> CliError {
    CliError::Internal(format!("Ausgabefehler: {}", e.kind()))
}

/// `lastwerk plans`: ID, Status, Erstellungsdatum (ohne Uhrzeit) und Wochen, die neuesten
/// zuerst. Ein Plan, dessen Text sich nicht lesen lässt, steht mit `-` bei den Wochen.
pub fn list(store: &Store, out: &mut dyn Write) -> Result<(), CliError> {
    let plans = store.list_plans()?;
    if plans.is_empty() {
        return writeln!(out, "Keine Pläne gespeichert (`lastwerk plan`).")
            .map_err(|e| output_error(&e));
    }
    for p in plans {
        let weeks = match store.load_plan(&p.id) {
            Ok(Some(plan)) => plan.weeks.len().to_string(),
            _ => "-".to_string(),
        };
        writeln!(
            out,
            "{}  {:<8}  erstellt: {}  Wochen: {}",
            p.id,
            p.status.as_str(),
            p.created_at.date_naive(),
            weeks
        )
        .map_err(|e| output_error(&e))?;
    }
    Ok(())
}

/// `lastwerk plan-status`: setzt den Status eines gespeicherten Plans. Eine unbekannte ID ist
/// ein Eingabefehler (Exit 2, `StoreError::PlanNotFound`).
pub fn set_status(
    store: &mut Store,
    id: &str,
    status: PlanStatus,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    // Vorher merken, welcher andere Plan aktiv ist, damit die Meldung die Archivierung nennt.
    // Die Entscheidung selbst trifft allein `Store::set_plan_status` (keine zweite Logik).
    let previous_active: Option<String> = if status == PlanStatus::Active {
        store
            .list_plans()?
            .into_iter()
            .find(|p| p.status == PlanStatus::Active && p.id != id)
            .map(|p| p.id)
    } else {
        None
    };
    store.set_plan_status(id, status)?;
    writeln!(out, "Plan {id}: Status {}.", status.as_str()).map_err(|e| output_error(&e))?;
    if let Some(old) = previous_active {
        writeln!(out, "Der bisher aktive Plan {old} ist jetzt archiviert.")
            .map_err(|e| output_error(&e))?;
    }
    Ok(())
}
