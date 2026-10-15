//! Golden-Tests für `plan.json` und `plan.ics` (SPEC 11.4, `insta`). Die Eingabe ist ein fester,
//! synthetischer Kontext: Datenbank im Speicher mit Ziel und Verfügbarkeit, feste `plan_id`
//! und fester Zeitpunkt (die Funktion `planning::generate` bekommt beides als Parameter,
//! damit der Snapshot stabil bleibt). Der Snapshot von `pmc.csv` steht bei `report.rs`.
//!
//! Ein Snapshot wird mit `cargo insta accept` oder `INSTA_UPDATE=always cargo test` neu
//! geschrieben und im Diff geprüft.

use lastwerk_core::plan::{GoalKind, Locale, Plan, PlanSport, Priority};
use lastwerk_store::{NewGoal, PlanStatus, Store};

use crate::ics::plan_to_ics;
use crate::planning::{PlanRequest, generate, plan_json};
use crate::testctx::{PLAN_ID, created_at, date, monday};
use crate::texts::Texts;

/// Datenbank mit Ziel (Termin Samstag, 27. März 2027, Rad) und typischer Verfügbarkeit:
/// Freitag frei, Samstag lang, Montag, Mittwoch und Sonntag ohne harte Einheiten.
fn golden_store() -> Store {
    let mut store = Store::open_in_memory().unwrap();
    store
        .set_goal(&NewGoal {
            kind: GoalKind::Event,
            sport: Some(PlanSport::Cycling),
            event_date: Some(date(2027, 3, 27)),
            label: Some("Zielrennen".to_string()),
            priority: Some(Priority::A),
        })
        .unwrap();
    for (weekday, minutes, hard, rest) in [
        (0, 60, false, false),
        (1, 90, true, false),
        (2, 75, false, false),
        (3, 90, true, false),
        (4, 0, false, true),
        (5, 240, true, false),
        (6, 180, false, false),
    ] {
        store
            .set_availability(weekday, minutes, hard, rest)
            .unwrap();
    }
    store
}

fn golden_plan(locale: Locale) -> Plan {
    let store = golden_store();
    let req = PlanRequest {
        week_start: monday(),
        goal_date: None,
        goal_sport: None,
        weeks: None,
        locale,
        sports: vec![PlanSport::Cycling, PlanSport::Running],
        strength_per_week: 1,
    };
    let generated = generate(&store, &req, PLAN_ID.to_string(), created_at()).unwrap();
    // Das Ziel kommt aus der Datenbank und der Plan verweist darauf.
    assert_eq!(generated.goal_id, Some(1));
    generated.plan
}

#[test]
fn golden_plan_json() {
    let plan = golden_plan(Locale::De);
    assert_eq!(plan.weeks.len(), 12);
    let text = plan_json(&plan).unwrap();
    assert!(text.ends_with("}\n"));
    insta::assert_snapshot!("plan_json", text);
}

#[test]
fn golden_plan_ics() {
    let plan = golden_plan(Locale::De);
    let ics = plan_to_ics(&plan, &Texts::load(Locale::De).unwrap());
    insta::assert_snapshot!("plan_ics", ics);
}

#[test]
fn golden_plan_ics_english() {
    let plan = golden_plan(Locale::En);
    let ics = plan_to_ics(&plan, &Texts::load(Locale::En).unwrap());
    // Nur der Anfang: Die englischen Titel und Beschreibungen kommen aus en.json.
    let head: String = ics.lines().take(24).collect::<Vec<_>>().join("\n");
    insta::assert_snapshot!("plan_ics_en_head", head);
}

#[test]
fn the_plan_is_saved_as_a_draft_and_the_text_gives_the_same_plan_back() {
    let mut store = golden_store();
    let plan = golden_plan(Locale::De);
    store
        .save_plan_for_goal(&plan, PlanStatus::Draft, Some(1))
        .unwrap();
    let text = plan_json(&plan).unwrap();
    let reread: Plan = serde_json::from_str(&text).unwrap();
    assert_eq!(reread, plan);
    assert_eq!(store.load_plan(PLAN_ID).unwrap(), Some(plan));
}
