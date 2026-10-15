//! Tests des Eingangs (SPEC 9.3): nur aggregierte Werte, nur feste Felder, keine Rohwerte.

mod common;

use std::collections::BTreeSet;

use common::{base_plan, catalog, context, date, rules};
use lastwerk_coach::{FakeClient, RefineConfig, RefineInput, RefineOutcome, refine_plan};
use lastwerk_core::plan::{PlanWarning, RecoveryDay, Severity};
use serde_json::Value;

/// Alle Schlüssel, die der Eingang haben darf. Ein neues Feld braucht einen Eintrag hier und
/// damit eine bewusste Entscheidung gegen SPEC 9.3.
const ALLOWED_KEYS: &[&str] = &[
    "input_version",
    "goal",
    "kind",
    "sport",
    "weeks_to_event",
    "fitness",
    "ctl",
    "atl",
    "tsb",
    "recovery_signal",
    "weekly_load",
    "weeks_ago",
    "load_by_sport",
    "cycling",
    "running",
    "swimming",
    "strength",
    "other",
    "availability",
    "weekday",
    "max_minutes",
    "allow_hard",
    "fixed_rest",
    "weeks",
    "week_start",
    "phase",
    "base_plan",
    "date",
    "slot",
    "template_id",
    "duration_min",
    "allowed_templates",
    "class",
    "min_duration_min",
    "max_duration_min",
    "guardrails",
    "code",
    "hardness",
    "params",
    "max_ctl_rise_per_week",
    "warn_at",
    "reject_at",
    "max_hard_per_week",
    "min_rest_days_per_week",
    "load_weeks",
    "recovery_min",
    "recovery_max",
    "min_per_week",
    "max_per_week",
    "max_per_week_taper",
    "min_easy_share",
    "taper_days_min",
    "taper_days_max",
    "tsb_warn_below",
    "max_sessions_per_day",
    "hrv_drop_fraction",
    "sleep_deficit_fraction",
];

fn collect_keys(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            for (k, x) in map {
                out.insert(k.clone());
                collect_keys(x, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|x| collect_keys(x, out)),
        _ => {}
    }
}

fn collect_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Object(map) => map.values().for_each(|x| collect_strings(x, out)),
        Value::Array(items) => items.iter().for_each(|x| collect_strings(x, out)),
        _ => {}
    }
}

fn input() -> RefineInput {
    let ctx = context();
    RefineInput::build(&base_plan(&ctx), &ctx, &rules())
}

#[test]
fn aggregates_are_rounded_and_hand_calculated() {
    let i = input();
    // CTL 40,4 wird 40, ATL 45,6 wird 46, TSB = 40,4 - 45,6 = -5,2 wird -5.
    assert_eq!((i.fitness.ctl, i.fitness.atl, i.fitness.tsb), (40, 46, -5));
    // Termin am 16. Januar, Planbeginn am 4. Januar: 12 Tage, eine ganze Woche.
    assert_eq!(i.goal.weeks_to_event, Some(1));
    assert_eq!(i.goal.kind, "event");
    // Woche 1 vor dem Planbeginn: Rad 250 + 10 * 1 = 260, Lauf 80.
    assert_eq!(i.weekly_load.len(), 4);
    assert_eq!(i.weekly_load[0].weeks_ago, 1);
    assert_eq!(i.weekly_load[0].load_by_sport["cycling"], 260);
    assert_eq!(i.weekly_load[0].load_by_sport["running"], 80);
    assert_eq!(i.weekly_load[3].load_by_sport["cycling"], 290);
    assert_eq!(i.availability.len(), 7);
    assert_eq!(i.availability[0].weekday, "mon");
    assert_eq!(i.availability[6].weekday, "sun");
    assert_eq!(i.weeks.len(), 2);
    assert!(!i.recovery_signal);
}

#[test]
fn only_allowed_keys_and_known_strings_appear() {
    let value: Value = serde_json::from_str(&input().to_json()).unwrap();
    let mut keys = BTreeSet::new();
    collect_keys(&value, &mut keys);
    let allowed: BTreeSet<String> = ALLOWED_KEYS.iter().map(|s| (*s).to_string()).collect();
    let extra: Vec<_> = keys.difference(&allowed).collect();
    assert!(extra.is_empty(), "unerlaubte Schlüssel: {extra:?}");

    // Jeder Text ist ein fester Code, eine Vorlage des Katalogs oder ein Datum.
    let cat = catalog();
    let fixed: BTreeSet<&str> = [
        "event",
        "continuous",
        "cycling",
        "running",
        "swimming",
        "strength",
        "am",
        "pm",
        "easy",
        "moderate",
        "hard",
        "soft",
        "mon",
        "tue",
        "wed",
        "thu",
        "fri",
        "sat",
        "sun",
        "base",
        "build",
        "peak",
        "taper",
        "recovery",
        "race",
        "R01",
        "R02",
        "R03",
        "R04",
        "R05",
        "R06",
        "R07",
        "R08",
        "R09",
        "R10",
        "R11",
        "R12",
    ]
    .into_iter()
    .collect();
    let mut strings = Vec::new();
    collect_strings(&value, &mut strings);
    for s in strings {
        let known = fixed.contains(s.as_str())
            || cat.get(&s).is_some()
            || chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").is_ok();
        assert!(known, "unerwarteter Text im Eingang: {s:?}");
    }
}

#[test]
fn recovery_values_never_appear_only_a_flag() {
    let mut ctx = context();
    // Basislinie 60 ms über sieben Tage, letzter Tag 51 ms: Abfall 15 Prozent, Signal.
    for k in 2..=8 {
        ctx.recovery.push(RecoveryDay {
            date: ctx.week_start - chrono::TimeDelta::days(k),
            hrv_overnight_ms: Some(60.0),
            resting_hr_bpm: Some(47.25),
            sleep_duration_s: Some(27_001),
            sleep_score: Some(83),
        });
    }
    ctx.recovery.push(RecoveryDay {
        date: ctx.week_start - chrono::TimeDelta::days(1),
        hrv_overnight_ms: Some(51.0),
        resting_hr_bpm: Some(47.25),
        sleep_duration_s: Some(27_001),
        sleep_score: Some(83),
    });
    let base = base_plan(&ctx);
    let i = RefineInput::build(&base, &ctx, &rules());
    assert!(i.recovery_signal);
    let json = i.to_json();
    for raw in [
        "47.25",
        "27001",
        "27_001",
        "\"51",
        "hrv_overnight",
        "sleep_duration",
        "sleep_score",
        "resting",
    ] {
        assert!(!json.contains(raw), "{raw} im Eingang: {json}");
    }
}

#[test]
fn no_goal_label_date_or_path_like_text_exists_in_the_input() {
    let json = input().to_json();
    // Das Datum des Termins steht nicht im Eingang, nur der Abstand in Wochen.
    assert!(!json.contains("2027-01-16"));
    assert!(!json.contains("event_date"));
    assert!(!json.contains("label"));
    assert!(!json.contains("explanation"));
    assert!(!json.contains(".fit"));
    assert!(!json.contains('/'));
    assert!(!json.contains('\\'));
}

#[test]
fn notes_of_the_base_plan_stay_in_the_refined_plan() {
    let ctx = context();
    let mut base = base_plan(&ctx);
    // Ein Hinweis, den keine Regelprüfung ableitet (wie R12 als Info nach dem Austausch).
    let note = PlanWarning {
        code: "R12".into(),
        severity: Severity::Info,
        date: Some(date(2027, 1, 5)),
    };
    base.warnings = vec![note.clone()];
    let client = FakeClient::with_replies([common::reply("", "ok")]);
    let outcome = refine_plan(&base, &ctx, &rules(), &client, &RefineConfig::default());
    let RefineOutcome::Applied { plan, .. } = outcome else {
        panic!("Applied erwartet");
    };
    assert!(plan.warnings.contains(&note));
}
