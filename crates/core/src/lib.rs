//! lastwerk-core: Reine Berechnungslogik ohne Ein- und Ausgabe (Belastung, PMC, Planer)
//!
//! Die Schnittstellen stehen in docs/SPEC.md, Abschnitt 5.4. Das Crate kennt
//! weder Dateisystem noch Netzwerk, Systemzeit oder Zufall. Zeit und Zufall
//! werden übergeben (SPEC 11.1).

pub mod best_effort;
pub mod best_power;
pub mod estimate;
pub mod load;
pub mod metrics;
pub mod plan;
pub mod pmc;
pub mod zones;

pub use best_effort::{
    BEST_EFFORT_DISTANCES_M, BestEffort, RUN_SPEED_MAX_MPS, best_efforts, best_efforts_prepared,
};
pub use best_power::{
    BEST_POWER_DURATIONS_S, BestPower, POWER_CURVE_DURATIONS_S, best_power, best_power_prepared,
};
pub use estimate::{
    FTP_FRACTION_OF_BEST_20_MIN, HrMax, resolve_hr_max, suggest_css, suggest_ftp,
    suggest_lthr_from_test,
};
pub use load::{
    ActivityInput, Load, LoadConfig, LoadError, LoadMethod, NP_WINDOW_S, Plausibility,
    PreparedSamples, Sample, Sex, Sport, Thresholds, TrimpParams, check_plausibility, compute_load,
    compute_load_prepared, estimate_hr_max, normalized_power, session_rpe_au,
};
pub use plan::{
    Plan, PlanContext, PlanError, Rules, TemplateCatalog, Violation, generate_plan, validate_plan,
};
pub use pmc::{DailyLoad, PmcParams, PmcPoint, pmc};
pub use zones::{
    ZoneBasis, ZoneCatalog, ZoneChannel, ZoneError, ZoneScheme, zone_seconds, zone_seconds_prepared,
};

/// Name des Crates, nur zum Prüfen des Gerüsts.
pub const CRATE_NAME: &str = "lastwerk-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
