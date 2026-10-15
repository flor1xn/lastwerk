//! lastwerk-store: SQLite-Speicher, Cache und Nutzerdaten
//!
//! Die Datenbank ist für alles, was aus FIT ableitbar ist, ein Cache (SPEC 6.1, 6.3). Die
//! Schnittstellen stehen in docs/SPEC.md, Abschnitt 5.4, das Schema in Abschnitt 6.2.
//!
//! - [`Store::open`] führt die Migrationen aus ([`migrate`]).
//! - [`Store::import_dir`] und [`Store::rebuild_cache`] lesen einen Ordner ein ([`import`]).
//! - [`duplicates`] enthält die Duplikatregel für Radcomputer und Uhr (SPEC 6.4).
//! - Beim Import entstehen auch Zeit in Zonen und Bestleistungen (SPEC 8.5, 6.2). Die Zonen
//!   kommen aus dem eingebetteten Katalog `catalog/zones.json` ([`embedded_zone_catalog`]).
//! - [`Store::recompute_loads`] rechnet nach einer geänderten Schwelle die betroffenen
//!   Aktivitäten aus den FIT-Dateien neu.
//! - [`Store::export_user_data`] und [`Store::import_user_data`] sichern die Nutzerdaten.
//! - [`Store::activity`], [`Store::activities_between`] und weitere gezielte Lesezugriffe
//!   für die Oberfläche ([`read`]).
//! - [`Store::export_user_data`] und [`Store::import_user_data`] sichern die Nutzerdaten
//!   (`backup_version` 1, SPEC 6.3).
//! - [`Store::save_plan`], [`Store::list_plans`], [`Store::load_plan`] und
//!   [`Store::set_plan_status`] verwalten die Pläne ([`plan_store`]), [`Store::set_goal`] und
//!   [`Store::set_availability`] die Eingaben des Planers ([`goals`]),
//!   [`Store::week_history`] und [`Store::pmc_state_at`] liefern ihm die Historie
//!   ([`history`]).
//! - [`Store::open_read_only`] gehört zum MCP-Server
//!   ([`readonly`]).
//! - [`Store::upsert_recovery_day`] und [`Store::recovery_days`] führen die Erholungstage aus dem
//!   Zusatzprogramm ([`recovery`]). Sie sind Nutzerdaten (nicht aus FIT ableitbar).
//! - [`Store::garmin_settings`] und die zugehörigen Setter halten die Einstellungen des
//!   Abgleichs mit Garmin ([`garmin_settings`]).
//! - [`Store::plausibility_summary`] liefert nur Kennzahlen für `lastwerk plausibility`.
//!
//! Die Uhrzeit kommt immer von außen (Parameter `now`), damit Tests deterministisch sind.

mod catalog;
mod derived;
pub mod duplicates;
mod error;
mod garmin_settings;
mod goals;
mod history;
pub mod import;
pub mod migrate;
mod plan_store;
mod read;
mod readonly;
mod recompute;
mod recovery;
mod rows;
mod statements;
mod store;
mod summary;
mod userdata;

pub use catalog::{embedded_zone_catalog, parse_zone_catalog};
pub use derived::SETTING_ATHLETE_BIRTH_YEAR;
pub use duplicates::DuplicateRule;
pub use error::{Result, StoreError};
pub use garmin_settings::{
    DEFAULT_BACKFILL_DAYS, GarminSettings, MAX_BACKFILL_DAYS, MIN_BACKFILL_DAYS,
    SETTING_GARMIN_BACKFILL_DAYS, SETTING_GARMIN_LAST_SYNC, SETTING_GARMIN_SYNC_ON_START,
};
pub use goals::{AvailabilityRecord, GoalRecord, MAX_AVAILABLE_MINUTES, NewGoal, plan_sport_str};
pub use history::{
    SETTING_PMC_ATL0, SETTING_PMC_CTL0, SETTING_PMC_TAU_ATL, SETTING_PMC_TAU_CTL, monday_of,
};
pub use import::{
    DERIVED_CACHE_VERSION, FileIssue, ImportOptions, ImportReport, Progress, sha256_hex,
};
pub use plan_store::{PlanStatus, PlanSummary};
pub use recompute::RecomputeReport;
pub use recovery::{
    BODY_BATTERY_MAX, HRV_MAX_MS, RESTING_HR_MAX_BPM, RESTING_HR_MIN_BPM, RecoveryDayRecord,
    SLEEP_MAX_S, SLEEP_SCORE_MAX, SOURCE_MAX_CHARS, UpsertOutcome,
};
pub use rows::{
    ActivityRecord, ActivityRow, BestEffortRow, BestPowerRow, LoadRow, LoadStatus, METHOD_NONE,
    NewThreshold, PeakPower, SourceFileRecord, SourceFileRow, SourceStatus, ThresholdKind,
    ThresholdOrigin, ThresholdRecord, ZoneTimeRow, method_to_str, sport_from_str, sport_to_str,
};
// `DailyLoad` führt seit T05 `lastwerk-core`, der Speicher gibt den Typ unverändert weiter.
pub use lastwerk_core::DailyLoad;
pub use store::{SETTING_ATHLETE_SEX, SETTING_MCP_RECOVERY_ENABLED, Store, ThresholdLookup};
pub use summary::{NP_RATIO_FLAG, PlausibilitySummary, Range};
pub use userdata::{
    AvailabilityExport, GoalExport, ManualSessionExport, PlanExport, RecoveryDayExport,
    SettingExport, ThresholdExport, USER_DATA_FORMAT_VERSION, UserDataExport,
};

/// Name des Crates, nur zum Prüfen des Gerüsts.
pub const CRATE_NAME: &str = "lastwerk-store";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
