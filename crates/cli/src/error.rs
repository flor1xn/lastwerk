//! Fehler der Befehle und ihre Exit-Codes (SPEC 5.4).

use lastwerk_store::StoreError;

/// Fehler eines Befehls. Jede Art hat genau einen Exit-Code.
#[derive(Debug)]
pub enum CliError {
    /// Exit-Code 1: Eingaben sind formal da, aber inhaltlich ungültig (Plan gegen Schema oder
    /// harte Regel).
    Validation(String),
    /// Exit-Code 2: Ordner fehlt, Datenbank unbrauchbar, Pfad ungültig.
    Input(String),
    /// Exit-Code 3: Fehler, mit dem der Nutzer nichts tun kann.
    Internal(String),
}

impl CliError {
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Validation(_) => 1,
            Self::Input(_) => 2,
            Self::Internal(_) => 3,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Validation(m) | Self::Input(m) | Self::Internal(m) => m,
        }
    }
}

impl From<StoreError> for CliError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotADirectory => Self::Input("Der Ordner existiert nicht.".into()),
            StoreError::SchemaTooNew { found, supported } => Self::Input(format!(
                "Die Datenbank hat Schemaversion {found}, dieses Programm kennt nur {supported}. \
                 Bitte Lastwerk aktualisieren."
            )),
            StoreError::UnsupportedExportVersion(v) => {
                Self::Input(format!("Unbekannte Version {v} der Nutzerdaten."))
            }
            StoreError::InvalidInput(m) => Self::Input(format!("Ungültige Eingabe: {m}.")),
            StoreError::PlanNotFound(_) => Self::Input("Einen solchen Plan gibt es nicht.".into()),
            ref other if is_not_a_database(other) => {
                Self::Input("Die Datei ist keine Lastwerk-Datenbank.".into())
            }
            other => Self::Internal(other.to_string()),
        }
    }
}

/// Erkennung des SQLite-Fehlers "Datei ist keine Datenbank". Die CLI hängt nicht vom
/// SQLite-Crate ab; `StoreError` reicht den Text von SQLite unverändert weiter.
fn is_not_a_database(e: &StoreError) -> bool {
    matches!(e, StoreError::Db(_)) && e.to_string().contains("file is not a database")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_the_spec() {
        assert_eq!(CliError::Validation(String::new()).exit_code(), 1);
        assert_eq!(CliError::Input(String::new()).exit_code(), 2);
        assert_eq!(CliError::Internal(String::new()).exit_code(), 3);
    }

    #[test]
    fn store_errors_map_to_input_or_internal() {
        assert_eq!(CliError::from(StoreError::NotADirectory).exit_code(), 2);
        assert_eq!(
            CliError::from(StoreError::SchemaTooNew {
                found: 9,
                supported: 1
            })
            .exit_code(),
            2
        );
        assert_eq!(
            CliError::from(StoreError::InvalidData("x".into())).exit_code(),
            3
        );
        assert_eq!(CliError::from(StoreError::Io("x".into())).exit_code(), 3);
    }
}
