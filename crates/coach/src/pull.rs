//! Modell laden über `POST /api/pull` (T29, SPEC 9.2).
//!
//! Gleicher HTTP-Aufbau wie die Aufrufe an das Modell (`client::agent`): nur die geprüfte
//! Basisadresse (`BaseUrl`, Loopback), keine Weiterleitung, kein Proxy aus der Umgebung. Anders
//! als dort läuft der Aufruf lange (ein Modell hat einige GB) und die Antwort kommt als Strom
//! von JSON-Zeilen (NDJSON). Der Strom wird auf einem Hilfsthread gelesen, damit ein Abbruch
//! sofort wirkt und nicht erst mit der nächsten Zeile; verwirft der Aufrufer den Strom, endet der
//! Hilfsthread mit der nächsten Zeile oder dem Ende der Verbindung und schließt sie dabei. Ollama
//! setzt abgebrochene Ladevorgänge beim nächsten Aufruf fort.
//!
//! Was Ollama als Fehlertext liefert, geht nie weiter: [`PullError`] trägt nur einen Code aus
//! einer festen Liste, abgeleitet aus Stichwörtern (Faustregel, die Texte von Ollama sind nicht
//! stabil).

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use thiserror::Error;

use crate::catalog::is_valid_model_name;
use crate::client::{LlmError, agent, map_ureq_error};
use crate::provider::BaseUrl;

/// Obergrenze für die ganze Verbindung in Sekunden (Faustregel): sechs Stunden, das reicht für
/// 14 GB auch bei 1 MB/s (rund vier Stunden). Danach bricht der Aufruf ab, statt ewig zu hängen.
const GLOBAL_TIMEOUT_S: u32 = 6 * 3600;

/// Wie lange keine Zeile kommen darf, bevor der Aufruf als hängend gilt (Faustregel). Ollama
/// meldet beim Laden mehrmals je Sekunde; fünf Minuten Stille heißen, dass etwas feststeckt
/// (Ollama wiederholt kurze Netzstörungen selbst, ohne zu melden).
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Längste gelesene Zeile in Bytes (Konvention). Echte Zeilen sind unter 500 Byte.
const MAX_LINE_BYTES: usize = 64 * 1024;

/// Wie oft der Hauptthread nach einem Abbruch schaut.
const POLL: Duration = Duration::from_millis(100);

/// Phase eines Ladevorgangs, aus dem `status` von Ollama abgeleitet. Fremder Text geht nicht
/// weiter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PullPhase {
    Manifest,
    Download,
    Verify,
    Write,
    Done,
    Other,
}

impl PullPhase {
    fn from_status(status: &str) -> Self {
        let s = status.trim().to_ascii_lowercase();
        if s == "success" {
            Self::Done
        } else if s.starts_with("pulling manifest") {
            Self::Manifest
        } else if s.starts_with("pulling") || s.starts_with("downloading") {
            Self::Download
        } else if s.starts_with("verifying") {
            Self::Verify
        } else if s.starts_with("writing") || s.starts_with("removing") {
            Self::Write
        } else {
            Self::Other
        }
    }
}

/// Fortschritt über alle Dateien des Modells (Summe, siehe [`Aggregate`]). `total == 0` heißt:
/// noch unbekannt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PullProgress {
    pub phase: PullPhase,
    pub completed: u64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PullError {
    #[error("the model name is invalid")]
    InvalidModel,
    #[error("the download was cancelled")]
    Cancelled,
    #[error("the download stalled")]
    Timeout,
    #[error("the model server is not reachable: {0}")]
    Transport(String),
    #[error("the model server answered with status {0}")]
    Status(u16),
    #[error("the answer of the model server has an unexpected shape: {0}")]
    BadResponse(String),
    #[error("the model does not exist")]
    ModelNotFound,
    #[error("the disk is full")]
    DiskFull,
    #[error("the model server could not reach the library")]
    Network,
    #[error("the model server reported an error")]
    Failed,
}

impl PullError {
    /// Stabiler Fehlercode für Oberfläche und Protokolle.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidModel => "invalid_model",
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::Transport(_) => "transport",
            Self::Status(_) => "status",
            Self::BadResponse(_) => "bad_response",
            Self::ModelNotFound => "model_not_found",
            Self::DiskFull => "disk_full",
            Self::Network => "network",
            Self::Failed => "pull_failed",
        }
    }
}

impl From<LlmError> for PullError {
    fn from(e: LlmError) -> Self {
        match e {
            LlmError::Timeout => Self::Timeout,
            LlmError::Transport(m) => Self::Transport(m),
            LlmError::Status(s) => Self::Status(s),
            LlmError::BadResponse(m) => Self::BadResponse(m),
        }
    }
}

/// Ordnet den Fehlertext von Ollama einem Code zu (Stichwörter, Faustregel). Der Text selbst
/// wird verworfen.
fn classify_error(text: &str) -> PullError {
    let s = text.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| s.contains(w));
    if has(&["does not exist", "not found"]) {
        PullError::ModelNotFound
    } else if has(&["no space left", "not enough space", "disk full"]) {
        PullError::DiskFull
    } else if has(&[
        "dial tcp",
        "lookup",
        "connection",
        "timeout",
        "timed out",
        "tls",
        "eof",
        "unreachable",
    ]) {
        PullError::Network
    } else {
        PullError::Failed
    }
}

/// Summe über die Dateien eines Modells. Ollama meldet `completed` und `total` je Datei (mit
/// `digest`), nacheinander; die Summe hält die Anzeige ruhig, statt je Datei von vorn zu
/// beginnen. Sie wächst, wenn eine weitere Datei auftaucht, `total` also auch.
#[derive(Debug, Default)]
struct Aggregate {
    files: BTreeMap<String, (u64, u64)>,
}

impl Aggregate {
    /// Nimmt eine Zeile auf und liefert den Fortschritt, falls sie einen trägt.
    fn feed(&mut self, line: &Value) -> Option<PullProgress> {
        let status = line.get("status").and_then(Value::as_str)?;
        let phase = PullPhase::from_status(status);
        let completed = line.get("completed").and_then(Value::as_u64);
        let total = line.get("total").and_then(Value::as_u64);
        if let Some(total) = total {
            // Ohne digest zählt der Statustext als Schlüssel.
            let key = line
                .get("digest")
                .and_then(Value::as_str)
                .unwrap_or(status)
                .chars()
                .take(100)
                .collect::<String>();
            let done = completed.unwrap_or(0).min(total);
            self.files.insert(key, (done, total));
        }
        let (c, t) = self.files.values().fold((0u64, 0u64), |(c, t), (fc, ft)| {
            (c.saturating_add(*fc), t.saturating_add(*ft))
        });
        Some(PullProgress {
            phase,
            // Am Ende ist alles geladen, auch wenn die letzte Zeile keine Zahlen trägt.
            completed: if phase == PullPhase::Done { t } else { c },
            total: t,
        })
    }
}

enum Event {
    Line(Value),
    Eof,
    Failed(PullError),
}

/// Liest den Strom auf dem Hilfsthread. Endet, sobald der Empfänger weg ist.
fn read_stream(url: String, body: String, tx: Sender<Event>) {
    let outcome = (|| -> Result<(), PullError> {
        let response = agent(GLOBAL_TIMEOUT_S)
            .post(&url)
            .header("Content-Type", "application/json")
            .send(body)
            .map_err(|e| PullError::from(map_ureq_error(e)))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(PullError::Status(status));
        }
        let mut reader = BufReader::new(response.into_body().into_reader());
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = (&mut reader)
                .take(MAX_LINE_BYTES as u64 + 1)
                .read_until(b'\n', &mut buf)
                .map_err(|e| PullError::from(map_ureq_error(ureq::Error::Io(e))))?;
            if n == 0 {
                return Ok(());
            }
            if buf.len() > MAX_LINE_BYTES {
                return Err(PullError::BadResponse("line too long".into()));
            }
            let text = String::from_utf8_lossy(&buf);
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(text)
                .map_err(|_| PullError::BadResponse("line is not json".into()))?;
            if tx.send(Event::Line(value)).is_err() {
                // Der Aufrufer ist weg (Abbruch): Verbindung schließen.
                return Ok(());
            }
        }
    })();
    let _ = tx.send(match outcome {
        Ok(()) => Event::Eof,
        Err(e) => Event::Failed(e),
    });
}

/// Lädt `name` über `POST {base}/api/pull` mit `stream: true`. `progress` bekommt den Fortschritt
/// bei jeder Zeile (der Aufrufer dünnt aus), `cancel` bricht ab (Prüfung alle 100 ms, auch
/// während des Verbindungsaufbaus). Ein Name, der nicht dem einfachen Muster entspricht
/// ([`is_valid_model_name`]), wird nicht gesendet.
pub fn pull_model(
    base: &BaseUrl,
    name: &str,
    progress: &mut dyn FnMut(PullProgress),
    cancel: &AtomicBool,
) -> Result<(), PullError> {
    pull_with_idle(base, name, progress, cancel, IDLE_TIMEOUT)
}

#[doc(hidden)]
pub fn pull_with_idle(
    base: &BaseUrl,
    name: &str,
    progress: &mut dyn FnMut(PullProgress),
    cancel: &AtomicBool,
    idle: Duration,
) -> Result<(), PullError> {
    if !is_valid_model_name(name) {
        return Err(PullError::InvalidModel);
    }
    let body = serde_json::to_string(&json!({ "model": name, "stream": true }))
        .map_err(|_| PullError::InvalidModel)?;
    let (tx, rx) = mpsc::channel();
    let url = base.endpoint("/api/pull", false);
    thread::spawn(move || read_stream(url, body, tx));

    let mut aggregate = Aggregate::default();
    let mut last_activity = Instant::now();
    let mut succeeded = false;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(PullError::Cancelled);
        }
        match rx.recv_timeout(POLL) {
            Ok(Event::Line(line)) => {
                last_activity = Instant::now();
                if let Some(err) = line.get("error") {
                    // Der Text bleibt hier: nur der Code geht weiter.
                    return Err(classify_error(err.as_str().unwrap_or("")));
                }
                if let Some(p) = aggregate.feed(&line) {
                    succeeded |= p.phase == PullPhase::Done;
                    progress(p);
                }
            }
            Ok(Event::Eof) => {
                return if succeeded {
                    Ok(())
                } else {
                    Err(PullError::BadResponse("the stream ended early".into()))
                };
            }
            Ok(Event::Failed(e)) => return Err(e),
            Err(RecvTimeoutError::Timeout) => {
                if last_activity.elapsed() > idle {
                    return Err(PullError::Timeout);
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(PullError::Transport("the reader stopped".into()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(a: &mut Aggregate, v: Value) -> PullProgress {
        a.feed(&v).unwrap()
    }

    #[test]
    fn phases_come_from_the_status() {
        for (text, phase) in [
            ("pulling manifest", PullPhase::Manifest),
            ("pulling 6e9f90f02bb3", PullPhase::Download),
            ("verifying sha256 digest", PullPhase::Verify),
            ("writing manifest", PullPhase::Write),
            ("removing any unused layers", PullPhase::Write),
            ("success", PullPhase::Done),
            ("something new", PullPhase::Other),
        ] {
            assert_eq!(PullPhase::from_status(text), phase, "{text}");
        }
    }

    /// Zwei Dateien: erst 1000 von 2000 (Summe 1000/2000), dann Datei zwei mit 0 von 500
    /// (Summe 1000/2500), dann Datei eins fertig mit 2000 und Datei zwei 250 (Summe 2250/2500).
    /// `completed` über `total` wird auf `total` begrenzt (Summe bleibt <= total).
    #[test]
    fn progress_is_summed_over_files() {
        let mut a = Aggregate::default();
        let p = feed(
            &mut a,
            json!({"status":"pulling aaa","digest":"aaa","total":2000,"completed":1000}),
        );
        assert_eq!(
            (p.completed, p.total, p.phase),
            (1000, 2000, PullPhase::Download)
        );
        let p = feed(
            &mut a,
            json!({"status":"pulling bbb","digest":"bbb","total":500}),
        );
        assert_eq!((p.completed, p.total), (1000, 2500));
        feed(
            &mut a,
            json!({"status":"pulling aaa","digest":"aaa","total":2000,"completed":2000}),
        );
        let p = feed(
            &mut a,
            json!({"status":"pulling bbb","digest":"bbb","total":500,"completed":250}),
        );
        assert_eq!((p.completed, p.total), (2250, 2500));
        let p = feed(
            &mut a,
            json!({"status":"pulling bbb","digest":"bbb","total":500,"completed":900}),
        );
        assert_eq!(p.completed, 2500);
        let p = feed(&mut a, json!({"status":"success"}));
        assert_eq!(
            (p.completed, p.total, p.phase),
            (2500, 2500, PullPhase::Done)
        );
    }

    #[test]
    fn a_line_without_status_carries_no_progress() {
        let mut a = Aggregate::default();
        assert!(a.feed(&json!({"total": 5})).is_none());
        assert!(a.feed(&json!({"status": 3})).is_none());
        let p = feed(&mut a, json!({"status":"pulling manifest"}));
        assert_eq!((p.completed, p.total), (0, 0));
    }

    #[test]
    fn error_texts_become_codes() {
        for (text, code) in [
            (
                "pull model manifest: file does not exist",
                "model_not_found",
            ),
            ("write /x: no space left on device", "disk_full"),
            (
                "dial tcp: lookup registry.ollama.ai: no such host",
                "network",
            ),
            ("max retries exceeded: EOF", "network"),
            ("something unexpected /home/user/secret", "pull_failed"),
            ("", "pull_failed"),
        ] {
            assert_eq!(classify_error(text).code(), code, "{text}");
        }
        // Der Fehlertext steckt nicht im Fehler.
        let shown = format!("{:?}", classify_error("secret /home/user"));
        assert!(!shown.contains("secret"));
    }
}
