//! Tests des Plan-Schemas (SPEC 7.1, 11.4): Jeder erzeugte Plan besteht das Schema, und jede
//! Verletzung, die das Schema verbietet, wird erkannt. Alle Pläne sind synthetisch.

use lastwerk_core::plan::{PlanContext, generate_plan};
use proptest::prelude::*;
use serde_json::{Value, json};

use crate::schema::{plan_errors, plan_validator};
use crate::testctx::{arb_ctx, continuous_ctx, event_ctx, rules_and_catalog};

fn valid(plan: &Value) -> bool {
    plan_validator().unwrap().is_valid(plan)
}

fn generated(ctx: &PlanContext) -> Value {
    let (rules, _) = rules_and_catalog();
    serde_json::to_value(generate_plan(ctx, &rules).unwrap()).unwrap()
}

/// Kleinster gültiger Plan: eine Woche, sieben leere Tage.
fn minimal_plan() -> Value {
    let days: Vec<Value> = (4..=10)
        .map(|d| json!({"date": format!("2027-01-{d:02}"), "sessions": []}))
        .collect();
    json!({
        "schema_version": 1,
        "plan_id": "00000000-0000-4000-8000-000000000001",
        "created_at": "2026-10-13T08:00:00Z",
        "locale": "de",
        "source": "rules",
        "rules_version": "1.0.0",
        "goal": {"kind": "continuous"},
        "weeks": [{
            "week_start": "2027-01-04",
            "phase": "base",
            "target_load": 100.0,
            "days": days
        }]
    })
}

#[test]
fn the_embedded_schema_builds_and_rejects_an_empty_object() {
    let errors = plan_errors(&json!({})).unwrap();
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|l| l.contains("schema_version")));
}

#[test]
fn the_minimal_plan_is_valid_and_formats_are_checked() {
    assert!(valid(&minimal_plan()));
    // Nur das Format ist falsch: das zeigt, dass die Formatprüfung aktiv ist.
    for (pointer, bad) in [
        ("/plan_id", "not-a-uuid"),
        ("/created_at", "gestern"),
        ("/weeks/0/week_start", "2027-13-40"),
        ("/weeks/0/days/0/date", "04.01.2027"),
    ] {
        let mut plan = minimal_plan();
        *plan.pointer_mut(pointer).unwrap() = json!(bad);
        assert!(!valid(&plan), "{pointer}");
    }
}

// --- Erzeugte Pläne bestehen das Schema (SPEC 11.4) -------------------------------------

#[test]
fn the_golden_plans_satisfy_the_schema() {
    for ctx in [event_ctx(), continuous_ctx()] {
        let plan = generated(&ctx);
        let errors = plan_errors(&plan).unwrap();
        assert!(errors.is_empty(), "{errors:#?}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 300, ..ProptestConfig::default() })]

    /// Jeder erzeugte Plan besteht das Schema, auch nach dem Weg über Text und zurück.
    #[test]
    fn generated_plans_satisfy_the_schema(ctx in arb_ctx()) {
        let (rules, _) = rules_and_catalog();
        let plan = generate_plan(&ctx, &rules).unwrap();
        let value = serde_json::to_value(&plan).unwrap();
        let errors = plan_errors(&value).unwrap();
        prop_assert!(errors.is_empty(), "{:#?}", errors);
        let text = crate::planning::plan_json(&plan).unwrap();
        // Der geschriebene Text besteht das Schema und gibt denselben Plan zurück.
        let back: Value = serde_json::from_str(&text).unwrap();
        prop_assert!(plan_errors(&back).unwrap().is_empty());
        let reread: lastwerk_core::plan::Plan = serde_json::from_str(&text).unwrap();
        prop_assert_eq!(reread, plan);
    }
}

// --- Negativtests: jede Pflichtangabe fehlt einmal --------------------------------------

/// Plan mit mindestens einer Einheit, dazu Ort der ersten Einheit.
fn plan_with_session() -> (Value, usize, usize) {
    let plan = generated(&event_ctx());
    for (w, week) in plan["weeks"].as_array().unwrap().iter().enumerate() {
        for (d, day) in week["days"].as_array().unwrap().iter().enumerate() {
            if !day["sessions"].as_array().unwrap().is_empty() {
                return (plan, w, d);
            }
        }
    }
    panic!("der Plan enthält keine Einheit");
}

fn remove_at(plan: &mut Value, pointer: &str) {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    let parent = if parent.is_empty() {
        &mut *plan
    } else {
        plan.pointer_mut(parent).unwrap()
    };
    let removed = parent.as_object_mut().unwrap().remove(key);
    assert!(removed.is_some(), "{pointer} fehlt im Beispielplan");
}

#[test]
fn every_required_field_is_enforced() {
    let (mut plan, w, d) = plan_with_session();
    // Der Beispielplan hat ein Ziel mit Termin, damit auch `event_date` fehlen kann.
    assert_eq!(plan["goal"]["kind"], "event");
    assert!(valid(&plan));
    let week = format!("/weeks/{w}");
    let day = format!("{week}/days/{d}");
    let session = format!("{day}/sessions/0");
    let mut required: Vec<String> = [
        "schema_version",
        "plan_id",
        "created_at",
        "locale",
        "source",
        "rules_version",
        "goal",
        "weeks",
    ]
    .iter()
    .map(|k| format!("/{k}"))
    .collect();
    required.push("/goal/kind".to_string());
    required.push("/goal/event_date".to_string());
    for k in ["week_start", "phase", "target_load", "days"] {
        required.push(format!("{week}/{k}"));
    }
    for k in ["date", "sessions"] {
        required.push(format!("{day}/{k}"));
    }
    for k in [
        "slot",
        "sport",
        "template_id",
        "duration_min",
        "planned_load",
        "hard",
    ] {
        required.push(format!("{session}/{k}"));
    }
    assert_eq!(required.len(), 8 + 2 + 4 + 2 + 6);
    for pointer in &required {
        let mut broken = plan.clone();
        remove_at(&mut broken, pointer);
        let errors = plan_errors(&broken).unwrap();
        let field = pointer.rsplit('/').next().unwrap();
        assert!(
            errors.iter().any(|l| l.contains(field)),
            "{pointer} wurde nicht erkannt: {errors:#?}"
        );
    }
    // Die optionalen Felder dürfen fehlen.
    for pointer in ["/warnings", "/explanation", &format!("{week}/planned_load")] {
        let mut ok = plan.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        let parent = if parent.is_empty() {
            &mut ok
        } else {
            ok.pointer_mut(parent).unwrap()
        };
        parent.as_object_mut().unwrap().remove(key);
        assert!(valid(&ok), "{pointer} ist optional");
    }
    plan["explanation"] = json!("Kurzer Text");
    assert!(valid(&plan));
}

#[test]
fn unknown_fields_are_rejected_at_every_level() {
    let (plan, w, d) = plan_with_session();
    for pointer in [
        "",
        "/goal",
        &format!("/weeks/{w}"),
        &format!("/weeks/{w}/days/{d}"),
        &format!("/weeks/{w}/days/{d}/sessions/0"),
    ] {
        let mut broken = plan.clone();
        let target = if pointer.is_empty() {
            &mut broken
        } else {
            broken.pointer_mut(pointer).unwrap()
        };
        target
            .as_object_mut()
            .unwrap()
            .insert("zusatz".into(), json!(1));
        assert!(!valid(&broken), "{pointer:?}");
    }
    let mut broken = minimal_plan();
    broken["warnings"] = json!([{"code": "R02", "severity": "warn", "extra": true}]);
    assert!(!valid(&broken));
}

#[test]
fn an_event_goal_needs_an_event_date() {
    let mut plan = minimal_plan();
    plan["goal"] = json!({"kind": "event", "sport": "cycling"});
    assert!(!valid(&plan));
    plan["goal"] = json!({"kind": "event", "event_date": "2027-03-27"});
    assert!(valid(&plan));
    plan["goal"] = json!({"kind": "continuous"});
    assert!(valid(&plan));
}

#[test]
fn counts_and_value_ranges_are_enforced() {
    let (plan, w, d) = plan_with_session();
    let days = format!("/weeks/{w}/days");
    let day = format!("{days}/{d}");
    let session = format!("{day}/sessions/0");
    let edit = |pointer: &str, f: &dyn Fn(&mut Vec<Value>)| {
        let mut p = plan.clone();
        f(p.pointer_mut(pointer).unwrap().as_array_mut().unwrap());
        p
    };
    // 8 und 6 Tage statt 7.
    assert!(!valid(&edit(&days, &|v| v.push(v[0].clone()))));
    assert!(!valid(&edit(&days, &|v| {
        v.pop();
    })));
    // 3 Einheiten an einem Tag.
    let sessions = format!("{day}/sessions");
    assert!(!valid(&edit(&sessions, &|v| {
        let s = v[0].clone();
        v.resize(3, s);
    })));
    // Keine Wochen und mehr als 26 Wochen.
    assert!(!valid(&edit("/weeks", &|v| v.clear())));
    assert!(!valid(&edit("/weeks", &|v| {
        let first = v[0].clone();
        v.resize(27, first);
    })));
    // Werte der Einheit.
    for (key, bad) in [
        ("template_id", json!("ohnepunkt")),
        ("template_id", json!("Bike.Endurance")),
        ("template_id", json!("bike.")),
        ("duration_min", json!(9)),
        ("duration_min", json!(361)),
        ("duration_min", json!(45.5)),
        ("planned_load", json!(-1)),
        ("hard", json!("ja")),
        ("slot", json!("noon")),
        ("sport", json!("rowing")),
        ("rpe", json!(0)),
        ("rpe", json!(11)),
        ("reason_keys", json!(["Reason Week"])),
    ] {
        let mut p = plan.clone();
        p.pointer_mut(&session).unwrap()[key] = bad.clone();
        assert!(!valid(&p), "{key} = {bad}");
    }
    // Werte des Kopfes und der Woche.
    for (pointer, bad) in [
        ("/schema_version".to_string(), json!(2)),
        ("/locale".to_string(), json!("fr")),
        ("/source".to_string(), json!("magic")),
        ("/rules_version".to_string(), json!("1.0")),
        (format!("/weeks/{w}/phase"), json!("sprint")),
        (format!("/weeks/{w}/target_load"), json!(-5)),
    ] {
        let mut p = plan.clone();
        *p.pointer_mut(&pointer).unwrap() = bad.clone();
        assert!(!valid(&p), "{pointer}");
    }
    let mut p = plan.clone();
    p["explanation"] = json!("x".repeat(1201));
    assert!(!valid(&p));
    p["explanation"] = json!("x".repeat(1200));
    assert!(valid(&p));
    // Warnungen.
    for bad in [
        json!({"code": "R1", "severity": "warn"}),
        json!({"code": "R02", "severity": "fatal"}),
        json!({"severity": "warn"}),
        json!({"code": "R02", "severity": "warn", "date": "morgen"}),
    ] {
        let mut p = plan.clone();
        p["warnings"] = json!([bad]);
        assert!(!valid(&p));
    }
    let mut p = plan.clone();
    p["warnings"] = json!([{"code": "R02", "severity": "warn", "date": "2027-01-18"}]);
    assert!(valid(&p));
}
