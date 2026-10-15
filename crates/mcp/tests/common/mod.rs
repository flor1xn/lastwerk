//! Gemeinsame Hilfen der Integrationstests. Alle Daten sind synthetisch, alle Dateien liegen
//! unter `target/`.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use lastwerk_core::plan::{
    DayAvailability, GoalKind, GoalSpec, Locale, Plan, PlanContext, PlanSource, PlanSport,
    Priority, Rules, SportWeight, TemplateCatalog, generate_plan,
};
use lastwerk_core::{PmcParams, Sport};
use lastwerk_mcp::LastwerkMcp;
use lastwerk_mcp::calllog::CallLog;
use lastwerk_mcp::server::Clock;
use lastwerk_store::{
    ActivityRow, ImportOptions, LoadRow, LoadStatus, NewGoal, NewThreshold, PlanStatus,
    SourceFileRow, SourceStatus, Store, ThresholdKind, ThresholdOrigin,
};
use lastwerk_testkit::fixtures;

const TEMPLATES_JSON: &str = include_str!("../../../../catalog/templates.json");
const RULES_JSON: &str = include_str!("../../../../catalog/rules.default.json");

// Markerwerte (SPEC 11.4): Sie stehen in Feldern, die der Server nie ausgeben darf. Kein
// Marker darf in einer Antwort oder im Aufrufprotokoll vorkommen.
pub const M_LABEL: &str = "MARKER-GOAL-LABEL-7f3a";
pub const M_NOTE: &str = "MARKER-THRESHOLD-NOTE-91c2";
pub const M_DIR: &str = "MARKER-DIR-5d1e";
pub const M_FILE: &str = "MARKER-FILE-2b8c";
pub const M_SERIAL: u32 = 4_242_424_242;
pub const M_PRODUCT: &str = "MARKER-PRODUCT-c4d0";
pub const M_SUB_SPORT: &str = "MARKER-SUBSPORT-83aa";
pub const M_SETTING_VALUE: &str = "MARKER-SETTING-VALUE-e60b";
pub const M_SETTING_KEY: &str = "MARKER-SETTING-KEY-1d2f";
pub const M_EXPLANATION: &str = "MARKER-EXPLANATION-a9f4";
pub const M_RECOVERY_SOURCE: &str = "MARKER-RECOVERY-SOURCE-0e77";
/// Koordinaten, wie sie in einem Namen oder einer Notiz stehen könnten.
pub const M_LAT: &str = "52.520008";
pub const M_LON: &str = "13.404954";

/// Alle Marker als Texte, die in keiner Ausgabe vorkommen dürfen.
pub fn markers() -> Vec<String> {
    vec![
        M_LABEL.to_string(),
        M_NOTE.to_string(),
        M_DIR.to_string(),
        M_FILE.to_string(),
        M_SERIAL.to_string(),
        M_PRODUCT.to_string(),
        M_SUB_SPORT.to_string(),
        M_SETTING_VALUE.to_string(),
        M_SETTING_KEY.to_string(),
        M_EXPLANATION.to_string(),
        M_RECOVERY_SOURCE.to_string(),
        M_LAT.to_string(),
        M_LON.to_string(),
        "MARKER".to_string(),
        ".fit".to_string(),
    ]
}

pub fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

/// Leerer Arbeitsordner unter `target/tmp` für einen Test.
pub fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("mcp-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap()
}

/// Uhr mit festem Datum "heute" (Montag, 25. Mai 2026, 09:30 UTC).
pub fn fixed_clock() -> Clock {
    Arc::new(|| {
        (
            Utc.with_ymd_and_hms(2026, 5, 25, 9, 30, 0).unwrap(),
            date(2026, 5, 25),
        )
    })
}

pub fn threshold(
    sport: Option<Sport>,
    kind: ThresholdKind,
    value: f64,
    from: NaiveDate,
    note: Option<&str>,
) -> NewThreshold {
    NewThreshold {
        sport,
        kind,
        value,
        valid_from: from,
        origin: ThresholdOrigin::Manual,
        note: note.map(str::to_string),
    }
}

/// Plan für vier Wochen ab dem 4. Januar 2027 (Rad, Ziel am 1. Mai 2027).
pub fn sample_plan(plan_id: &str, source: PlanSource, explanation: Option<&str>) -> Plan {
    let rules: Rules = serde_json::from_str(RULES_JSON).unwrap();
    let catalog: TemplateCatalog = serde_json::from_str(TEMPLATES_JSON).unwrap();
    let day = DayAvailability {
        max_minutes: 90,
        allow_hard: true,
        fixed_rest: false,
    };
    let ctx = PlanContext {
        plan_id: plan_id.to_string(),
        // 08:30 Uhr: die Uhrzeit darf nicht in der Ausgabe stehen (O20).
        created_at: Utc.with_ymd_and_hms(2026, 10, 13, 8, 30, 15).unwrap(),
        locale: Locale::De,
        week_start: date(2027, 1, 4),
        goal: GoalSpec::Event {
            event_date: date(2027, 5, 1),
            sport: PlanSport::Cycling,
            priority: Priority::A,
        },
        horizon_weeks: 4,
        ctl0: 30.0,
        atl0: 35.0,
        history: Vec::new(),
        recovery: Vec::new(),
        availability: [day; 7],
        sports: vec![SportWeight {
            sport: PlanSport::Cycling,
            weight: 1.0,
        }],
        strength_per_week: 1,
        ctl_rise_per_week: None,
        pmc_params: PmcParams::default(),
        catalog,
    };
    let mut plan = generate_plan(&ctx, &rules).unwrap();
    plan.source = source;
    plan.explanation = explanation.map(str::to_string);
    plan
}

pub const PLAN_ID: &str = "00000000-0000-4000-8000-00000000000a";

/// Schreibt die Fixtures unter einem Ordner und einer Datei mit Markernamen (so entsteht ein
/// `source_file.rel_path` mit Marker).
fn write_marker_fixtures(dir: &Path) -> PathBuf {
    let out = dir.join("fit");
    let sub = out.join(M_DIR);
    fs::create_dir_all(&sub).unwrap();
    for f in fixtures::all() {
        fs::write(sub.join(format!("{M_FILE}-{}", f.file_name())), &f.bytes).unwrap();
    }
    out
}

/// Synthetische Datenbank mit Markerwerten in allen Feldern, die nicht ausgegeben werden
/// dürfen: Ziel, Notizen, Dateipfade, Gerätenummern, Produktnamen, Unterart, Einstellungen
/// und die Erklärung des Plans. Dazu echte Zahlen (Fixtures, Handaktivitäten, Plan).
pub fn build_marker_db(dir: &Path) -> PathBuf {
    let path = dir.join("lastwerk.db");
    let mut store = Store::open(&path).unwrap();
    let from = date(2026, 1, 1);
    let note = format!("{M_NOTE} {M_LAT},{M_LON}");
    for t in [
        threshold(
            Some(Sport::Cycling),
            ThresholdKind::FtpW,
            250.0,
            from,
            Some(&note),
        ),
        threshold(
            Some(Sport::Cycling),
            ThresholdKind::LthrBpm,
            160.0,
            from,
            Some(&note),
        ),
        threshold(
            Some(Sport::Running),
            ThresholdKind::RunSpeedMps,
            3.5,
            from,
            None,
        ),
        threshold(
            Some(Sport::Swimming),
            ThresholdKind::CssMps,
            1.2,
            from,
            None,
        ),
        threshold(None, ThresholdKind::HrRestBpm, 50.0, from, Some(&note)),
        threshold(None, ThresholdKind::HrMaxBpm, 190.0, from, None),
        // Ein späterer FTP-Wert: zum 1. Mai 2026 gilt er.
        threshold(
            Some(Sport::Cycling),
            ThresholdKind::FtpW,
            260.0,
            date(2026, 5, 1),
            None,
        ),
    ] {
        store.insert_threshold(&t).unwrap();
    }
    store.set_setting("athlete.sex", "male").unwrap();
    store.set_setting("athlete.birth_year", "1990").unwrap();
    store.set_setting("app.locale", "de").unwrap();
    store
        .set_setting("app.fit_dir", &format!("/home/{M_SETTING_VALUE}/fit"))
        .unwrap();
    store.set_setting(M_SETTING_KEY, M_SETTING_VALUE).unwrap();

    let fit_dir = write_marker_fixtures(dir);
    store
        .import_dir(&fit_dir, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();

    // Handaktivitäten: 14 Tage ab dem 11. Mai 2026, mit Markern in allen Textfeldern.
    for n in 0..14u32 {
        let day = date(2026, 5, 11) + chrono::TimeDelta::days(i64::from(n));
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

    store
        .set_goal(&NewGoal {
            kind: GoalKind::Event,
            sport: Some(PlanSport::Cycling),
            event_date: Some(date(2027, 5, 1)),
            label: Some(format!("{M_LABEL} {M_LAT},{M_LON}")),
            priority: Some(Priority::A),
        })
        .unwrap();
    store.set_availability(0, 60, true, false).unwrap();
    let plan = sample_plan(PLAN_ID, PlanSource::Llm, Some(M_EXPLANATION));
    store.save_plan(&plan, PlanStatus::Active).unwrap();
    path
}

/// Server über einer schreibgeschützten Verbindung mit dem festen Datum.
pub fn server_for(db: &Path, log: &Path) -> LastwerkMcp {
    let store = Store::open_read_only(db).unwrap();
    LastwerkMcp::with_clock(store, CallLog::open(log).unwrap(), fixed_clock())
}
