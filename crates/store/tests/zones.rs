//! Zeit in Zonen, Bestleistungen und Neuberechnung nach Schwellen gegen die synthetischen
//! Fixtures (SPEC 6.2, 8.5, 8.6, 12 M2). Alle Dateien liegen unter `target/`.

mod common;

use std::collections::BTreeMap;
use std::fs;

use common::{add_thresholds, date, import, now, threshold, work_dir, write_fixtures};
use lastwerk_core::{LoadConfig, Sport, check_plausibility};
use lastwerk_fit::parse_activity;
use lastwerk_store::{
    ImportOptions, LoadStatus, SETTING_ATHLETE_BIRTH_YEAR, Store, ThresholdKind,
    embedded_zone_catalog,
};

/// Aktivitäts-ID zu einem Dateinamen der Fixtures.
fn id_of(store: &Store, name: &str) -> i64 {
    let file = store
        .source_files()
        .unwrap()
        .into_iter()
        .find(|f| f.row.rel_path == name)
        .unwrap_or_else(|| panic!("Datei {name} fehlt"));
    store
        .activities()
        .unwrap()
        .into_iter()
        .find(|a| a.row.source_file_id == file.id)
        .unwrap_or_else(|| panic!("Aktivität zu {name} fehlt"))
        .id
}

/// Sekunden je Zone (Index 0 ist Zone 1) eines Schemas einer Aktivität.
fn zones_of(store: &Store, activity_id: i64, scheme: &str) -> Vec<u32> {
    store
        .zone_times()
        .unwrap()
        .into_iter()
        .filter(|z| z.activity_id == activity_id && z.scheme == scheme)
        .map(|z| z.seconds)
        .collect()
}

fn schemes_of(store: &Store, activity_id: i64) -> Vec<String> {
    let mut schemes: Vec<String> = store
        .zone_times()
        .unwrap()
        .into_iter()
        .filter(|z| z.activity_id == activity_id)
        .map(|z| z.scheme)
        .collect();
    schemes.dedup();
    schemes
}

/// Handrechnung: `power_constant_200w_3600s` fährt 3600 s mit 200 W, FTP 250. Anteil
/// 200 / 250 = 0,8, das liegt zwischen 0,75 und 0,90, also in Zone 3 von `power7`. Alle 3600 s
/// dort, sechs weitere Zonen mit 0.
#[test]
fn constant_200_w_at_ftp_250_spends_3600_s_in_zone_3() {
    let dir = work_dir("zone3");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    import(&mut store, &fix);
    let id = id_of(&store, "power_constant_200w_3600s.fit");
    assert_eq!(zones_of(&store, id, "power7"), vec![0, 0, 3600, 0, 0, 0, 0]);
}

/// Zeit in Zonen summiert sich je Aktivität und Schema auf die Zahl der Samples mit Kanal
/// (nach den Plausibilitätsregeln), Lücken zählen nicht (SPEC 8.5, Abnahme T05).
#[test]
fn zone_time_sums_to_the_samples_with_channel() {
    let dir = work_dir("zone_sums");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    import(&mut store, &fix);

    let files = store.source_files().unwrap();
    let activities = store.activities().unwrap();
    let mut sums: BTreeMap<(i64, String), u32> = BTreeMap::new();
    for z in store.zone_times().unwrap() {
        *sums.entry((z.activity_id, z.scheme)).or_default() += z.seconds;
    }
    assert!(!sums.is_empty());

    for ((activity_id, scheme), total) in &sums {
        let activity = activities.iter().find(|a| a.id == *activity_id).unwrap();
        let file = files
            .iter()
            .find(|f| f.id == activity.row.source_file_id)
            .unwrap();
        let raw = parse_activity(&fs::read(fix.join(&file.row.rel_path)).unwrap()).unwrap();
        let prepared = check_plausibility(&raw.samples, &LoadConfig::default()).unwrap();
        let samples = prepared.segments.iter().flatten();
        let expected = match scheme.as_str() {
            "power7" => samples.filter(|s| s.power_w.is_some()).count(),
            "hr_lthr5" | "hr_karvonen" => samples.filter(|s| s.hr_bpm.is_some()).count(),
            "run_speed5" | "swim_css5" => samples.filter(|s| s.speed_mps.is_some()).count(),
            other => panic!("unbekanntes Schema {other}"),
        };
        assert_eq!(
            *total as usize, expected,
            "{} ({scheme})",
            file.row.rel_path
        );
    }
}

/// Je Sportart gelten nur die Schemata aus dem Katalog, und nur wenn Bezugswert und Kanal da
/// sind. Mit den Schwellen aus `add_thresholds` (FTP, LTHR, Ruhepuls, Maximalpuls, Laufschwelle,
/// CSS):
///
/// - Rad mit Leistung und Herzfrequenz: `power7`, `hr_lthr5`, `hr_karvonen`.
/// - Rad ohne Leistung (`no_power`): nur die Herzfrequenzschemata.
/// - Rad ohne Herzfrequenz (`no_heart_rate`): nur `power7`.
/// - Lauf: `hr_lthr5`, `hr_karvonen`, `run_speed5`.
/// - Schwimmen ohne Herzfrequenz: nur `swim_css5`.
/// - Kraft mit Herzfrequenz: nur die Herzfrequenzschemata.
#[test]
fn schemes_follow_sport_channels_and_thresholds() {
    let dir = work_dir("schemes");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    import(&mut store, &fix);
    let schemes = |name: &str| schemes_of(&store, id_of(&store, name));

    assert_eq!(
        schemes("power_constant_200w_3600s.fit"),
        ["hr_karvonen", "hr_lthr5", "power7"]
    );
    assert_eq!(schemes("no_power.fit"), ["hr_karvonen", "hr_lthr5"]);
    assert_eq!(schemes("no_heart_rate.fit"), ["power7"]);
    assert_eq!(
        schemes("run_speed.fit"),
        ["hr_karvonen", "hr_lthr5", "run_speed5"]
    );
    assert_eq!(schemes("swim_pool_25m.fit"), ["swim_css5"]);
    assert_eq!(schemes("strength.fit"), ["hr_karvonen", "hr_lthr5"]);
}

/// Ohne Schwellenwerte entstehen keine Zonenzeilen (nicht berechenbar, keine Nullen), die
/// Bestleistungen hängen nicht von Schwellen ab und sind trotzdem da.
#[test]
fn without_thresholds_there_is_no_zone_time_but_best_power() {
    let dir = work_dir("zone_no_thresholds");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    import(&mut store, &fix);
    assert!(store.zone_times().unwrap().is_empty());
    let id = id_of(&store, "power_constant_200w_3600s.fit");
    let best: Vec<_> = store
        .best_powers()
        .unwrap()
        .into_iter()
        .filter(|b| b.activity_id == id)
        .collect();
    // Raster von 1 s bis 2 h (T33), eine Stunde Fahrt: alle Dauern bis 3600 s, also 20.
    assert_eq!(best.len(), 20);
}

/// Handrechnung: konstant 200 W über 3600 s ergibt für jede Dauer des Rasters bis 3600 s je
/// 200 W (seit T33 das Raster `POWER_CURVE_DURATIONS_S`, 5400 und 7200 s fehlen: zu kurz).
/// Wechsel 300/100 W im 30-s-Takt: 1, 5 und 30 s geben 300 (ein ganzer Block), 60 s gibt
/// `(30 * 300 + 30 * 100) / 60 = 200`. Aktivitäten ohne Leistung haben keine Zeilen.
#[test]
fn best_power_is_stored_for_the_fixed_durations() {
    let dir = work_dir("best_power");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    import(&mut store, &fix);
    let powers = store.best_powers().unwrap();
    let of = |id: i64| -> Vec<(u32, f64)> {
        powers
            .iter()
            .filter(|b| b.activity_id == id)
            .map(|b| (b.duration_s, b.watts))
            .collect()
    };

    let constant = of(id_of(&store, "power_constant_200w_3600s.fit"));
    assert_eq!(
        constant.iter().map(|b| b.0).collect::<Vec<_>>(),
        lastwerk_core::POWER_CURVE_DURATIONS_S
            .iter()
            .copied()
            .filter(|d| *d <= 3600)
            .collect::<Vec<_>>()
    );
    assert!(constant.iter().all(|b| (b.1 - 200.0).abs() < 1e-3));

    let intervals = of(id_of(&store, "power_intervals_100w_300w_30s.fit"));
    let get = |d: u32| intervals.iter().find(|b| b.0 == d).unwrap().1;
    assert!((get(5) - 300.0).abs() < 1e-3);
    assert!((get(60) - 200.0).abs() < 1e-3);
    assert!((get(1) - 300.0).abs() < 1e-3);
    assert!((get(30) - 300.0).abs() < 1e-3);

    // 1800 s Fahrt: es gibt keinen Wert für 3600 s.
    let ramp = of(id_of(&store, "power_ramp_100w_300w.fit"));
    assert!(ramp.iter().all(|b| b.0 != 3600));
    assert!(ramp.iter().any(|b| b.0 == 1200));

    assert!(of(id_of(&store, "no_power.fit")).is_empty());

    // Bestwert über 20 Minuten unter allen zählenden Radfahrten: Die Rampe von 100 auf 300 W
    // liegt am Ende bei hohen Werten und schlägt die 200-W-Fahrten.
    let peak = store
        .best_power_peak(Sport::Cycling, 1200)
        .unwrap()
        .unwrap();
    assert_eq!(peak.activity_id, id_of(&store, "power_ramp_100w_300w.fit"));
    assert!(peak.watts > 200.0 && peak.watts < 300.0, "{peak:?}");
    assert_eq!(store.best_power_peak(Sport::Swimming, 1200).unwrap(), None);
}

/// Fehlt die maximale Herzfrequenz, gilt `208 - 0,7 * Alter` (Q7). Geburtsjahr 1986, Fahrt im
/// Mai 2026: Alter 40, HR_max = 180 (T-HRMAX-1), als Schätzung gekennzeichnet. Reserve
/// 180 - 50 = 130, die Fixture fährt mit 140 bpm: (140 - 50) / 130 = 0,69, Zone 2 der
/// Karvonen-Zonen (bis 0,70). Ohne Geburtsjahr gibt es dieses Schema nicht, aber keinen Fehler.
#[test]
fn missing_hr_max_is_estimated_from_the_birth_year_and_flagged() {
    let dir = work_dir("hr_max_estimate");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    let from = date(2026, 1, 1);
    for t in [
        threshold(None, ThresholdKind::HrRestBpm, 50.0, from),
        threshold(None, ThresholdKind::LthrBpm, 160.0, from),
    ] {
        store.insert_threshold(&t).unwrap();
    }
    let report = import(&mut store, &fix);
    assert_eq!(report.errors.len(), 3);
    let id = id_of(&store, "power_constant_200w_3600s.fit");
    assert!(zones_of(&store, id, "hr_karvonen").is_empty());
    assert!(!zones_of(&store, id, "hr_lthr5").is_empty());

    store
        .set_setting(SETTING_ATHLETE_BIRTH_YEAR, "1986")
        .unwrap();
    let hr_max = store
        .effective_hr_max(Sport::Cycling, date(2026, 5, 11))
        .unwrap()
        .unwrap();
    assert!((hr_max.bpm - 180.0).abs() < 1e-3);
    assert!(hr_max.estimated);

    // Die Zonen entstehen beim Import, ein Wiederaufbau rechnet sie mit der Schätzung.
    store
        .rebuild_cache(&fix, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();
    let id = id_of(&store, "power_constant_200w_3600s.fit");
    assert_eq!(zones_of(&store, id, "hr_karvonen"), vec![0, 3600, 0, 0, 0]);
    let rows: Vec<_> = store
        .zone_times()
        .unwrap()
        .into_iter()
        .filter(|z| z.activity_id == id)
        .collect();
    for z in &rows {
        assert_eq!(z.estimated, z.scheme == "hr_karvonen", "{z:?}");
    }

    // Ein gemessener Wert hat Vorrang und ist keine Schätzung.
    store
        .insert_threshold(&threshold(None, ThresholdKind::HrMaxBpm, 190.0, from))
        .unwrap();
    let measured = store
        .effective_hr_max(Sport::Cycling, date(2026, 5, 11))
        .unwrap()
        .unwrap();
    assert!((measured.bpm - 190.0).abs() < 1e-3);
    assert!(!measured.estimated);
}

/// Ein Geburtsjahr nach dem Tag der Fahrt oder kein Jahr ist keine Altersangabe.
#[test]
fn implausible_birth_year_gives_no_estimate() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .insert_threshold(&threshold(
            None,
            ThresholdKind::HrRestBpm,
            50.0,
            date(2026, 1, 1),
        ))
        .unwrap();
    for bad in ["2030", "1800", "abc", ""] {
        store.set_setting(SETTING_ATHLETE_BIRTH_YEAR, bad).unwrap();
        assert_eq!(
            store
                .effective_hr_max(Sport::Cycling, date(2026, 5, 11))
                .unwrap(),
            None,
            "{bad:?}"
        );
    }
}

/// Eine Schwelle mit späterem Datum ändert frühere Lasten nicht (SPEC 3, Abnahme T05).
///
/// FTP 250 ab 1. Januar, dazu FTP 300 ab 15. Mai. Fahrten vom 11. bis 14. Mai behalten ihre
/// Last, ihre Zonen und die Schwelle, aus der sie stammen. Fahrten ab 15. Mai werden neu
/// gerechnet. Handrechnung: `IF = NP / FTP` und `TSS = t * NP * IF / (FTP * 3600) * 100`,
/// also `TSS` proportional zu `1 / FTP^2` bei gleicher NP. Neue Last = alte Last *
/// `(250 / 300)^2` = alte Last * 0,69444. `gaps_short` fährt mit 200 W, der Anteil an der
/// neuen FTP ist 200 / 300 = 0,67, das ist Zone 2 von `power7` statt Zone 3.
#[test]
fn a_later_threshold_leaves_earlier_loads_untouched() {
    let dir = work_dir("later_threshold");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    import(&mut store, &fix);

    let early = [
        "power_constant_200w_3600s.fit",
        "power_intervals_100w_300w_30s.fit",
        "power_ramp_100w_300w.fit",
        "hr_drift_130_160.fit",
    ];
    let early_ids: Vec<i64> = early.iter().map(|n| id_of(&store, n)).collect();
    let late_id = id_of(&store, "gaps_short.fit");
    let before_loads = store.activity_loads().unwrap();
    let before_zones = store.zone_times().unwrap();

    let new_ftp = store
        .insert_threshold(&threshold(
            Some(Sport::Cycling),
            ThresholdKind::FtpW,
            300.0,
            date(2026, 5, 15),
        ))
        .unwrap();
    assert_eq!(store.threshold_valid_until(new_ftp).unwrap(), None);
    let later = now() + chrono::TimeDelta::days(1);
    let report = store
        .recompute_loads(
            &fix,
            Some(Sport::Cycling),
            date(2026, 5, 15),
            None,
            later,
            &ImportOptions::default(),
        )
        .unwrap();
    // Radfahrten ab 15. Mai: gaps_short (15.), spikes_dropouts (16.), pair_edge und
    // pair_watch (17.), no_power (18.), no_heart_rate (19.).
    assert_eq!(report.candidates, 6, "{report:?}");
    assert_eq!(report.recomputed, 6);
    assert_eq!(report.file_missing + report.file_changed, 0);

    let after_loads = store.activity_loads().unwrap();
    let after_zones = store.zone_times().unwrap();
    for id in &early_ids {
        let b = before_loads.iter().find(|l| l.activity_id == *id).unwrap();
        let a = after_loads.iter().find(|l| l.activity_id == *id).unwrap();
        assert_eq!(a, b, "frühere Last unverändert");
        assert_eq!(
            after_zones
                .iter()
                .filter(|z| z.activity_id == *id)
                .collect::<Vec<_>>(),
            before_zones
                .iter()
                .filter(|z| z.activity_id == *id)
                .collect::<Vec<_>>(),
            "frühere Zonen unverändert"
        );
    }

    // Die Fahrt vom 15. Mai nimmt die neue Schwelle.
    let b = before_loads
        .iter()
        .find(|l| l.activity_id == late_id)
        .unwrap();
    let a = after_loads
        .iter()
        .find(|l| l.activity_id == late_id)
        .unwrap();
    assert_eq!(a.status, LoadStatus::Ok);
    assert!((a.value.unwrap() - b.value.unwrap() * (250.0_f64 / 300.0).powi(2)).abs() < 0.01);
    assert_eq!(a.threshold_id, Some(new_ftp));
    assert_eq!(a.computed_at, later);
    let zones = zones_of(&store, late_id, "power7");
    assert_eq!(
        zones[1],
        zones.iter().sum::<u32>(),
        "alles in Zone 2: {zones:?}"
    );

    // Die erste FTP gilt jetzt bis zum 14. Mai.
    let first_ftp = b.threshold_id.unwrap();
    assert_eq!(
        store.threshold_valid_until(first_ftp).unwrap(),
        Some(date(2026, 5, 14))
    );
}

/// Ein Neuberechnen mit einem Ordner ohne die Dateien verändert nichts und meldet die
/// fehlenden Dateien. Ein geänderter Inhalt wird nicht stillschweigend genutzt.
#[test]
fn recompute_skips_missing_and_changed_files() {
    let dir = work_dir("recompute_skips");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    add_thresholds(&mut store);
    import(&mut store, &fix);
    let before = (store.activity_loads().unwrap(), store.zone_times().unwrap());

    let empty = dir.join("empty");
    fs::create_dir_all(&empty).unwrap();
    let opts = ImportOptions::default();
    let r = store
        .recompute_loads(&empty, None, date(2026, 1, 1), None, now(), &opts)
        .unwrap();
    assert!(r.candidates > 0);
    assert_eq!(r.file_missing, r.candidates);
    assert_eq!(r.recomputed, 0);

    // Eine Datei mit anderem Inhalt unter demselben Namen: andere Prüfsumme, übersprungen.
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::copy(
        fix.join("no_heart_rate.fit"),
        other.join("power_constant_200w_3600s.fit"),
    )
    .unwrap();
    let r = store
        .recompute_loads(
            &other,
            Some(Sport::Cycling),
            date(2026, 5, 11),
            Some(date(2026, 5, 11)),
            now(),
            &opts,
        )
        .unwrap();
    assert_eq!((r.candidates, r.file_changed, r.recomputed), (1, 1, 0));

    assert_eq!(
        before,
        (store.activity_loads().unwrap(), store.zone_times().unwrap())
    );
    // Kein Ordner ist ein Eingabefehler.
    assert!(
        store
            .recompute_loads(
                &dir.join("nope"),
                None,
                date(2026, 1, 1),
                None,
                now(),
                &opts
            )
            .is_err()
    );
}

/// Eine Schwelle nachträglich zu setzen und neu zu rechnen ergibt dieselben Zahlen wie ein
/// Import mit der Schwelle von Anfang an.
#[test]
fn recompute_after_setting_a_threshold_equals_import_with_it() {
    let dir = work_dir("recompute_equals_import");
    let fix = write_fixtures(&dir);

    let mut direct = Store::open_in_memory().unwrap();
    add_thresholds(&mut direct);
    import(&mut direct, &fix);

    let mut late = Store::open_in_memory().unwrap();
    import(&mut late, &fix);
    assert!(late.zone_times().unwrap().is_empty());
    add_thresholds(&mut late);
    late.recompute_loads(
        &fix,
        None,
        date(2026, 1, 1),
        None,
        now(),
        &ImportOptions::default(),
    )
    .unwrap();

    assert_eq!(
        direct.activity_loads().unwrap(),
        late.activity_loads().unwrap()
    );
    assert_eq!(direct.zone_times().unwrap(), late.zone_times().unwrap());
    assert_eq!(direct.best_powers().unwrap(), late.best_powers().unwrap());
    assert_eq!(
        direct
            .daily_loads(date(2026, 1, 1), date(2026, 12, 31))
            .unwrap(),
        late.daily_loads(date(2026, 1, 1), date(2026, 12, 31))
            .unwrap()
    );
}

#[test]
fn thresholds_can_be_listed_and_the_first_load_day_is_known() {
    let dir = work_dir("list_thresholds");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(store.first_load_date().unwrap(), None);
    add_thresholds(&mut store);
    import(&mut store, &fix);
    let list = store.list_thresholds().unwrap();
    assert_eq!(list.len(), 7);
    let ftp = list.iter().find(|t| t.kind == ThresholdKind::FtpW).unwrap();
    assert_eq!(ftp.sport, Some(Sport::Cycling));
    assert_eq!(ftp.valid_from, date(2026, 1, 1));
    assert!((ftp.value - 250.0).abs() < 1e-9);
    assert!(list.iter().any(|t| t.sport.is_none()));
    // Erste zählende Last: 11. Mai (Tag 1 nach dem 10. Mai), die 200-W-Fahrt.
    assert!(store.first_load_date().unwrap().is_some());
}

#[test]
fn embedded_catalog_matches_the_repository_file() {
    let from_repo = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../catalog/zones.json"
    ))
    .unwrap();
    let parsed = lastwerk_store::parse_zone_catalog(&from_repo).unwrap();
    assert_eq!(parsed, embedded_zone_catalog());
}

/// T37, Handrechnung: Der Lauf `run_speed` hat 2400 s mit 3,5 m/s, also 8400 m. 1000 m dauern
/// 1000 / 3,5 = 285,7 s, auf ganze Sekunden nach oben 286 s; 5000 m dauern 1428,6 s, also
/// 1429 s. 10 km und mehr erreicht er nicht. Radeinheiten bekommen keine Bestzeiten.
const RUN_1000M_S: u32 = 286;
const RUN_5000M_S: u32 = 1429;

#[test]
fn best_efforts_are_stored_for_runs_only() {
    let dir = work_dir("best_effort");
    let fix = write_fixtures(&dir);
    let mut store = Store::open_in_memory().unwrap();
    import(&mut store, &fix);
    let run = id_of(&store, "run_speed.fit");
    let efforts: Vec<(u32, u32)> = store
        .best_efforts()
        .unwrap()
        .into_iter()
        .filter(|e| e.activity_id == run)
        .map(|e| (e.distance_m, e.seconds))
        .collect();
    assert_eq!(efforts, [(1000, RUN_1000M_S), (5000, RUN_5000M_S)]);
    let ride = id_of(&store, "power_constant_200w_3600s.fit");
    assert!(
        store
            .best_efforts()
            .unwrap()
            .iter()
            .all(|e| e.activity_id != ride)
    );
}
