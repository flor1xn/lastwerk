//! Befehl `import-recovery`: liest `exchange/out/recovery/YYYY-MM-DD.json` in die Datenbank
//! (SPEC 5.4, 7.3, 12 M4d, Task T14).
//!
//! Alles unter `exchange/out/` ist nicht vertrauenswürdige Eingabe (SPEC 5.4). Deshalb gilt je
//! Datei, in dieser Reihenfolge:
//!
//! 1. Nur Dateien direkt im Ordner mit der Endung `.json`. Unterordner und andere Dateien
//!    werden übersprungen. Symbolische Links und Sonderdateien sind ungültig und werden nie
//!    geöffnet.
//! 2. Der Name muss `YYYY-MM-DD.json` sein und ein echtes Datum nennen. Der Name entscheidet
//!    über das Datum der Zeile, nie ein Pfad aus dem Inhalt: Der Inhalt enthält keinen Pfad,
//!    und sein Feld `date` muss zum Namen passen.
//! 3. Höchstens [`MAX_FILE_BYTES`] (Faustregel: eine Datei hat unter 1 KiB, 64 KiB lassen
//!    reichlich Platz und schützen vor einem vollen Speicher). Gelesen wird nie mehr als das
//!    Limit plus ein Byte.
//! 4. Gültiges JSON und gültig gegen `schema/recovery-day.schema.json` (eingebettet, mit
//!    Formatprüfung für `date` und `date-time`).
//!
//! Ungültige Dateien werden abgelehnt, gezählt und mit Grund gemeldet, der Import läuft zu
//! Ende. Gültige Dateien landen in einer Transaktion in `recovery_day`. Ein zweiter Lauf mit
//! denselben Dateien schreibt nichts. Meldungen enthalten nur Dateinamen aus dem Ordner (von
//! Steuerzeichen bereinigt und gekürzt) und nie Messwerte oder den Pfad des Ordners.

use std::io::{Read, Write};
use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use jsonschema::Validator;
use lastwerk_store::{RecoveryDayRecord, Store, UpsertOutcome};
use serde_json::Value;

use crate::error::CliError;

/// Inhalt von `schema/recovery-day.schema.json`.
pub const RECOVERY_SCHEMA: &str = include_str!("../../../schema/recovery-day.schema.json");

/// Größte zulässige Datei in Bytes (Faustregel).
pub const MAX_FILE_BYTES: u64 = 64 * 1024;

/// So viele Einträge des Ordners liest der Import höchstens (Faustregel: 50 Jahre Tage mit
/// Reserve). Weitere Einträge werden nicht angesehen und im Bericht vermerkt.
pub const MAX_ENTRIES: usize = 20_000;

/// So viele abgelehnte Dateien nennt der Bericht einzeln.
const REPORT_LIMIT: usize = 50;

/// Längster Name oder Grund im Bericht in Zeichen.
const TEXT_LIMIT: usize = 120;

/// Grund für die Ablehnung einer Datei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// Der Name ist nicht `YYYY-MM-DD.json` mit einem echten Datum.
    BadName,
    /// Symbolischer Link oder Sonderdatei statt einer regulären Datei.
    NotRegularFile,
    TooLarge,
    Unreadable,
    NotJson,
    /// Schemaverletzung, mit Pfad im Dokument und Meldung (ohne Werte).
    Schema(String),
    /// Das Feld `date` nennt einen anderen Tag als der Dateiname.
    DateMismatch,
}

impl Reason {
    /// Stabiler englischer Code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadName => "bad_name",
            Self::NotRegularFile => "not_regular_file",
            Self::TooLarge => "too_large",
            Self::Unreadable => "unreadable",
            Self::NotJson => "not_json",
            Self::Schema(_) => "schema",
            Self::DateMismatch => "date_mismatch",
        }
    }

    fn text(&self) -> String {
        match self {
            Self::BadName => "Name ist nicht JJJJ-MM-TT.json".to_string(),
            Self::NotRegularFile => "keine reguläre Datei".to_string(),
            Self::TooLarge => format!("größer als {} Byte", MAX_FILE_BYTES),
            Self::Unreadable => "nicht lesbar".to_string(),
            Self::NotJson => "kein gültiges JSON".to_string(),
            Self::Schema(m) => format!("verletzt das Schema ({m})"),
            Self::DateMismatch => "date weicht vom Dateinamen ab".to_string(),
        }
    }
}

/// Eine abgelehnte Datei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    /// Dateiname, bereinigt und gekürzt.
    pub file: String,
    pub reason: Reason,
}

/// Ergebnis von `import-recovery`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub invalid: Vec<Rejected>,
    /// Unterordner, Dateien ohne Endung `.json` und Einträge mit nicht lesbarem Namen.
    pub skipped: usize,
    /// Der Ordner hatte mehr als [`MAX_ENTRIES`] Einträge, der Rest wurde nicht angesehen.
    pub truncated: bool,
}

impl RecoveryReport {
    /// Anzahl gültiger Dateien.
    pub fn valid(&self) -> usize {
        self.inserted + self.updated + self.unchanged
    }
}

/// Ersetzt Steuerzeichen und kürzt, damit Namen und Meldungen aus einer fremden Datei das
/// Terminal nicht steuern können.
fn sanitize(text: &str) -> String {
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .take(TEXT_LIMIT)
        .collect();
    if text.chars().count() > TEXT_LIMIT {
        out.push_str("...");
    }
    out
}

fn validator() -> Result<Validator, CliError> {
    let schema: Value = serde_json::from_str(RECOVERY_SCHEMA)
        .map_err(|e| CliError::Internal(format!("Das eingebettete Schema ist kein JSON: {e}")))?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|e| CliError::Internal(format!("Das eingebettete Schema ist ungültig: {e}")))
}

/// Datum aus einem Dateinamen `YYYY-MM-DD.json`. Streng: genau zehn Zeichen vor der Endung,
/// Ziffern und Bindestriche an festen Stellen, ein echtes Datum.
fn date_of_name(name: &str) -> Option<NaiveDate> {
    let stem = name.strip_suffix(".json")?;
    let b = stem.as_bytes();
    let shape = b.len() == 10
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            _ => c.is_ascii_digit(),
        });
    if !shape {
        return None;
    }
    NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()
}

/// Ganzzahl aus einem JSON-Wert. Das Schema lässt `27000.0` als Ganzzahl zu.
fn as_whole(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| {
        let f = v.as_f64()?;
        (f.fract() == 0.0 && f.abs() < 1e15).then_some(f as i64)
    })
}

/// Prüft ein Dokument und macht daraus eine Zeile. `name_date` ist das Datum aus dem Namen.
fn record_of(
    validator: &Validator,
    doc: &Value,
    name_date: NaiveDate,
    now: DateTime<Utc>,
) -> Result<RecoveryDayRecord, Reason> {
    let mut errors: Vec<String> = validator
        .iter_errors(doc)
        .map(|e| {
            let path = e.instance_path().to_string();
            let path = if path.is_empty() {
                "/".to_string()
            } else {
                path
            };
            // Die Meldung der Bibliothek nennt den Wert. Der Bericht nennt nur Pfad und das
            // verletzte Schlüsselwort (`maximum`, `required`, ...).
            format!("{path}: {}", e.kind().keyword())
        })
        .collect();
    errors.sort();
    errors.dedup();
    if let Some(first) = errors.into_iter().next() {
        return Err(Reason::Schema(sanitize(&first)));
    }
    let obj = doc.as_object().ok_or(Reason::NotJson)?;
    let date = obj
        .get("date")
        .and_then(Value::as_str)
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    if date != Some(name_date) {
        return Err(Reason::DateMismatch);
    }
    Ok(RecoveryDayRecord {
        date: name_date,
        hrv_overnight_ms: obj.get("hrv_overnight_ms").and_then(Value::as_f64),
        resting_hr_bpm: obj.get("resting_hr_bpm").and_then(Value::as_f64),
        sleep_duration_s: obj
            .get("sleep_duration_s")
            .and_then(as_whole)
            .and_then(|v| u32::try_from(v).ok()),
        sleep_score: obj
            .get("sleep_score")
            .and_then(as_whole)
            .and_then(|v| u8::try_from(v).ok()),
        body_battery_high: obj
            .get("body_battery_high")
            .and_then(as_whole)
            .and_then(|v| u8::try_from(v).ok()),
        body_battery_low: obj
            .get("body_battery_low")
            .and_then(as_whole)
            .and_then(|v| u8::try_from(v).ok()),
        source: obj
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        imported_at: now,
    })
}

/// Liest eine Datei mit Größenlimit. `None` heißt zu groß.
fn read_limited(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    let file = std::fs::File::open(path)?;
    let mut buf = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut buf)?;
    Ok((buf.len() as u64 <= MAX_FILE_BYTES).then_some(buf))
}

/// Prüft eine Datei. `Ok(None)` heißt: übersprungen (kein Kandidat).
fn check_entry(
    path: &Path,
    name: &str,
    validator: &Validator,
    now: DateTime<Utc>,
) -> Result<Option<RecoveryDayRecord>, Reason> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| Reason::Unreadable)?;
    if meta.is_dir() {
        return Ok(None);
    }
    if !meta.is_file() {
        return Err(Reason::NotRegularFile);
    }
    let date = date_of_name(name).ok_or(Reason::BadName)?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(Reason::TooLarge);
    }
    let bytes = match read_limited(path) {
        Ok(Some(b)) => b,
        Ok(None) => return Err(Reason::TooLarge),
        Err(_) => return Err(Reason::Unreadable),
    };
    let doc: Value = serde_json::from_slice(&bytes).map_err(|_| Reason::NotJson)?;
    record_of(validator, &doc, date, now).map(Some)
}

/// Liest `dir` und schreibt die gültigen Tage in `store`.
pub fn import_dir(
    store: &mut Store,
    dir: &Path,
    now: DateTime<Utc>,
) -> Result<RecoveryReport, CliError> {
    let validator = validator()?;
    let read = std::fs::read_dir(dir).map_err(|e| {
        CliError::Input(format!(
            "Der Ordner mit den Erholungsdaten lässt sich nicht lesen: {}",
            e.kind()
        ))
    })?;
    let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut report = RecoveryReport::default();
    for entry in read {
        let Ok(entry) = entry else {
            report.skipped += 1;
            continue;
        };
        if entries.len() >= MAX_ENTRIES {
            report.truncated = true;
            break;
        }
        match entry.file_name().into_string() {
            Ok(name) => entries.push((name, entry.path())),
            Err(_) => report.skipped += 1,
        }
    }
    // Feste Reihenfolge, damit der Bericht und bei doppelten Daten die Auswahl stabil sind.
    entries.sort();

    let mut records: Vec<RecoveryDayRecord> = Vec::new();
    for (name, path) in &entries {
        // Kandidaten sind Einträge mit der Endung `.json` (Groß- und Kleinschreibung egal)
        // und Unterordner nie. Alles andere ist kein Erholungstag und wird übersprungen.
        let is_json = Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        if !is_json {
            report.skipped += 1;
            continue;
        }
        match check_entry(path, name, &validator, now) {
            Ok(Some(record)) => records.push(record),
            Ok(None) => report.skipped += 1,
            Err(reason) => report.invalid.push(Rejected {
                file: sanitize(name),
                reason,
            }),
        }
    }

    for outcome in store.upsert_recovery_days(&records)? {
        match outcome {
            UpsertOutcome::Inserted => report.inserted += 1,
            UpsertOutcome::Updated => report.updated += 1,
            UpsertOutcome::Unchanged => report.unchanged += 1,
        }
    }
    Ok(report)
}

/// Schreibt den Bericht.
pub fn write_report(out: &mut dyn Write, r: &RecoveryReport) -> std::io::Result<()> {
    writeln!(out, "Import der Erholungstage abgeschlossen.")?;
    writeln!(
        out,
        "  gültig:       {} (neu {}, aktualisiert {}, unverändert {})",
        r.valid(),
        r.inserted,
        r.updated,
        r.unchanged
    )?;
    writeln!(out, "  ungültig:     {}", r.invalid.len())?;
    writeln!(
        out,
        "  übersprungen: {} (Unterordner und Dateien ohne Endung .json)",
        r.skipped
    )?;
    if r.truncated {
        writeln!(
            out,
            "  Der Ordner hat mehr als {MAX_ENTRIES} Einträge, der Rest wurde nicht angesehen."
        )?;
    }
    if !r.invalid.is_empty() {
        writeln!(out, "Abgelehnte Dateien:")?;
        for item in r.invalid.iter().take(REPORT_LIMIT) {
            writeln!(
                out,
                "  {}: {} [{}]",
                item.file,
                item.reason.text(),
                item.reason.code()
            )?;
        }
        if r.invalid.len() > REPORT_LIMIT {
            writeln!(out, "  ... und {} weitere", r.invalid.len() - REPORT_LIMIT)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_must_be_exactly_a_date_with_json_extension() {
        let ok = |n: &str| date_of_name(n).is_some();
        assert!(ok("2026-10-13.json"));
        assert!(ok("2024-02-29.json"));
        for bad in [
            "2026-10-13.JSON",
            "2026-10-13.json.json",
            "2026-1-13.json",
            "2026-10-13 .json",
            "2026-02-30.json",
            "2023-02-29.json",
            "2026-13-01.json",
            "20261013.json",
            "../2026-10-13.json",
            "2026-10-13.json/",
            ".2026-10-13.json",
            "x2026-10-13.json",
            "２０２６-10-13.json",
            "",
        ] {
            assert!(!ok(bad), "{bad:?} darf nicht gelten");
        }
    }

    #[test]
    fn sanitize_removes_control_characters_and_shortens() {
        assert_eq!(sanitize("a\u{1b}[31mb\nc"), "a?[31mb?c");
        let long = "x".repeat(300);
        let s = sanitize(&long);
        assert_eq!(s.chars().count(), TEXT_LIMIT + 3);
        assert!(s.ends_with("..."));
    }

    #[test]
    fn whole_numbers_accept_integral_floats_only() {
        assert_eq!(as_whole(&serde_json::json!(27000)), Some(27000));
        assert_eq!(as_whole(&serde_json::json!(27000.0)), Some(27000));
        assert_eq!(as_whole(&serde_json::json!(27000.5)), None);
        assert_eq!(as_whole(&serde_json::json!("1")), None);
    }
}
