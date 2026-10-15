//! Start des Servers: Datenbank öffnen, Protokoll öffnen, Werkzeugliste ausgeben.

use std::path::{Path, PathBuf};

use lastwerk_store::{Store, StoreError};
use serde_json::{Value, json};

use crate::calllog::{CallLog, DEFAULT_LOG_NAME};
use crate::server::LastwerkMcp;

/// Fehler beim Start. Alle sind Eingabefehler und enden mit Exit-Code 2 (SPEC 5.4).
#[derive(Debug)]
pub enum StartupError {
    /// Die Datenbank fehlt, ist keine Lastwerk-Datenbank oder hat ein anderes Schema.
    Database(String),
    /// Das Protokoll ließ sich nicht öffnen.
    Log(String),
}

impl StartupError {
    pub const fn exit_code(&self) -> u8 {
        2
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Database(m) | Self::Log(m) => m,
        }
    }
}

/// Pfad des Protokolls: `--log`, sonst `mcp-calls.log` im Ordner der Datenbank.
pub fn log_path(db: &Path, log: Option<&Path>) -> PathBuf {
    match log {
        Some(p) => p.to_path_buf(),
        None => db
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .join(DEFAULT_LOG_NAME),
    }
}

/// Öffnet Datenbank (schreibgeschützt) und Protokoll und baut den Server. Die Meldungen
/// nennen keinen Pfad.
pub fn open_server(db: &Path, log: Option<&Path>) -> Result<LastwerkMcp, StartupError> {
    let store = Store::open_read_only(db).map_err(|e| {
        StartupError::Database(match e {
            StoreError::SchemaTooNew { found, supported } => format!(
                "the database has schema version {found}, this program knows {supported}; \
                 update Lastwerk"
            ),
            StoreError::InvalidData(m) => m,
            _ => "the database does not exist or is not a Lastwerk database".to_string(),
        })
    })?;
    let log = CallLog::open(&log_path(db, log))
        .map_err(|e| StartupError::Log(format!("the call log cannot be opened ({})", e.kind())))?;
    Ok(LastwerkMcp::new(store, log))
}

/// Werkzeugliste für `--list-tools`: Name, Beschreibung und Hinweise je Werkzeug als JSON.
pub fn list_tools_json() -> Value {
    let tools: Vec<Value> = LastwerkMcp::tool_list()
        .into_iter()
        .map(|t| {
            let annotations = t
                .annotations
                .as_ref()
                .and_then(|a| serde_json::to_value(a).ok())
                .unwrap_or(Value::Null);
            json!({
                "name": t.name,
                "description": t.description,
                "annotations": annotations,
            })
        })
        .collect();
    json!({ "tools": tools })
}
