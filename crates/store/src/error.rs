//! Fehler des Speichers.

use thiserror::Error;

/// Fehler bei Zugriffen auf die Datenbank. Fehler einzelner FIT-Dateien gehören nicht
/// hierher, sie stehen im Importbericht (SPEC 5.4: eine fehlerhafte Datei bricht den Import
/// nicht ab).
#[derive(Debug, Error)]
pub enum StoreError {
    /// Fehler von SQLite.
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    /// Die Datenbank stammt von einer neueren Version von Lastwerk.
    #[error("database schema version {found} is newer than the supported version {supported}")]
    SchemaTooNew { found: u32, supported: u32 },
    /// Ein Wert in der Datenbank lässt sich nicht lesen (zum Beispiel unbekannte Sportart).
    #[error("invalid value in database: {0}")]
    InvalidData(String),
    /// Fehler beim Lesen oder Schreiben der Nutzerdaten als JSON.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// Der Ordner für Import oder Wiederaufbau existiert nicht oder ist kein Ordner.
    #[error("not a directory")]
    NotADirectory,
    /// Das Format der Nutzerdaten (`backup_version`) ist unbekannt.
    #[error("unsupported user data format version {0}")]
    UnsupportedExportVersion(u32),
    /// Ein Plan mit dieser ID gibt es nicht.
    #[error("plan not found: {0}")]
    PlanNotFound(String),
    /// Eingabe, die die Tabelle nicht aufnehmen kann (zum Beispiel ein Ziel ohne Termin).
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Dateisystemfehler beim Anlegen der Datenbank.
    #[error("io error: {0}")]
    Io(String),
}

/// Ergebnis mit [`StoreError`].
pub type Result<T> = std::result::Result<T, StoreError>;

/// Wert aus der Datenbank, der sich nicht in einen Typ der Anwendung wandeln lässt.
#[derive(Debug, Error)]
#[error("{0}")]
pub struct ParseError(pub String);

/// Verpackt einen Wandlungsfehler als SQLite-Lesefehler, damit er aus einer Zeilenfunktion
/// von `rusqlite` herausgereicht werden kann.
pub(crate) fn conversion<T, E>(column: usize, r: std::result::Result<T, E>) -> rusqlite::Result<T>
where
    E: std::error::Error + Send + Sync + 'static,
{
    r.map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, Box::new(e))
    })
}
