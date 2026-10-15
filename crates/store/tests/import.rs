//! Import, Wiederaufbau und Nutzerdaten gegen die synthetischen Fixtures (SPEC 11.3, 12 M1).

mod common;

use std::fs;

use common::{add_thresholds, date, import, now, work_dir, write_fixtures};
use lastwerk_core::Sport;
use lastwerk_store::{
    ImportOptions, LoadRow, LoadStatus, SourceStatus, Store, StoreError, ThresholdKind,
    UserDataExport, sport_from_str, sport_to_str,
};
use lastwerk_testkit::fixtures;

/// Vergleichbare Sicht auf die Cache-Tabellen: ohne Zeitstempel des Imports und der
/// Berechnung und ohne die Änderungszeit der Datei, die vom Dateisystem kommt.
type Snapshot = (
    Vec<(i64, lastwerk_store::SourceFileRow)>,
    Vec<lastwerk_store::ActivityRecord>,
    Vec<LoadRow>,
    Vec<lastwerk_store::ZoneTimeRow>,
    Vec<lastwerk_store::BestPowerRow>,
);

fn snapshot(store: &Store) -> Snapshot {
    let epoch = chrono::DateTime::from_timestamp(0, 0).unwrap();
    let files = store
        .source_files()
        .unwrap()
        .into_iter()
        .map(|mut f| {
            f.row.imported_at = epoch;
            f.row.mtime_utc = epoch;
            (f.id, f.row)
        })
        .collect();
    let loads = store
        .activity_loads()
        .unwrap()
        .into_iter()
        .map(|mut l| {
            l.computed_at = epoch;
            l
        })
        .collect();
    (
        files,
        store.activities().unwrap(),
        loads,
        store.zone_times().unwrap(),
        store.best_powers().unwrap(),
    )
}

#[test]
fn imports_every_fixture_as_expected() {
    let dir = work_dir("import_all");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    let report = import(&mut store, &fix);

    let all = fixtures::all();
    let readable = all.iter().filter(|f| f.expect.activities == 1).count();
    assert_eq!(report.files_found, all.len());
    assert_eq!(report.new, readable);
    assert_eq!(report.updated, 0);
    assert_eq!(report.unchanged, 0);
    assert_eq!(report.not_activity, 1, "Monitoring");
    // Beschädigte und leere Datei erscheinen als Fehler, der Import lief zu Ende.
    let mut codes: Vec<_> = report
        .errors
        .iter()
        .map(|e| (e.rel_path.as_str(), e.code.as_str()))
        .collect();
    codes.sort_unstable();
    assert_eq!(
        codes,
        [
            ("corrupt_crc.fit", "crc_mismatch"),
            ("corrupt_truncated.fit", "truncated"),
            ("empty.fit", "empty_file"),
        ]
    );

    // Je lesbare Fixture eine Aktivität, mit Sportart, Dauer und Kanälen der Erwartung.
    let activities = store.activities().unwrap();
    let files = store.source_files().unwrap();
    assert_eq!(activities.len(), readable);
    for f in all.iter().filter(|f| f.expect.activities == 1) {
        let path = f.file_name();
        let src = files.iter().find(|s| s.row.rel_path == path).unwrap();
        assert_eq!(src.row.status, SourceStatus::Ok, "{}", f.name);
        let a = activities
            .iter()
            .find(|a| a.row.source_file_id == src.id)
            .unwrap_or_else(|| panic!("{}: keine Aktivität", f.name));
        if let Some(sport) = f.expect.sport {
            assert_eq!(
                sport_to_str(a.row.sport),
                format!("{sport:?}").to_lowercase(),
                "{}",
                f.name
            );
        }
        assert_eq!(
            a.row.duration_s,
            i64::from(f.expect.duration_s.unwrap()),
            "{}",
            f.name
        );
        assert_eq!(a.row.has_power, f.expect.has_power, "{}", f.name);
        assert_eq!(a.row.has_hr, f.expect.has_heart_rate, "{}", f.name);
        assert_eq!(
            a.row.start_utc.timestamp(),
            f.expect.start_unix_s.unwrap(),
            "{}",
            f.name
        );
        assert_eq!(
            Some(a.row.local_offset_s),
            f.expect.local_offset_s,
            "{}",
            f.name
        );
    }
    // Fehlerhafte Dateien tragen den Fehlercode, Monitoring den Status not_activity.
    let status_of = |name: &str| {
        let s = files.iter().find(|s| s.row.rel_path == name).unwrap();
        (s.row.status, s.row.error_code.clone())
    };
    assert_eq!(
        status_of("corrupt_crc.fit"),
        (SourceStatus::Error, Some("crc_mismatch".into()))
    );
    assert_eq!(
        status_of("monitoring.fit"),
        (SourceStatus::NotActivity, Some("no_activity".into()))
    );
    assert_eq!(
        status_of("empty.fit"),
        (SourceStatus::Error, Some("empty_file".into()))
    );

    // Das Edge/Uhr-Paar ergibt genau eine primäre Aktivität, die mit Leistung.
    assert_eq!(report.duplicates, 1);
    assert_eq!(report.primary, readable - 1);
    let pair: Vec<_> = activities
        .iter()
        .filter(|a| {
            let src = files.iter().find(|s| s.id == a.row.source_file_id).unwrap();
            src.row.rel_path.starts_with("pair_")
        })
        .collect();
    assert_eq!(pair.len(), 2);
    assert_eq!(pair.iter().filter(|a| a.is_primary).count(), 1);
    let primary = pair.iter().find(|a| a.is_primary).unwrap();
    let dup = pair.iter().find(|a| !a.is_primary).unwrap();
    assert!(primary.row.has_power && !dup.row.has_power);
    assert_eq!(dup.duplicate_of, Some(primary.id));
    // Alle anderen Fixtures bleiben primär: die Startzeiten der Fixtures sind gestaffelt.
    assert_eq!(
        activities.iter().filter(|a| a.is_primary).count(),
        report.primary
    );
}

#[test]
fn without_thresholds_every_load_is_an_error_row_never_zero() {
    let dir = work_dir("no_thresholds");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    let report = import(&mut store, &fix);
    assert_eq!(report.loads_ok, 0);
    assert_eq!(
        report.loads_missing_threshold + report.loads_missing_channel + report.loads_implausible,
        report.new
    );
    assert!(report.loads_missing_threshold > 0);
    let loads = store.activity_loads().unwrap();
    assert_eq!(loads.len(), report.new);
    for l in &loads {
        assert_ne!(l.status, LoadStatus::Ok);
        assert_eq!(l.value, None, "keine Null als Last");
        assert_eq!(l.threshold_id, None);
    }
    // Krafttraining hat keinen Kanal für eine Last (RPE fehlt), kein fehlender Schwellenwert.
    let strength = store
        .activities()
        .unwrap()
        .into_iter()
        .find(|a| a.row.sport == Sport::Strength)
        .unwrap();
    let l = loads.iter().find(|l| l.activity_id == strength.id).unwrap();
    assert_eq!(l.status, LoadStatus::MissingChannel);
    // Ohne berechenbare Last gibt es keine Tageslast.
    assert!(
        store
            .daily_loads(date(2026, 1, 1), date(2026, 12, 31))
            .unwrap()
            .is_empty()
    );
}

/// Handrechnung: `power_constant_200w_3600s` fährt 3600 s mit 200 W. NP = 200 W (konstante
/// Leistung), FTP 250 W, IF = 200 / 250 = 0,8, TSS = 1 h * 0,8^2 * 100 = 64.
#[test]
fn load_uses_the_threshold_valid_on_the_day() {
    let dir = work_dir("loads_ok");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    let report = import(&mut store, &fix);
    assert!(report.loads_ok >= 12, "{report:?}");

    let files = store.source_files().unwrap();
    let activities = store.activities().unwrap();
    let id_of = |name: &str| {
        let src = files.iter().find(|s| s.row.rel_path == name).unwrap().id;
        activities
            .iter()
            .find(|a| a.row.source_file_id == src)
            .unwrap()
            .id
    };
    let loads = store.activity_loads().unwrap();
    let l = loads
        .iter()
        .find(|l| l.activity_id == id_of("power_constant_200w_3600s.fit"))
        .unwrap();
    assert_eq!(l.method, "tss");
    assert_eq!(l.status, LoadStatus::Ok);
    assert!((l.value.unwrap() - 64.0).abs() < 0.01, "{:?}", l.value);
    assert!((l.intensity_factor.unwrap() - 0.8).abs() < 1e-4);
    // Die Zeile nennt die FTP-Schwelle, aus der sie stammt.
    let ftp = store
        .threshold_lookup(Sport::Cycling, date(2026, 5, 11))
        .unwrap()
        .unwrap()
        .ids[&ThresholdKind::FtpW];
    assert_eq!(l.threshold_id, Some(ftp));

    // Tageslast: nur die primäre Fahrt des Paars (Edge, 64) zählt, nicht die Uhr (hrTSS).
    // Das Paar liegt am 17. Mai (Tag 7 nach dem 10. Mai).
    let daily = store
        .daily_loads(date(2026, 5, 17), date(2026, 5, 17))
        .unwrap();
    assert_eq!(daily.len(), 1);
    assert!((daily[0].load - 64.0).abs() < 0.01, "{daily:?}");
    // Nicht im Bereich liegende Tage fehlen.
    assert!(
        store
            .daily_loads(date(2025, 1, 1), date(2025, 12, 31))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn daily_loads_skip_non_primary_and_failed_loads() {
    let dir = work_dir("daily_primary");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    import(&mut store, &fix);
    let all = store
        .daily_loads(date(2026, 1, 1), date(2026, 12, 31))
        .unwrap();
    let activities = store.activities().unwrap();
    let loads = store.activity_loads().unwrap();
    let expected: f64 = activities
        .iter()
        .filter(|a| a.is_primary)
        .filter_map(|a| {
            loads
                .iter()
                .find(|l| l.activity_id == a.id && l.status == LoadStatus::Ok)
        })
        .filter_map(|l| l.value)
        .sum();
    let got: f64 = all.iter().map(|d| f64::from(d.load)).sum();
    assert!((got - expected).abs() < 0.01, "{got} gegen {expected}");
    // Das Duplikat hat eine eigene berechenbare Last, sie zählt nicht.
    let dup = activities.iter().find(|a| !a.is_primary).unwrap();
    assert!(
        loads
            .iter()
            .any(|l| l.activity_id == dup.id && l.status == LoadStatus::Ok)
    );
    // Tage sind aufsteigend sortiert.
    assert!(all.windows(2).all(|w| w[0].date < w[1].date));
}

#[test]
fn thresholds_for_picks_the_latest_value_valid_on_the_date() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(
        store
            .thresholds_for(Sport::Cycling, date(2026, 6, 1))
            .unwrap()
            .is_none()
    );
    let cyc = Some(Sport::Cycling);
    let ids: Vec<i64> = [
        common::threshold(cyc, ThresholdKind::FtpW, 200.0, date(2025, 1, 1)),
        common::threshold(cyc, ThresholdKind::FtpW, 250.0, date(2026, 1, 1)),
        common::threshold(cyc, ThresholdKind::FtpW, 260.0, date(2026, 9, 1)),
        common::threshold(
            Some(Sport::Running),
            ThresholdKind::FtpW,
            999.0,
            date(2025, 1, 1),
        ),
        common::threshold(None, ThresholdKind::HrRestBpm, 50.0, date(2025, 6, 1)),
        common::threshold(None, ThresholdKind::LthrBpm, 150.0, date(2025, 1, 1)),
        common::threshold(cyc, ThresholdKind::LthrBpm, 160.0, date(2025, 1, 1)),
    ]
    .iter()
    .map(|t| store.insert_threshold(t).unwrap())
    .collect();

    let on = |store: &Store, y, m, d| store.thresholds_for(Sport::Cycling, date(y, m, d)).unwrap();
    // Vor dem ersten Eintrag gilt nichts.
    assert!(on(&store, 2024, 12, 31).is_none());
    // Am Gültigkeitstag selbst gilt der neue Wert, am Vortag noch der alte.
    assert_eq!(on(&store, 2025, 12, 31).unwrap().ftp_w, Some(200.0));
    assert_eq!(on(&store, 2026, 1, 1).unwrap().ftp_w, Some(250.0));
    assert_eq!(on(&store, 2026, 8, 31).unwrap().ftp_w, Some(250.0));
    assert_eq!(on(&store, 2026, 9, 1).unwrap().ftp_w, Some(260.0));
    // valid_from ist das jüngste Datum der verwendeten Werte.
    assert_eq!(on(&store, 2025, 7, 1).unwrap().valid_from, date(2025, 6, 1));
    // Der Ruhepuls für alle Sportarten gilt erst ab seinem Datum.
    assert_eq!(on(&store, 2025, 5, 31).unwrap().hr_rest_bpm, None);
    assert_eq!(on(&store, 2025, 6, 1).unwrap().hr_rest_bpm, Some(50.0));
    // Bei gleichem Datum gewinnt der Wert der Sportart vor dem für alle.
    assert_eq!(on(&store, 2025, 7, 1).unwrap().lthr_bpm, Some(160.0));
    let run = store
        .thresholds_for(Sport::Running, date(2025, 7, 1))
        .unwrap()
        .unwrap();
    assert_eq!(run.lthr_bpm, Some(150.0));
    assert_eq!(run.ftp_w, Some(999.0));
    // Die Zeilen-IDs zeigen, woher ein Wert stammt.
    let l = store
        .threshold_lookup(Sport::Cycling, date(2026, 1, 1))
        .unwrap()
        .unwrap();
    assert_eq!(l.ids[&ThresholdKind::FtpW], ids[1]);
    assert_eq!(l.ids[&ThresholdKind::LthrBpm], ids[6]);
    // Das Geschlecht kommt aus der Einstellung und fehlt ohne sie.
    assert_eq!(on(&store, 2026, 1, 1).unwrap().sex, None);
    store.set_setting("athlete.sex", "female").unwrap();
    assert_eq!(
        on(&store, 2026, 1, 1).unwrap().sex,
        Some(lastwerk_core::Sex::Female)
    );
    store.set_setting("athlete.sex", "unbekannt").unwrap();
    assert_eq!(on(&store, 2026, 1, 1).unwrap().sex, None);
}

#[test]
fn second_import_of_the_same_files_is_all_unchanged() {
    let dir = work_dir("second_import");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    let first = import(&mut store, &fix);
    let before = snapshot(&store);
    let second = import(&mut store, &fix);
    assert_eq!(second.unchanged, first.files_found);
    assert_eq!(second.new + second.updated + second.not_activity, 0);
    assert!(second.errors.is_empty());
    assert_eq!(
        second.loads_ok + second.loads_missing_threshold,
        0,
        "nichts neu berechnet"
    );
    assert_eq!(second.duplicates, first.duplicates);
    assert_eq!(before, snapshot(&store));
}

#[test]
fn import_into_fresh_db_and_rebuild_give_identical_figures() {
    let dir = work_dir("rebuild_equal");
    let fix = write_fixtures(&dir);

    let mut fresh = Store::open_in_memory().unwrap();
    add_thresholds(&mut fresh);
    let fresh_report = import(&mut fresh, &fix);

    // Die zweite Datenbank enthält erst etwas anderes (nur ein Teil, dazu eine Datei, die es
    // im Ordner nicht gibt) und wird dann neu aufgebaut.
    let partial = dir.join("partial");
    fs::create_dir_all(&partial).unwrap();
    fs::copy(fix.join("no_power.fit"), partial.join("only_here.fit")).unwrap();
    fs::copy(fix.join("pair_edge.fit"), partial.join("pair_edge.fit")).unwrap();
    let mut rebuilt = Store::open_in_memory().unwrap();
    add_thresholds(&mut rebuilt);
    import(&mut rebuilt, &partial);
    let mut steps = Vec::new();
    let rebuilt_report = rebuilt
        .rebuild_cache(&fix, now(), &ImportOptions::default(), &mut |p| {
            steps.push(p)
        })
        .unwrap();

    assert_eq!(fresh_report, rebuilt_report);
    assert_eq!(snapshot(&fresh), snapshot(&rebuilt));
    // Fortschritt: je Datei ein Aufruf, der Zähler läuft von 0 bis total - 1.
    assert_eq!(steps.len(), fresh_report.files_found);
    assert!(
        steps
            .iter()
            .enumerate()
            .all(|(i, p)| p.done == i && p.total == steps.len())
    );
    // Die Nutzerdaten sind in beiden gleich.
    assert_eq!(
        fresh.export_user_data().unwrap(),
        rebuilt.export_user_data().unwrap()
    );
}

fn sample_user_data() -> UserDataExport {
    let json = r#"{
      "backup_version": 1,
      "thresholds": [
        {"id": 7, "sport": "cycling", "kind": "ftp_w", "value": 250.0,
         "valid_from": "2026-01-01", "origin": "manual", "note": "Rampentest"},
        {"id": 9, "sport": "all", "kind": "hr_rest_bpm", "value": 48.0,
         "valid_from": "2026-02-01", "origin": "suggested_accepted", "note": null}
      ],
      "goals": [
        {"id": 3, "kind": "event", "sport": "cycling", "event_date": "2026-09-20",
         "label": "Zielrennen", "priority": "A", "active": true},
        {"id": 4, "kind": "continuous", "sport": null, "event_date": null,
         "label": null, "priority": null, "active": false}
      ],
      "availability": [
        {"weekday": 0, "max_minutes": 0, "allow_hard": false, "fixed_rest": true},
        {"weekday": 5, "max_minutes": 180, "allow_hard": true, "fixed_rest": false}
      ],
      "plans": [
        {"id": "7d9f0f4e-7b1c-4a39-9a54-0a4f1c0d2b11", "created_at": "2026-10-13T08:00:00Z",
         "goal_id": 3, "source": "rules", "rules_version": "1", "locale": "de",
         "json": "{\"weeks\":[]}", "status": "draft"}
      ],
      "manual_sessions": [
        {"id": 2, "local_date": "2026-05-12", "sport": "strength", "duration_s": 2700, "rpe": 7}
      ],
      "settings": [
        {"key": "athlete.sex", "value": "male"},
        {"key": "ui.locale", "value": "de"}
      ]
    }"#;
    UserDataExport::from_json(json).unwrap()
}

#[test]
fn rebuild_works_on_disk_databases_and_leaves_user_data_alone() {
    let dir = work_dir("rebuild_user_data");
    let fix = write_fixtures(&dir);
    let db = dir.join("lastwerk.db");
    let mut store = Store::open(&db).unwrap();
    // Die Beispieldaten ersetzen den Bestand, Schwelle 7 ist die FTP.
    store.import_user_data(&sample_user_data()).unwrap();
    import(&mut store, &fix);
    let before = store.export_user_data().unwrap();
    assert_eq!(before, sample_user_data());
    assert!(
        store
            .activity_loads()
            .unwrap()
            .iter()
            .any(|l| l.status == LoadStatus::Ok)
    );

    store
        .rebuild_cache(&fix, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();
    assert_eq!(
        store.export_user_data().unwrap(),
        before,
        "Nutzerdaten überleben rebuild"
    );
    assert!(!store.activities().unwrap().is_empty());

    // Auch ein Wiederaufbau aus einem leeren Ordner löscht nur den Cache.
    let empty = dir.join("empty_dir");
    fs::create_dir_all(&empty).unwrap();
    let report = store
        .rebuild_cache(&empty, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();
    assert_eq!(report.files_found, 0);
    assert!(store.activities().unwrap().is_empty());
    assert!(store.source_files().unwrap().is_empty());
    assert_eq!(store.export_user_data().unwrap(), before);

    // Die Datei lässt sich wieder öffnen, die Daten sind dauerhaft.
    drop(store);
    let reopened = Store::open(&db).unwrap();
    assert_eq!(reopened.export_user_data().unwrap(), before);
}

#[test]
fn user_data_export_and_import_roundtrip() {
    let user = sample_user_data();
    let mut a = Store::open_in_memory().unwrap();
    a.import_user_data(&user).unwrap();
    let exported = a.export_user_data().unwrap();
    assert_eq!(exported, user);

    // Über JSON in eine zweite, frische Datenbank.
    let text = exported.to_json().unwrap();
    let mut b = Store::open_in_memory().unwrap();
    b.import_user_data(&UserDataExport::from_json(&text).unwrap())
        .unwrap();
    assert_eq!(b.export_user_data().unwrap(), user);

    // Einspielen ersetzt den Bestand, es fügt nichts doppelt hinzu.
    b.import_user_data(&user).unwrap();
    assert_eq!(b.export_user_data().unwrap(), user);
    let empty = UserDataExport {
        thresholds: vec![],
        goals: vec![],
        plans: vec![],
        ..user.clone()
    };
    b.import_user_data(&empty).unwrap();
    assert_eq!(b.export_user_data().unwrap(), empty);
}

#[test]
fn failed_user_data_import_keeps_the_old_state() {
    let user = sample_user_data();
    let mut store = Store::open_in_memory().unwrap();
    store.import_user_data(&user).unwrap();
    // Verweis auf ein Ziel, das es nicht gibt: der Fremdschlüssel schlägt beim Abschluss an.
    let mut bad = user.clone();
    bad.plans[0].goal_id = Some(99);
    assert!(store.import_user_data(&bad).is_err());
    assert_eq!(store.export_user_data().unwrap(), user);
    // Unbekannte Formatversion.
    let mut newer = user.clone();
    newer.backup_version = 99;
    assert!(matches!(
        store.import_user_data(&newer),
        Err(StoreError::UnsupportedExportVersion(99))
    ));
    assert_eq!(store.export_user_data().unwrap(), user);
}

#[test]
fn replacing_thresholds_detaches_loads_that_reference_missing_ones() {
    let dir = work_dir("threshold_refs");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    import(&mut store, &fix);
    assert!(
        store
            .activity_loads()
            .unwrap()
            .iter()
            .any(|l| l.threshold_id.is_some())
    );
    let empty = UserDataExport {
        thresholds: vec![],
        ..store.export_user_data().unwrap()
    };
    store.import_user_data(&empty).unwrap();
    assert!(
        store
            .activity_loads()
            .unwrap()
            .iter()
            .all(|l| l.threshold_id.is_none())
    );
}

#[test]
fn changed_and_removed_content_updates_the_cache() {
    let dir = work_dir("changed_file");
    let src = dir.join("src");
    fs::create_dir_all(&src).unwrap();
    let get = |n: &str| fixtures::by_name(n).unwrap().bytes;
    fs::write(src.join("ride.fit"), get("power_constant_200w_3600s")).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let first = import(&mut store, &src);
    assert_eq!((first.new, first.updated), (1, 0));

    // Neuer Inhalt am selben Pfad: geändert, es bleibt bei einer Aktivität.
    fs::write(src.join("ride.fit"), get("power_ramp_100w_300w")).unwrap();
    let second = import(&mut store, &src);
    assert_eq!((second.new, second.updated, second.unchanged), (0, 1, 0));
    let acts = store.activities().unwrap();
    assert_eq!(acts.len(), 1);
    assert_eq!(acts[0].row.duration_s, 1800);
    assert_eq!(store.activity_loads().unwrap().len(), 1);

    // Wird die Datei beschädigt, bleibt keine alte Aktivität zurück.
    fs::write(src.join("ride.fit"), get("corrupt_crc")).unwrap();
    let third = import(&mut store, &src);
    assert_eq!(third.errors.len(), 1);
    assert!(store.activities().unwrap().is_empty());
    assert!(store.activity_loads().unwrap().is_empty());
    assert_eq!(
        store.source_files().unwrap()[0].row.status,
        SourceStatus::Error
    );
}

#[test]
fn identical_copy_in_another_folder_is_a_duplicate() {
    let dir = work_dir("identical_copy");
    let src = dir.join("src");
    fs::create_dir_all(src.join("backup")).unwrap();
    let bytes = fixtures::by_name("power_constant_200w_3600s")
        .unwrap()
        .bytes;
    fs::write(src.join("a.fit"), &bytes).unwrap();
    fs::write(src.join("backup").join("a_copy.FIT"), &bytes).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let report = import(&mut store, &src);
    assert_eq!((report.new, report.duplicates, report.primary), (2, 1, 1));
    let acts = store.activities().unwrap();
    // Gleicher Rang: die Datei mit der kleineren source_file.id gewinnt.
    assert!(acts[0].is_primary);
    assert_eq!(acts[1].duplicate_of, Some(acts[0].id));
}

#[test]
fn duplicate_rule_in_the_import_respects_the_time_distance() {
    use lastwerk_testkit::generators::{ActivityParams, activity};
    let ride = |start_offset_s: u32| {
        activity(&ActivityParams::cycling(
            fixtures::START_FIT + start_offset_s,
            fixtures::LOCAL_OFFSET_S,
            1800,
        ))
    };
    let run_case = |name: &str, offset_s: u32| {
        let dir = work_dir(name);
        fs::write(dir.join("a.fit"), ride(0)).unwrap();
        fs::write(dir.join("b.fit"), ride(offset_s)).unwrap();
        let mut store = Store::open_in_memory().unwrap();
        import(&mut store, &dir)
    };
    // 3 Minuten Abstand sind zwei Fahrten, 2 Minuten (Grenze) eine.
    let apart = run_case("dup_180s", 180);
    assert_eq!((apart.primary, apart.duplicates), (2, 0));
    let close = run_case("dup_120s", 120);
    assert_eq!((close.primary, close.duplicates), (1, 1));
    // Die Schwelle ist einstellbar.
    let dir = work_dir("dup_custom");
    fs::write(dir.join("a.fit"), ride(0)).unwrap();
    fs::write(dir.join("b.fit"), ride(180)).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let opts = ImportOptions {
        duplicates: lastwerk_store::DuplicateRule {
            max_start_diff_s: 200,
            ..Default::default()
        },
        ..Default::default()
    };
    let report = store.import_dir(&dir, now(), &opts, &mut |_| {}).unwrap();
    assert_eq!((report.primary, report.duplicates), (1, 1));
}

#[test]
fn local_date_follows_the_offset_from_the_file() {
    use lastwerk_testkit::generators::{ActivityParams, activity};
    // START_FIT ist 06:00 UTC am 10. Mai. Plus 15 h 30 min = 55 800 s ergibt 21:30 UTC, bei
    // UTC+2 lokal 23:30 desselben Tages.
    let dir = work_dir("local_date");
    fs::write(
        dir.join("late.fit"),
        activity(&ActivityParams::cycling(
            fixtures::START_FIT + 55_800,
            7200,
            1800,
        )),
    )
    .unwrap();
    // Plus 16 h 30 min = 59 400 s und einen Tag ergibt 22:30 UTC am 11. Mai, lokal 00:30
    // am 12. Mai.
    fs::write(
        dir.join("after_midnight.fit"),
        activity(&ActivityParams::cycling(
            fixtures::START_FIT + 59_400 + 86_400,
            7200,
            1800,
        )),
    )
    .unwrap();
    let mut store = Store::open_in_memory().unwrap();
    import(&mut store, &dir);
    let mut dates: Vec<_> = store
        .activities()
        .unwrap()
        .iter()
        .map(|a| a.row.local_date)
        .collect();
    dates.sort();
    assert_eq!(dates, [date(2026, 5, 10), date(2026, 5, 12)]);
}

#[test]
fn input_errors_and_foreign_keys() {
    let dir = work_dir("open");
    let mut store = Store::open(&dir.join("x.db")).unwrap();
    let missing = dir.join("gibt_es_nicht");
    assert!(matches!(
        store.import_dir(&missing, now(), &ImportOptions::default(), &mut |_| {}),
        Err(StoreError::NotADirectory)
    ));
    assert!(matches!(
        store.rebuild_cache(&missing, now(), &ImportOptions::default(), &mut |_| {}),
        Err(StoreError::NotADirectory)
    ));
    // Fremdschlüssel sind aktiv.
    let bad = lastwerk_store::ActivityRow {
        source_file_id: 12345,
        sport: sport_from_str("cycling").unwrap(),
        sub_sport: None,
        start_utc: now(),
        local_offset_s: 0,
        local_date: date(2026, 1, 1),
        duration_s: 1,
        moving_s: None,
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
    };
    assert!(store.upsert_activity(&bad).is_err());
    assert_eq!(
        store.schema_version().unwrap(),
        lastwerk_store::migrate::latest_version()
    );
}

/// Löschen entfernt nur den genannten Schlüssel; ein fehlender Schlüssel ist kein Fehler (T28).
#[test]
fn delete_setting_removes_only_that_key() {
    let mut s = Store::open_in_memory().unwrap();
    s.set_setting("a", "1").unwrap();
    s.set_setting("b", "2").unwrap();
    s.delete_setting("a").unwrap();
    s.delete_setting("gibt-es-nicht").unwrap();
    assert_eq!(s.get_setting("a").unwrap(), None);
    assert_eq!(s.get_setting("b").unwrap().as_deref(), Some("2"));
}

#[test]
fn opening_twice_is_idempotent_and_keeps_data() {
    let dir = work_dir("open_twice");
    let db = dir.join("lastwerk.db");
    {
        let mut s = Store::open(&db).unwrap();
        s.set_setting("k", "v").unwrap();
    }
    let s = Store::open(&db).unwrap();
    assert_eq!(s.get_setting("k").unwrap().as_deref(), Some("v"));
    assert_eq!(
        s.schema_version().unwrap(),
        lastwerk_store::migrate::latest_version()
    );
}

#[test]
fn plausibility_summary_reports_only_figures() {
    let dir = work_dir("summary");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    import(&mut store, &fix);
    let s = store.plausibility_summary().unwrap();
    let readable = fixtures::all()
        .iter()
        .filter(|f| f.expect.activities == 1)
        .count();
    assert_eq!(s.files_total, 19);
    assert_eq!(s.files_error, 3);
    assert_eq!(s.files_not_activity, 1);
    assert_eq!(s.activities, readable);
    assert_eq!(s.duplicate_pairs, 1);
    assert_eq!(s.duplicate_max_start_diff_s, Some(20));
    // 30 s von 3600 s sind 0,83 Prozent.
    assert!((s.duplicate_max_duration_diff_pct.unwrap() - 100.0 * 30.0 / 3600.0).abs() < 1e-9);
    assert_eq!(s.primary, readable - 1);
    assert!(s.load_computable > 0 && s.load_computable < s.primary);
    let share = s.share_not_computable().unwrap();
    assert!(share > 0.0 && share < 1.0);
    // NP geteilt durch Durchschnitt: konstante Leistung ergibt 1, die Intervalle mehr.
    let ratio = s.np_ratio.unwrap();
    assert!((ratio.min - 1.0).abs() < 0.01);
    assert!(ratio.max > 1.0);
    assert_eq!(s.load_per_year.len(), 1);
}
