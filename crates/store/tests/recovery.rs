//! Erholungstage im Speicher (SPEC 6.2, 6.3, 7.3, Task T14): Upsert, Lesen, Wertebereiche,
//! Backup und Wiederaufbau. Alle Werte sind erfunden.

mod common;

use chrono::{DateTime, Utc};
use common::{date, import, now, work_dir, write_fixtures};
use lastwerk_core::plan::RecoveryDay;
use lastwerk_store::{
    ImportOptions, RecoveryDayRecord, Store, StoreError, UpsertOutcome, UserDataExport,
};

fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).unwrap()
}

fn record(day: u32) -> RecoveryDayRecord {
    RecoveryDayRecord {
        date: date(2026, 10, day),
        hrv_overnight_ms: Some(61.5),
        resting_hr_bpm: Some(48.0),
        sleep_duration_s: Some(27_000),
        sleep_score: Some(82),
        body_battery_high: Some(95),
        body_battery_low: Some(12),
        source: "garmin-bridge 0.1.0".to_string(),
        imported_at: now(),
    }
}

#[test]
fn upsert_inserts_updates_and_leaves_identical_rows_alone() {
    let mut store = Store::open_in_memory().unwrap();
    let first = record(10);
    assert_eq!(
        store.upsert_recovery_day(&first).unwrap(),
        UpsertOutcome::Inserted
    );

    // Gleiche Werte, späterer Importzeitpunkt: nichts ändert sich, auch imported_at nicht.
    let later = RecoveryDayRecord {
        imported_at: at(1_800_000_000),
        ..first.clone()
    };
    assert_eq!(
        store.upsert_recovery_day(&later).unwrap(),
        UpsertOutcome::Unchanged
    );
    let rows = store
        .recovery_days(date(2026, 10, 1), date(2026, 10, 31))
        .unwrap();
    assert_eq!(rows, vec![first.clone()]);

    // Garmin finalisiert nachträglich: andere Werte ersetzen die Zeile samt Zeitpunkt.
    let revised = RecoveryDayRecord {
        hrv_overnight_ms: Some(58.0),
        sleep_score: None,
        imported_at: at(1_800_000_000),
        ..first.clone()
    };
    assert_eq!(
        store.upsert_recovery_day(&revised).unwrap(),
        UpsertOutcome::Updated
    );
    let rows = store
        .recovery_days(date(2026, 10, 10), date(2026, 10, 10))
        .unwrap();
    assert_eq!(rows, vec![revised]);
}

#[test]
fn a_batch_writes_in_one_transaction_and_refuses_the_whole_batch_on_one_bad_row() {
    let mut store = Store::open_in_memory().unwrap();
    let outcomes = store
        .upsert_recovery_days(&[record(10), record(11), record(10)])
        .unwrap();
    // Gleiches Datum zweimal: die erste Zeile legt an, die zweite ist gleich und bleibt stehen.
    assert_eq!(
        outcomes,
        [
            UpsertOutcome::Inserted,
            UpsertOutcome::Inserted,
            UpsertOutcome::Unchanged
        ]
    );
    let all = |s: &Store| {
        s.recovery_days(date(2026, 1, 1), date(2026, 12, 31))
            .unwrap()
    };
    assert_eq!(all(&store).len(), 2);

    let bad = RecoveryDayRecord {
        sleep_score: Some(250),
        ..record(12)
    };
    assert!(matches!(
        store.upsert_recovery_days(&[record(13), bad]),
        Err(StoreError::InvalidInput(_))
    ));
    assert_eq!(all(&store).len(), 2, "nichts aus dem abgelehnten Stapel");
    assert!(store.upsert_recovery_days(&[]).unwrap().is_empty());
}

#[test]
fn recovery_days_are_returned_in_date_order_within_an_inclusive_range() {
    let mut store = Store::open_in_memory().unwrap();
    for day in [12, 10, 15, 11, 20] {
        store.upsert_recovery_day(&record(day)).unwrap();
    }
    let days: Vec<_> = store
        .recovery_days(date(2026, 10, 10), date(2026, 10, 15))
        .unwrap()
        .into_iter()
        .map(|r| r.date)
        .collect();
    assert_eq!(
        days,
        [10, 11, 12, 15].map(|d| date(2026, 10, d)),
        "von und bis gelten einschließlich"
    );
    assert!(
        store
            .recovery_days(date(2026, 11, 1), date(2026, 11, 30))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn missing_values_stay_missing() {
    let mut store = Store::open_in_memory().unwrap();
    let sparse = RecoveryDayRecord {
        hrv_overnight_ms: None,
        resting_hr_bpm: None,
        sleep_duration_s: Some(0),
        sleep_score: None,
        ..record(10)
    };
    store.upsert_recovery_day(&sparse).unwrap();
    let rows = store
        .recovery_days(date(2026, 10, 10), date(2026, 10, 10))
        .unwrap();
    assert_eq!(rows, vec![sparse]);
}

#[test]
fn values_outside_the_schema_ranges_are_refused_and_nothing_is_written() {
    let mut store = Store::open_in_memory().unwrap();
    let cases = [
        RecoveryDayRecord {
            hrv_overnight_ms: Some(400.1),
            ..record(10)
        },
        RecoveryDayRecord {
            hrv_overnight_ms: Some(-1.0),
            ..record(10)
        },
        RecoveryDayRecord {
            resting_hr_bpm: Some(19.9),
            ..record(10)
        },
        RecoveryDayRecord {
            resting_hr_bpm: Some(120.5),
            ..record(10)
        },
        RecoveryDayRecord {
            hrv_overnight_ms: Some(f64::NAN),
            ..record(10)
        },
        RecoveryDayRecord {
            sleep_duration_s: Some(86_401),
            ..record(10)
        },
        RecoveryDayRecord {
            sleep_score: Some(101),
            ..record(10)
        },
        RecoveryDayRecord {
            body_battery_high: Some(101),
            ..record(10)
        },
        RecoveryDayRecord {
            body_battery_low: Some(101),
            ..record(10)
        },
        RecoveryDayRecord {
            source: "x".repeat(65),
            ..record(10)
        },
    ];
    for bad in cases {
        assert!(
            matches!(
                store.upsert_recovery_day(&bad),
                Err(StoreError::InvalidInput(_))
            ),
            "{bad:?}"
        );
    }
    assert!(
        store
            .recovery_days(date(2026, 1, 1), date(2026, 12, 31))
            .unwrap()
            .is_empty()
    );
    // Genau auf den Grenzen ist erlaubt.
    let edge = RecoveryDayRecord {
        hrv_overnight_ms: Some(400.0),
        resting_hr_bpm: Some(20.0),
        sleep_duration_s: Some(86_400),
        sleep_score: Some(100),
        body_battery_high: Some(100),
        body_battery_low: Some(0),
        source: "x".repeat(64),
        ..record(10)
    };
    store.upsert_recovery_day(&edge).unwrap();
}

#[test]
fn a_row_converts_to_the_planner_input() {
    let r = record(10);
    let day = RecoveryDay::from(&r);
    assert_eq!(day.date, date(2026, 10, 10));
    assert_eq!(day.hrv_overnight_ms, Some(61.5));
    assert_eq!(day.resting_hr_bpm, Some(48.0));
    assert_eq!(day.sleep_duration_s, Some(27_000));
    assert_eq!(day.sleep_score, Some(82));
}

#[test]
fn recovery_days_survive_rebuild_and_reopening() {
    let dir = work_dir("recovery_rebuild");
    let fix = write_fixtures(&dir);
    let db = dir.join("lastwerk.db");
    let mut store = Store::open(&db).unwrap();
    store.upsert_recovery_day(&record(10)).unwrap();
    store.upsert_recovery_day(&record(11)).unwrap();
    import(&mut store, &fix);
    store
        .rebuild_cache(&fix, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();
    // Auch ein Wiederaufbau aus einem leeren Ordner löscht nur den Cache.
    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    store
        .rebuild_cache(&empty, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();
    drop(store);
    let reopened = Store::open(&db).unwrap();
    assert_eq!(
        reopened
            .recovery_days(date(2026, 10, 1), date(2026, 10, 31))
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn backup_roundtrip_carries_recovery_days() {
    let mut a = Store::open_in_memory().unwrap();
    a.upsert_recovery_day(&record(11)).unwrap();
    a.upsert_recovery_day(&RecoveryDayRecord {
        hrv_overnight_ms: None,
        sleep_score: None,
        ..record(10)
    })
    .unwrap();
    let export = a.export_user_data().unwrap();
    // Sortiert nach Datum, Werte als Text und Zahl wie in der Tabelle.
    assert_eq!(export.recovery_days.len(), 2);
    assert_eq!(export.recovery_days[0].local_date, "2026-10-10");
    assert_eq!(export.recovery_days[0].hrv_overnight_ms, None);
    assert_eq!(export.recovery_days[1].sleep_duration_s, Some(27_000));
    assert_eq!(export.recovery_days[1].imported_at, "2026-09-21T14:13:20Z");

    let text = export.to_json().unwrap();
    let mut b = Store::open_in_memory().unwrap();
    b.import_user_data(&UserDataExport::from_json(&text).unwrap())
        .unwrap();
    assert_eq!(b.export_user_data().unwrap(), export);
    assert_eq!(
        b.recovery_days(date(2026, 10, 1), date(2026, 10, 31))
            .unwrap(),
        a.recovery_days(date(2026, 10, 1), date(2026, 10, 31))
            .unwrap()
    );
}

#[test]
fn a_backup_without_recovery_days_empties_the_table() {
    // Sicherungen vor T14 kennen den Schlüssel nicht. Einspielen ersetzt die Nutzerdaten
    // vollständig, die Tabelle ist danach leer und lässt sich mit import-recovery füllen.
    let mut store = Store::open_in_memory().unwrap();
    store.upsert_recovery_day(&record(10)).unwrap();
    let old = UserDataExport::from_json(
        r#"{"backup_version": 1, "thresholds": [], "goals": [], "availability": [],
            "plans": [], "manual_sessions": [], "settings": []}"#,
    )
    .unwrap();
    assert!(old.recovery_days.is_empty());
    store.import_user_data(&old).unwrap();
    assert!(
        store
            .recovery_days(date(2026, 1, 1), date(2026, 12, 31))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn an_unreadable_recovery_row_in_a_backup_keeps_the_old_state() {
    let mut store = Store::open_in_memory().unwrap();
    store.upsert_recovery_day(&record(10)).unwrap();
    let before = store.export_user_data().unwrap();
    let mut bad = before.clone();
    bad.recovery_days[0].local_date = "kein Datum".to_string();
    assert!(matches!(
        store.import_user_data(&bad),
        Err(StoreError::InvalidInput(_))
    ));
    assert_eq!(store.export_user_data().unwrap(), before);
}

#[test]
fn recovery_values_outside_the_schema_ranges_in_a_backup_keep_the_old_state() {
    // Integrationsprüfung vom 14. Oktober 2026: Eine Sicherung mit -60 s Schlaf, Schlafwert 300
    // und HRV 9999 ms kam bisher durch und erschien danach über MCP mit umgebrochenen Werten
    // (-60 als u32 ergab 4294967236 s). Jetzt gilt dieselbe Prüfung wie beim Import.
    let mut store = Store::open_in_memory().unwrap();
    store.upsert_recovery_day(&record(10)).unwrap();
    let before = store.export_user_data().unwrap();
    let cases: [fn(&mut lastwerk_store::RecoveryDayExport); 6] = [
        |d| d.body_battery_high = Some(101),
        |d| d.body_battery_low = Some(-1),
        |d| d.sleep_duration_s = Some(-60),
        |d| d.sleep_score = Some(300),
        |d| d.hrv_overnight_ms = Some(9999.0),
        |d| d.sleep_duration_s = Some(86_401),
    ];
    for change in cases {
        let mut bad = before.clone();
        change(&mut bad.recovery_days[0]);
        assert!(
            matches!(
                store.import_user_data(&bad),
                Err(StoreError::InvalidInput(_))
            ),
            "{:?}",
            bad.recovery_days[0]
        );
        assert_eq!(store.export_user_data().unwrap(), before);
    }
}

#[test]
fn body_battery_survives_upsert_and_backup_and_can_be_absent() {
    let mut a = Store::open_in_memory().unwrap();
    a.upsert_recovery_day(&record(10)).unwrap();
    a.upsert_recovery_day(&RecoveryDayRecord {
        body_battery_high: None,
        body_battery_low: None,
        ..record(11)
    })
    .unwrap();
    let export = a.export_user_data().unwrap();
    assert_eq!(export.recovery_days[0].body_battery_high, Some(95));
    assert_eq!(export.recovery_days[0].body_battery_low, Some(12));
    assert_eq!(export.recovery_days[1].body_battery_high, None);

    let mut b = Store::open_in_memory().unwrap();
    b.import_user_data(&UserDataExport::from_json(&export.to_json().unwrap()).unwrap())
        .unwrap();
    assert_eq!(b.export_user_data().unwrap(), export);
    let rows = b
        .recovery_days(date(2026, 10, 10), date(2026, 10, 11))
        .unwrap();
    assert_eq!(rows[0].body_battery_high, Some(95));
    assert_eq!(rows[0].body_battery_low, Some(12));
    assert_eq!(rows[1].body_battery_low, None);
}

#[test]
fn a_backup_from_before_t22_without_body_battery_still_restores() {
    // Sicherung im Format von T14: Erholungszeilen ohne die Schlüssel body_battery_*.
    // `backup_version` bleibt 1, die Felder sind optional (T22).
    let old = UserDataExport::from_json(
        r#"{"backup_version": 1, "thresholds": [], "goals": [], "availability": [],
            "plans": [], "manual_sessions": [],
            "recovery_days": [{"local_date": "2026-10-10", "hrv_overnight_ms": 61.5,
              "resting_hr_bpm": 48.0, "sleep_duration_s": 27000, "sleep_score": 82,
              "source": "garmin-bridge 0.1.0", "imported_at": "2026-10-11T00:00:00Z"}],
            "settings": []}"#,
    )
    .unwrap();
    let mut store = Store::open_in_memory().unwrap();
    store.import_user_data(&old).unwrap();
    let rows = store
        .recovery_days(date(2026, 10, 10), date(2026, 10, 10))
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].hrv_overnight_ms, Some(61.5));
    assert_eq!(rows[0].sleep_score, Some(82));
    assert_eq!(rows[0].body_battery_high, None);
    assert_eq!(rows[0].body_battery_low, None);
}
