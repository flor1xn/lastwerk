//! Fehler der Befehle als strukturiertes Objekt `{ code, message_key }`.
//!
//! Das Frontend zeigt den Text zu `message_key` aus den Sprachdateien (`ui.error.<code>`).
//! Die Fehler tragen nie einen Pfad und nie die Meldung eines Fremdcrates: Pfade gehören
//! nicht in die Oberfläche (außer dem festen FIT-Stamm zur Anzeige), und Meldungen
//! von SQLite oder dem Betriebssystem können sie enthalten.

use lastwerk_store::StoreError;
use serde::Serialize;

/// Alle Fehlercodes. Zu jedem gehört der Schlüssel `ui.error.<code>` in `de.json` und
/// `en.json`, ein Test in `lib.rs` prüft das.
pub const ERROR_CODES: &[&str] = &[
    "invalid_date",
    "invalid_value",
    "invalid_sport",
    "invalid_year",
    "invalid_setting",
    "not_a_directory",
    "not_found",
    "database",
    "database_too_new",
    "database_io",
    "fit_root_io",
    "dialog_failed",
    "not_monday",
    "plan_failed",
    "plan_invalid",
    "export_failed",
    "restore_invalid",
    "unconfirmed",
    "update_failed",
    "no_update",
    // Planer mit drei Fragen und Gespräch mit dem Coach (T26 Teil B).
    "too_little_time",
    "coach_busy",
    "no_plan",
    "empty_message",
    "message_too_long",
    "coach_unavailable",
    "coach_timeout",
    "coach_unreachable",
    "coach_invalid_output",
    "coach_cancelled",
    "proposal_stale",
    "internal",
    // Einrichtung des lokalen Coachs (T29): Ollama installieren (Codes des Skripts) und
    // Modell laden (Codes von `lastwerk-coach`).
    "coach_setup_busy",
    "coach_setup_unsupported",
    "coach_setup_missing_script",
    "coach_setup_failed",
    "coach_setup_timeout",
    "coach_setup_no_windows",
    "coach_setup_winget_failed",
    "coach_setup_download_failed",
    "coach_setup_signature_invalid",
    "coach_setup_install_failed",
    "coach_setup_start_timeout",
    "coach_setup_pull_failed",
    "coach_setup_bad_args",
    "coach_pull_invalid_model",
    "coach_pull_cancelled",
    "coach_pull_timeout",
    "coach_pull_unreachable",
    "coach_pull_bad_response",
    "coach_pull_model_not_found",
    "coach_pull_disk_full",
    "coach_pull_network",
    "coach_pull_failed",
    // Garmin (T23). Nach `garmin_` folgen die Codes von `lastwerk-garmin` und drei eigene.
    "busy",
    "garmin_not_connected",
    "garmin_auth_failed",
    "garmin_throttled",
    "garmin_locked",
    "garmin_not_available",
    "garmin_network",
    "garmin_invalid_response",
    "garmin_store_failed",
    "garmin_invalid_input",
    "garmin_window_failed",
    "garmin_io",
];

/// Fehler eines Befehls, wie er an das Frontend geht.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppError {
    pub code: String,
    pub message_key: String,
}

impl AppError {
    /// Fehler mit einem Code aus [`ERROR_CODES`].
    pub fn new(code: &str) -> Self {
        debug_assert!(ERROR_CODES.contains(&code), "unbekannter Fehlercode {code}");
        Self {
            code: code.to_owned(),
            message_key: format!("ui.error.{code}"),
        }
    }
}

impl From<StoreError> for AppError {
    fn from(e: StoreError) -> Self {
        // Nur die Art des Fehlers, nie die Meldung (kann Pfade enthalten).
        match e {
            StoreError::SchemaTooNew { .. } => Self::new("database_too_new"),
            StoreError::NotADirectory => Self::new("not_a_directory"),
            StoreError::PlanNotFound(_) => Self::new("not_found"),
            StoreError::InvalidInput(_) => Self::new("invalid_value"),
            StoreError::UnsupportedExportVersion(_) => Self::new("restore_invalid"),
            StoreError::Io(_) => Self::new("database_io"),
            StoreError::Db(_) | StoreError::InvalidData(_) | StoreError::Json(_) => {
                Self::new("database")
            }
        }
    }
}

impl From<lastwerk_garmin::GarminError> for AppError {
    fn from(e: lastwerk_garmin::GarminError) -> Self {
        // Nur der stabile Code. Die Fehler von `lastwerk-garmin` tragen ohnehin weder URL noch
        // Token, aber auch ihre Meldung erreicht die Oberfläche nicht.
        Self::garmin(e.code())
    }
}

impl AppError {
    /// Fehler zu einem Code von `lastwerk-garmin` (`garmin_<code>`). Ein unbekannter Code wird
    /// `internal`, damit nie ein Schlüssel ohne Text entsteht.
    pub fn garmin(code: &str) -> Self {
        let full = format!("garmin_{code}");
        if ERROR_CODES.contains(&full.as_str()) {
            Self {
                message_key: format!("ui.error.{full}"),
                code: full,
            }
        } else {
            Self::new("internal")
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
