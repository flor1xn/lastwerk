//! Der Planer und die Prüfung rechnen mit den Parametern des Verlaufs aus den Einstellungen
//! `pmc.*` (SPEC 5.4, Nachtrag vom 14. Oktober 2026). Ohne Einstellungen gelten die
//! Standardwerte, dann bleiben die Golden-Snapshots unverändert (`golden_tests.rs`).

use lastwerk_core::plan::{Locale, PlanSport};
use lastwerk_core::{PmcParams, Sport};
use lastwerk_store::{ActivityRow, LoadRow, LoadStatus, SourceFileRow, SourceStatus, Store};

use crate::planning::{PlanRequest, generate, load_rules, validation_context};
use crate::testctx::{PLAN_ID, created_at, date, monday};

/// Eine Aktivität mit Last 100 am Sonntag, 3. Januar 2027, dem Tag vor dem Planbeginn
/// (Montag, 4. Januar 2027). Sie ist die erste und einzige Last, der Verlauf beginnt dort.
fn store_with_one_load() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    let day = date(2027, 1, 3);
    let start = day.and_hms_opt(6, 0, 0).unwrap().and_utc();
    let file_id = store
        .upsert_source_file(&SourceFileRow {
            rel_path: "synthetisch.fit".into(),
            sha256: format!("{:064x}", 1),
            size_bytes: 1000,
            mtime_utc: start,
            imported_at: start,
            status: SourceStatus::Ok,
            error_code: None,
        })
        .unwrap();
    let activity_id = store
        .upsert_activity(&ActivityRow {
            source_file_id: file_id,
            sport: Sport::Cycling,
            sub_sport: None,
            start_utc: start,
            local_offset_s: 0,
            local_date: day,
            duration_s: 3600,
            moving_s: Some(3600),
            distance_m: None,
            ascent_m: None,
            avg_hr: None,
            max_hr: None,
            avg_power_w: None,
            np_w: None,
            device_serial: None,
            device_product: None,
            has_power: false,
            has_hr: false,
        })
        .unwrap();
    store
        .upsert_activity_load(&LoadRow {
            missing: None,
            activity_id,
            method: "tss".into(),
            value: Some(100.0),
            intensity_factor: None,
            threshold_id: None,
            status: LoadStatus::Ok,
            computed_at: start,
        })
        .unwrap();
    store
}

fn request() -> PlanRequest {
    PlanRequest {
        week_start: monday(),
        goal_date: None,
        goal_sport: None,
        weeks: Some(4),
        locale: Locale::De,
        sports: vec![PlanSport::Cycling],
        strength_per_week: 1,
    }
}

/// CTL, ATL und Zeitkonstanten, mit denen `validate` einen Plan dieser Datenbank prüft.
fn validation_start(store: &Store) -> (f32, f32, PmcParams) {
    let generated = generate(store, &request(), PLAN_ID.to_string(), created_at()).unwrap();
    let (rules, catalog) = load_rules().unwrap();
    let ctx = validation_context(store, &generated.plan, &rules, catalog, &mut Vec::new()).unwrap();
    (ctx.ctl0, ctx.atl0, ctx.pmc_params)
}

fn close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{a} statt {b}");
}

fn target_loads(store: &Store) -> Vec<f32> {
    generate(store, &request(), PLAN_ID.to_string(), created_at())
        .unwrap()
        .plan
        .weeks
        .iter()
        .map(|w| w.target_load)
        .collect()
}

#[test]
fn defaults_give_the_values_of_the_default_time_constants() {
    let store = store_with_one_load();
    let (ctl, atl, params) = validation_start(&store);
    assert_eq!(params, PmcParams::default());
    // Handrechnung (SPEC 8.2): Der Verlauf beginnt am 3. Januar mit CTL = ATL = 0 und Last
    // 100. CTL = 0 + (100 - 0) * (1 - exp(-1/42)) = 100 * 0,0235283 = 2,35283,
    // ATL = 100 * (1 - exp(-1/7)) = 100 * 0,1331221 = 13,31221.
    close(ctl, 2.352_83);
    close(atl, 13.312_21);
}

#[test]
fn time_constants_from_the_settings_change_the_start_values() {
    let mut store = store_with_one_load();
    store.set_setting("pmc.tau_ctl_d", "50").unwrap();
    store.set_setting("pmc.tau_atl_d", "15").unwrap();
    let (ctl, atl, params) = validation_start(&store);
    assert_eq!((params.tau_ctl_d, params.tau_atl_d), (50.0, 15.0));
    // Handrechnung: CTL = 100 * (1 - exp(-1/50)) = 100 * 0,0198013 = 1,98013,
    // ATL = 100 * (1 - exp(-1/15)) = 100 * 0,0644930 = 6,44930.
    close(ctl, 1.980_13);
    close(atl, 6.449_30);
}

#[test]
fn start_values_from_the_settings_enter_the_first_day() {
    let mut store = store_with_one_load();
    store.set_setting("pmc.ctl0", "30").unwrap();
    store.set_setting("pmc.atl0", "20").unwrap();
    let (ctl, atl, _) = validation_start(&store);
    // Handrechnung: CTL = 30 + (100 - 30) * 0,0235283 = 30 + 1,64698 = 31,64698,
    // ATL = 20 + (100 - 20) * 0,1331221 = 20 + 10,64977 = 30,64977.
    close(ctl, 31.646_98);
    close(atl, 30.649_77);
}

/// Ohne Last im Verlauf gelten die Startwerte selbst (`pmc_state_at`), auch im Planer.
#[test]
fn without_any_load_the_start_values_are_the_settings() {
    let mut store = Store::open_in_memory().unwrap();
    store.set_setting("pmc.ctl0", "30").unwrap();
    store.set_setting("pmc.atl0", "20").unwrap();
    let (ctl, atl, _) = validation_start(&store);
    assert_eq!((ctl, atl), (30.0, 20.0));
}

/// Ohne Last in der Woche vor dem Planbeginn baut der Planer die erste Woche auf der Fitness
/// auf (CTL am Vortag). Die Startwerte aus den Einstellungen verändern deshalb die Zielwerte.
#[test]
fn the_generated_plan_follows_the_settings() {
    let default_store = Store::open_in_memory().unwrap();
    let mut tuned = Store::open_in_memory().unwrap();
    tuned.set_setting("pmc.ctl0", "60").unwrap();
    tuned.set_setting("pmc.atl0", "60").unwrap();
    tuned.set_setting("pmc.tau_ctl_d", "20").unwrap();
    assert_ne!(target_loads(&default_store), target_loads(&tuned));
}

/// Unlesbare Einstellungen fallen je Wert auf den Standard zurück, der Planer läuft.
#[test]
fn unreadable_settings_fall_back_to_the_defaults() {
    let mut store = store_with_one_load();
    store.set_setting("pmc.tau_ctl_d", "viel").unwrap();
    let (ctl, _, params) = validation_start(&store);
    assert_eq!(params, PmcParams::default());
    close(ctl, 2.352_83);
}
