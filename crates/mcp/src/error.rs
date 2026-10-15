//! Fehler der Werkzeuge.
//!
//! Die Meldungen gehen an den Client und damit an das Gespräch. Sie enthalten deshalb nie den
//! Text eines Datenbankfehlers, einen Pfad oder einen Wert aus der Datenbank, sondern nur
//! feste Sätze und die Argumente des Aufrufs.

use lastwerk_store::StoreError;

/// Fehler eines Werkzeugaufrufs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    /// Ein Argument ist außerhalb der erlaubten Grenzen.
    InvalidParams(String),
    /// Das Werkzeug ist gesperrt (Erholungswerte ohne Freigabe).
    Disabled(String),
    /// Es gibt keine passenden Daten (Aktivität, Plan, Woche, Regel).
    NotFound(String),
    /// Die Datenbank ließ sich nicht lesen. Die Ursache steht nur in der Fehlerausgabe.
    Internal,
}

impl ToolError {
    /// Kurzcode für das Aufrufprotokoll.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidParams(_) => "invalid_params",
            Self::Disabled(_) => "disabled",
            Self::NotFound(_) => "not_found",
            Self::Internal => "internal",
        }
    }

    /// Meldung an den Client (Englisch, wie die Werkzeugbeschreibungen).
    pub fn message(&self) -> String {
        match self {
            Self::InvalidParams(m) => format!("invalid parameters: {m}"),
            Self::Disabled(m) => format!("not available: {m}"),
            Self::NotFound(m) => format!("not found: {m}"),
            Self::Internal => "the database could not be read".to_string(),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for ToolError {}

impl From<StoreError> for ToolError {
    fn from(e: StoreError) -> Self {
        // Nur auf die Fehlerausgabe des Servers, nie an den Client.
        eprintln!("lastwerk-mcp: database error: {e}");
        Self::Internal
    }
}

pub type ToolResult<T> = Result<T, ToolError>;
