//! `get_dashboard` gegen eine echte Datenbank mit den synthetischen Fixtures aus
//! `lastwerk-testkit` (Mai 2026, siehe `tests/service.rs`). Alle Dateien liegen unter `target/`,
//! es gibt keine echten Daten.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use lastwerk_app::dto::{Dashboard, GeneratePlanInput, PlanLocale, PlanSport, ThresholdInput};
use lastwerk_app::service::{self, dashboard::dashboard, plans};
use lastwerk_core::Sport;
use lastwerk_store::{PlanStatus, RecoveryDayRecord, Store, ThresholdKind};
use lastwerk_testkit::fixtures;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("app-dashboard-tests")
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

/// Datenbank mit importierten Fixtures (10. bis etwa 20. Mai 2026) und FTP 250 W.
fn with_fixtures(name: &str) -> Store {
    let dir = work_dir(name);
    let fit = dir.join("fit");
    fs::create_dir_all(&fit).unwrap();
    for f in fixtures::all() {
        fs::write(fit.join(f.file_name()), &f.bytes).unwrap();
    }
    let mut store = Store::open(&dir.join("lastwerk.db")).unwrap();
    service::import(&mut store, &fit, now(), &mut |_| {}).unwrap();
    service::set_threshold(
        &mut store,
        &fit,
        &ThresholdInput {
            sport: Some(Sport::Cycling),
            kind: ThresholdKind::FtpW,
            value: 250.0,
            valid_from: "2026-01-01".into(),
            note: None,
        },
        now(),
    )
    .unwrap();
    store
}

fn recovery(
    day: NaiveDate,
    hrv: Option<f64>,
    rhr: Option<f64>,
    sleep_h: Option<f64>,
) -> RecoveryDayRecord {
    RecoveryDayRecord {
        date: day,
        hrv_overnight_ms: hrv,
        resting_hr_bpm: rhr,
        sleep_duration_s: sleep_h.map(|h| (h * 3600.0) as u32),
        sleep_score: None,
        body_battery_high: None,
        body_battery_low: None,
        source: "test".into(),
        imported_at: now(),
    }
}

fn get(store: &Store, today: &str) -> Dashboard {
    dashboard(store, today).unwrap()
}

// --- Leere Datenbank -------------------------------------------------------------------

#[test]
fn an_empty_database_gives_codes_and_empty_series_but_no_invented_numbers() {
    let store = Store::open_in_memory().unwrap();
    let d = get(&store, "2026-06-10");
    assert_eq!(d.today, "2026-06-10");
    assert!(d.form.is_none());
    assert!(d.pmc.is_empty() && d.projection.is_empty());
    assert_eq!(d.pmc_first_load_date, None);
    assert!(d.plan.is_none());

    assert_eq!(d.ramp.current, None);
    assert_eq!(d.ramp.state, "ramp.no_data");
    assert!(d.ramp.series.is_empty());
    // Grenze aus R1 ist eine Regel, keine Messung, und bleibt sichtbar.
    assert!(d.ramp.limit > 0.0);

    assert_eq!(d.distribution.state, "distribution.no_data");
    assert_eq!(d.distribution.window_easy_percent, None);
    assert_eq!(d.distribution.weeks.len(), 8);
    assert!(
        d.distribution
            .weeks
            .iter()
            .all(|w| w.easy_percent.is_none() && w.total_s == 0)
    );
    assert_eq!(d.distribution.min_easy_percent, 70.0);

    assert_eq!(d.adherence.state, "adherence.no_data");
    assert!(d.adherence.weeks.is_empty());

    assert_eq!(d.efficiency.trend, "efficiency.no_data");
    assert_eq!((d.efficiency.current, d.efficiency.sessions), (None, 0));
    assert!(d.efficiency.series.is_empty());

    assert_eq!(d.power_curve.durations.len(), 5);
    assert!(d.power_curve.durations.iter().all(|p| {
        p.current_w.is_none() && p.previous_w.is_none() && p.trend == "power_curve.no_data"
    }));

    assert_eq!(d.readiness.state, "readiness.no_data");
    assert!(d.readiness.hrv.is_none() && d.readiness.resting_hr.is_none());
    assert!(d.readiness.sleep_h.is_none() && d.readiness.series.is_empty());
    assert_eq!(d.readiness.last_date, None);
    assert!(d.readiness.body_battery_high.is_none() && d.readiness.body_battery_low.is_none());
    assert!(!d.readiness.plan_adjusted);

    assert!(d.load_days.is_empty());
}

#[test]
fn an_invalid_day_is_rejected_with_a_code() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(
        dashboard(&store, "morgen").unwrap_err().code,
        "invalid_date"
    );
}

// --- Daten ohne Schwelle ----------------------------------------------------------------

#[test]
fn activities_without_a_threshold_have_no_load_and_therefore_no_form() {
    let dir = work_dir("no-threshold");
    let fit = dir.join("fit");
    fs::create_dir_all(&fit).unwrap();
    for f in fixtures::all() {
        fs::write(fit.join(f.file_name()), &f.bytes).unwrap();
    }
    let mut store = Store::open(&dir.join("lastwerk.db")).unwrap();
    service::import(&mut store, &fit, now(), &mut |_| {}).unwrap();
    let d = get(&store, "2026-05-31");
    assert!(d.form.is_none() && d.pmc.is_empty() && d.load_days.is_empty());
    // Zeit in Zonen und Bestwerte brauchen keine Last: die Verteilung rechnet trotzdem.
    assert!(
        d.distribution.weeks.iter().any(|w| w.total_s > 0) || d.distribution.window_total_s == 0
    );
}

// --- Daten ------------------------------------------------------------------------------

#[test]
fn form_ramp_calendar_and_rounding_come_from_the_loads() {
    let store = with_fixtures("form");
    let d = get(&store, "2026-05-31");
    let form = d.form.as_ref().expect("Last und Schwelle vorhanden");
    assert!(
        form.ctl > 0.0 && form.atl > 0.0,
        "Last ist eingeflossen: {form:?}"
    );
    assert!(form.state.starts_with("form."));
    // TSB ist CTL minus ATL des Vortags (SPEC 8.2): am letzten Tag stimmt es mit dem Verlauf.
    let last = d.pmc.last().unwrap();
    assert_eq!(last.date, "2026-05-31");
    assert!((f64::from(last.tsb) - form.tsb).abs() < 1e-6);
    // Fenster: 120 Tage bis heute, lückenlos.
    assert_eq!(d.pmc.len(), 120);
    assert_eq!(d.pmc.first().unwrap().date, "2026-02-01");
    // Eine Nachkommastelle in allen Zahlen.
    let one_decimal = |v: f64| ((v * 10.0).round() - v * 10.0).abs() < 1e-3;
    for p in &d.pmc {
        for v in [p.ctl, p.atl, p.tsb, p.load] {
            assert!(one_decimal(f64::from(v)), "{v}");
        }
    }
    assert_eq!(d.pmc_first_load_date.as_deref(), Some("2026-05-10"));
    assert!(d.projection.is_empty(), "ohne Plan keine Prognose");

    // Rampe: CTL wächst nach dem Start, 84 Tage Reihe, Code aus core.
    assert!(d.ramp.current.is_some());
    assert!(d.ramp.state.starts_with("ramp."));
    assert!(d.ramp.series.len() <= 84 && !d.ramp.series.is_empty());

    // Kalender: nur Tage mit Last, nie 0.
    assert!(!d.load_days.is_empty());
    assert!(d.load_days.iter().all(|l| l.load > 0.0));
    assert!(d.load_days.windows(2).all(|w| w[0].date < w[1].date));
    assert!(d.load_days.first().unwrap().date.as_str() >= "2026-05-10");
}

#[test]
fn the_form_stops_today_and_a_day_before_the_first_load_gives_nothing() {
    let store = with_fixtures("form-early");
    let d = get(&store, "2026-05-09");
    assert!(
        d.form.is_none(),
        "vor dem ersten Tag mit Last gibt es nichts zu zeigen"
    );
    let mid = get(&store, "2026-05-12");
    assert_eq!(mid.pmc.last().unwrap().date, "2026-05-12");
}

#[test]
fn distribution_uses_zone_time_with_one_scheme_per_activity() {
    let store = with_fixtures("distribution");
    let d = get(&store, "2026-05-31");
    assert_eq!(d.distribution.weeks.len(), 8);
    // Laufende Woche zuletzt: Montag 25. Mai.
    assert_eq!(
        d.distribution.weeks.last().unwrap().week_start,
        "2026-05-25"
    );
    assert_eq!(
        d.distribution.weeks.first().unwrap().week_start,
        "2026-04-06"
    );
    assert!(
        d.distribution.window_total_s > 0,
        "Radeinheiten mit Zonenzeit im Fenster"
    );
    let p = d.distribution.window_easy_percent.unwrap();
    assert!((0.0..=100.0).contains(&p));
    assert!(d.distribution.state.starts_with("distribution."));
    // Wochen ohne Zeit haben keinen Anteil, nicht 0.
    for w in &d.distribution.weeks {
        assert_eq!(w.easy_percent.is_none(), w.total_s == 0);
    }
}

#[test]
fn power_curve_compares_the_period_with_the_one_before() {
    let store = with_fixtures("power");
    // Heute 5. August: Zeitraum ab 8. Mai (90 Tage), Vorzeitraum 9. Februar bis 7. Mai. Alle
    // Fixtures liegen ab 10. Mai, also nur im Zeitraum: kein Vergleich, aber Werte.
    let d = get(&store, "2026-08-05");
    let hour = d
        .power_curve
        .durations
        .iter()
        .find(|p| p.duration_s == 3600)
        .unwrap();
    assert!(hour.current_w.is_some());
    assert_eq!(hour.previous_w, None);
    assert_eq!(hour.trend, "power_curve.no_data");
    assert_eq!(hour.change_percent, None);
    // Später: die Fahrten liegen im Vorzeitraum, der Zeitraum ist leer.
    let later = get(&store, "2026-10-01");
    let hour = later
        .power_curve
        .durations
        .iter()
        .find(|p| p.duration_s == 3600)
        .unwrap();
    assert_eq!(hour.current_w, None);
    assert!(hour.previous_w.is_some());
    assert_eq!(hour.trend, "power_curve.no_data");
}

#[test]
fn readiness_uses_the_own_baseline_and_flags_a_clear_drop() {
    let mut store = Store::open_in_memory().unwrap();
    let today = date(2026, 6, 10);
    // Sieben Tage Basis (HRV 50, Ruhepuls 50, Schlaf 8 h), dann ein schlechter Tag.
    let mut days: Vec<RecoveryDayRecord> = (1..=7)
        .map(|n| {
            recovery(
                today - chrono::TimeDelta::days(n),
                Some(50.0),
                Some(50.0),
                Some(8.0),
            )
        })
        .collect();
    days.push(recovery(today, Some(40.0), Some(55.0), Some(6.0)));
    store.upsert_recovery_days(&days).unwrap();

    let d = get(&store, "2026-06-10");
    let r = &d.readiness;
    assert_eq!(r.state, "readiness.low");
    assert_eq!(r.date.as_deref(), Some("2026-06-10"));
    // HRV: (40 - 50) / 50 = -20 Prozent, Basis 50.
    let hrv = r.hrv.as_ref().unwrap();
    assert_eq!(
        (hrv.value, hrv.baseline, hrv.deviation_percent),
        (40.0, Some(50.0), Some(-20.0))
    );
    // Ruhepuls: (55 - 50) / 50 = +10 Prozent, wird angezeigt, löst aber nichts aus (R12).
    let rhr = r.resting_hr.as_ref().unwrap();
    assert_eq!(rhr.deviation_percent, Some(10.0));
    // Schlaf: (6 - 8) / 8 = -25 Prozent.
    let sleep = r.sleep_h.as_ref().unwrap();
    assert_eq!(
        (sleep.value, sleep.baseline, sleep.deviation_percent),
        (6.0, Some(8.0), Some(-25.0))
    );
    assert_eq!(r.series.len(), 8);
    assert_eq!(r.last_date.as_deref(), Some("2026-06-10"));
    assert!(!r.plan_adjusted, "ohne Plan hat niemand etwas ersetzt");
    assert!(r.body_battery_high.is_none() && r.body_battery_low.is_none());
}

#[test]
fn readiness_is_normal_within_the_threshold_and_missing_with_a_short_baseline() {
    let mut store = Store::open_in_memory().unwrap();
    let today = date(2026, 6, 10);
    let mut days: Vec<RecoveryDayRecord> = (1..=7)
        .map(|n| {
            recovery(
                today - chrono::TimeDelta::days(n),
                Some(50.0),
                Some(50.0),
                Some(8.0),
            )
        })
        .collect();
    days.push(recovery(today, Some(48.0), Some(51.0), Some(7.8)));
    store.upsert_recovery_days(&days).unwrap();
    let d = get(&store, "2026-06-10");
    assert_eq!(d.readiness.state, "readiness.normal");

    // Nur zwei Tage Basis: Wert da, aber keine Basis und keine Deutung.
    let mut short = Store::open_in_memory().unwrap();
    short
        .upsert_recovery_days(&[
            recovery(date(2026, 6, 9), Some(50.0), None, None),
            recovery(today, Some(30.0), None, None),
        ])
        .unwrap();
    let d = get(&short, "2026-06-10");
    assert_eq!(d.readiness.state, "readiness.no_data");
    let hrv = d.readiness.hrv.as_ref().unwrap();
    assert_eq!((hrv.baseline, hrv.deviation_percent), (None, None));
    assert!(
        d.readiness.resting_hr.is_none(),
        "kein Wert am Tag, kein Eintrag"
    );
}

#[test]
fn stale_recovery_data_is_listed_but_not_interpreted() {
    let mut store = Store::open_in_memory().unwrap();
    let days: Vec<RecoveryDayRecord> = (0..8)
        .map(|n| {
            recovery(
                date(2026, 4, 1) + chrono::TimeDelta::days(n),
                Some(50.0),
                None,
                None,
            )
        })
        .collect();
    store.upsert_recovery_days(&days).unwrap();
    // Zwei Monate später: der letzte Wert ist zu alt für eine Aussage über heute.
    let d = get(&store, "2026-06-10");
    assert_eq!(d.readiness.state, "readiness.no_data");
    assert_eq!(d.readiness.date, None);
    assert!(d.readiness.hrv.is_none());
    assert_eq!(d.readiness.last_date.as_deref(), Some("2026-04-08"));
}

// --- Plan -------------------------------------------------------------------------------

const PLAN_ID: &str = "00000000-0000-4000-8000-0000000000d1";

fn with_active_plan(name: &str) -> Store {
    let mut store = with_fixtures(name);
    plans::generate(
        &mut store,
        &GeneratePlanInput {
            week_start: "2026-06-01".into(),
            goal_date: None,
            goal_sport: None,
            weeks: Some(4),
            locale: PlanLocale::De,
            sports: vec![PlanSport::Cycling],
            strength_per_week: 1,
        },
        PLAN_ID.into(),
        now(),
    )
    .unwrap();
    plans::set_plan_status(&mut store, PLAN_ID, "active").unwrap();
    store
}

#[test]
fn the_projection_continues_from_the_actual_values_into_the_plan() {
    let store = with_active_plan("plan");
    let d = get(&store, "2026-06-03");
    let plan = d.plan.as_ref().expect("aktiver Plan");
    assert_eq!(
        (plan.first_day.as_str(), plan.last_day.as_str()),
        ("2026-06-01", "2026-06-28")
    );
    // Prognose beginnt am Tag nach heute und endet am Planende.
    assert_eq!(d.projection.first().unwrap().date, "2026-06-04");
    assert_eq!(d.projection.last().unwrap().date, "2026-06-28");
    assert_eq!(d.pmc.last().unwrap().date, "2026-06-03");
    // Lückenlos, TSB folgt CTL minus ATL des Vortags auch über die Nahtstelle.
    let all: Vec<_> = d.pmc.iter().chain(&d.projection).collect();
    for w in all.windows(2) {
        assert!((w[1].tsb - (w[0].ctl - w[0].atl)).abs() < 0.11, "{w:?}");
    }
    // Die Nahtstelle springt nicht: CTL ändert sich von einem zum nächsten Tag um höchstens die
    // Tageslast durch 42, hier grob unter 5.
    let seam = (d.projection[0].ctl - d.pmc.last().unwrap().ctl).abs();
    assert!(seam < 5.0, "{seam}");

    // Planerfüllung: vier Wochen, erste hat Soll bis gestern, spätere noch keins.
    assert_eq!(d.adherence.weeks.len(), 4);
    assert_eq!(d.adherence.weeks[0].week_start, "2026-06-01");
    assert!(!d.adherence.weeks[0].complete);
    assert_eq!(d.adherence.weeks[3].ratio_percent, None);
    assert_eq!(d.adherence.weeks[3].planned_to_date, 0.0);
    assert!(d.adherence.weeks[3].planned_total > 0.0);
}

#[test]
fn adherence_counts_planned_load_only_up_to_yesterday() {
    let store = with_active_plan("plan-monday");
    // Montag morgens: noch nichts fällig, also kein Verhältnis und keine Deutung.
    let d = get(&store, "2026-06-01");
    assert_eq!(d.adherence.ratio_percent, None);
    assert_eq!(d.adherence.state, "adherence.no_data");
    assert!(d.adherence.weeks[0].planned_total > 0.0);
    assert_eq!(d.adherence.weeks[0].planned_to_date, 0.0);
    // Am Sonntag danach ist die Woche mit allem, was bis Samstag geplant war, abgerechnet;
    // nichts wurde gefahren, also deutlich unter Plan.
    let sunday = get(&store, "2026-06-07");
    assert_eq!(sunday.adherence.state, "adherence.under");
    assert_eq!(sunday.adherence.ratio_percent, Some(0.0));
}

#[test]
fn a_recovery_swap_in_the_plan_is_reported_only_when_the_plan_really_did_it() {
    let mut store = with_fixtures("swap");
    let day = |n: i64| date(2026, 5, 31) - chrono::TimeDelta::days(n);
    let mut days: Vec<RecoveryDayRecord> = (1..=7)
        .map(|n| recovery(day(n), Some(60.0), None, Some(8.0)))
        .collect();
    // HRV 30 Prozent unter der Basis, über der Schwelle von R12.
    days.push(recovery(day(0), Some(42.0), None, Some(8.0)));
    store.upsert_recovery_days(&days).unwrap();
    plans::generate(
        &mut store,
        &GeneratePlanInput {
            week_start: "2026-06-01".into(),
            goal_date: None,
            goal_sport: None,
            weeks: Some(4),
            locale: PlanLocale::De,
            sports: vec![PlanSport::Cycling],
            strength_per_week: 1,
        },
        PLAN_ID.into(),
        now(),
    )
    .unwrap();
    plans::set_plan_status(&mut store, PLAN_ID, "active").unwrap();

    // Der Planer der Fixtures kennt noch keine harte Einheit (kurze Historie). Der Tausch von R12
    // selbst ist in `core` geprüft; hier zählt, dass das Dashboard seine Spur im Plan erkennt.
    let mut plan = store.load_plan(PLAN_ID).unwrap().unwrap();
    let session = plan.weeks[0]
        .days
        .iter_mut()
        .find_map(|d| d.sessions.first_mut())
        .expect("eine Einheit in der ersten Woche");
    session
        .reason_keys
        .push(lastwerk_core::plan::reason_keys::SESSION_RECOVERY_SWAP.to_owned());
    store.save_plan(&plan, PlanStatus::Active).unwrap();

    let d = get(&store, "2026-06-02");
    assert_eq!(d.readiness.state, "readiness.low");
    assert!(d.readiness.plan_adjusted);
    // Ein Plan ohne Tausch löst den Hinweis nicht aus.
    let plain = with_active_plan("swap-none");
    assert!(!get(&plain, "2026-06-02").readiness.plan_adjusted);
    // Wochen später liegt die Einheit außerhalb der letzten sechs Tage.
    assert!(!get(&store, "2026-06-20").readiness.plan_adjusted);
}

#[test]
fn efficiency_needs_long_rides_with_heart_rate_and_enough_of_them() {
    let store = with_fixtures("ef");
    let d = get(&store, "2026-05-31");
    let e = &d.efficiency;
    // Die Fixtures haben wenige Fahrten ab 45 Minuten mit Herzfrequenz. Ob es für ein Mittel
    // reicht, entscheidet `core`; hier zählt, dass Zahl, Reihe und Code zusammenpassen.
    assert_eq!(e.current.is_some(), !e.series.is_empty());
    for p in &e.series {
        assert!(p.ef > 0.0 && p.ef < 10.0, "{p:?}");
    }
    // Ohne Vergleichsfenster kein Trend.
    assert_eq!(e.trend, "efficiency.no_data");
    assert_eq!(e.change_percent, None);
}

// --- Body Battery (T22) -----------------------------------------------------------------

#[test]
fn body_battery_shows_latest_day_with_value() {
    let store = store_with_recovery_for_body_battery();
    let d = get(&store, "2026-03-10");
    assert_eq!(d.readiness.body_battery_high, Some(88.0));
    assert_eq!(d.readiness.body_battery_low, Some(21.0));
}

fn store_with_recovery_for_body_battery() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    let mut a = recovery(date(2026, 3, 8), Some(60.0), None, None);
    a.body_battery_high = Some(70);
    a.body_battery_low = Some(10);
    let mut b = recovery(date(2026, 3, 9), Some(61.0), None, None);
    b.body_battery_high = Some(88);
    b.body_battery_low = Some(21);
    // Jüngster Tag ohne Body Battery: angezeigt wird der Vortag.
    let c = recovery(date(2026, 3, 10), Some(59.0), None, None);
    store.upsert_recovery_days(&[a, b, c]).unwrap();
    store
}

/// T34: Seite "Erholung". Zehn Tage HRV 60 ms, Ruhepuls 50 bpm, Schlaf 7 h (12. bis 21. Mai),
/// dann am 22. Mai HRV 50 ms: Basis 60, Untergrenze nach R12 60 * 0,85 = 51 ms, also niedrig.
/// Die Fixtures liefern Tageslast im Mai.
#[test]
fn recovery_page_shows_baseline_floor_low_days_and_load() {
    let mut store = with_fixtures("recovery-page");
    for d in 12..=21 {
        store
            .upsert_recovery_day(&recovery(
                date(2026, 5, d),
                Some(60.0),
                Some(50.0),
                Some(7.0),
            ))
            .unwrap();
    }
    store
        .upsert_recovery_day(&recovery(
            date(2026, 5, 22),
            Some(50.0),
            Some(52.0),
            Some(7.0),
        ))
        .unwrap();
    let page = lastwerk_app::service::recovery::recovery(&store, "2026-05-31", 30).unwrap();
    assert_eq!(page.points.len(), 30);
    assert_eq!(page.points.first().unwrap().date, "2026-05-02");
    assert_eq!(page.last_date.as_deref(), Some("2026-05-22"));
    let low: Vec<&str> = page
        .points
        .iter()
        .filter(|p| p.low)
        .map(|p| p.date.as_str())
        .collect();
    assert_eq!(low, ["2026-05-22"]);
    assert_eq!(page.low_days, 1);
    let p22 = page.points.iter().find(|p| p.date == "2026-05-22").unwrap();
    assert_eq!(p22.hrv_base, Some(60.0));
    assert_eq!(p22.hrv_floor, Some(51.0));
    assert_eq!(p22.sleep_h, Some(7.0));
    assert_eq!(p22.resting_hr_base, Some(50.0));
    // Kein Wert am 23. Mai: kein Wert, nie 0.
    let p23 = page.points.iter().find(|p| p.date == "2026-05-23").unwrap();
    assert_eq!(p23.hrv_ms, None);
    // Tageslast aus den Fixtures (10. bis etwa 20. Mai).
    assert!(page.points.iter().any(|p| p.load.is_some_and(|l| l > 0.0)));
    assert_eq!(page.hrv_drop_percent, Some(15.0));
    let hrv = page.summary.iter().find(|s| s.metric == "hrv").unwrap();
    // Mittel aller elf Tage: (10 * 60 + 50) / 11 = 59,09 ms.
    assert_eq!(hrv.period, Some(59.1));
    // Die letzten sieben Tage (25. bis 31. Mai) haben keinen Wert.
    assert_eq!(hrv.recent, None);
    assert!(lastwerk_app::service::recovery::recovery(&store, "2026-05-31", 14).is_err());
}

// --- Tagesentscheidung und Erkenntnisse (T44, SPEC 8.7) ---------------------------------

/// Setzt am Tag `day` des aktiven Plans genau eine harte Radeinheit (Schwelle 2x20, 75 min).
/// Der Planer der Fixtures erzeugt mit der kurzen Historie keine harte Einheit.
fn put_hard_session(store: &mut Store, day: NaiveDate) {
    use lastwerk_core::plan::{PlanSession, Slot};
    let mut plan = store.load_plan(PLAN_ID).unwrap().unwrap();
    let d = plan
        .weeks
        .iter_mut()
        .flat_map(|w| w.days.iter_mut())
        .find(|d| d.date == day)
        .expect("Tag im Plan");
    d.sessions = vec![PlanSession {
        slot: Slot::Am,
        sport: PlanSport::Cycling,
        template_id: "bike.threshold.2x20".into(),
        duration_min: 75,
        planned_load: 90.0,
        hard: true,
        rpe: None,
        reason_keys: Vec::new(),
    }];
    store.save_plan(&plan, PlanStatus::Active).unwrap();
}

#[test]
fn low_hrv_before_a_hard_session_lightens_it_without_changing_the_plan() {
    let mut store = with_active_plan("decision-lighten");
    let today = date(2026, 6, 3);
    put_hard_session(&mut store, today);
    // Basis HRV 60 ms an sieben Tagen, heute 42 ms: (60 - 42) / 60 = 0,30, über der Schwelle
    // von R12 (0,15, Faustregel).
    let mut days: Vec<RecoveryDayRecord> = (1..=7)
        .map(|n| {
            recovery(
                today - chrono::TimeDelta::days(n),
                Some(60.0),
                None,
                Some(8.0),
            )
        })
        .collect();
    days.push(recovery(today, Some(42.0), None, Some(8.0)));
    store.upsert_recovery_days(&days).unwrap();
    let before = store.load_plan(PLAN_ID).unwrap().unwrap();

    let d = get(&store, "2026-06-03");
    assert_eq!(d.readiness.state, "readiness.low");
    let dec = &d.decision;
    assert_eq!(dec.state, "decision.lighten");
    let main = dec
        .main
        .as_ref()
        .expect("lockere Radvorlage mit passendem Fenster");
    assert_eq!(main.kind, "lighter");
    assert!(!main.hard);
    assert_eq!(main.sport, PlanSport::Cycling);
    // Gleiche Dauer wie geplant, Vorlage aus dem Katalog.
    assert_eq!(main.duration_min, 75);
    assert_eq!(dec.alternatives.len(), 1);
    assert_eq!(dec.alternatives[0].kind, "planned");
    assert_eq!(dec.alternatives[0].template_id, "bike.threshold.2x20");
    assert!(
        dec.reasons
            .contains(&"decision.reason.readiness_low".to_owned())
    );
    assert!(dec.rules.contains(&"R12".to_owned()));
    // Die Entscheidung schreibt nichts in den Plan.
    assert_eq!(store.load_plan(PLAN_ID).unwrap().unwrap(), before);
    // Bereitschaft niedrig steht unter den Erkenntnissen.
    assert!(d.insights.iter().any(|i| i.code == "readiness.low"));
}

#[test]
fn without_a_plan_the_decision_is_no_plan_and_insights_stay_short() {
    let store = with_fixtures("decision-none");
    let d = get(&store, "2026-05-31");
    assert_eq!(d.decision.state, "decision.no_plan");
    assert!(d.decision.main.is_none() && d.decision.alternatives.is_empty());
    assert!(d.insights.len() <= 3);

    let empty = Store::open_in_memory().unwrap();
    let d = get(&empty, "2026-06-10");
    assert_eq!(d.decision.state, "decision.no_plan");
    assert!(d.insights.is_empty(), "ohne Daten keine Erkenntnis");
}

#[test]
fn a_day_without_session_is_rest_and_missing_inputs_are_named() {
    let mut store = with_active_plan("decision-rest");
    let today = date(2026, 6, 3);
    let mut plan = store.load_plan(PLAN_ID).unwrap().unwrap();
    for d in plan.weeks.iter_mut().flat_map(|w| w.days.iter_mut()) {
        if d.date == today {
            d.sessions.clear();
        }
    }
    store.save_plan(&plan, PlanStatus::Active).unwrap();
    let d = get(&store, "2026-06-03");
    assert_eq!(d.decision.state, "decision.rest");
    assert!(d.decision.main.is_none());

    // Ohne Erholungswerte steht die Bereitschaft in `missing`, ein harter Tag bleibt geplant.
    put_hard_session(&mut store, date(2026, 6, 4));
    let d = get(&store, "2026-06-04");
    assert!(d.decision.missing.contains(&"readiness".to_owned()));
    assert_ne!(d.decision.state, "decision.lighten");
}

#[test]
fn insights_are_ranked_codes_of_the_sections_and_at_most_three() {
    let store = with_active_plan("decision-insights");
    // Am Sonntag nach einer Woche ohne Training ist der Plan unterfüllt.
    let d = get(&store, "2026-06-07");
    assert_eq!(d.adherence.state, "adherence.under");
    assert!(d.insights.len() <= 3);
    assert!(d.insights.iter().any(|i| i.code == "adherence.under"));
    let shown = [
        d.ramp.state.as_str(),
        d.distribution.state.as_str(),
        d.adherence.state.as_str(),
        d.efficiency.trend.as_str(),
        d.readiness.state.as_str(),
        d.form.as_ref().map_or("", |f| f.state.as_str()),
        "power_curve.rising",
    ];
    for i in &d.insights {
        assert!(shown.contains(&i.code.as_str()), "{}", i.code);
    }
}
