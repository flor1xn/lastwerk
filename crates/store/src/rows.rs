//! Zeilentypen der Tabellen aus SPEC 6.2 und die Umwandlung von und zu Text.

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use lastwerk_core::{LoadError, LoadMethod, Sport};
use serde::{Deserialize, Serialize};

use crate::error::ParseError;

/// Zeitstempel als ISO 8601 in UTC mit Sekunden, zum Beispiel `2026-05-10T06:00:00Z`.
pub(crate) fn fmt_utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub(crate) fn parse_utc(s: &str) -> Result<DateTime<Utc>, ParseError> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| ParseError(format!("timestamp {s:?}: {e}")))
}

/// Datum als `YYYY-MM-DD`. Der Textvergleich entspricht der Datumsordnung.
pub(crate) fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub(crate) fn parse_date(s: &str) -> Result<NaiveDate, ParseError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| ParseError(format!("date {s:?}: {e}")))
}

/// Sportart als Text in der Datenbank.
pub fn sport_to_str(s: Sport) -> &'static str {
    match s {
        Sport::Cycling => "cycling",
        Sport::Running => "running",
        Sport::Swimming => "swimming",
        Sport::Strength => "strength",
        Sport::Other => "other",
    }
}

pub fn sport_from_str(s: &str) -> Result<Sport, ParseError> {
    Ok(match s {
        "cycling" => Sport::Cycling,
        "running" => Sport::Running,
        "swimming" => Sport::Swimming,
        "strength" => Sport::Strength,
        "other" => Sport::Other,
        _ => return Err(ParseError(format!("sport {s:?}"))),
    })
}

/// Methode als Text in `activity_load.method`.
pub fn method_to_str(m: LoadMethod) -> &'static str {
    match m {
        LoadMethod::Tss => "tss",
        LoadMethod::HrTss => "hr_tss",
        LoadMethod::RTss => "r_tss",
        LoadMethod::STss => "s_tss",
        LoadMethod::SessionRpe => "session_rpe",
    }
}

/// Methode, wenn die Berechnung scheiterte und keine feststeht.
pub const METHOD_NONE: &str = "none";

/// Status einer Datei in `source_file.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Ok,
    NotActivity,
    Error,
}

impl SourceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::NotActivity => "not_activity",
            Self::Error => "error",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<Self, ParseError> {
        match s {
            "ok" => Ok(Self::Ok),
            "not_activity" => Ok(Self::NotActivity),
            "error" => Ok(Self::Error),
            _ => Err(ParseError(format!("source status {s:?}"))),
        }
    }
}

/// Status einer Belastung in `activity_load.status`: `ok` oder der Code eines [`LoadError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadStatus {
    Ok,
    MissingThreshold,
    MissingChannel,
    Implausible,
}

impl LoadStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::MissingThreshold => "missing_threshold",
            Self::MissingChannel => "missing_channel",
            Self::Implausible => "implausible",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<Self, ParseError> {
        match s {
            "ok" => Ok(Self::Ok),
            "missing_threshold" => Ok(Self::MissingThreshold),
            "missing_channel" => Ok(Self::MissingChannel),
            "implausible" => Ok(Self::Implausible),
            _ => Err(ParseError(format!("load status {s:?}"))),
        }
    }

    /// Status zu einem Berechnungsfehler. Der Text ist `LoadError::code()`.
    pub fn from_error(e: &LoadError) -> Self {
        match e {
            LoadError::MissingThreshold(_) => Self::MissingThreshold,
            LoadError::MissingChannel(_) => Self::MissingChannel,
            LoadError::Implausible(_) => Self::Implausible,
        }
    }
}

/// Zeile von `source_file` (ohne `id`).
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFileRow {
    pub rel_path: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub mtime_utc: DateTime<Utc>,
    pub imported_at: DateTime<Utc>,
    pub status: SourceStatus,
    pub error_code: Option<String>,
}

/// Zeile von `source_file` mit `id`.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFileRecord {
    pub id: i64,
    pub row: SourceFileRow,
}

/// Zeile von `activity` ohne `id`, `is_primary` und `duplicate_of`. Die beiden letzten setzt
/// die Duplikatregel (SPEC 6.4) nach dem Import über alle Aktivitäten.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityRow {
    pub source_file_id: i64,
    pub sport: Sport,
    pub sub_sport: Option<String>,
    pub start_utc: DateTime<Utc>,
    pub local_offset_s: i32,
    pub local_date: NaiveDate,
    pub duration_s: i64,
    pub moving_s: Option<i64>,
    pub distance_m: Option<f64>,
    pub ascent_m: Option<f64>,
    pub avg_hr: Option<f64>,
    pub max_hr: Option<f64>,
    pub avg_power_w: Option<f64>,
    pub np_w: Option<f64>,
    pub device_serial: Option<u32>,
    pub device_product: Option<String>,
    pub has_power: bool,
    pub has_hr: bool,
}

/// Zeile von `activity` mit allen Spalten.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityRecord {
    pub id: i64,
    pub row: ActivityRow,
    pub is_primary: bool,
    pub duplicate_of: Option<i64>,
}

/// Zeile von `activity_load`. `value` ist bei einem Fehlerstatus `None`, nie Null (SPEC 5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct LoadRow {
    pub activity_id: i64,
    /// `tss`, `hr_tss`, `r_tss`, `s_tss`, `session_rpe` oder [`METHOD_NONE`].
    pub method: String,
    pub value: Option<f64>,
    pub intensity_factor: Option<f64>,
    pub threshold_id: Option<i64>,
    pub status: LoadStatus,
    pub computed_at: DateTime<Utc>,
    /// Bei `missing_threshold` die fehlende Art (`ftp_w`, `lthr_bpm`, ..., `sex`), sonst `None`
    /// (seit T39, SPEC 8.6).
    pub missing: Option<String>,
}

/// Art eines Schwellenwerts in `threshold.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThresholdKind {
    #[serde(rename = "ftp_w")]
    FtpW,
    #[serde(rename = "lthr_bpm")]
    LthrBpm,
    #[serde(rename = "hr_max_bpm")]
    HrMaxBpm,
    #[serde(rename = "hr_rest_bpm")]
    HrRestBpm,
    #[serde(rename = "run_speed_mps")]
    RunSpeedMps,
    #[serde(rename = "css_mps")]
    CssMps,
}

impl ThresholdKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FtpW => "ftp_w",
            Self::LthrBpm => "lthr_bpm",
            Self::HrMaxBpm => "hr_max_bpm",
            Self::HrRestBpm => "hr_rest_bpm",
            Self::RunSpeedMps => "run_speed_mps",
            Self::CssMps => "css_mps",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<Self, ParseError> {
        Ok(match s {
            "ftp_w" => Self::FtpW,
            "lthr_bpm" => Self::LthrBpm,
            "hr_max_bpm" => Self::HrMaxBpm,
            "hr_rest_bpm" => Self::HrRestBpm,
            "run_speed_mps" => Self::RunSpeedMps,
            "css_mps" => Self::CssMps,
            _ => return Err(ParseError(format!("threshold kind {s:?}"))),
        })
    }

    /// Schwelle, deren Zeile `activity_load.threshold_id` für die Methode nennt. Session-RPE
    /// braucht keine Schwelle.
    pub const fn for_method(m: LoadMethod) -> Option<Self> {
        match m {
            LoadMethod::Tss => Some(Self::FtpW),
            LoadMethod::HrTss => Some(Self::LthrBpm),
            LoadMethod::RTss => Some(Self::RunSpeedMps),
            LoadMethod::STss => Some(Self::CssMps),
            LoadMethod::SessionRpe => None,
        }
    }
}

/// Herkunft eines Schwellenwerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdOrigin {
    Manual,
    SuggestedAccepted,
    /// Beim Abgleich aus Garmin übernommen (seit T39, SPEC 8.6). Verliert bei gleichem Datum
    /// gegen jede andere Herkunft.
    Garmin,
}

impl ThresholdOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::SuggestedAccepted => "suggested_accepted",
            Self::Garmin => "garmin",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<Self, ParseError> {
        match s {
            "manual" => Ok(Self::Manual),
            "suggested_accepted" => Ok(Self::SuggestedAccepted),
            "garmin" => Ok(Self::Garmin),
            _ => Err(ParseError(format!("threshold origin {s:?}"))),
        }
    }
}

/// Zeile von `threshold` mit allen Spalten, für `lastwerk thresholds list`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdRecord {
    pub id: i64,
    /// `None` gilt für alle Sportarten (Spaltenwert `all`).
    pub sport: Option<Sport>,
    pub kind: ThresholdKind,
    pub value: f64,
    pub valid_from: NaiveDate,
    pub origin: ThresholdOrigin,
    pub note: Option<String>,
}

/// Zeile von `activity_zone_time`. `zone` zählt ab 1. `estimated` ist gesetzt, wenn die Zonen
/// auf einer aus dem Alter geschätzten maximalen Herzfrequenz beruhen (SPEC 8.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneTimeRow {
    pub activity_id: i64,
    pub scheme: String,
    pub zone: u32,
    pub seconds: u32,
    pub estimated: bool,
}

/// Zeile von `activity_best_power`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BestPowerRow {
    pub activity_id: i64,
    pub duration_s: u32,
    pub watts: f64,
}

/// Zeile von `activity_best_effort` (T37).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BestEffortRow {
    pub activity_id: i64,
    pub distance_m: u32,
    pub seconds: u32,
}

/// Beste Leistung über eine Dauer in der ganzen Datenbank, mit der Aktivität, aus der sie
/// stammt (für die FTP-Schätzung, SPEC 8.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeakPower {
    pub activity_id: i64,
    pub local_date: NaiveDate,
    pub duration_s: u32,
    pub watts: f64,
}

/// Neuer Schwellenwert für [`crate::Store::insert_threshold`].
#[derive(Debug, Clone, PartialEq)]
pub struct NewThreshold {
    /// `None` gilt für alle Sportarten (Spaltenwert `all`).
    pub sport: Option<Sport>,
    pub kind: ThresholdKind,
    pub value: f64,
    pub valid_from: NaiveDate,
    pub origin: ThresholdOrigin,
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sport_text_roundtrip() {
        for s in [
            Sport::Cycling,
            Sport::Running,
            Sport::Swimming,
            Sport::Strength,
            Sport::Other,
        ] {
            assert_eq!(sport_from_str(sport_to_str(s)).unwrap(), s);
        }
        assert!(sport_from_str("rowing").is_err());
    }

    #[test]
    fn load_status_matches_core_error_codes() {
        let errors = [
            LoadError::MissingThreshold("x".into()),
            LoadError::MissingChannel("x".into()),
            LoadError::Implausible("x".into()),
        ];
        for e in errors {
            assert_eq!(LoadStatus::from_error(&e).as_str(), e.code());
        }
    }

    #[test]
    fn timestamps_and_dates_roundtrip() {
        let t = DateTime::from_timestamp(1_778_392_800, 0).unwrap();
        assert_eq!(fmt_utc(t), "2026-05-10T06:00:00Z");
        assert_eq!(parse_utc("2026-05-10T06:00:00Z").unwrap(), t);
        let d = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        assert_eq!(fmt_date(d), "2026-01-05");
        assert_eq!(parse_date("2026-01-05").unwrap(), d);
    }
}
