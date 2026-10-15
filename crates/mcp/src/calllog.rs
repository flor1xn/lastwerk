//! Lokales Aufrufprotokoll (SPEC 5.4, 9.6 Punkt 6).
//!
//! Eine Zeile JSON je Aufruf (JSON Lines) mit Zeitstempel, Werkzeug, Parametern, Zahl der
//! Zeilen der Antwort, Dauer und Ausgang. Das Protokoll enthält die Argumente des Aufrufs,
//! die der Client geschickt hat, aber nie einen Wert aus der Antwort. Es liegt standardmäßig
//! neben der Datenbank (`mcp-calls.log`) und verlässt den Rechner nicht.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::Value;

/// Dateiname des Protokolls neben der Datenbank.
pub const DEFAULT_LOG_NAME: &str = "mcp-calls.log";

/// Eine Zeile des Protokolls.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CallRecord {
    /// UTC, auf die Sekunde, RFC 3339.
    pub ts: String,
    pub tool: String,
    pub params: Value,
    pub rows: usize,
    pub duration_ms: u64,
    /// `ok` oder der Kurzcode des Fehlers (`invalid_params`, `disabled`, `not_found`,
    /// `internal`).
    pub status: String,
}

impl CallRecord {
    pub fn new(
        at: DateTime<Utc>,
        tool: &str,
        params: Value,
        rows: usize,
        duration_ms: u64,
        status: &str,
    ) -> Self {
        Self {
            ts: at.to_rfc3339_opts(SecondsFormat::Secs, true),
            tool: tool.to_string(),
            params,
            rows,
            duration_ms,
            status: status.to_string(),
        }
    }
}

/// Protokoll in einer Datei. Jeder Eintrag wird angehängt und sofort geschrieben.
pub struct CallLog {
    path: PathBuf,
    file: Mutex<File>,
}

impl CallLog {
    /// Öffnet die Datei zum Anhängen und legt sie bei Bedarf an. Ein Fehler hier bricht den
    /// Start des Servers ab: Ein Server, dessen Aufrufe sich nicht nachvollziehen lassen,
    /// soll keine Daten herausgeben.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Hängt eine Zeile an. Schlägt das Schreiben später fehl, geht der Aufruf trotzdem
    /// durch und der Fehler steht auf der Fehlerausgabe (ohne Pfad).
    pub fn record(&self, rec: &CallRecord) {
        let Ok(mut line) = serde_json::to_string(rec) else {
            return;
        };
        line.push('\n');
        let written = self
            .file
            .lock()
            .map_err(|_| std::io::ErrorKind::Other)
            .and_then(|mut f| {
                f.write_all(line.as_bytes())
                    .and_then(|()| f.flush())
                    .map_err(|e| e.kind())
            });
        if let Err(kind) = written {
            eprintln!("lastwerk-mcp: call log could not be written ({kind})");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_appended_as_one_json_line_each() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/mcp-calllog-unit");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let log = CallLog::open(&path).unwrap();
        let at = DateTime::from_timestamp(1_790_000_000, 0).unwrap();
        log.record(&CallRecord::new(
            at,
            "get_activity",
            serde_json::json!({"id": 3}),
            1,
            2,
            "ok",
        ));
        log.record(&CallRecord::new(
            at,
            "explain_rule",
            serde_json::json!({"code": "R99"}),
            0,
            1,
            "not_found",
        ));
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["tool"], "get_activity");
        assert_eq!(first["params"]["id"], 3);
        assert_eq!(first["rows"], 1);
        assert_eq!(first["ts"], "2026-09-21T14:13:20Z");
        let _ = std::fs::remove_file(&path);
    }
}
