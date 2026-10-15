//! Ausgabetypen der Werkzeuge mit fester Feldliste (SPEC 5.4, 9.6).
//!
//! Datenbankzeilen werden nie direkt serialisiert. Jeder Typ hier hat genau die Felder, die
//! SPEC 5.4 nennt, und wird aus den Zeilen der Datenbank von Hand gefüllt. Textfelder sind
//! Konstanten des Programms (Sportart, Methode, Schemakennung, Art der Schwelle), nie Text aus
//! Dateien oder Nutzereingaben. Ein Test (`tests/views.rs`) hält die Feldlisten fest.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use lastwerk_core::{LoadMethod, Sport as CoreSport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Gespiegelt aus `lastwerk-core`, weil `core` keine Schema-Bibliothek kennt.
/// Sport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sport {
    Cycling,
    Running,
    Swimming,
    Strength,
    Other,
}

impl From<CoreSport> for Sport {
    fn from(s: CoreSport) -> Self {
        match s {
            CoreSport::Cycling => Self::Cycling,
            CoreSport::Running => Self::Running,
            CoreSport::Swimming => Self::Swimming,
            CoreSport::Strength => Self::Strength,
            CoreSport::Other => Self::Other,
        }
    }
}

impl From<Sport> for CoreSport {
    fn from(s: Sport) -> Self {
        match s {
            Sport::Cycling => Self::Cycling,
            Sport::Running => Self::Running,
            Sport::Swimming => Self::Swimming,
            Sport::Strength => Self::Strength,
            Sport::Other => Self::Other,
        }
    }
}

/// Method used to compute the load of an activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Tss,
    HrTss,
    RTss,
    STss,
    SessionRpe,
}

impl From<LoadMethod> for Method {
    fn from(m: LoadMethod) -> Self {
        match m {
            LoadMethod::Tss => Self::Tss,
            LoadMethod::HrTss => Self::HrTss,
            LoadMethod::RTss => Self::RTss,
            LoadMethod::STss => Self::STss,
            LoadMethod::SessionRpe => Self::SessionRpe,
        }
    }
}

impl Method {
    /// Methode aus dem Text der Datenbank (`activity_load.method`). Unbekannte Werte und
    /// `none` ergeben `None`: ein Text aus der Datenbank wird nie weitergereicht.
    pub fn from_db(s: &str) -> Option<Self> {
        Some(match s {
            "tss" => Self::Tss,
            "hr_tss" => Self::HrTss,
            "r_tss" => Self::RTss,
            "s_tss" => Self::STss,
            "session_rpe" => Self::SessionRpe,
            _ => return None,
        })
    }
}

/// Time spent in one zone of a zone scheme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ZoneMinutes {
    /// Zone scheme, for example `power7` (power zones), `hr_lthr5` or `hr_karvonen` (heart rate).
    pub scheme: String,
    /// Zone number, starting at 1.
    pub zone: u32,
    /// Minutes in the zone, rounded to one decimal.
    pub minutes: f64,
    /// The zones rest on a maximum heart rate estimated from age.
    pub estimated: bool,
}

/// Best average power over a duration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BestPower {
    /// 5, 60, 300, 1200 or 3600 seconds.
    pub duration_s: u32,
    pub watts: f64,
}

/// One activity without name, location, device or file data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActivityView {
    /// Number in the local database, not an identifier of any provider.
    pub id: i64,
    /// Local date. There is no time of day.
    pub local_date: NaiveDate,
    pub sport: Sport,
    /// Duration in minutes, rounded.
    pub duration_min: u32,
    /// Training load, `null` if it cannot be computed (missing data is not 0).
    pub load: Option<f64>,
    pub load_method: Option<Method>,
    pub intensity_factor: Option<f64>,
    pub zone_minutes: Vec<ZoneMinutes>,
    /// Only in `get_activity`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_power_w: Option<Vec<BestPower>>,
}

/// Fitness, fatigue and form on one day.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FitnessView {
    pub date: NaiveDate,
    /// Fitness (CTL) at the end of the day.
    pub ctl: f64,
    /// Fatigue (ATL) at the end of the day.
    pub atl: f64,
    /// Form in the morning: CTL minus ATL of the previous day.
    pub tsb: f64,
}

/// Load and training time of one sport in one calendar week.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeeklyLoadView {
    /// Monday of the week.
    pub week_start: NaiveDate,
    pub sport: Sport,
    /// Sum of the computed loads (the ones that enter CTL). Activities without a computed
    /// load (for example strength from a FIT file) add 0 here.
    pub load: f64,
    /// Training time of all primary activities, with or without a computed load.
    pub duration_min: u32,
}

/// Result of `get_fitness_status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FitnessStatus {
    /// One entry per day, starting no earlier than the first day with a counted load.
    pub days: Vec<FitnessView>,
}

/// Result of `get_weekly_load`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeeklyLoad {
    /// Weeks and sports without a primary activity are left out.
    pub weeks: Vec<WeeklyLoadView>,
}

/// Result of `list_activities`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActivityList {
    /// Oldest first, at most 200.
    pub activities: Vec<ActivityView>,
    /// There are more activities in the range. Narrow the range to see them.
    pub truncated: bool,
}

/// One threshold with the day it is valid from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ThresholdValue {
    /// `ftp_w`, `lthr_bpm`, `hr_max_bpm`, `hr_rest_bpm`, `run_speed_mps` oder `css_mps`.
    pub kind: String,
    pub value: f64,
    /// Unit of the value: `W`, `bpm` or `m/s`.
    pub unit: String,
    /// The value is valid from this day on.
    pub valid_from: NaiveDate,
}

/// Thresholds of one sport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SportThresholds {
    pub sport: Sport,
    pub values: Vec<ThresholdValue>,
}

/// Result of `get_thresholds`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ThresholdsView {
    pub on_date: NaiveDate,
    /// Only sports with at least one valid value on that day.
    pub sports: Vec<SportThresholds>,
}

/// Result of `explain_rule`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RuleExplanation {
    pub code: String,
    /// Short title and explanation from the text catalog, in German if the user interface is
    /// set to German, otherwise in English.
    pub title: String,
    pub explanation: String,
    /// `hard` rejects a plan, `soft` only warns.
    pub hardness: String,
    /// Source: a reference `Qnn` or one of `Faustregel` (rule of thumb), `Konvention`
    /// (convention) and `Nutzereingabe` (user input).
    pub source: String,
    /// Further references `Qnn`.
    pub references: Vec<String>,
    /// Numeric parameters of the rule.
    pub parameters: BTreeMap<String, serde_json::Value>,
}

/// Recovery values of one day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecoveryDayView {
    pub date: NaiveDate,
    pub hrv_overnight_ms: Option<f64>,
    pub resting_hr_bpm: Option<f64>,
    pub sleep_duration_min: Option<u32>,
}

/// Result of `get_recovery_days`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecoveryDays {
    pub days: Vec<RecoveryDayView>,
}

/// Zahl der Zeilen einer Antwort, für das Aufrufprotokoll.
pub trait RowCount {
    fn row_count(&self) -> usize;
}

impl RowCount for FitnessStatus {
    fn row_count(&self) -> usize {
        self.days.len()
    }
}
impl RowCount for WeeklyLoad {
    fn row_count(&self) -> usize {
        self.weeks.len()
    }
}
impl RowCount for ActivityList {
    fn row_count(&self) -> usize {
        self.activities.len()
    }
}
impl RowCount for ActivityView {
    fn row_count(&self) -> usize {
        1
    }
}
impl RowCount for ThresholdsView {
    fn row_count(&self) -> usize {
        self.sports.iter().map(|s| s.values.len()).sum()
    }
}
impl RowCount for RuleExplanation {
    fn row_count(&self) -> usize {
        1
    }
}
impl RowCount for RecoveryDays {
    fn row_count(&self) -> usize {
        self.days.len()
    }
}
