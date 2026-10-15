//! Jede Fixture aus `lastwerk-testkit` liefert die erwartete Aktivität oder den erwarteten
//! Fehlercode (Task T03, SPEC 11.3).

mod common;

use common::code_of;
use lastwerk_core::load::Sport;
use lastwerk_fit::parse_activity;
use lastwerk_testkit::fixtures::{self, FixtureKind, SportName};

/// Toleranz beim Vergleich der Dauer in Sekunden. Die Dauer steht in der Datei in
/// Millisekunden, die Erwartung in ganzen Sekunden; bei Dateien ohne `session` leitet der
/// Parser sie aus den Zeitstempeln ab. Alle Fixtures treffen sie exakt, die Toleranz von
/// einer Sekunde deckt Rundung ab.
const DURATION_TOLERANCE_S: f64 = 1.0;

fn sport_of(name: SportName) -> Sport {
    match name {
        SportName::Cycling => Sport::Cycling,
        SportName::Running => Sport::Running,
        SportName::Swimming => Sport::Swimming,
        SportName::Strength => Sport::Strength,
    }
}

#[test]
fn every_fixture_matches_its_expectation() {
    let all = fixtures::all();
    assert!(!all.is_empty());
    for f in all {
        let e = &f.expect;
        let result = parse_activity(&f.bytes);

        if let Some(code) = e.error {
            let err = result.expect_err(&format!("{}: Fehler erwartet", f.name));
            assert_eq!(err.code(), code_of(code), "{}: Fehlercode", f.name);
            continue;
        }
        assert_eq!(e.kind, FixtureKind::Activity, "{}", f.name);
        let a = result.unwrap_or_else(|err| panic!("{}: {err}", f.name));

        // Sportart: ohne session steht sie nirgends, dann Other
        match e.sport {
            Some(s) => assert_eq!(a.sport, sport_of(s), "{}: Sportart", f.name),
            None => assert_eq!(a.sport, Sport::Other, "{}: Sportart", f.name),
        }
        assert_eq!(
            a.session.total_elapsed_time_s.is_some(),
            e.has_session,
            "{}: session",
            f.name
        );

        let duration = a.duration_s().expect("Dauer");
        let expected = f64::from(e.duration_s.expect("Erwartung mit Dauer"));
        assert!(
            (duration - expected).abs() <= DURATION_TOLERANCE_S,
            "{}: Dauer {duration} statt {expected}",
            f.name
        );

        assert_eq!(a.has_power(), e.has_power, "{}: Leistung", f.name);
        assert_eq!(
            a.has_heart_rate(),
            e.has_heart_rate,
            "{}: Herzfrequenz",
            f.name
        );
        assert_eq!(
            a.samples.len(),
            e.record_count as usize,
            "{}: Anzahl Messpunkte",
            f.name
        );
        assert_eq!(
            Some(a.start_utc.timestamp()),
            e.start_unix_s,
            "{}: Start",
            f.name
        );
        assert_eq!(
            Some(a.local_offset_s),
            e.local_offset_s,
            "{}: Offset",
            f.name
        );
        assert_eq!(
            a.session.pool_length_m, e.pool_length_m,
            "{}: Becken",
            f.name
        );

        // Lücken: kurze und lange zusammen, wie in `Expectation.gap_seconds`
        let p = &a.plausibility;
        assert_eq!(
            p.short_gap_s + p.long_gap_s,
            e.gap_seconds,
            "{}: Lücken",
            f.name
        );
        assert_eq!(
            p.discarded_power_samples, e.power_outliers,
            "{}: Leistungsausreißer",
            f.name
        );
        assert_eq!(
            p.discarded_hr_samples, e.heart_rate_outliers,
            "{}: Herzfrequenzausreißer",
            f.name
        );

        // Raster: streng aufsteigend, beginnt bei 0
        assert_eq!(a.samples.first().map(|s| s.t_s), Some(0), "{}", f.name);
        assert!(
            a.samples.windows(2).all(|w| w[0].t_s < w[1].t_s),
            "{}: t_s nicht streng aufsteigend",
            f.name
        );
    }
}

#[test]
fn unreadable_fixtures_cover_all_four_codes() {
    let mut seen = std::collections::BTreeSet::new();
    for f in fixtures::all() {
        if let Some(code) = f.expect.error {
            seen.insert(code_of(code));
        }
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        vec!["crc_mismatch", "empty_file", "no_activity", "truncated"]
    );
}

#[test]
fn constant_power_fixture_holds_200w_and_140bpm() {
    let a = parse_activity(
        &fixtures::by_name("power_constant_200w_3600s")
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert_eq!(a.samples.len(), 3600);
    assert!(a.samples.iter().all(|s| s.power_w == Some(200.0)));
    assert!(a.samples.iter().all(|s| s.hr_bpm == Some(140.0)));
    // 8 m/s mal 1000 geteilt durch 1000
    assert!(a.samples.iter().all(|s| s.speed_mps == Some(8.0)));
    assert_eq!(a.samples[3599].t_s, 3599);
    assert_eq!(a.laps.len(), 1);
    assert_eq!(a.laps[0].total_elapsed_time_s, Some(3600.0));
    // 8 m/s * 3600 s = 28 800 m
    assert_eq!(a.session.total_distance_m, Some(28_800.0));
    assert_eq!(a.session.total_timer_time_s, Some(3600.0));
    assert_eq!(a.sub_sport.as_deref(), Some("road"));
    assert_eq!(a.device_serial, Some(3_900_000_001));
    assert_eq!(a.device_product.as_deref(), Some("garmin:4242"));
    assert_eq!(a.plausibility, Default::default());
}

#[test]
fn intervals_keep_their_order() {
    let a = parse_activity(
        &fixtures::by_name("power_intervals_100w_300w_30s")
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert_eq!(a.samples[29].power_w, Some(100.0));
    assert_eq!(a.samples[30].power_w, Some(300.0));
}

#[test]
fn gaps_stay_gaps_and_are_counted() {
    let short = parse_activity(&fixtures::by_name("gaps_short").unwrap().bytes).unwrap();
    assert_eq!(short.samples.len(), 1785);
    assert_eq!(short.plausibility.short_gaps, 2);
    assert_eq!(short.plausibility.short_gap_s, 15);
    assert_eq!(short.plausibility.long_gaps, 0);
    // Nichts aufgefüllt: Sekunde 300 fehlt, 299 und 305 sind benachbart
    let at = short.samples.iter().position(|s| s.t_s == 299).unwrap();
    assert_eq!(short.samples[at + 1].t_s, 305);

    let long = parse_activity(&fixtures::by_name("gaps_long").unwrap().bytes).unwrap();
    assert_eq!(long.plausibility.long_gaps, 1);
    assert_eq!(long.plausibility.long_gap_s, 60);
    assert_eq!(long.plausibility.short_gaps, 0);
    assert_eq!(long.samples[599].t_s, 599);
    assert_eq!(long.samples[600].t_s, 660);
}

#[test]
fn outliers_stay_in_samples_but_are_counted() {
    let a = parse_activity(&fixtures::by_name("spikes_dropouts").unwrap().bytes).unwrap();
    assert_eq!(a.samples[600].power_w, Some(3000.0));
    assert_eq!(a.samples[700].hr_bpm, Some(0.0));
    assert_eq!(a.samples[800].hr_bpm, Some(250.0));
    assert_eq!(a.plausibility.discarded_power_samples, 1);
    assert_eq!(a.plausibility.discarded_hr_samples, 2);
    assert!(a.plausibility.power_flagged());
}

#[test]
fn pair_files_differ_in_start_and_power() {
    let edge = parse_activity(&fixtures::by_name("pair_edge").unwrap().bytes).unwrap();
    let watch = parse_activity(&fixtures::by_name("pair_watch").unwrap().bytes).unwrap();
    assert_eq!(
        (watch.start_utc - edge.start_utc).num_seconds(),
        i64::from(fixtures::PAIR_OFFSET_S)
    );
    assert!(edge.has_power());
    assert!(!watch.has_power());
    assert_eq!(watch.device_product.as_deref(), Some("garmin:4343"));
    assert_eq!(watch.device_serial, Some(3_900_000_002));
}

#[test]
fn sports_are_mapped() {
    let sport = |name: &str| {
        parse_activity(&fixtures::by_name(name).unwrap().bytes)
            .unwrap()
            .sport
    };
    assert_eq!(sport("run_speed"), Sport::Running);
    assert_eq!(sport("swim_pool_25m"), Sport::Swimming);
    assert_eq!(sport("strength"), Sport::Strength);
    assert_eq!(sport("power_constant_200w_3600s"), Sport::Cycling);
    let run = parse_activity(&fixtures::by_name("run_speed").unwrap().bytes).unwrap();
    assert_eq!(run.samples[0].speed_mps, Some(3.5));
    assert!(!run.has_power());
}

#[test]
fn no_session_file_derives_start_and_duration_from_records() {
    let a = parse_activity(&fixtures::by_name("no_session").unwrap().bytes).unwrap();
    assert_eq!(a.session, Default::default());
    assert!(a.laps.is_empty());
    assert_eq!(a.duration_s(), Some(1800.0));
    assert_eq!(a.start_utc.timestamp(), fixtures::START_UNIX);
}

#[test]
fn fixtures_are_read_identically_twice() {
    for f in fixtures::all() {
        assert_eq!(
            parse_activity(&f.bytes),
            parse_activity(&f.bytes),
            "{}",
            f.name
        );
    }
}
