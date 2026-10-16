//! Gemeinsame Hilfen der Integrationstests der CLI zum Sprachmodell (`refine`, `chat`):
//! Datenbank mit Markerwerten in allen Feldern, die nie zum Modell gehen dürfen (SPEC 9.3,
//! 11.4). Alle Daten sind synthetisch, alle Dateien liegen unter `target/`.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, TimeDelta, TimeZone, Utc};
use lastwerk_cli::planning::{self, PlanRequest};
use lastwerk_core::Sport;
use lastwerk_core::plan::{GoalKind, Locale, PlanSport, Priority};
use lastwerk_store::{
    ActivityRow, ImportOptions, LoadRow, LoadStatus, NewGoal, NewThreshold, PlanStatus,
    RecoveryDayRecord, SourceFileRow, SourceStatus, Store, ThresholdKind, ThresholdOrigin,
};
use lastwerk_testkit::fixtures;

// Markerwerte (SPEC 11.4): Sie stehen in Feldern, die nie zum Modell gehen dürfen.
pub const M_LABEL: &str = "MARKER-GOAL-LABEL-7f3a";
pub const M_NOTE: &str = "MARKER-THRESHOLD-NOTE-91c2";
pub const M_DIR: &str = "MARKER-DIR-5d1e";
pub const M_FILE: &str = "MARKER-FILE-2b8c";
pub const M_SERIAL: u32 = 4_242_424_242;
pub const M_PRODUCT: &str = "MARKER-PRODUCT-c4d0";
pub const M_SUB_SPORT: &str = "MARKER-SUBSPORT-83aa";
pub const M_SETTING_VALUE: &str = "MARKER-SETTING-VALUE-e60b";
pub const M_SETTING_KEY: &str = "MARKER-SETTING-KEY-1d2f";
pub const M_RECOVERY_SOURCE: &str = "MARKER-RECOVERY-SOURCE-0e77";
pub const M_LAT: &str = "52.520008";
pub const M_LON: &str = "13.404954";

pub fn markers() -> Vec<String> {
    [
        M_LABEL,
        M_NOTE,
        M_DIR,
        M_FILE,
        M_PRODUCT,
        M_SUB_SPORT,
        M_SETTING_VALUE,
        M_SETTING_KEY,
        M_RECOVERY_SOURCE,
        M_LAT,
        M_LON,
        "MARKER",
        ".fit",
    ]
    .iter()
    .map(ToString::to_string)
    .chain([M_SERIAL.to_string()])
    .collect()
}

pub fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

pub fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 9, 0, 0).unwrap()
}

/// Leerer Arbeitsordner `target/tmp/<group>/<name>`.
pub fn work_dir(group: &str, name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(group)
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn threshold(
    sport: Option<Sport>,
    kind: ThresholdKind,
    value: f64,
    note: Option<&str>,
) -> NewThreshold {
    NewThreshold {
        sport,
        kind,
        value,
        valid_from: date(2026, 1, 1),
        origin: ThresholdOrigin::Manual,
        note: note.map(str::to_string),
    }
}

/// Datenbank mit Markerwerten in allen Feldern, die nie zum Modell gehen dürfen (wie in den
/// Tests des MCP-Servers, T10): Zielbezeichnung, Notizen, Dateipfade, Gerätenummer, Produktname,
/// Unterart, Einstellungen und Quelle der Erholungsdaten. Dazu echte Zahlen für die Historie.
pub fn build_marker_db(dir: &Path) -> PathBuf {
    let path = dir.join("lastwerk.db");
    let mut store = Store::open(&path).unwrap();
    let note = format!("{M_NOTE} {M_LAT},{M_LON}");
    for t in [
        threshold(
            Some(Sport::Cycling),
            ThresholdKind::FtpW,
            250.0,
            Some(&note),
        ),
        threshold(
            Some(Sport::Cycling),
            ThresholdKind::LthrBpm,
            160.0,
            Some(&note),
        ),
        threshold(Some(Sport::Running), ThresholdKind::RunSpeedMps, 3.5, None),
        threshold(None, ThresholdKind::HrMaxBpm, 190.0, None),
        threshold(None, ThresholdKind::HrRestBpm, 50.0, Some(&note)),
    ] {
        store.insert_threshold(&t).unwrap();
    }
    store.set_setting("athlete.sex", "male").unwrap();
    store
        .set_setting("app.fit_dir", &format!("/home/{M_SETTING_VALUE}/fit"))
        .unwrap();
    store.set_setting(M_SETTING_KEY, M_SETTING_VALUE).unwrap();

    // Fixtures unter einem Ordner und mit Dateinamen mit Marker.
    let fit_dir = dir.join("fit");
    let sub = fit_dir.join(M_DIR);
    fs::create_dir_all(&sub).unwrap();
    for f in fixtures::all() {
        fs::write(sub.join(format!("{M_FILE}-{}", f.file_name())), &f.bytes).unwrap();
    }
    store
        .import_dir(&fit_dir, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();

    // Handaktivitäten: 14 Tage ab dem 11. Mai 2026 (zwei volle Wochen vor dem Planbeginn).
    for n in 0..14u32 {
        let day = date(2026, 5, 11) + TimeDelta::days(i64::from(n));
        let file_id = store
            .upsert_source_file(&SourceFileRow {
                rel_path: format!("{M_DIR}/{M_FILE}-hand-{n}.fit"),
                sha256: format!("{n:064x}"),
                size_bytes: 1000,
                mtime_utc: now(),
                imported_at: now(),
                status: SourceStatus::Ok,
                error_code: None,
            })
            .unwrap();
        let activity_id = store
            .upsert_activity(&ActivityRow {
                source_file_id: file_id,
                sport: if n % 2 == 0 {
                    Sport::Cycling
                } else {
                    Sport::Running
                },
                sub_sport: Some(format!("{M_SUB_SPORT} {M_LAT},{M_LON}")),
                start_utc: day.and_hms_opt(6, 0, 0).unwrap().and_utc(),
                local_offset_s: 7200,
                local_date: day,
                duration_s: 3600 + i64::from(n) * 60,
                moving_s: Some(3500),
                distance_m: Some(30_000.0),
                ascent_m: None,
                avg_hr: Some(140.0),
                max_hr: Some(170.0),
                avg_power_w: Some(200.0),
                np_w: Some(210.0),
                device_serial: Some(M_SERIAL),
                device_product: Some(format!("{M_PRODUCT} {M_LAT}")),
                has_power: true,
                has_hr: true,
            })
            .unwrap();
        store
            .upsert_activity_load(&LoadRow {
                missing: None,
                activity_id,
                method: "tss".into(),
                value: Some(50.0 + f64::from(n % 5) * 10.0),
                intensity_factor: Some(0.8),
                threshold_id: None,
                status: LoadStatus::Ok,
                computed_at: now(),
            })
            .unwrap();
    }
    // Erholungstage mit Marker in der Quelle, HRV deutlich unter der Basislinie.
    for k in 1..=9 {
        store
            .upsert_recovery_day(&RecoveryDayRecord {
                date: date(2026, 5, 25) - TimeDelta::days(k),
                hrv_overnight_ms: Some(if k == 1 { 40.0 } else { 60.0 }),
                resting_hr_bpm: Some(48.0),
                sleep_duration_s: Some(25_200),
                sleep_score: Some(80),
                body_battery_high: None,
                body_battery_low: None,
                source: M_RECOVERY_SOURCE.into(),
                imported_at: now(),
            })
            .unwrap();
    }
    store
        .set_goal(&NewGoal {
            kind: GoalKind::Event,
            sport: Some(PlanSport::Cycling),
            event_date: Some(date(2026, 6, 14)),
            label: Some(format!("{M_LABEL} {M_LAT},{M_LON}")),
            priority: Some(Priority::A),
        })
        .unwrap();
    store.set_availability(0, 60, true, false).unwrap();
    path
}

/// Plan aus der Markerdatenbank, als Datei geschrieben. Gibt Datenbank und Plandatei zurück.
pub fn marker_plan(dir: &Path) -> (PathBuf, PathBuf) {
    let db = build_marker_db(dir);
    let mut store = Store::open(&db).unwrap();
    let req = PlanRequest {
        week_start: date(2026, 5, 25),
        goal_date: None,
        goal_sport: None,
        weeks: Some(3),
        locale: Locale::De,
        sports: vec![],
        strength_per_week: 1,
    };
    let generated = planning::generate(
        &store,
        &req,
        "00000000-0000-4000-8000-000000000001".into(),
        now(),
    )
    .unwrap();
    let path = dir.join("plan.json");
    fs::write(&path, planning::plan_json(&generated.plan).unwrap()).unwrap();
    store
        .save_plan_for_goal(&generated.plan, PlanStatus::Draft, generated.goal_id)
        .unwrap();
    (db, path)
}
