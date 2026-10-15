//! Pläne, Ziele, Verfügbarkeit und die Historie für den Planer (T08, SPEC 5.4, 6.2, 6.3).
//! Alle Daten sind synthetisch.

mod common;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use common::date;
use lastwerk_core::plan::{
    DayAvailability, GoalKind, GoalSpec, Locale, Plan, PlanContext, PlanSport, Priority, Rules,
    SportWeight, TemplateCatalog, generate_plan,
};
use lastwerk_core::{PmcParams, Sport};
use lastwerk_store::{
    ActivityRow, LoadRow, LoadStatus, NewGoal, PlanStatus, SourceFileRow, SourceStatus, Store,
    StoreError, UserDataExport, monday_of,
};

const TEMPLATES_JSON: &str = include_str!("../../../catalog/templates.json");
const RULES_JSON: &str = include_str!("../../../catalog/rules.default.json");

fn created_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 13, 8, 0, 0).unwrap()
}

/// Plan für vier Wochen ab dem 4. Januar 2027, mit fester ID.
fn sample_plan(plan_id: &str, created: DateTime<Utc>) -> Plan {
    let rules: Rules = serde_json::from_str(RULES_JSON).unwrap();
    let catalog: TemplateCatalog = serde_json::from_str(TEMPLATES_JSON).unwrap();
    let day = DayAvailability {
        max_minutes: 90,
        allow_hard: true,
        fixed_rest: false,
    };
    let ctx = PlanContext {
        plan_id: plan_id.to_string(),
        created_at: created,
        locale: Locale::De,
        week_start: date(2027, 1, 4),
        goal: GoalSpec::Continuous,
        horizon_weeks: 4,
        ctl0: 30.0,
        atl0: 35.0,
        history: Vec::new(),
        availability: [day; 7],
        sports: vec![SportWeight {
            sport: PlanSport::Cycling,
            weight: 1.0,
        }],
        strength_per_week: 1,
        ctl_rise_per_week: None,
        pmc_params: PmcParams::default(),
        catalog,
        recovery: Vec::new(),
    };
    generate_plan(&ctx, &rules).unwrap()
}

const ID_A: &str = "00000000-0000-4000-8000-00000000000a";
const ID_B: &str = "00000000-0000-4000-8000-00000000000b";

#[test]
fn saved_plans_come_back_unchanged() {
    let mut store = Store::open_in_memory().unwrap();
    let plan = sample_plan(ID_A, created_at());
    store.save_plan(&plan, PlanStatus::Draft).unwrap();
    assert_eq!(store.load_plan(ID_A).unwrap(), Some(plan.clone()));
    assert_eq!(store.load_plan(ID_B).unwrap(), None);

    let list = store.list_plans().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, ID_A);
    assert_eq!(list[0].status, PlanStatus::Draft);
    assert_eq!(list[0].created_at, created_at());
    assert_eq!(list[0].rules_version, plan.rules_version);
    assert_eq!(list[0].locale, Locale::De);
    assert_eq!(list[0].goal_id, None);

    // Gleiche ID: ersetzt, keine zweite Zeile.
    store.save_plan(&plan, PlanStatus::Archived).unwrap();
    let list = store.list_plans().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].status, PlanStatus::Archived);
}

#[test]
fn only_one_plan_is_active_at_a_time() {
    let mut store = Store::open_in_memory().unwrap();
    let a = sample_plan(ID_A, created_at());
    let b = sample_plan(ID_B, created_at() + chrono::TimeDelta::hours(1));
    store.save_plan(&a, PlanStatus::Active).unwrap();
    assert_eq!(store.active_plan().unwrap(), Some(a.clone()));
    // Ein Entwurf berührt den aktiven Plan nicht.
    store.save_plan(&b, PlanStatus::Draft).unwrap();
    assert_eq!(store.active_plan().unwrap(), Some(a.clone()));
    // Aktivieren archiviert den bisherigen.
    store.set_plan_status(ID_B, PlanStatus::Active).unwrap();
    assert_eq!(store.active_plan().unwrap(), Some(b.clone()));
    let status = |s: &Store, id: &str| {
        s.list_plans()
            .unwrap()
            .into_iter()
            .find(|p| p.id == id)
            .unwrap()
            .status
    };
    assert_eq!(status(&store, ID_A), PlanStatus::Archived);
    assert_eq!(status(&store, ID_B), PlanStatus::Active);
    // Neueste zuerst.
    assert_eq!(store.list_plans().unwrap()[0].id, ID_B);
    // Speichern als aktiv archiviert den anderen ebenfalls.
    store.save_plan(&a, PlanStatus::Active).unwrap();
    assert_eq!(status(&store, ID_B), PlanStatus::Archived);
    assert_eq!(
        store
            .list_plans()
            .unwrap()
            .iter()
            .filter(|p| p.status == PlanStatus::Active)
            .count(),
        1
    );
    // Unbekannte ID.
    assert!(matches!(
        store.set_plan_status("nope", PlanStatus::Active),
        Err(StoreError::PlanNotFound(_))
    ));
    assert_eq!(store.active_plan().unwrap(), Some(a));
}

#[test]
fn goals_and_availability_are_stored() {
    let mut store = Store::open_in_memory().unwrap();
    let event = NewGoal {
        kind: GoalKind::Event,
        sport: Some(PlanSport::Cycling),
        event_date: Some(date(2027, 5, 1)),
        label: Some("Zielrennen".to_string()),
        priority: Some(Priority::A),
    };
    let first = store.set_goal(&event).unwrap();
    let second = store
        .set_goal(&NewGoal {
            kind: GoalKind::Continuous,
            sport: None,
            event_date: None,
            label: None,
            priority: None,
        })
        .unwrap();
    assert_ne!(first, second);
    let goals = store.list_goals().unwrap();
    assert_eq!(goals.len(), 2);
    assert!(!goals[0].active);
    assert_eq!(goals[0].goal, event);
    assert!(goals[1].active);
    assert_eq!(store.active_goal().unwrap().unwrap().id, second);

    // Ein Termin braucht ein Datum, ein fortlaufendes Ziel hat keins.
    let bad = NewGoal {
        event_date: None,
        ..event.clone()
    };
    assert!(matches!(
        store.set_goal(&bad),
        Err(StoreError::InvalidInput(_))
    ));
    let bad = NewGoal {
        kind: GoalKind::Continuous,
        ..event
    };
    assert!(matches!(
        store.set_goal(&bad),
        Err(StoreError::InvalidInput(_))
    ));

    store.set_availability(4, 0, false, true).unwrap();
    store.set_availability(5, 240, true, false).unwrap();
    store.set_availability(5, 180, true, false).unwrap();
    let av = store.list_availability().unwrap();
    assert_eq!(av.len(), 2);
    assert_eq!((av[0].weekday, av[0].max_minutes), (4, 0));
    assert!(av[0].fixed_rest && !av[0].allow_hard);
    assert_eq!((av[1].weekday, av[1].max_minutes), (5, 180));
    assert!(store.set_availability(7, 60, true, false).is_err());
    assert!(store.set_availability(0, 1441, true, false).is_err());
}

#[test]
fn plan_goal_reference_survives_backup_and_restore() {
    let mut store = Store::open_in_memory().unwrap();
    let goal_id = store
        .set_goal(&NewGoal {
            kind: GoalKind::Event,
            sport: Some(PlanSport::Running),
            event_date: Some(date(2027, 5, 1)),
            label: None,
            priority: Some(Priority::B),
        })
        .unwrap();
    let plan = sample_plan(ID_A, created_at());
    store
        .save_plan_for_goal(&plan, PlanStatus::Active, Some(goal_id))
        .unwrap();
    store.set_availability(2, 60, false, false).unwrap();
    store.set_setting("athlete.sex", "female").unwrap();

    let backup = store
        .export_user_data()
        .unwrap()
        .with_exported_at(created_at());
    assert_eq!(backup.backup_version, 1);
    assert_eq!(backup.exported_at.as_deref(), Some("2026-10-13T08:00:00Z"));
    let text = backup.to_json().unwrap();
    assert!(text.contains("\"backup_version\": 1"));
    assert!(text.contains("\"exported_at\": \"2026-10-13T08:00:00Z\""));

    let mut fresh = Store::open_in_memory().unwrap();
    fresh
        .import_user_data(&UserDataExport::from_json(&text).unwrap())
        .unwrap();
    assert_eq!(fresh.load_plan(ID_A).unwrap(), Some(plan));
    assert_eq!(fresh.list_plans().unwrap()[0].goal_id, Some(goal_id));
    assert_eq!(fresh.list_goals().unwrap(), store.list_goals().unwrap());
    assert_eq!(
        fresh.list_availability().unwrap(),
        store.list_availability().unwrap()
    );
    // Gleiche Nutzerdaten, bis auf den Zeitpunkt des Exports.
    assert_eq!(
        fresh.export_user_data().unwrap(),
        store.export_user_data().unwrap()
    );

    // Eine Datei mit fremder Version wird abgelehnt, der Bestand bleibt.
    let newer = text.replace("\"backup_version\": 1", "\"backup_version\": 2");
    assert!(matches!(
        fresh.import_user_data(&UserDataExport::from_json(&newer).unwrap()),
        Err(StoreError::UnsupportedExportVersion(2))
    ));
    assert!(fresh.load_plan(ID_A).unwrap().is_some());
    // Ältere Dateien aus T04 nannten das Feld `format_version`.
    let old = text.replace("\"backup_version\"", "\"format_version\"");
    UserDataExport::from_json(&old).unwrap();
}

// ---------------------------------------------------------------------------
// Historie
// ---------------------------------------------------------------------------

struct Seed<'a> {
    store: &'a mut Store,
    next: usize,
}

impl Seed<'_> {
    /// Legt eine Aktivität mit einer Belastung an. Datum lokal, Dauer in Sekunden.
    fn activity(
        &mut self,
        sport: Sport,
        day: NaiveDate,
        duration_s: i64,
        load: f64,
        status: LoadStatus,
    ) {
        self.next += 1;
        let at = Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap();
        let file = self
            .store
            .upsert_source_file(&SourceFileRow {
                rel_path: format!("synthetic/{}.fit", self.next),
                sha256: format!("{:064x}", self.next),
                size_bytes: 1,
                mtime_utc: at,
                imported_at: at,
                status: SourceStatus::Ok,
                error_code: None,
            })
            .unwrap();
        let start = day.and_hms_opt(8, 0, 0).unwrap().and_utc();
        let id = self
            .store
            .upsert_activity(&ActivityRow {
                source_file_id: file,
                sport,
                sub_sport: None,
                start_utc: start,
                local_offset_s: 0,
                local_date: day,
                duration_s,
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
        self.store
            .upsert_activity_load(&LoadRow {
                activity_id: id,
                method: "tss".to_string(),
                value: (status == LoadStatus::Ok).then_some(load),
                intensity_factor: None,
                threshold_id: None,
                status,
                computed_at: at,
            })
            .unwrap();
    }
}

#[test]
fn week_history_sums_load_and_minutes_per_iso_week_and_sport() {
    let mut store = Store::open_in_memory().unwrap();
    let mut seed = Seed {
        store: &mut store,
        next: 0,
    };
    let ok = LoadStatus::Ok;
    // Woche ab Montag 28.12.2026: Rad 3600 s mit 60 und 7200 s mit 100, Lauf 1800 s mit 30.
    seed.activity(Sport::Cycling, date(2026, 12, 29), 3600, 60.0, ok);
    seed.activity(Sport::Cycling, date(2027, 1, 2), 7200, 100.0, ok);
    seed.activity(Sport::Running, date(2026, 12, 30), 1800, 30.0, ok);
    // Ohne Schwelle: keine Last (fehlende Daten sind nicht Null), die Zeit zählt aber (R2
    // vergleicht die Trainingszeit aller Einheiten, SPEC 8.4).
    seed.activity(
        Sport::Cycling,
        date(2026, 12, 31),
        3600,
        0.0,
        LoadStatus::MissingThreshold,
    );
    // Kraft aus FIT hat nie eine Last (`missing_channel`, RPE nur von Hand), 2700 s zählen als
    // Zeit, die Last bleibt 0.
    seed.activity(
        Sport::Strength,
        date(2027, 1, 1),
        2700,
        0.0,
        LoadStatus::MissingChannel,
    );
    // Woche ab Montag 21.12.2026, Sonntag 27.12. gehört noch dazu.
    seed.activity(Sport::Swimming, date(2026, 12, 27), 2700, 40.0, ok);
    // Woche ab 4.1.2027 (die Woche von `before`) und Woche ab 14.12.2026 (zu alt für 2 Wochen).
    seed.activity(Sport::Cycling, date(2027, 1, 4), 3600, 50.0, ok);
    seed.activity(Sport::Cycling, date(2026, 12, 15), 3600, 70.0, ok);

    let monday = date(2027, 1, 4);
    let h = store.week_history(monday, 2).unwrap();
    let rows: Vec<_> = h
        .iter()
        .map(|w| (w.week_start, w.sport, w.load, w.minutes))
        .collect();
    assert_eq!(
        rows,
        vec![
            (date(2026, 12, 21), Sport::Swimming, 40.0, 45.0),
            // 60 + 100 = 160 Last (die Fahrt ohne Schwelle zählt nicht),
            // 3600 s + 7200 s + 3600 s = 240 Minuten (sie zählt als Zeit).
            (date(2026, 12, 28), Sport::Cycling, 160.0, 240.0),
            (date(2026, 12, 28), Sport::Running, 30.0, 30.0),
            // 2700 s = 45 Minuten, Last 0.
            (date(2026, 12, 28), Sport::Strength, 0.0, 45.0),
        ]
    );
    // Wochensumme wie in `PlanContext::previous_week`: Last 160 + 30 = 190 (nur `ok`),
    // Zeit 240 + 30 + 45 = 315 Minuten (alle primären Aktivitäten).
    let last: Vec<_> = h
        .iter()
        .filter(|w| w.week_start == date(2026, 12, 28))
        .collect();
    assert_eq!(last.iter().map(|w| w.load).sum::<f32>(), 190.0);
    assert_eq!(last.iter().map(|w| w.minutes).sum::<f32>(), 315.0);
    // Ein Datum mitten in der Woche meint dieselbe Woche.
    assert_eq!(store.week_history(date(2027, 1, 6), 2).unwrap(), h);
    // Eine Woche zurück: nur die jüngste.
    assert_eq!(store.week_history(monday, 1).unwrap().len(), 3);
    // Drei Wochen: auch die vom 15.12.
    assert_eq!(store.week_history(monday, 3).unwrap().len(), 5);
    assert!(store.week_history(monday, 0).unwrap().is_empty());
    assert_eq!(monday_of(date(2027, 1, 6)), monday);
}

#[test]
fn pmc_state_at_follows_the_hand_calculation() {
    let mut store = Store::open_in_memory().unwrap();
    let mut seed = Seed {
        store: &mut store,
        next: 0,
    };
    // Zwei Tage mit Last 100 (SPEC 8.2): Tag 1 CTL 2,3528 und ATL 13,3122, Tag 2 CTL 4,6503
    // und ATL 24,8523. Tag 3 ohne Last: CTL = 4,6503 * (1 - 0,023528) = 4,5409 und
    // ATL = 24,8523 * (1 - 0,133122) = 21,5439.
    let ok = LoadStatus::Ok;
    seed.activity(Sport::Cycling, date(2027, 1, 1), 3600, 100.0, ok);
    seed.activity(Sport::Cycling, date(2027, 1, 2), 3600, 100.0, ok);
    let p = PmcParams::default();

    let day2 = store.pmc_state_at(date(2027, 1, 2), &p).unwrap();
    assert!((day2.ctl - 4.6503).abs() < 1e-3, "{}", day2.ctl);
    assert!((day2.atl - 24.8523).abs() < 1e-3, "{}", day2.atl);
    let day3 = store.pmc_state_at(date(2027, 1, 3), &p).unwrap();
    assert!((day3.ctl - 4.5409).abs() < 1e-3, "{}", day3.ctl);
    assert!((day3.atl - 21.5439).abs() < 2e-3, "{}", day3.atl);
    // Vor dem ersten Tag mit Last gelten die Startwerte.
    let start = PmcParams {
        ctl0: 10.0,
        atl0: 20.0,
        ..p
    };
    let before = store.pmc_state_at(date(2026, 12, 31), &start).unwrap();
    assert_eq!((before.ctl, before.atl), (10.0, 20.0));
    // Spätere Lasten verändern den Stand eines früheren Tages nicht.
    let mut seed = Seed {
        store: &mut store,
        next: 10,
    };
    seed.activity(Sport::Cycling, date(2027, 1, 10), 3600, 500.0, ok);
    assert_eq!(
        store.pmc_state_at(date(2027, 1, 2), &p).unwrap().ctl,
        day2.ctl
    );
    // Ohne jede Last: Startwerte.
    let empty = Store::open_in_memory().unwrap();
    let e = empty.pmc_state_at(date(2027, 1, 2), &start).unwrap();
    assert_eq!((e.ctl, e.atl), (10.0, 20.0));
}
