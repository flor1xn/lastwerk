//! Pläne, Ziele, Verfügbarkeit und Export gegen eine echte Datenbank mit den synthetischen
//! Fixtures aus `lastwerk-testkit`. Alle Dateien liegen unter `target/`, es gibt keine echten
//! Daten. Die Plantests prüfen mit denselben Funktionen wie `lastwerk validate`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use lastwerk_app::dto::{
    AvailabilityDay, GeneratePlanInput, GoalInput, PlanDetail, PlanLocale, PlanSport, Priority,
};
use lastwerk_app::service::{self, plans};
use lastwerk_cli::{ics, planning, schema, texts::Texts};
use lastwerk_core::Sport;
use lastwerk_core::plan::{Hardness, validate_plan};
use lastwerk_store::{Store, ThresholdKind};
use lastwerk_testkit::fixtures;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("app-plan-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap()
}

/// Zwei feste IDs, damit die Tests ohne Zufall laufen.
const ID_A: &str = "00000000-0000-4000-8000-00000000000a";
const ID_B: &str = "00000000-0000-4000-8000-00000000000b";

/// Datenbank mit importierten Fixtures (Mai 2026) und FTP, damit Lasten und eine Historie
/// vorliegen.
fn database(name: &str) -> Store {
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
        &lastwerk_app::dto::ThresholdInput {
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

/// Montag, 1. Juni 2026 (zwei Wochen nach den Fixtures).
const START: &str = "2026-06-01";

fn input(weeks: Option<u32>) -> GeneratePlanInput {
    GeneratePlanInput {
        week_start: START.into(),
        goal_date: None,
        goal_sport: None,
        weeks,
        locale: PlanLocale::De,
        sports: vec![PlanSport::Cycling],
        strength_per_week: 1,
    }
}

fn code<T>(r: Result<T, lastwerk_app::error::AppError>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(e) => e.code,
    }
}

fn sessions(d: &PlanDetail) -> usize {
    d.plan
        .weeks
        .iter()
        .flat_map(|w| &w.days)
        .map(|day| day.sessions.len())
        .sum()
}

/// Der Plan aus dem Befehl besteht Schema und Leitplanken, geprüft mit den Funktionen von
/// `lastwerk validate` und unabhängig von der Prüfung in `detail`.
#[test]
fn a_generated_plan_passes_schema_and_validate_and_is_saved_as_draft() {
    let mut store = database("generate");
    let d = plans::generate(&mut store, &input(Some(3)), ID_A.into(), now()).unwrap();
    assert_eq!(d.status, "draft");
    assert_eq!(d.plan.plan_id, ID_A);
    assert_eq!(d.plan.weeks.len(), 3);
    assert!(sessions(&d) > 0);

    // Schema, wie `lastwerk validate` es zuerst prüft.
    let value = serde_json::to_value(&d.plan).unwrap();
    assert!(schema::plan_errors(&value).unwrap().is_empty());
    assert!(d.validation.schema_errors.is_empty());

    // Leitplanken gegen den Kontext aus der Datenbank.
    let (rules, catalog) = planning::load_rules().unwrap();
    let ctx =
        planning::validation_context(&store, &d.plan, &rules, catalog, &mut Vec::new()).unwrap();
    let hard: Vec<_> = validate_plan(&d.plan, &ctx, &rules)
        .into_iter()
        .filter(|v| v.hardness == Hardness::Hard)
        .collect();
    assert!(hard.is_empty(), "{hard:?}");
    assert!(d.validation.violations.iter().all(|v| !v.hard));

    // Gespeichert, in der Liste, und beim Zurücklesen derselbe Plan.
    let list = plans::list_plans(&store).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(
        (list[0].id.as_str(), list[0].status.as_str()),
        (ID_A, "draft")
    );
    assert_eq!(list[0].weeks, 3);
    assert_eq!(list[0].first_week.as_deref(), Some(START));
    let again = plans::get_plan(&store, ID_A).unwrap();
    assert_eq!(again.plan, d.plan);
}

/// Handrechnung (SPEC 8.2): Der Verlauf beginnt am Montag der ersten Woche aus `ctl0` und
/// `atl0`. CTL des ersten Tages = ctl0 + (L1 - ctl0) * (1 - exp(-1 / 42)), ATL entsprechend
/// mit 7, TSB = ctl0 - atl0. Er umfasst sieben Tage je Woche.
#[test]
fn the_projection_starts_from_the_state_before_the_plan() {
    let mut store = database("projection");
    let d = plans::generate(&mut store, &input(Some(2)), ID_A.into(), now()).unwrap();
    assert_eq!(d.projection.len(), 14);
    assert_eq!(d.projection[0].date, START);
    let first_load: f32 = d.plan.weeks[0].days[0]
        .sessions
        .iter()
        .map(|s| s.planned_load)
        .sum();
    assert_eq!(d.projection[0].load, first_load);
    let w_ctl = 1.0 - (-1.0f32 / 42.0).exp();
    let w_atl = 1.0 - (-1.0f32 / 7.0).exp();
    let ctl1 = d.ctl0 + (first_load - d.ctl0) * w_ctl;
    let atl1 = d.atl0 + (first_load - d.atl0) * w_atl;
    assert!((d.projection[0].ctl - ctl1).abs() < 1e-3);
    assert!((d.projection[0].atl - atl1).abs() < 1e-3);
    assert!((d.projection[0].tsb - (d.ctl0 - d.atl0)).abs() < 1e-3);
    // Mit Fixtures im Mai gibt es einen Anfangszustand über 0.
    assert!(d.ctl0 > 0.0);
}

/// Der Planer und die Vorausberechnung rechnen mit den Einstellungen des Verlaufs (SPEC 5.4,
/// Nachtrag vom 14. Oktober 2026), nicht mit den Standardwerten. Handrechnung wie oben mit
/// Zeitkonstanten 50 und 15: Gewicht CTL = 1 - exp(-1/50) = 0,0198013, Gewicht ATL =
/// 1 - exp(-1/15) = 0,0644930.
#[test]
fn the_projection_uses_the_pmc_settings() {
    let mut store = database("projection_settings");
    store.set_setting("pmc.tau_ctl_d", "50").unwrap();
    store.set_setting("pmc.tau_atl_d", "15").unwrap();
    let d = plans::generate(&mut store, &input(Some(2)), ID_A.into(), now()).unwrap();
    let first_load: f32 = d.plan.weeks[0].days[0]
        .sessions
        .iter()
        .map(|s| s.planned_load)
        .sum();
    let w_ctl = 1.0 - (-1.0f32 / 50.0).exp();
    let w_atl = 1.0 - (-1.0f32 / 15.0).exp();
    assert!((w_ctl - 0.019_801_3).abs() < 1e-6 && (w_atl - 0.064_493).abs() < 1e-5);
    let ctl1 = d.ctl0 + (first_load - d.ctl0) * w_ctl;
    let atl1 = d.atl0 + (first_load - d.atl0) * w_atl;
    assert!((d.projection[0].ctl - ctl1).abs() < 1e-3);
    assert!((d.projection[0].atl - atl1).abs() < 1e-3);
    // Die Startwerte am Vortag sind die der Einstellungen, nicht die der Standardwerte.
    let defaults = database("projection_defaults");
    let state = defaults
        .pmc_state_at(
            chrono::NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
            &lastwerk_core::PmcParams::default(),
        )
        .unwrap();
    assert!(
        (d.ctl0 - state.ctl).abs() > 1e-4,
        "{} {}",
        d.ctl0,
        state.ctl
    );
}

#[test]
fn invalid_inputs_are_rejected_before_anything_is_saved() {
    let mut store = database("invalid");
    let mut bad = input(Some(3));
    bad.week_start = "2026-06-02".into();
    assert_eq!(
        code(plans::generate(&mut store, &bad, ID_A.into(), now())),
        "not_monday"
    );
    bad.week_start = "kein Datum".into();
    assert_eq!(
        code(plans::generate(&mut store, &bad, ID_A.into(), now())),
        "invalid_date"
    );
    assert_eq!(
        code(plans::generate(
            &mut store,
            &input(Some(27)),
            ID_A.into(),
            now()
        )),
        "invalid_value"
    );
    assert_eq!(
        code(plans::generate(
            &mut store,
            &input(Some(0)),
            ID_A.into(),
            now()
        )),
        "invalid_value"
    );
    let mut bad = input(Some(3));
    bad.strength_per_week = 3;
    assert_eq!(
        code(plans::generate(&mut store, &bad, ID_A.into(), now())),
        "invalid_value"
    );
    let mut bad = input(Some(3));
    bad.sports = vec![PlanSport::Strength];
    assert_eq!(
        code(plans::generate(&mut store, &bad, ID_A.into(), now())),
        "invalid_sport"
    );
    assert!(plans::list_plans(&store).unwrap().is_empty());
}

#[test]
fn at_most_one_plan_is_active() {
    let mut store = database("status");
    plans::generate(&mut store, &input(Some(2)), ID_A.into(), now()).unwrap();
    plans::generate(&mut store, &input(Some(2)), ID_B.into(), now()).unwrap();
    let list = plans::set_plan_status(&mut store, ID_A, "active").unwrap();
    let status = |id: &str| list.iter().find(|p| p.id == id).unwrap().status.clone();
    assert_eq!(
        (status(ID_A).as_str(), status(ID_B).as_str()),
        ("active", "draft")
    );
    let list = plans::set_plan_status(&mut store, ID_B, "active").unwrap();
    let status = |id: &str| list.iter().find(|p| p.id == id).unwrap().status.clone();
    assert_eq!(
        (status(ID_A).as_str(), status(ID_B).as_str()),
        ("archived", "active")
    );
    assert_eq!(plans::get_plan(&store, ID_B).unwrap().status, "active");
    assert_eq!(
        code(plans::set_plan_status(&mut store, ID_B, "unsinn")),
        "invalid_value"
    );
    assert_eq!(
        code(plans::set_plan_status(&mut store, "gibt-es-nicht", "draft")),
        "not_found"
    );
    assert_eq!(code(plans::get_plan(&store, "gibt-es-nicht")), "not_found");
}

#[test]
fn goal_and_availability_feed_the_planner() {
    let mut store = database("inputs");
    let empty = plans::planning_inputs(&store).unwrap();
    assert!(empty.goals.is_empty());
    assert_eq!(empty.availability.len(), 7);
    assert_eq!(empty.availability_set, 0);
    assert_eq!(empty.default_minutes, 90);
    assert!(
        empty
            .availability
            .iter()
            .all(|d| d.max_minutes == 90 && d.allow_hard)
    );

    // Ein Termin in der Planwoche 4, Sonntag fester Ruhetag, Mittwoch nur 30 Minuten.
    let inputs = plans::set_goal(
        &mut store,
        &GoalInput {
            event_date: Some("2026-06-27".into()),
            sport: Some(PlanSport::Cycling),
            priority: Some(Priority::A),
            label: Some("  Mein Rennen  ".into()),
        },
    )
    .unwrap();
    assert_eq!(inputs.goals.len(), 1);
    assert!(inputs.goals[0].active);
    assert_eq!(inputs.goals[0].kind, "event");
    assert_eq!(inputs.goals[0].label.as_deref(), Some("Mein Rennen"));
    let days = [
        AvailabilityDay {
            weekday: 2,
            max_minutes: 30,
            allow_hard: false,
            fixed_rest: false,
        },
        AvailabilityDay {
            weekday: 6,
            max_minutes: 0,
            allow_hard: false,
            fixed_rest: true,
        },
    ];
    let inputs = plans::set_availability(&mut store, &days).unwrap();
    assert_eq!(inputs.availability_set, 2);
    assert_eq!(inputs.availability[2].max_minutes, 30);
    assert!(inputs.availability[6].fixed_rest);

    let d = plans::generate(&mut store, &input(None), ID_A.into(), now()).unwrap();
    assert_eq!(
        d.plan.goal.event_date.map(|x| x.to_string()).as_deref(),
        Some("2026-06-27")
    );
    // Bis zur Woche des Termins: Woche vom 22. Juni, vier Wochen ab 1. Juni.
    assert_eq!(d.plan.weeks.len(), 4);
    for w in &d.plan.weeks {
        let wed = &w.days[2];
        assert!(wed.sessions.iter().map(|s| s.duration_min).sum::<u32>() <= 30);
        assert!(w.days[6].sessions.is_empty(), "fester Ruhetag am Sonntag");
    }
    assert!(d.validation.violations.iter().all(|v| !v.hard));

    // Ein späteres Ziel ohne Datum ist fortlaufend und ersetzt das aktive.
    let inputs = plans::set_goal(
        &mut store,
        &GoalInput {
            event_date: None,
            sport: Some(PlanSport::Cycling),
            priority: Some(Priority::B),
            label: None,
        },
    )
    .unwrap();
    assert_eq!(inputs.goals.len(), 2);
    assert_eq!(inputs.goals[0].kind, "continuous");
    assert_eq!(
        inputs.goals[0].priority, None,
        "Priorität gibt es nur bei einem Termin"
    );
    assert_eq!(inputs.goals.iter().filter(|g| g.active).count(), 1);
}

#[test]
fn invalid_goals_and_availability_change_nothing() {
    let mut store = database("invalid-inputs");
    let goal = |date: Option<&str>, sport: Option<PlanSport>, label: Option<String>| GoalInput {
        event_date: date.map(Into::into),
        sport,
        priority: None,
        label,
    };
    assert_eq!(
        code(plans::set_goal(
            &mut store,
            &goal(Some("2026-13-40"), None, None)
        )),
        "invalid_date"
    );
    assert_eq!(
        code(plans::set_goal(
            &mut store,
            &goal(None, Some(PlanSport::Strength), None)
        )),
        "invalid_sport"
    );
    assert_eq!(
        code(plans::set_goal(
            &mut store,
            &goal(None, None, Some("x".repeat(201)))
        )),
        "invalid_value"
    );
    assert!(plans::planning_inputs(&store).unwrap().goals.is_empty());

    let ok = AvailabilityDay {
        weekday: 0,
        max_minutes: 60,
        allow_hard: true,
        fixed_rest: false,
    };
    let too_long = AvailabilityDay {
        weekday: 1,
        max_minutes: 1441,
        ..ok
    };
    let wrong_day = AvailabilityDay { weekday: 7, ..ok };
    assert_eq!(
        code(plans::set_availability(&mut store, &[ok, too_long])),
        "invalid_value"
    );
    assert_eq!(
        code(plans::set_availability(&mut store, &[ok, wrong_day])),
        "invalid_value"
    );
    assert_eq!(
        code(plans::set_availability(&mut store, &[ok, ok])),
        "invalid_value"
    );
    assert_eq!(plans::planning_inputs(&store).unwrap().availability_set, 0);
}

#[test]
fn exports_follow_the_cli_files() {
    let mut store = database("exports");
    let d = plans::generate(&mut store, &input(Some(2)), ID_A.into(), now()).unwrap();

    let (ics_text, events, name) = plans::export_ics(&store, ID_A).unwrap();
    assert_eq!(name, "lastwerk-plan-2026-06-01.ics");
    assert_eq!(events as usize, sessions(&d));
    // Dieselbe Datei wie `lastwerk export-ics`.
    let texts = Texts::load(d.plan.locale).unwrap();
    assert_eq!(ics_text, ics::plan_to_ics(&d.plan, &texts));
    assert_eq!(ics_text.matches("BEGIN:VEVENT").count(), sessions(&d));
    assert!(!ics_text.contains("LOCATION") && !ics_text.contains("GEO"));

    let (json, name) = plans::export_plan_json(&store, ID_A).unwrap();
    assert_eq!(name, "lastwerk-plan-2026-06-01.json");
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(schema::plan_errors(&value).unwrap().is_empty());
    assert_eq!(json, planning::plan_json(&d.plan).unwrap());

    assert_eq!(
        code(plans::export_ics(&store, "gibt-es-nicht")),
        "not_found"
    );
}

/// Wiederherstellungstest (SPEC 6.3): Backup, frische Datenbank, Einspielen: Schwellenwerte,
/// Ziele, Verfügbarkeit und Pläne sind wieder da.
#[test]
fn backup_and_restore_bring_the_user_data_back() {
    let mut store = database("backup");
    plans::set_goal(
        &mut store,
        &GoalInput {
            event_date: Some("2026-06-27".into()),
            sport: None,
            priority: Some(Priority::A),
            label: None,
        },
    )
    .unwrap();
    plans::generate(&mut store, &input(Some(2)), ID_A.into(), now()).unwrap();
    let (text, counts, name) = plans::export_backup(&store, now()).unwrap();
    assert!(name.starts_with("lastwerk-backup-") && name.ends_with(".json"));
    assert_eq!((counts.goals, counts.plans, counts.thresholds), (1, 1, 1));
    assert_eq!(counts.recovery_days, 0);
    assert!(text.contains("\"exported_at\""));

    let mut fresh = Store::open(&work_dir("backup-fresh").join("lastwerk.db")).unwrap();
    assert!(plans::list_plans(&fresh).unwrap().is_empty());
    let restored = plans::restore_backup(&mut fresh, &text).unwrap();
    assert_eq!(restored, counts);
    assert_eq!(
        plans::list_plans(&fresh).unwrap(),
        plans::list_plans(&store).unwrap()
    );
    assert_eq!(
        plans::get_plan(&fresh, ID_A).unwrap().plan,
        plans::get_plan(&store, ID_A).unwrap().plan
    );
    assert_eq!(plans::planning_inputs(&fresh).unwrap().goals.len(), 1);
    assert_eq!(fresh.list_thresholds().unwrap().len(), 1);
}

#[test]
fn an_invalid_backup_leaves_the_data_unchanged() {
    let mut store = database("restore-invalid");
    plans::generate(&mut store, &input(Some(2)), ID_A.into(), now()).unwrap();
    assert_eq!(
        code(plans::restore_backup(&mut store, "kein json")),
        "restore_invalid"
    );
    assert_eq!(
        code(plans::restore_backup(
            &mut store,
            "{\"backup_version\": 99}"
        )),
        "restore_invalid"
    );
    let (text, _, _) = plans::export_backup(&store, now()).unwrap();
    let wrong_version = text.replace("\"backup_version\": 1", "\"backup_version\": 99");
    assert_ne!(wrong_version, text);
    assert_eq!(
        code(plans::restore_backup(&mut store, &wrong_version)),
        "restore_invalid"
    );
    assert_eq!(plans::list_plans(&store).unwrap().len(), 1);
}
