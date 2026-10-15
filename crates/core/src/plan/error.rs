//! Fehler des Planers. Jeder Fehler hat einen stabilen englischen Code (`PlanError::code`).

use chrono::NaiveDate;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PlanError {
    #[error("week_start {0} is not a Monday")]
    WeekStartNotMonday(NaiveDate),
    #[error("horizon_weeks must be between 1 and {max}, got {got}")]
    InvalidHorizon { got: u32, max: u32 },
    #[error("event date {event} lies before the first week start {week_start}")]
    EventBeforeStart {
        event: NaiveDate,
        week_start: NaiveDate,
    },
    #[error("goal is invalid: {0}")]
    InvalidGoal(String),
    #[error("no endurance sport with a positive weight")]
    NoSports,
    #[error("invalid sport weight for {0}")]
    InvalidSportWeight(String),
    #[error("invalid start values: {0}")]
    InvalidStartValues(String),
    #[error("ctl rise setting {got} must be above 0 and at most {max}")]
    InvalidCtlRise { got: f32, max: f32 },
    #[error("catalog template {template_id:?} is invalid: {reason}")]
    Catalog { template_id: String, reason: String },
    #[error("catalog has no usable template for {sport} ({class})")]
    MissingTemplate { sport: String, class: String },
    #[error("rules are invalid: {0}")]
    InvalidRules(String),
}

impl PlanError {
    /// Stabiler Fehlercode für Oberfläche und Protokolle.
    pub fn code(&self) -> &'static str {
        match self {
            Self::WeekStartNotMonday(_) => "week_start_not_monday",
            Self::InvalidHorizon { .. } => "invalid_horizon",
            Self::EventBeforeStart { .. } => "event_before_start",
            Self::InvalidGoal(_) => "invalid_goal",
            Self::NoSports => "no_sports",
            Self::InvalidSportWeight(_) => "invalid_sport_weight",
            Self::InvalidStartValues(_) => "invalid_start_values",
            Self::InvalidCtlRise { .. } => "invalid_ctl_rise",
            Self::Catalog { .. } => "catalog_invalid",
            Self::MissingTemplate { .. } => "missing_template",
            Self::InvalidRules(_) => "invalid_rules",
        }
    }
}
