//! Typen des Plans nach `plan.schema.json` (SPEC 7.1).
//!
//! `serde_json::to_value(&plan)` erzeugt genau die Feldnamen und Aufzählungswerte des
//! Schemas. Texte stehen nie im Plan, nur Schlüssel und Parameter. `planned_load` und
//! `target_load` berechnet immer `core`, nie das Sprachmodell.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use super::rules::Hardness;
use super::validate::Violation;

/// Version des Plan-Schemas. Das Schema fordert die Konstante 1.
pub const PLAN_SCHEMA_VERSION: u32 = 1;
/// Längster Planungshorizont in Wochen (Konvention, SPEC 7.1).
pub const MAX_PLAN_WEEKS: u32 = 26;
/// Mehr Einheiten je Tag erlaubt das Schema nicht.
pub const MAX_SESSIONS_PER_DAY: usize = 2;
/// Kürzeste und längste Einheit laut Schema in Minuten.
pub const MIN_SESSION_MINUTES: u32 = 10;
pub const MAX_SESSION_MINUTES: u32 = 360;

/// Sportart im Plan. Anders als `Sport` in der Belastungsberechnung ohne `Other`, dafür mit
/// `Rest` (Ruhetag, nur als Katalogeintrag, nie als geplante Einheit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanSport {
    Cycling,
    Running,
    Swimming,
    Strength,
    Rest,
}

impl PlanSport {
    /// Ausdauersportarten, die der Planer nach Gewicht verteilt.
    pub fn is_endurance(self) -> bool {
        matches!(self, Self::Cycling | Self::Running | Self::Swimming)
    }
}

/// Sprache der Anzeigetexte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locale {
    De,
    En,
}

/// Herkunft des Plans: Regeln oder Sprachmodell (das Modell wählt nur, rechnet nie).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanSource {
    Rules,
    Llm,
}

/// Trainingsphase einer Woche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Base,
    Build,
    Peak,
    Taper,
    Recovery,
    Race,
}

impl Phase {
    /// Belastungswoche im Sinn von R6 (zählt für die Blocklänge).
    pub fn is_load_week(self) -> bool {
        matches!(self, Self::Base | Self::Build | Self::Peak)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalKind {
    Event,
    Continuous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    A,
    B,
    C,
}

/// Ziel im Plan. Bei `kind = event` ist `event_date` Pflicht (Schema).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub kind: GoalKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sport: Option<PlanSport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Am,
    Pm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanSession {
    pub slot: Slot,
    pub sport: PlanSport,
    pub template_id: String,
    pub duration_min: u32,
    pub planned_load: f32,
    pub hard: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpe: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanDay {
    pub date: NaiveDate,
    pub sessions: Vec<PlanSession>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanWeek {
    pub week_start: NaiveDate,
    pub phase: Phase,
    pub target_load: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_load: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_keys: Vec<String>,
    pub days: Vec<PlanDay>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warn,
}

/// Warnung zu einer verletzten weichen Regel. `code` ist die Regelnummer (`R02`), die
/// Oberfläche schlägt Text und Quelle nach (SPEC 8.4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlanWarning {
    pub code: String,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<NaiveDate>,
}

fn deserialize_schema_version<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = u32::deserialize(d)?;
    if v == PLAN_SCHEMA_VERSION {
        Ok(v)
    } else {
        Err(serde::de::Error::custom(format!(
            "schema_version muss {PLAN_SCHEMA_VERSION} sein, ist {v}"
        )))
    }
}

/// Wochenplan nach `plan.schema.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u32,
    /// UUID, vom Aufrufer vergeben (`core` erzeugt keine).
    pub plan_id: String,
    /// RFC 3339, vom Aufrufer übergeben (`core` liest keine Uhr).
    pub created_at: DateTime<Utc>,
    pub locale: Locale,
    pub source: PlanSource,
    /// Version von `rules.default.json` (`x.y.z`).
    pub rules_version: String,
    pub goal: Goal,
    pub weeks: Vec<PlanWeek>,
    #[serde(default)]
    pub warnings: Vec<PlanWarning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

impl PlanSession {
    /// Die Einheit gehört zu den Ausdauereinheiten (alles außer Kraft).
    pub fn is_endurance(&self) -> bool {
        self.sport.is_endurance()
    }
}

impl PlanWeek {
    /// Summe der Dauern aller Einheiten der Woche in Minuten (Wochenvolumen, R2).
    pub fn total_minutes(&self) -> u32 {
        self.days
            .iter()
            .flat_map(|d| d.sessions.iter())
            .map(|s| s.duration_min)
            .sum()
    }

    /// Summe der geplanten Belastung der Einheiten.
    pub fn sessions_load(&self) -> f32 {
        self.days
            .iter()
            .flat_map(|d| d.sessions.iter())
            .map(|s| s.planned_load)
            .sum()
    }
}

impl Plan {
    /// Ein Plan ist annehmbar, wenn keine harte Regel verletzt ist (SPEC 8.4). Weiche
    /// Verletzungen erzeugen nur Warnungen.
    pub fn is_acceptable(&self, violations: &[Violation]) -> bool {
        is_acceptable(violations)
    }
}

/// Wie [`Plan::is_acceptable`], ohne Plan.
pub fn is_acceptable(violations: &[Violation]) -> bool {
    !violations.iter().any(|v| v.hardness == Hardness::Hard)
}
