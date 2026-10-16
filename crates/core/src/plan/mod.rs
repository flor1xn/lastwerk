//! Regelbasierter Planer: Typen, Katalog, Regeln, Erzeugung, Prüfung und Tagesentscheidung
//! (SPEC 7.1, 7.4, 8.3, 8.4, 8.7).
//!
//! `core` liest keine Dateien. Katalog (`catalog/templates.json`) und Regeln
//! (`catalog/rules.default.json`) kommen als deserialisierte Strukturen vom Aufrufer, ebenso
//! `plan_id` und `created_at`.

mod catalog;
mod context;
mod error;
mod generate;
pub(crate) mod recovery;
mod rules;
mod today;
mod types;
mod validate;

#[cfg(test)]
mod tests;

pub use catalog::{
    DurationWindow, IntensityClass, Step, StepKind, Template, TemplateCatalog, is_valid_reason_key,
    is_valid_template_id,
};
pub use context::{DayAvailability, GoalSpec, PlanContext, PreviousWeek, SportWeight, WeekHistory};
pub use error::PlanError;
pub use generate::{generate_plan, keys as reason_keys};
pub use recovery::{RecoveryDay, RecoverySignal, recovery_signal};
pub use rules::{
    ContinuousPhases, EventPhases, GeneratorParams, Hardness, LoadModel, PhaseParams, PhaseProfile,
    RuleMeta, Rules, StructureParams, is_valid_source, round1,
};
pub use today::{
    DecisionReason, DecisionState, TodayDecision, TodayInput, Variant, VariantKind,
    missing as decision_missing, today_decision,
};
pub use types::{
    Goal, GoalKind, Locale, MAX_PLAN_WEEKS, MAX_SESSION_MINUTES, MAX_SESSIONS_PER_DAY,
    MIN_SESSION_MINUTES, PLAN_SCHEMA_VERSION, Phase, Plan, PlanDay, PlanSession, PlanSource,
    PlanSport, PlanWarning, PlanWeek, Priority, Severity, Slot, is_acceptable,
};
pub use validate::{Violation, validate_plan, violations_to_warnings};
