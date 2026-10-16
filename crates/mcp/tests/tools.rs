//! Verhalten der Werkzeuge mit synthetischen Daten und handgerechneten Referenzwerten.

mod common;

use chrono::NaiveDate;
use common::{date, now, sample_plan, threshold, work_dir};
use lastwerk_core::Sport;
use lastwerk_core::plan::PlanSource;
use lastwerk_mcp::args::{
    ActivityArgs, FitnessArgs, ListActivitiesArgs, NoArgs, PlanWeekArgs, RecoveryArgs, RuleArgs,
    ThresholdsArgs, WeeklyLoadArgs,
};
use lastwerk_mcp::error::ToolError;
use lastwerk_mcp::tools::{self, Ctx};
use lastwerk_mcp::views::{Method, Sport as ViewSport};
use lastwerk_store::{
    ActivityRow, ImportOptions, LoadRow, LoadStatus, PlanStatus, SourceFileRow, SourceStatus,
    Store, ThresholdKind,
};

const TODAY: (i32, u32, u32) = (2026, 5, 25);

fn today() -> NaiveDate {
    date(TODAY.0, TODAY.1, TODAY.2)
}

/// Fügt eine primäre Aktivität mit einer Belastung hinzu.
fn add(
    store: &mut Store,
    n: u32,
    day: NaiveDate,
    sport: Sport,
    minutes: i64,
    load: Option<f64>,
) -> i64 {
    let file_id = store
        .upsert_source_file(&SourceFileRow {
            rel_path: format!("a/{n}.fit"),
            sha256: format!("{n:064x}"),
            size_bytes: 1,
            mtime_utc: now(),
            imported_at: now(),
            status: SourceStatus::Ok,
            error_code: None,
        })
        .unwrap();
    let id = store
        .upsert_activity(&ActivityRow {
            source_file_id: file_id,
            sport,
            sub_sport: None,
            start_utc: day.and_hms_opt(6, 0, 0).unwrap().and_utc(),
            local_offset_s: 0,
            local_date: day,
            duration_s: minutes * 60,
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
        })
        .unwrap();
    let row = match load {
        Some(v) => LoadRow {
            missing: None,
            activity_id: id,
            method: "tss".into(),
            value: Some(v),
            intensity_factor: Some(0.856),
            threshold_id: None,
            status: LoadStatus::Ok,
            computed_at: now(),
        },
        None => LoadRow {
            missing: None,
            activity_id: id,
            method: "none".into(),
            value: None,
            intensity_factor: None,
            threshold_id: None,
            status: LoadStatus::MissingThreshold,
            computed_at: now(),
        },
    };
    store.upsert_activity_load(&row).unwrap();
    id
}

fn ctx(store: &Store) -> Ctx<'_> {
    Ctx {
        store,
        today: today(),
    }
}

fn err_of<T: std::fmt::Debug>(r: Result<T, ToolError>) -> ToolError {
    r.unwrap_err()
}

// --- get_fitness_status --------------------------------------------------------------

/// Handrechnung (SPEC 8.2): ein Tag mit Last 100 am 2. März 2026, danach Ruhe.
///
/// - Gewichte: `k_ctl = 1 - exp(-1/42) = 0,0235283`, `k_atl = 1 - exp(-1/7) = 0,1331221`.
/// - Tag 1: CTL = 100 * 0,0235283 = 2,35283, ATL = 100 * 0,1331221 = 13,31221, TSB = 0
///   (Startwerte 0 minus 0).
/// - Tag 2 ohne Last: TSB = CTL1 - ATL1 = 2,35283 - 13,31221 = -10,95938,
///   CTL = 2,35283 * (1 - 0,0235283) = 2,29747, ATL = 13,31221 * (1 - 0,1331221) = 11,54012.
///
/// Die Antwort rundet auf eine Nachkommastelle.
#[test]
fn fitness_matches_the_hand_calculation() {
    let mut store = Store::open_in_memory().unwrap();
    add(
        &mut store,
        1,
        date(2026, 3, 2),
        Sport::Cycling,
        60,
        Some(100.0),
    );
    let out = tools::get_fitness_status(
        ctx(&store),
        &FitnessArgs {
            from: date(2026, 3, 1),
            to: date(2026, 3, 3),
        },
    )
    .unwrap();
    // Der 1. März liegt vor dem ersten Tag mit Last: keine Grundlage, kein Eintrag.
    assert_eq!(out.days.len(), 2);
    let d1 = out.days[0];
    assert_eq!(d1.date, date(2026, 3, 2));
    assert_eq!((d1.ctl, d1.atl, d1.tsb), (2.4, 13.3, 0.0));
    let d2 = out.days[1];
    assert_eq!(d2.date, date(2026, 3, 3));
    assert_eq!((d2.ctl, d2.atl, d2.tsb), (2.3, 11.5, -11.0));

    // Beginnt der Zeitraum später, sind die Werte dieselben: der Verlauf startet am ersten
    // Tag mit Last, nicht bei `from`.
    let later = tools::get_fitness_status(
        ctx(&store),
        &FitnessArgs {
            from: date(2026, 3, 3),
            to: date(2026, 3, 3),
        },
    )
    .unwrap();
    assert_eq!(later.days, vec![d2]);
}

#[test]
fn fitness_leaves_out_the_future_and_missing_data() {
    let mut store = Store::open_in_memory().unwrap();
    let empty = tools::get_fitness_status(
        ctx(&store),
        &FitnessArgs {
            from: date(2026, 5, 1),
            to: date(2026, 5, 31),
        },
    )
    .unwrap();
    assert!(empty.days.is_empty(), "no data is not a row of zeros");

    add(
        &mut store,
        1,
        date(2026, 5, 20),
        Sport::Cycling,
        60,
        Some(80.0),
    );
    let out = tools::get_fitness_status(
        ctx(&store),
        &FitnessArgs {
            from: date(2026, 5, 18),
            to: date(2026, 6, 30),
        },
    )
    .unwrap();
    assert_eq!(out.days.first().unwrap().date, date(2026, 5, 20));
    assert_eq!(out.days.last().unwrap().date, today());

    let future = tools::get_fitness_status(
        ctx(&store),
        &FitnessArgs {
            from: date(2026, 6, 1),
            to: date(2026, 6, 30),
        },
    )
    .unwrap();
    assert!(future.days.is_empty());
}

#[test]
fn fitness_uses_the_pmc_settings() {
    let mut store = Store::open_in_memory().unwrap();
    add(
        &mut store,
        1,
        date(2026, 3, 2),
        Sport::Cycling,
        60,
        Some(100.0),
    );
    store.set_setting("pmc.tau_ctl_d", "50").unwrap();
    store.set_setting("pmc.tau_atl_d", "15").unwrap();
    // Handrechnung: 100 * (1 - exp(-1/50)) = 1,98013 und 100 * (1 - exp(-1/15)) = 6,44930.
    let out = tools::get_fitness_status(
        ctx(&store),
        &FitnessArgs {
            from: date(2026, 3, 2),
            to: date(2026, 3, 2),
        },
    )
    .unwrap();
    assert_eq!((out.days[0].ctl, out.days[0].atl), (2.0, 6.4));
}

#[test]
fn fitness_range_is_checked() {
    let store = Store::open_in_memory().unwrap();
    let call = |from, to| tools::get_fitness_status(ctx(&store), &FitnessArgs { from, to });
    assert!(matches!(
        err_of(call(date(2026, 3, 2), date(2026, 3, 1))),
        ToolError::InvalidParams(_)
    ));
    // Beide Enden eingeschlossen: 366 Tage gehen, 367 nicht.
    assert!(call(date(2025, 1, 1), date(2025, 12, 31)).is_ok()); // 365 Tage
    assert!(call(date(2025, 1, 1), date(2026, 1, 1)).is_ok()); // 366 Tage
    assert!(call(date(2025, 1, 1), date(2026, 1, 2)).is_err()); // 367 Tage
    assert!(call(date(2024, 1, 1), date(2024, 12, 31)).is_ok()); // Schaltjahr, 366 Tage
    assert!(call(date(2024, 1, 1), date(2025, 1, 1)).is_err()); // 367 Tage
}

// --- get_weekly_load -----------------------------------------------------------------

#[test]
fn weekly_load_sums_per_week_and_sport() {
    let mut store = Store::open_in_memory().unwrap();
    // 25. Mai 2026 ist ein Montag.
    add(
        &mut store,
        1,
        date(2026, 5, 24),
        Sport::Cycling,
        60,
        Some(60.0),
    ); // Woche 18. Mai
    add(
        &mut store,
        2,
        date(2026, 5, 19),
        Sport::Cycling,
        30,
        Some(30.0),
    ); // Woche 18. Mai
    add(
        &mut store,
        3,
        date(2026, 5, 25),
        Sport::Running,
        40,
        Some(40.0),
    ); // Woche 25. Mai
    add(
        &mut store,
        4,
        date(2026, 4, 27),
        Sport::Cycling,
        90,
        Some(20.0),
    ); // Woche 27. April
    // Ohne Last: zählt nur als Zeit (R2 und Wochenumfang meinen alle Einheiten), Last 0.
    add(&mut store, 5, date(2026, 5, 20), Sport::Running, 50, None);
    let get = |weeks, sport| {
        tools::get_weekly_load(ctx(&store), &WeeklyLoadArgs { weeks, sport })
            .unwrap()
            .weeks
    };

    let two = get(2, None);
    assert_eq!(two.len(), 3);
    // Rad in der Woche des 18. Mai: 60 + 30 = 90 Punkte, 60 + 30 = 90 Minuten.
    assert_eq!(
        (
            two[0].week_start,
            two[0].sport,
            two[0].load,
            two[0].duration_min
        ),
        (date(2026, 5, 18), ViewSport::Cycling, 90.0, 90)
    );
    // Lauf in der Woche des 18. Mai: keine berechnete Last, 50 Minuten.
    assert_eq!(
        (
            two[1].week_start,
            two[1].sport,
            two[1].load,
            two[1].duration_min
        ),
        (date(2026, 5, 18), ViewSport::Running, 0.0, 50)
    );
    assert_eq!(
        (
            two[2].week_start,
            two[2].sport,
            two[2].load,
            two[2].duration_min
        ),
        (date(2026, 5, 25), ViewSport::Running, 40.0, 40)
    );

    assert_eq!(get(1, None).len(), 1);
    // Vier Wochen reichen bis zum 4. Mai zurück: die Woche des 27. April fehlt noch.
    assert_eq!(get(4, None).len(), 3);
    assert_eq!(get(5, None).len(), 4);
    assert_eq!(get(52, Some(ViewSport::Cycling)).len(), 2);
    assert!(get(52, Some(ViewSport::Swimming)).is_empty());
}

#[test]
fn weekly_load_weeks_are_bounded() {
    let store = Store::open_in_memory().unwrap();
    for weeks in [0, 53, 1000] {
        assert!(matches!(
            err_of(tools::get_weekly_load(
                ctx(&store),
                &WeeklyLoadArgs { weeks, sport: None }
            )),
            ToolError::InvalidParams(_)
        ));
    }
    for weeks in [1, 52] {
        assert!(
            tools::get_weekly_load(ctx(&store), &WeeklyLoadArgs { weeks, sport: None }).is_ok()
        );
    }
}

// --- list_activities, get_activity ----------------------------------------------------

#[test]
fn list_activities_filters_orders_and_marks_missing_load() {
    let mut store = Store::open_in_memory().unwrap();
    let a = add(
        &mut store,
        1,
        date(2026, 5, 2),
        Sport::Cycling,
        90,
        Some(66.66),
    );
    let b = add(&mut store, 2, date(2026, 5, 1), Sport::Running, 45, None);
    add(
        &mut store,
        3,
        date(2026, 6, 1),
        Sport::Cycling,
        45,
        Some(10.0),
    );
    let list = tools::list_activities(
        ctx(&store),
        &ListActivitiesArgs {
            from: date(2026, 5, 1),
            to: date(2026, 5, 31),
            sport: None,
        },
    )
    .unwrap();
    assert!(!list.truncated);
    assert_eq!(list.activities.len(), 2);
    assert_eq!(list.activities[0].id, b);
    assert_eq!(list.activities[1].id, a);
    // Ohne berechenbare Last: null, nicht 0.
    let missing = &list.activities[0];
    assert_eq!(
        (missing.load, missing.load_method, missing.intensity_factor),
        (None, None, None)
    );
    let ride = &list.activities[1];
    assert_eq!(ride.sport, ViewSport::Cycling);
    assert_eq!(ride.duration_min, 90);
    assert_eq!(ride.load, Some(66.7));
    assert_eq!(ride.load_method, Some(Method::Tss));
    assert_eq!(ride.intensity_factor, Some(0.86));
    assert!(ride.best_power_w.is_none(), "not in the list");

    let only = tools::list_activities(
        ctx(&store),
        &ListActivitiesArgs {
            from: date(2026, 5, 1),
            to: date(2026, 5, 31),
            sport: Some(ViewSport::Running),
        },
    )
    .unwrap();
    assert_eq!(only.activities.len(), 1);
    assert!(matches!(
        err_of(tools::list_activities(
            ctx(&store),
            &ListActivitiesArgs {
                from: date(2026, 6, 1),
                to: date(2026, 5, 1),
                sport: None
            }
        )),
        ToolError::InvalidParams(_)
    ));
}

#[test]
fn list_activities_returns_at_most_200_rows() {
    let mut store = Store::open_in_memory().unwrap();
    for n in 0..201u32 {
        let day = date(2025, 1, 1) + chrono::TimeDelta::days(i64::from(n));
        add(&mut store, n, day, Sport::Cycling, 30, Some(20.0));
    }
    let args = |to| ListActivitiesArgs {
        from: date(2025, 1, 1),
        to,
        sport: None,
    };
    let all = tools::list_activities(ctx(&store), &args(date(2025, 12, 31))).unwrap();
    assert_eq!(all.activities.len(), 200);
    assert!(all.truncated);
    // Älteste zuerst: der 201. Tag (20. Juli 2025) fehlt.
    assert_eq!(all.activities[0].local_date, date(2025, 1, 1));
    assert_eq!(all.activities[199].local_date, date(2025, 7, 19));

    // Genau 200 sind nicht abgeschnitten.
    let exact = tools::list_activities(ctx(&store), &args(date(2025, 7, 19))).unwrap();
    assert_eq!(exact.activities.len(), 200);
    assert!(!exact.truncated);
}

#[test]
fn get_activity_checks_the_id() {
    let mut store = Store::open_in_memory().unwrap();
    let id = add(
        &mut store,
        1,
        date(2026, 5, 2),
        Sport::Cycling,
        90,
        Some(50.0),
    );
    let v = tools::get_activity(ctx(&store), &ActivityArgs { id }).unwrap();
    assert_eq!(v.id, id);
    assert_eq!(v.best_power_w, Some(vec![]));
    assert!(matches!(
        err_of(tools::get_activity(ctx(&store), &ActivityArgs { id: 999 })),
        ToolError::NotFound(_)
    ));
    for bad in [0, -1] {
        assert!(matches!(
            err_of(tools::get_activity(ctx(&store), &ActivityArgs { id: bad })),
            ToolError::InvalidParams(_)
        ));
    }
}

#[test]
fn imported_duplicates_are_not_listed_but_zones_and_best_power_are_there() {
    let dir = work_dir("tools-import");
    let fit = dir.join("fit");
    std::fs::create_dir_all(&fit).unwrap();
    for f in lastwerk_testkit::fixtures::all() {
        std::fs::write(fit.join(f.file_name()), &f.bytes).unwrap();
    }
    let mut store = Store::open_in_memory().unwrap();
    for t in [
        threshold(
            Some(Sport::Cycling),
            ThresholdKind::FtpW,
            250.0,
            date(2026, 1, 1),
            None,
        ),
        threshold(
            Some(Sport::Cycling),
            ThresholdKind::LthrBpm,
            160.0,
            date(2026, 1, 1),
            None,
        ),
    ] {
        store.insert_threshold(&t).unwrap();
    }
    store.set_setting("athlete.sex", "male").unwrap();
    store
        .import_dir(&fit, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();

    let list = tools::list_activities(
        ctx(&store),
        &ListActivitiesArgs {
            from: date(2026, 1, 1),
            to: date(2026, 12, 31),
            sport: Some(ViewSport::Cycling),
        },
    )
    .unwrap();
    for a in &list.activities {
        assert!(
            store.activity(a.id).unwrap().unwrap().is_primary,
            "{}",
            a.id
        );
    }
    // Das Edge/Uhr-Paar zählt einmal: weniger Einträge als primäre plus doppelte Dateien.
    let all_cycling = store
        .activities()
        .unwrap()
        .iter()
        .filter(|a| a.row.sport == Sport::Cycling)
        .count();
    assert_eq!(list.activities.len(), all_cycling - 1);

    // Eine Fahrt mit Leistung hat Zeit in den sieben Leistungszonen und Bestwerte.
    let steady = list
        .activities
        .iter()
        .find(|a| a.zone_minutes.iter().any(|z| z.scheme == "power7"))
        .expect("a ride with power zones");
    let zones: Vec<_> = steady
        .zone_minutes
        .iter()
        .filter(|z| z.scheme == "power7")
        .collect();
    assert_eq!(zones.len(), 7);
    let total: f64 = zones.iter().map(|z| z.minutes).sum();
    assert!(total > 0.0);
    let detail = tools::get_activity(ctx(&store), &ActivityArgs { id: steady.id }).unwrap();
    let best = detail.best_power_w.unwrap();
    assert!(!best.is_empty());
    assert!(
        best.iter()
            .all(|b| [5, 60, 300, 1200, 3600].contains(&b.duration_s))
    );
    assert!(best.windows(2).all(|w| w[0].duration_s < w[1].duration_s));
}

// --- get_thresholds ------------------------------------------------------------------

#[test]
fn thresholds_come_with_their_valid_from_date() {
    let mut store = Store::open_in_memory().unwrap();
    let cyc = Some(Sport::Cycling);
    for t in [
        threshold(
            cyc,
            ThresholdKind::FtpW,
            250.0,
            date(2026, 1, 1),
            Some("private note"),
        ),
        threshold(cyc, ThresholdKind::FtpW, 260.0, date(2026, 5, 1), None),
        threshold(
            Some(Sport::Running),
            ThresholdKind::RunSpeedMps,
            3.5,
            date(2026, 2, 1),
            None,
        ),
        threshold(
            None,
            ThresholdKind::HrRestBpm,
            48.0,
            date(2026, 1, 15),
            None,
        ),
    ] {
        store.insert_threshold(&t).unwrap();
    }
    let get = |on_date| tools::get_thresholds(ctx(&store), &ThresholdsArgs { on_date }).unwrap();

    let before = get(date(2025, 12, 31));
    assert!(before.sports.is_empty(), "no value is valid yet");

    let april = get(date(2026, 4, 30));
    let cycling = april
        .sports
        .iter()
        .find(|s| s.sport == ViewSport::Cycling)
        .unwrap();
    let ftp = cycling.values.iter().find(|v| v.kind == "ftp_w").unwrap();
    assert_eq!(
        (ftp.value, ftp.unit.as_str(), ftp.valid_from),
        (250.0, "W", date(2026, 1, 1))
    );
    // Der Ruhepuls gilt für alle Sportarten.
    let rest = cycling
        .values
        .iter()
        .find(|v| v.kind == "hr_rest_bpm")
        .unwrap();
    assert_eq!((rest.value, rest.unit.as_str()), (48.0, "bpm"));
    let running = april
        .sports
        .iter()
        .find(|s| s.sport == ViewSport::Running)
        .unwrap();
    let speed = running
        .values
        .iter()
        .find(|v| v.kind == "run_speed_mps")
        .unwrap();
    assert_eq!((speed.value, speed.unit.as_str()), (3.5, "m/s"));
    assert!(
        !serde_json::to_string(&april)
            .unwrap()
            .contains("private note")
    );

    let may = get(date(2026, 5, 1));
    let ftp = may
        .sports
        .iter()
        .find(|s| s.sport == ViewSport::Cycling)
        .unwrap()
        .values
        .iter()
        .find(|v| v.kind == "ftp_w")
        .unwrap();
    assert_eq!((ftp.value, ftp.valid_from), (260.0, date(2026, 5, 1)));
}

// --- explain_rule --------------------------------------------------------------------

#[test]
fn explain_rule_checks_the_code_format() {
    let store = Store::open_in_memory().unwrap();
    let call = |code: &str| tools::explain_rule(ctx(&store), &RuleArgs { code: code.into() });
    assert!(call("R03").is_ok());
    for bad in [
        "r03",
        "R3",
        "R003",
        "R0x",
        "",
        "../x",
        "R03; DROP TABLE",
        " R03",
        "R-3",
    ] {
        assert!(
            matches!(err_of(call(bad)), ToolError::InvalidParams(_)),
            "{bad:?}"
        );
    }
    // R12 ist seit T14 eine echte Regel; R13 gibt es nicht.
    assert!(matches!(err_of(call("R13")), ToolError::NotFound(_)));
}

#[test]
fn explain_rule_follows_the_locale_setting() {
    let mut store = Store::open_in_memory().unwrap();
    let call =
        |store: &Store| tools::explain_rule(ctx(store), &RuleArgs { code: "R05".into() }).unwrap();
    assert_eq!(call(&store).title, "Rest day");
    store.set_setting("app.locale", "de").unwrap();
    assert_eq!(call(&store).title, "Ruhetag");
}

// --- get_current_plan, get_plan_week -------------------------------------------------

#[test]
fn plan_tools_need_an_active_plan() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(matches!(
        err_of(tools::get_current_plan(ctx(&store), &NoArgs {})),
        ToolError::NotFound(_)
    ));
    let draft = sample_plan(common::PLAN_ID, PlanSource::Rules, None);
    store.save_plan(&draft, PlanStatus::Draft).unwrap();
    assert!(tools::get_current_plan(ctx(&store), &NoArgs {}).is_err());
    store
        .set_plan_status(common::PLAN_ID, PlanStatus::Active)
        .unwrap();
    let plan = tools::get_current_plan(ctx(&store), &NoArgs {}).unwrap();
    assert_eq!(plan["weeks"].as_array().unwrap().len(), 4);
}

#[test]
fn plan_week_is_selected_by_any_day_of_the_week() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .save_plan(
            &sample_plan(common::PLAN_ID, PlanSource::Rules, None),
            PlanStatus::Active,
        )
        .unwrap();
    let get = |d| tools::get_plan_week(ctx(&store), &PlanWeekArgs { week_start: d });
    let monday = get(date(2027, 1, 11)).unwrap();
    assert_eq!(monday["week_start"], "2027-01-11");
    assert_eq!(monday["days"].as_array().unwrap().len(), 7);
    // Mittwoch derselben Woche, derselbe Inhalt.
    assert_eq!(get(date(2027, 1, 13)).unwrap(), monday);
    // Eine Woche außerhalb des Plans.
    assert!(matches!(
        err_of(get(date(2027, 3, 1))),
        ToolError::NotFound(_)
    ));
    assert!(matches!(
        err_of(get(date(2026, 12, 28))),
        ToolError::NotFound(_)
    ));
}

#[test]
fn plan_numbers_are_printed_as_written() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .save_plan(
            &sample_plan(common::PLAN_ID, PlanSource::Rules, None),
            PlanStatus::Active,
        )
        .unwrap();
    // `planned_load` ist ein f32 mit einer Nachkommastelle, im JSON steht kein 66.69999...
    let text = tools::get_current_plan(ctx(&store), &NoArgs {})
        .unwrap()
        .to_string();
    assert!(!text.contains("99999") && !text.contains("00001"), "{text}");
}

// --- get_recovery_days ---------------------------------------------------------------

#[test]
fn recovery_days_are_blocked_without_the_setting() {
    let mut store = Store::open_in_memory().unwrap();
    let args = RecoveryArgs {
        from: date(2026, 5, 1),
        to: date(2026, 5, 31),
    };
    assert!(matches!(
        err_of(tools::get_recovery_days(ctx(&store), &args)),
        ToolError::Disabled(_)
    ));
    store.set_setting("mcp.recovery_enabled", "false").unwrap();
    assert!(tools::get_recovery_days(ctx(&store), &args).is_err());
    store.set_setting("mcp.recovery_enabled", "true").unwrap();
    // Freigegeben, aber ohne Tabelle: leere Liste.
    assert!(
        tools::get_recovery_days(ctx(&store), &args)
            .unwrap()
            .days
            .is_empty()
    );
    // Die Bereichsgrenze gilt auch freigegeben.
    assert!(matches!(
        err_of(tools::get_recovery_days(
            ctx(&store),
            &RecoveryArgs {
                from: date(2026, 1, 1),
                to: date(2026, 12, 31)
            }
        )),
        ToolError::InvalidParams(_)
    ));
}
