//! Die Befehle gegen eine echte Datenbank mit den synthetischen Fixtures aus
//! `lastwerk-testkit`. Alle Dateien liegen unter `target/`, es gibt keine echten Daten.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use lastwerk_app::dto::{
    ActivityFilter, ActivityItem, AthleteInput, SettingsPatch, ThresholdInput,
};
use lastwerk_app::service;
use lastwerk_core::Sport;
use lastwerk_store::{Store, ThresholdKind};
use lastwerk_testkit::fixtures;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("app-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap()
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

/// Datenbank im Speicher, FIT-Ordner mit allen Fixtures, Ordner gemerkt und importiert.
fn imported(name: &str) -> (Store, PathBuf) {
    let dir = work_dir(name);
    let fit = dir.join("fit");
    fs::create_dir_all(&fit).unwrap();
    for f in fixtures::all() {
        fs::write(fit.join(f.file_name()), &f.bytes).unwrap();
    }
    let mut store = Store::open(&dir.join("lastwerk.db")).unwrap();
    service::import(&mut store, &fit, now(), &mut |_| {}).unwrap();
    (store, fit)
}

fn may() -> ActivityFilter {
    ActivityFilter {
        from: "2026-05-01".into(),
        to: "2026-05-31".into(),
        sport: None,
    }
}

fn find(items: &[ActivityItem], duration_s: i64, sport: Sport) -> Vec<&ActivityItem> {
    items
        .iter()
        .filter(|i| i.duration_s == duration_s && i.sport == sport)
        .collect()
}

/// Fehlercode eines Ergebnisses, `ok` bei Erfolg.
fn err<T>(r: Result<T, lastwerk_app::error::AppError>) -> String {
    match r {
        Ok(_) => "ok".to_owned(),
        Err(e) => e.code,
    }
}

/// Stamm für Aufrufe, die vor jedem Dateizugriff mit einem Fehler enden.
fn unused() -> PathBuf {
    PathBuf::from("unbenutzt")
}

fn ftp(value: f64) -> ThresholdInput {
    ThresholdInput {
        sport: Some(Sport::Cycling),
        kind: ThresholdKind::FtpW,
        value,
        valid_from: "2026-01-01".into(),
        note: None,
    }
}

#[test]
fn import_summary_counts_files_without_exposing_paths() {
    let dir = work_dir("summary");
    let fit = dir.join("fit");
    fs::create_dir_all(&fit).unwrap();
    for f in fixtures::all() {
        fs::write(fit.join(f.file_name()), &f.bytes).unwrap();
    }
    let mut store = Store::open_in_memory().unwrap();
    let mut events = Vec::new();
    let s = service::import(&mut store, &fit, now(), &mut |p| {
        events.push((p.done, p.total))
    })
    .unwrap();
    assert_eq!(
        (s.files_found, s.new, s.not_activity, s.error_count),
        (19, 15, 1, 3)
    );
    assert_eq!((s.primary, s.duplicates), (14, 1));
    let mut codes: Vec<&str> = s.errors_by_code.iter().map(|e| e.code.as_str()).collect();
    codes.sort_unstable();
    assert_eq!(codes, ["crc_mismatch", "empty_file", "truncated"]);
    let json = debug_text(&s);
    assert!(
        !json.contains(".fit"),
        "Dateinamen dürfen nicht ins Frontend: {json}"
    );
    assert_eq!(events.last().copied(), Some((19, 19)));

    // Zweiter Lauf: nichts Neues.
    let again = service::import(&mut store, &fit, now(), &mut |_| {}).unwrap();
    assert_eq!((again.new, again.unchanged), (0, 15 + 1 + 3));
}

/// Der feste Stamm wird beim Import angelegt, falls er fehlt (frische Installation): Das ist ein
/// Import von null Dateien und kein Fehler (T28, vorher `no_folder`).
#[test]
fn a_fresh_root_is_created_and_imports_nothing() {
    let root = work_dir("fresh-root").join("Lastwerk").join("fit");
    let mut store = Store::open_in_memory().unwrap();
    let s = service::import(&mut store, &root, now(), &mut |_| {}).unwrap();
    assert_eq!((s.files_found, s.new, s.error_count), (0, 0, 0));
    assert!(root.is_dir());
}

fn debug_text(s: &lastwerk_app::dto::ImportSummary) -> String {
    format!("{s:?}")
}

#[test]
fn loads_are_not_computable_until_a_threshold_exists_and_never_zero() {
    let (store, _) = imported("not-computable");
    let items = service::list_activities(&store, &may()).unwrap();
    assert_eq!(items.len(), 15, "14 zählende plus ein Duplikat");
    // Neueste zuerst.
    assert!(items.windows(2).all(|w| w[0].start_utc >= w[1].start_utc));
    for i in &items {
        assert_eq!(
            i.load.value, None,
            "ohne Schwelle gibt es keine Zahl: {i:?}"
        );
        assert_ne!(i.load.status, "ok");
    }
    let power = find(&items, 3600, Sport::Cycling);
    assert!(power.iter().any(|i| i.load.status == "missing_threshold"));
    // Kraft ohne RPE hat keinen Messkanal für die Last.
    let strength = find(&items, 2700, Sport::Strength);
    assert_eq!(strength[0].load.status, "missing_channel");
    // Die Uhr-Aufnahme des Paares ist als Duplikat gekennzeichnet.
    let dup: Vec<_> = items.iter().filter(|i| !i.is_primary).collect();
    assert_eq!(dup.len(), 1);
    assert!(dup[0].duplicate_of.is_some());
}

/// Handrechnung (SPEC 8.1, T-TSS): `power_constant_200w_3600s` fährt 3600 s mit konstant
/// 200 W, FTP 250. NP = 200, IF = 200 / 250 = 0,8, TSS = 3600 * 200 * 0,8 / (250 * 3600) * 100
/// = 64,0.
const TSS_200W_AT_FTP_250: f64 = 64.0;

#[test]
fn a_threshold_makes_loads_computable_and_week_and_list_agree() {
    let (mut store, fit) = imported("threshold");
    let set = service::set_threshold(&mut store, &fit, &ftp(250.0), now()).unwrap();
    assert!(!set.recompute.skipped);
    assert!(set.recompute.recomputed > 0);
    assert_eq!(set.recompute.file_missing + set.recompute.file_changed, 0);

    let items = service::list_activities(&store, &may()).unwrap();
    let hour = items
        .iter()
        .find(|i| i.sport == Sport::Cycling && i.duration_s == 3600 && i.local_date == "2026-05-11")
        .expect("power_constant_200w_3600s am 11. Mai");
    assert_eq!(hour.load.status, "ok");
    assert_eq!(hour.load.method.as_deref(), Some("tss"));
    assert!(!hour.load.estimated);
    assert!((hour.load.value.unwrap() - TSS_200W_AT_FTP_250).abs() < 1e-3);
    assert!((hour.load.intensity_factor.unwrap() - 0.8).abs() < 1e-4);

    // Wochenübersicht der Woche ab Montag 11. Mai: Summe der Tage gleich Summe der zählenden
    // Aktivitäten der Liste in dieser Woche.
    let week = service::week_overview(&store, "2026-05-11", 12, date(2026, 6, 30)).unwrap();
    assert_eq!(
        (week.week_start.as_str(), week.week_end.as_str()),
        ("2026-05-11", "2026-05-17")
    );
    assert_eq!(week.days.len(), 7);
    let in_week: f64 = items
        .iter()
        .filter(|i| {
            i.is_primary
                && i.local_date.as_str() >= "2026-05-11"
                && i.local_date.as_str() <= "2026-05-17"
        })
        .filter_map(|i| i.load.value)
        .sum();
    let by_days: f64 = week
        .days
        .iter()
        .flat_map(|d| &d.by_sport)
        .map(|s| s.load)
        .sum();
    let by_totals: f64 = week.totals_by_sport.iter().map(|s| s.load).sum();
    assert!(in_week > 0.0);
    assert!(
        (by_days - in_week).abs() < 1e-3,
        "{by_days} gegen {in_week}"
    );
    assert!((by_totals - in_week).abs() < 1e-3);
    assert_eq!(
        week.not_computable, 0,
        "alle Fahrten der Woche haben Leistung und FTP"
    );
    assert_eq!(week.days[0].date, "2026-05-11");

    // Woche davor: am Sonntag 10. Mai liegen Lauf, Schwimmen und Kraft ohne Schwelle. Sie
    // zählen als nicht berechenbar und nicht als 0, nur die Fahrt hat eine Last.
    let before = service::week_overview(&store, "2026-05-04", 12, date(2026, 6, 30)).unwrap();
    assert!(before.not_computable >= 3, "{}", before.not_computable);
    let sunday = &before.days[6];
    assert_eq!(sunday.date, "2026-05-10");
    assert_eq!(sunday.not_computable, before.not_computable);
    assert!(
        sunday
            .by_sport
            .iter()
            .all(|s| s.sport == Sport::Cycling && s.load > 0.0)
    );
    assert!(
        before.days[..6]
            .iter()
            .all(|d| d.by_sport.is_empty() && d.not_computable == 0)
    );

    // Verlauf: TSB ist CTL minus ATL des Vortags (SPEC 8.2), Tage lückenlos bis zum Wochenende.
    assert_eq!(week.pmc.last().unwrap().date, "2026-05-17");
    for w in week.pmc.windows(2) {
        assert!((w[1].tsb - (w[0].ctl - w[0].atl)).abs() < 1e-3);
    }
    assert_eq!(week.pmc_first_load_date.as_deref(), Some("2026-05-10"));
}

#[test]
fn the_form_curve_stops_today_and_starts_at_the_window() {
    let (mut store, fit) = imported("pmc-window");
    service::set_threshold(&mut store, &fit, &ftp(250.0), now()).unwrap();
    // Heute liegt mitten in der Woche: der Verlauf endet dort, nicht am Wochenende.
    let week = service::week_overview(&store, "2026-05-11", 4, date(2026, 5, 14)).unwrap();
    assert_eq!(week.pmc.last().unwrap().date, "2026-05-14");
    // Fenster: vier Wochen bis zum 17. Mai einschließlich, erster Tag der 20. April.
    assert_eq!(week.pmc.first().unwrap().date, "2026-04-20");
    // Vor dem ersten Tag mit Last ist alles 0 (Startwerte), das ist gerechnet und kein Fehlen.
    assert_eq!(week.pmc.first().unwrap().ctl, 0.0);
    // Eine Woche in der Zukunft hat keinen Verlauf.
    let future = service::week_overview(&store, "2026-08-03", 4, date(2026, 5, 14)).unwrap();
    assert!(future.pmc.is_empty());
    assert!(future.days.iter().all(|d| d.by_sport.is_empty()));
}

#[test]
fn detail_has_zones_best_power_and_a_downsampled_series() {
    let (mut store, fit) = imported("detail");
    service::set_threshold(&mut store, &fit, &ftp(250.0), now()).unwrap();
    let items = service::list_activities(&store, &may()).unwrap();
    let id = items
        .iter()
        .find(|i| i.sport == Sport::Cycling && i.duration_s == 3600 && i.local_date == "2026-05-11")
        .unwrap()
        .id;
    let d = service::get_activity(&store, id, &fit).unwrap();
    // 200 W bei FTP 250: Anteil 0,8, Zone 3 von power7 (Handrechnung in crates/store/tests/zones.rs).
    let power7 = d.zones.iter().find(|z| z.id == "power7").unwrap();
    assert_eq!(power7.seconds.len(), 7);
    assert_eq!(power7.seconds[2], 3600);
    assert_eq!(power7.seconds.iter().sum::<u32>(), 3600);
    assert!(!power7.estimated);
    assert_eq!(power7.source, "Q16");
    assert_eq!(power7.status, "Sekundärquelle");
    let b1200 = d.best_power.iter().find(|b| b.duration_s == 1200).unwrap();
    assert!((b1200.watts - 200.0).abs() < 1e-3);
    assert_eq!(d.series.status, "ok");
    assert!(!d.series.t_s.is_empty() && d.series.t_s.len() <= service::MAX_SERIES_POINTS);
    assert!(
        d.series
            .power_w
            .iter()
            .flatten()
            .all(|p| (*p - 200.0).abs() < 1.0)
    );
    // T31: Geschwindigkeit hat dieselbe Länge wie die Zeitachse. Die Fixture hat eine Runde,
    // also keine Rundentabelle. Konstante Leistung über eine Stunde: Die Entkopplung ist
    // berechenbar, wenn die Datei Herzfrequenz trägt, und dann nahe 0 (gekoppelt).
    assert_eq!(d.series.speed_mps.len(), d.series.t_s.len());
    assert!(d.laps.is_empty());
    match d.decoupling.percent {
        Some(p) => {
            assert!(p.abs() < 5.0, "{p}");
            assert_eq!(d.decoupling.state, "decoupling.coupled");
        }
        None => assert_eq!(d.decoupling.state, "decoupling.no_data"),
    }
    // Mit veränderter Datei gibt es keine Reihe, aber die Kennzahlen bleiben.
    let file = fit.join("power_constant_200w_3600s.fit");
    let mut bytes = fs::read(&file).unwrap();
    bytes.push(0);
    fs::write(&file, bytes).unwrap();
    let changed = service::get_activity(&store, id, &fit).unwrap();
    assert_eq!(changed.series.status, "file_changed");
    assert_eq!(changed.decoupling.state, "decoupling.no_data");
    assert!(changed.laps.is_empty());
    assert_eq!(changed.efficiency_factor, d.efficiency_factor);
    assert_eq!(changed.zones.len(), d.zones.len());
    fs::remove_file(&file).unwrap();
    assert_eq!(
        service::get_activity(&store, id, &fit)
            .unwrap()
            .series
            .status,
        "file_missing"
    );
    assert_eq!(
        service::get_activity(&store, 999_999, &fit)
            .unwrap_err()
            .code,
        "not_found"
    );
}

#[test]
fn a_later_threshold_does_not_change_earlier_loads_and_lists_validity() {
    let (mut store, fit) = imported("validity");
    service::set_threshold(&mut store, &fit, &ftp(250.0), now()).unwrap();
    // Neuer FTP erst ab 13. Mai: Fahrten vorher bleiben bei 250 W (SPEC 3).
    let later = ThresholdInput {
        valid_from: "2026-05-13".into(),
        ..ftp(200.0)
    };
    service::set_threshold(&mut store, &fit, &later, now()).unwrap();
    let list = service::list_thresholds(&store, date(2026, 6, 1)).unwrap();
    assert_eq!(list.items.len(), 2);
    let first = list
        .items
        .iter()
        .find(|t| t.valid_from == "2026-01-01")
        .unwrap();
    assert_eq!(first.valid_until.as_deref(), Some("2026-05-12"));
    let second = list
        .items
        .iter()
        .find(|t| t.valid_from == "2026-05-13")
        .unwrap();
    assert_eq!(second.valid_until, None);
    assert_eq!(second.origin, "manual");

    let items = service::list_activities(&store, &may()).unwrap();
    let load_on = |day: &str| {
        items
            .iter()
            .find(|i| i.sport == Sport::Cycling && i.duration_s == 3600 && i.local_date == day)
            .and_then(|i| i.load.value)
    };
    // 11. Mai (vor dem Wechsel): 200 W bei FTP 250 gibt weiter 64,0.
    assert!((load_on("2026-05-11").unwrap() - TSS_200W_AT_FTP_250).abs() < 1e-3);
    // 14. Mai (hr_drift_130_160, 3600 s mit konstant 180 W) rechnet mit dem neuen FTP 200.
    // Handrechnung: NP = 180, IF = 180 / 200 = 0,9, TSS = 3600 * 180 * 0,9 / (200 * 3600) * 100
    // = 81,0. Mit dem alten FTP 250 wären es 51,84.
    assert!((load_on("2026-05-14").unwrap() - 81.0).abs() < 1e-3);
}

#[test]
fn estimated_hr_max_is_listed_only_without_a_measured_value() {
    let (mut store, fit) = imported("hrmax");
    let none = service::list_thresholds(&store, date(2026, 6, 1)).unwrap();
    assert!(
        none.hr_max_estimate.is_none(),
        "ohne Geburtsjahr keine Schätzung"
    );

    let set = service::set_athlete(
        &mut store,
        &fit,
        &AthleteInput {
            sex: Some("male".into()),
            birth_year: Some(1990),
        },
        now(),
        false,
    )
    .unwrap();
    assert_eq!(set.settings.athlete.birth_year, Some(1990));
    assert_eq!(set.settings.athlete.sex.as_deref(), Some("male"));
    assert!(!set.recompute.skipped);
    // Alter im Jahr 2026: 36. HR_max = 208 - 0,7 * 36 = 182,8 (Q7), immer geschätzt.
    let list = service::list_thresholds(&store, date(2026, 6, 1)).unwrap();
    let est = list.hr_max_estimate.unwrap();
    assert!(est.estimated);
    assert!((est.bpm - 182.8).abs() < 1e-3, "{}", est.bpm);

    // Mit gemessenem Wert entfällt die Schätzung.
    let measured = ThresholdInput {
        sport: None,
        kind: ThresholdKind::HrMaxBpm,
        value: 190.0,
        valid_from: "2026-01-01".into(),
        note: Some("  Test  ".into()),
    };
    service::set_threshold(&mut store, &fit, &measured, now()).unwrap();
    let list = service::list_thresholds(&store, date(2026, 6, 1)).unwrap();
    assert!(list.hr_max_estimate.is_none());
    assert_eq!(list.items[0].note.as_deref(), Some("Test"));
    assert_eq!(list.items[0].sport, None);
}

#[test]
fn invalid_input_is_rejected_with_a_code_and_changes_nothing() {
    let (mut store, _) = imported("invalid");
    assert_eq!(
        err(service::set_threshold(
            &mut store,
            &unused(),
            &ftp(0.0),
            now()
        )),
        "invalid_value"
    );
    assert_eq!(
        err(service::set_threshold(
            &mut store,
            &unused(),
            &ftp(f64::NAN),
            now()
        )),
        "invalid_value"
    );
    assert_eq!(
        err(service::set_threshold(
            &mut store,
            &unused(),
            &ftp(2600.0),
            now()
        )),
        "invalid_value"
    );
    let wrong_sport = ThresholdInput {
        sport: Some(Sport::Running),
        ..ftp(250.0)
    };
    assert_eq!(
        err(service::set_threshold(
            &mut store,
            &unused(),
            &wrong_sport,
            now()
        )),
        "invalid_sport"
    );
    let bad_date = ThresholdInput {
        valid_from: "13.10.2026".into(),
        ..ftp(250.0)
    };
    assert_eq!(
        err(service::set_threshold(
            &mut store,
            &unused(),
            &bad_date,
            now()
        )),
        "invalid_date"
    );
    let hr = ThresholdInput {
        sport: None,
        kind: ThresholdKind::HrMaxBpm,
        value: 250.0,
        valid_from: "2026-01-01".into(),
        note: None,
    };
    assert_eq!(
        err(service::set_threshold(&mut store, &unused(), &hr, now())),
        "invalid_value"
    );
    let long_note = ThresholdInput {
        note: Some("x".repeat(service::MAX_NOTE_CHARS + 1)),
        ..ftp(250.0)
    };
    assert_eq!(
        err(service::set_threshold(
            &mut store,
            &unused(),
            &long_note,
            now()
        )),
        "invalid_value"
    );
    assert!(
        service::list_thresholds(&store, date(2026, 6, 1))
            .unwrap()
            .items
            .is_empty()
    );

    let year = |y| AthleteInput {
        sex: None,
        birth_year: Some(y),
    };
    assert_eq!(
        err(service::set_athlete(
            &mut store,
            &unused(),
            &year(1899),
            now(),
            false
        )),
        "invalid_year"
    );
    assert_eq!(
        err(service::set_athlete(
            &mut store,
            &unused(),
            &year(2027),
            now(),
            false
        )),
        "invalid_year"
    );
    let sex = AthleteInput {
        sex: Some("x".into()),
        birth_year: None,
    };
    assert_eq!(
        err(service::set_athlete(
            &mut store,
            &unused(),
            &sex,
            now(),
            false
        )),
        "invalid_value"
    );
    assert_eq!(
        service::get_settings(&store, false)
            .unwrap()
            .athlete
            .birth_year,
        None
    );

    let backwards = ActivityFilter {
        from: "2026-06-01".into(),
        to: "2026-05-01".into(),
        sport: None,
    };
    assert_eq!(
        err(service::list_activities(&store, &backwards)),
        "invalid_date"
    );
    let garbage = ActivityFilter {
        from: "morgen".into(),
        to: "2026-05-01".into(),
        sport: None,
    };
    assert_eq!(
        err(service::list_activities(&store, &garbage)),
        "invalid_date"
    );
    assert_eq!(
        err(service::week_overview(
            &store,
            "2026-13-40",
            4,
            date(2026, 6, 1)
        )),
        "invalid_date"
    );
}

#[test]
fn settings_default_to_dark_german_and_roundtrip() {
    let mut store = Store::open_in_memory().unwrap();
    let s = service::get_settings(&store, true).unwrap();
    assert_eq!((s.locale.as_str(), s.theme.as_str()), ("de", "dark"));
    // Stamm und Übernahme füllt der Befehl, die Datenbank kennt sie nicht (T28).
    assert_eq!((s.fit_root.as_str(), s.fit_migration), ("", None));
    assert!(s.db_in_sync_folder);
    // Standard des Verlaufs aus core: 42 und 7 Tage, Start 0 (SPEC 8.2).
    assert_eq!(
        (s.pmc.tau_ctl_d, s.pmc.tau_atl_d, s.pmc.ctl0, s.pmc.atl0),
        (42.0, 7.0, 0.0, 0.0)
    );

    let patch = SettingsPatch {
        locale: Some("en".into()),
        theme: Some("light".into()),
        pmc: Some(lastwerk_app::dto::PmcSettings {
            tau_ctl_d: 50.0,
            tau_atl_d: 15.0,
            ctl0: 20.5,
            atl0: 10.0,
        }),
        mcp_recovery_enabled: None,
        onboarded: None,
        garmin_sync_on_start: None,
    };
    let s = service::set_settings(&mut store, &patch, false).unwrap();
    assert_eq!((s.locale.as_str(), s.theme.as_str()), ("en", "light"));
    assert_eq!(
        (s.pmc.tau_ctl_d, s.pmc.tau_atl_d, s.pmc.ctl0, s.pmc.atl0),
        (50.0, 15.0, 20.5, 10.0)
    );

    // Ungültige Teile ändern nichts, auch die gültigen Teile derselben Anfrage nicht.
    for bad in [
        SettingsPatch {
            locale: Some("fr".into()),
            theme: Some("dark".into()),
            pmc: None,
            mcp_recovery_enabled: None,
            onboarded: None,
            garmin_sync_on_start: None,
        },
        SettingsPatch {
            locale: None,
            theme: Some("neon".into()),
            pmc: None,
            mcp_recovery_enabled: None,
            onboarded: None,
            garmin_sync_on_start: None,
        },
        SettingsPatch {
            locale: Some("de".into()),
            theme: None,
            pmc: Some(lastwerk_app::dto::PmcSettings {
                tau_ctl_d: 0.0,
                tau_atl_d: 7.0,
                ctl0: 0.0,
                atl0: 0.0,
            }),
            mcp_recovery_enabled: None,
            onboarded: None,
            garmin_sync_on_start: None,
        },
        SettingsPatch {
            locale: None,
            theme: None,
            pmc: Some(lastwerk_app::dto::PmcSettings {
                tau_ctl_d: 42.0,
                tau_atl_d: 7.0,
                ctl0: -1.0,
                atl0: 0.0,
            }),
            mcp_recovery_enabled: None,
            onboarded: None,
            garmin_sync_on_start: None,
        },
    ] {
        assert!(service::set_settings(&mut store, &bad, false).is_err());
    }
    let s = service::get_settings(&store, false).unwrap();
    assert_eq!((s.locale.as_str(), s.theme.as_str()), ("en", "light"));
    assert_eq!(s.pmc.tau_ctl_d, 50.0);
}

/// Die Freigabe der Erholungswerte für den MCP-Server (SPEC 9.6, O19): Standard aus, die
/// Oberfläche schreibt genau `true` oder `false`, `get_recovery_days` liest genau `true`. Ein
/// Patch ohne das Feld lässt die Freigabe unverändert.
#[test]
fn the_recovery_switch_writes_exactly_true_or_false() {
    let mut store = Store::open_in_memory().unwrap();
    let patch = |enabled| SettingsPatch {
        locale: None,
        theme: None,
        pmc: None,
        mcp_recovery_enabled: enabled,
        onboarded: None,
        garmin_sync_on_start: None,
    };
    assert!(
        !service::get_settings(&store, false)
            .unwrap()
            .mcp_recovery_enabled
    );
    assert_eq!(store.get_setting("mcp.recovery_enabled").unwrap(), None);

    let s = service::set_settings(&mut store, &patch(Some(true)), false).unwrap();
    assert!(s.mcp_recovery_enabled);
    assert_eq!(
        store
            .get_setting("mcp.recovery_enabled")
            .unwrap()
            .as_deref(),
        Some("true")
    );
    // Ein Patch ohne das Feld ändert die Freigabe nicht.
    let s = service::set_settings(&mut store, &patch(None), false).unwrap();
    assert!(s.mcp_recovery_enabled);

    let s = service::set_settings(&mut store, &patch(Some(false)), false).unwrap();
    assert!(!s.mcp_recovery_enabled);
    assert_eq!(
        store
            .get_setting("mcp.recovery_enabled")
            .unwrap()
            .as_deref(),
        Some("false")
    );

    // Andere Werte in der Einstellung (von Hand oder aus einer Sicherung) geben nicht frei,
    // wie im MCP-Server: nur genau `true`.
    for other in ["TRUE", "1", "yes", " true", "true "] {
        store.set_setting("mcp.recovery_enabled", other).unwrap();
        assert!(
            !service::get_settings(&store, false)
                .unwrap()
                .mcp_recovery_enabled,
            "{other:?}"
        );
    }
}

/// `set_settings` nimmt den Schalter nur als JSON-Wahrheitswert: Zahlen, Texte und Listen
/// scheitern schon beim Einlesen, bevor etwas geschrieben wird.
#[test]
fn the_recovery_switch_accepts_only_a_boolean() {
    use serde_json::json;
    let ok: SettingsPatch =
        serde_json::from_value(json!({ "mcp_recovery_enabled": true })).unwrap();
    assert_eq!(ok.mcp_recovery_enabled, Some(true));
    let absent: SettingsPatch = serde_json::from_value(json!({})).unwrap();
    assert_eq!(absent.mcp_recovery_enabled, None);
    for bad in [
        json!("true"),
        json!(1),
        json!(0),
        json!("TRUE"),
        json!([true]),
        json!({}),
    ] {
        assert!(
            serde_json::from_value::<SettingsPatch>(json!({ "mcp_recovery_enabled": bad }))
                .is_err(),
            "{bad}"
        );
    }
}

/// Ein Stamm, den es noch nicht gibt (frische Installation), enthält nichts zum Neurechnen: Die
/// Schwelle wird gespeichert, das Neurechnen entfällt.
#[test]
fn with_a_root_that_does_not_exist_yet_a_threshold_is_saved_but_not_applied() {
    let mut store = Store::open_in_memory().unwrap();
    let root = work_dir("no-root").join("fit");
    let set = service::set_threshold(&mut store, &root, &ftp(250.0), now()).unwrap();
    assert!(set.recompute.skipped);
    assert_eq!(
        service::list_thresholds(&store, date(2026, 6, 1))
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn without_any_counted_load_there_is_no_curve_instead_of_zeros() {
    let (store, _) = imported("no-load");
    // Importiert, aber ohne Schwelle: keine Last ist berechenbar, also auch kein Verlauf.
    let week = service::week_overview(&store, "2026-05-11", 12, date(2026, 6, 30)).unwrap();
    assert!(week.pmc.is_empty());
    assert_eq!(week.pmc_first_load_date, None);
    assert!(week.not_computable > 0);
}

/// Der Abschluss des Erststarts liegt als Einstellung `app.onboarded` in der Datenbank
/// (nicht im Browser): Standard aus, genau `true` oder `false`, ein Patch ohne das Feld ändert
/// nichts, und ein Zahlenwert wird abgelehnt.
#[test]
fn the_onboarding_flag_is_stored_as_a_setting() {
    let mut store = Store::open_in_memory().unwrap();
    let patch = |done| SettingsPatch {
        locale: None,
        theme: None,
        pmc: None,
        mcp_recovery_enabled: None,
        onboarded: done,
        garmin_sync_on_start: None,
    };
    assert!(!service::get_settings(&store, false).unwrap().onboarded);
    assert_eq!(store.get_setting("app.onboarded").unwrap(), None);

    let s = service::set_settings(&mut store, &patch(Some(true)), false).unwrap();
    assert!(s.onboarded);
    assert_eq!(
        store.get_setting("app.onboarded").unwrap().as_deref(),
        Some("true")
    );
    assert!(
        service::set_settings(&mut store, &patch(None), false)
            .unwrap()
            .onboarded
    );
    assert!(
        !service::set_settings(&mut store, &patch(Some(false)), false)
            .unwrap()
            .onboarded
    );
    assert!(
        serde_json::from_value::<SettingsPatch>(serde_json::json!({ "onboarded": 1 })).is_err()
    );
}

/// Der Status des Austauschordners zählt nur Dateinamen: Erholungsdateien heißen
/// `YYYY-MM-DD.json`, FIT-Dateien enden auf `.fit`. Ein fehlender Ordner heißt "keine Daten".
#[test]
fn the_bridge_status_counts_names_and_never_fails_on_a_missing_folder() {
    let missing = std::env::temp_dir().join("lastwerk-t19-missing-exchange");
    let s = service::bridge_status(&missing);
    assert_eq!(
        (s.has_data, s.recovery_days, s.activity_files),
        (false, 0, 0)
    );
    assert_eq!(s.last_day, None);

    let dir = std::env::temp_dir().join(format!("lastwerk-t19-exchange-{}", std::process::id()));
    let rec = dir.join("out").join("recovery");
    let act = dir.join("out").join("activities");
    std::fs::create_dir_all(&rec).unwrap();
    std::fs::create_dir_all(&act).unwrap();
    for name in ["2026-10-01.json", "2026-10-03.json", "notiz.json"] {
        std::fs::write(rec.join(name), "{}").unwrap();
    }
    std::fs::write(act.join("a.fit"), b"").unwrap();
    std::fs::write(act.join("b.txt"), b"").unwrap();
    let s = service::bridge_status(&dir);
    assert!(s.has_data);
    assert_eq!((s.recovery_days, s.activity_files), (2, 1));
    assert_eq!(s.last_day.as_deref(), Some("2026-10-03"));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// T32: Die Tagessummen der Fortschrittsansicht stimmen mit der Aktivitätsliste überein: gleiche
/// Zahl zählender Aktivitäten, gleiche Dauer und Strecke, Last nur aus berechenbaren Werten,
/// Duplikate zählen nicht (SPEC 6.4).
#[test]
fn progress_sums_match_the_activity_list() {
    let (mut store, fit) = imported("progress");
    service::set_threshold(&mut store, &fit, &ftp(250.0), now()).unwrap();
    let items = service::list_activities(&store, &may()).unwrap();
    let primary: Vec<&ActivityItem> = items.iter().filter(|i| i.is_primary).collect();
    assert!(items.len() > primary.len(), "Fixtures enthalten Duplikate");

    let p = service::progress::progress(&store, "2026-05-31").unwrap();
    assert_eq!(p.from, "2024-06-01");
    let count: u32 = p.entries.iter().map(|e| e.count).sum();
    assert_eq!(count as usize, primary.len());
    let duration: i64 = p.entries.iter().map(|e| e.duration_s).sum();
    assert_eq!(duration, primary.iter().map(|i| i.duration_s).sum::<i64>());
    let distance: f64 = p.entries.iter().map(|e| e.distance_m).sum();
    let expected_distance: f64 = primary.iter().filter_map(|i| i.distance_m).sum();
    assert!((distance - expected_distance).abs() < 1.0);
    let load: f64 = p.entries.iter().map(|e| e.load).sum();
    let expected_load: f64 = primary
        .iter()
        .filter(|i| i.load.status == "ok")
        .filter_map(|i| i.load.value)
        .sum();
    assert!((load - expected_load).abs() < 0.5, "{load} {expected_load}");
    let missing: u32 = p.entries.iter().map(|e| e.load_missing).sum();
    assert_eq!(
        missing as usize,
        primary.iter().filter(|i| i.load.status != "ok").count()
    );
    // Aufsteigend nach Datum, je Tag und Sportart höchstens eine Zeile.
    assert!(p.entries.windows(2).all(|w| w[0].date <= w[1].date));
    let mut keys: Vec<(String, String)> = p
        .entries
        .iter()
        .map(|e| (e.date.clone(), format!("{:?}", e.sport)))
        .collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), p.entries.len());
    // Ein Tag vor allen Fixtures liefert nichts, ein ungültiges Datum einen Fehler.
    assert!(
        service::progress::progress(&store, "2026-04-01")
            .unwrap()
            .entries
            .is_empty()
    );
    assert!(service::progress::progress(&store, "2026-13-01").is_err());
}

/// T33: Leistungskurve über alle Radeinheiten. Handrechnung aus den Fixtures (siehe
/// `crates/store/tests/zones.rs`): Eine Stunde konstant 200 W liefert den einzigen Wert für
/// 3600 s; die Intervalle 300/100 W liefern 300 W für 1 s. 2 h fährt keine Fixture.
#[test]
fn performance_takes_the_best_ride_per_duration_with_its_origin() {
    let (store, _fit) = imported("performance");
    let items = service::list_activities(&store, &may()).unwrap();
    let p = service::performance::performance(&store, "2026-05-31", "all").unwrap();
    assert_eq!(p.power.len(), lastwerk_core::POWER_CURVE_DURATIONS_S.len());
    assert_eq!(p.from, None);
    let at = |d: u32| p.power.iter().find(|x| x.duration_s == d).unwrap();
    let hour = at(3600).current.as_ref().unwrap();
    assert!(hour.value >= 200.0 - 0.1);
    assert!(at(1).current.as_ref().unwrap().value >= 300.0 - 0.1);
    assert!(at(7200).current.is_none());
    assert!(p.power.iter().all(|x| x.previous_w.is_none()));
    assert!(p.power.iter().all(|x| x.trend == "power_curve.no_data"));
    // Der Stundenwert verweist auf eine Radeinheit des Zeitraums.
    let origin = items.iter().find(|i| i.id == hour.activity_id).unwrap();
    assert_eq!(origin.sport, Sport::Cycling);

    // 42 Tage bis 31. Mai enthalten alle Fixtures, der Vorzeitraum ist leer.
    let p42 = service::performance::performance(&store, "2026-05-31", "42").unwrap();
    assert_eq!(p42.from.as_deref(), Some("2026-04-20"));
    assert_eq!(p42.previous_from.as_deref(), Some("2026-03-09"));
    assert_eq!(
        p42.power.iter().filter(|x| x.current.is_some()).count(),
        p.power.iter().filter(|x| x.current.is_some()).count()
    );
    // Vor den Fixtures: nichts.
    let before = service::performance::performance(&store, "2026-04-01", "90").unwrap();
    assert!(before.power.iter().all(|x| x.current.is_none()));
    assert!(service::performance::performance(&store, "2026-05-31", "7").is_err());
}

/// T37: Bestzeiten im Laufen. Handrechnung in `crates/store/tests/zones.rs`: Der Lauf
/// `run_speed` (3,5 m/s, 2400 s) läuft 1000 m in 286 s und 5000 m in 1429 s, weiter nicht.
#[test]
fn performance_lists_run_bests_with_origin() {
    let (store, _fit) = imported("run-bests");
    let p = service::performance::performance(&store, "2026-05-31", "all").unwrap();
    let distances: Vec<u32> = p.run_bests.iter().map(|r| r.distance_m).collect();
    assert_eq!(distances, lastwerk_core::BEST_EFFORT_DISTANCES_M);
    let km = p.run_bests[0].current.as_ref().unwrap();
    assert_eq!(km.value, 286.0);
    assert_eq!(p.run_bests[1].current.as_ref().unwrap().value, 1429.0);
    assert!(p.run_bests[2].current.is_none());
    assert!(p.run_bests.iter().all(|r| r.previous_s.is_none()));
    let items = service::list_activities(&store, &may()).unwrap();
    let origin = items.iter().find(|i| i.id == km.activity_id).unwrap();
    assert_eq!(origin.sport, Sport::Running);
}

/// T36: Schwellen im Verlauf und FTP-Vorschlag je Monat. Der Vorschlag ist 95 Prozent der besten
/// 20-Minuten-Leistung des Monats (SPEC 8.6); die Fixtures liegen alle im Mai 2026, die beste
/// 20-Minuten-Leistung dort ist mindestens 200 W (Stunde konstant 200 W).
#[test]
fn performance_lists_thresholds_and_monthly_ftp_suggestions() {
    let (mut store, fit) = imported("perf-thresholds");
    service::set_threshold(&mut store, &fit, &ftp(250.0), now()).unwrap();
    let p = service::performance::performance(&store, "2026-05-31", "all").unwrap();
    let ftps: Vec<_> = p
        .thresholds
        .iter()
        .filter(|t| t.kind == ThresholdKind::FtpW)
        .collect();
    assert_eq!(ftps.len(), 1);
    assert_eq!(ftps[0].value, 250.0);
    assert_eq!(ftps[0].origin, "manual");
    assert_eq!(p.ftp_suggestions.len(), 1);
    let s = &p.ftp_suggestions[0];
    assert_eq!(s.month, "2026-05");
    assert!(s.best_20min_w >= 200.0 - 0.1);
    assert!((s.value_w - (s.best_20min_w * 0.95 * 10.0).round() / 10.0).abs() < 0.11);
    // Ein Schwellenwert, der erst nach "heute" gilt, erscheint nicht.
    let earlier = service::performance::performance(&store, "2025-12-31", "all").unwrap();
    assert!(earlier.thresholds.is_empty());
    assert!(earlier.ftp_suggestions.is_empty());
}
