//! Prüfung eines Plans gegen `schema/plan.schema.json` (SPEC 7.1, 11.4).
//!
//! Das Schema ist per `include_str!` eingebettet (Konvention wie beim Zonenkatalog in T05):
//! Das fertige Programm läuft ohne Nebendateien. Geprüft wird mit der Bibliothek `jsonschema`
//! (SPEC 5.3) in Draft 2020-12 und mit aktiver Formatprüfung, damit `uuid`, `date-time` und
//! `date` wirklich geprüft werden. Die Bibliothek löst keine Verweise über Netzwerk oder
//! Dateisystem auf (Features `resolve-http` und `resolve-file` sind aus).

use jsonschema::Validator;
use serde_json::Value;

use crate::error::CliError;

/// Inhalt von `schema/plan.schema.json`.
pub const PLAN_SCHEMA: &str = include_str!("../../../schema/plan.schema.json");

/// Baut den Prüfer für das Plan-Schema.
pub fn plan_validator() -> Result<Validator, CliError> {
    let schema: Value = serde_json::from_str(PLAN_SCHEMA)
        .map_err(|e| CliError::Internal(format!("Das eingebettete Schema ist kein JSON: {e}")))?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|e| CliError::Internal(format!("Das eingebettete Schema ist ungültig: {e}")))
}

/// Alle Schemaverletzungen eines Plans als Zeilen `<Pfad>: <Meldung>`. Leer heißt gültig.
/// Der Pfad ist der JSON-Pointer in den Plan (leer für die Wurzel, dann steht `/`).
pub fn plan_errors(plan: &Value) -> Result<Vec<String>, CliError> {
    let validator = plan_validator()?;
    let mut lines: Vec<String> = validator
        .iter_errors(plan)
        .map(|e| {
            let path = e.instance_path().to_string();
            let path = if path.is_empty() {
                "/".to_string()
            } else {
                path
            };
            format!("{path}: {e}")
        })
        .collect();
    // Die Bibliothek liefert keine feste Reihenfolge zu, die Ausgabe soll stabil sein.
    lines.sort();
    lines.dedup();
    Ok(lines)
}
