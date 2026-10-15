//! lastwerk-cli als Bibliothek: Befehle, Planung, Schemaprüfung, ICS und Sicherung.
//!
//! Die Bibliothek enthält die gesamte Logik der Kommandozeile (SPEC 5.4). Das Binary
//! `lastwerk` parst nur die Argumente. Die Desktop-App (`app/src-tauri`) nutzt dieselben
//! Module, damit Oberfläche und Kommandozeile identisch planen, prüfen und exportieren
//! (Kontextbildung aus der Datenbank, Schemaprüfung, ICS, Sicherung).

pub mod chat;
pub mod commands;
pub mod error;
pub mod fit_migration;
pub mod garmin;
#[cfg(test)]
pub mod golden_tests;
pub mod ics;
pub mod paths;
pub mod planning;
pub mod plans;
pub mod pmc;
#[cfg(test)]
mod pmc_settings_tests;
pub mod recovery;
pub mod refine;
pub mod report;
pub mod schema;
#[cfg(test)]
pub mod schema_tests;
#[cfg(test)]
pub mod testctx;
pub mod texts;
pub mod thresholds;
pub mod userdata;
