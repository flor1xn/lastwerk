//! Parameter der Werkzeuge (SPEC 5.4, 9.6, 10.2).
//!
//! Alle Argumente sind Datumswerte, ganze Zahlen, Aufzählungen und feste Codes. Es gibt kein
//! Freitextfeld, keinen Pfad und keine Abfragesprache (Schutz gegen Prompt-Injection über
//! Werkzeugargumente). Unbekannte Felder lehnt der Server ab (`deny_unknown_fields`). Die
//! Bereichsgrenzen stehen im Schema, die Prüfung selbst liegt in `tools.rs` und gilt
//! unabhängig davon, ob der Client das Schema beachtet.

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::views::Sport;

/// Längster Zeitraum von `get_fitness_status` in Tagen.
pub const MAX_FITNESS_DAYS: i64 = 366;
/// Höchstzahl der Wochen von `get_weekly_load`.
pub const MAX_WEEKS: u32 = 52;
/// Höchstzahl der Zeilen von `list_activities`.
pub const MAX_ACTIVITIES: usize = 200;
/// Längster Zeitraum von `get_recovery_days` in Tagen.
pub const MAX_RECOVERY_DAYS: i64 = 90;

// Zeitraum für `get_fitness_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FitnessArgs {
    /// First day, inclusive (YYYY-MM-DD).
    pub from: NaiveDate,
    /// Last day, inclusive (YYYY-MM-DD). At most 366 days after `from`. Days in the future
    /// are left out.
    pub to: NaiveDate,
}

// Parameter von `get_weekly_load`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WeeklyLoadArgs {
    /// Number of calendar weeks up to and including the current week (1 to 52).
    #[schemars(range(min = 1, max = 52))]
    pub weeks: u32,
    /// Only this sport. Omit for all sports.
    #[serde(default)]
    pub sport: Option<Sport>,
}

// Parameter von `list_activities`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListActivitiesArgs {
    /// First day, inclusive (YYYY-MM-DD).
    pub from: NaiveDate,
    /// Last day, inclusive (YYYY-MM-DD).
    pub to: NaiveDate,
    /// Only this sport. Omit for all sports.
    #[serde(default)]
    pub sport: Option<Sport>,
}

// Parameter von `get_activity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivityArgs {
    /// Activity number as returned by `list_activities`.
    #[schemars(range(min = 1))]
    pub id: i64,
}

// Parameter von `get_plan_week`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanWeekArgs {
    /// Monday of the week (YYYY-MM-DD). Any other day selects the week it lies in.
    pub week_start: NaiveDate,
}

// Parameter von `get_thresholds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ThresholdsArgs {
    /// Day for which the thresholds are valid (YYYY-MM-DD).
    pub on_date: NaiveDate,
}

// Parameter von `explain_rule`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleArgs {
    /// Rule code, for example `R03`. Plan warnings carry these codes.
    #[schemars(length(min = 3, max = 3), regex(pattern = r"^R[0-9]{2}$"))]
    pub code: String,
}

// Zeitraum für `get_recovery_days`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryArgs {
    /// First day, inclusive (YYYY-MM-DD).
    pub from: NaiveDate,
    /// Last day, inclusive (YYYY-MM-DD). At most 90 days after `from`.
    pub to: NaiveDate,
}

// Parameter ohne Felder (`get_current_plan`). Das Schema ist ein leeres Objekt, weitere
// Felder lehnt der Server ab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}
