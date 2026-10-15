//! Attrappen-Fälle für `refine_plan` (SPEC 11.4, "LLM (Attrappe)"): gültig, ungültiges JSON,
//! unbekannte Vorlage, falsche Sportart, Regelverletzung, Injektion im Erklärtext,
//! Zeitüberschreitung, höchstens zwei Korrekturen, gekürzte Erklärung.

mod common;

use common::{base_plan, context, date, reply, rules};
use lastwerk_coach::{
    FakeClient, FallbackReason, LlmError, RefineConfig, RefineOutcome, Role, refine_plan,
};
use lastwerk_core::plan::{
    Hardness, Plan, PlanSource, Slot, validate_plan, violations_to_warnings,
};
use serde_json::Value;

const PLAN_SCHEMA: &str = include_str!("../../../schema/plan.schema.json");

const REPLACE_SPIN_BY_ENDURANCE: &str = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"bike.endurance.steady","duration_min":60}"#;

fn run(client: &FakeClient) -> (RefineOutcome, Plan) {
    run_with(client, &RefineConfig::default())
}

fn run_with(client: &FakeClient, cfg: &RefineConfig) -> (RefineOutcome, Plan) {
    let ctx = context();
    let base = base_plan(&ctx);
    let outcome = refine_plan(&base, &ctx, &rules(), client, cfg);
    (outcome, base)
}

fn assert_schema_valid(plan: &Plan) {
    let schema: Value = serde_json::from_str(PLAN_SCHEMA).unwrap();
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .unwrap();
    let value = serde_json::to_value(plan).unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

fn fallback(outcome: RefineOutcome, base: &Plan) -> (FallbackReason, u8) {
    match outcome {
        RefineOutcome::Fallback {
            plan,
            reason,
            attempts,
        } => {
            // Der Regelplan kommt unverändert zurück.
            assert_eq!(&plan, base);
            (reason, attempts)
        }
        other => panic!("Fallback erwartet, bekommen: {other:?}"),
    }
}

#[test]
fn valid_reply_is_applied_with_recomputed_loads() {
    let client =
        FakeClient::with_replies([reply(REPLACE_SPIN_BY_ENDURANCE, "Längere lockere Einheit.")]);
    let (outcome, base) = run(&client);
    let RefineOutcome::Applied {
        plan,
        explanation,
        attempts,
    } = outcome
    else {
        panic!("Applied erwartet");
    };
    assert_eq!(attempts, 1);
    assert_eq!(client.calls(), 1);
    assert_eq!(explanation, "Längere lockere Einheit.");
    assert_eq!(
        plan.explanation.as_deref(),
        Some("Längere lockere Einheit.")
    );
    assert_eq!(plan.source, PlanSource::Llm);
    assert_eq!(base.source, PlanSource::Rules);

    let day = plan.weeks[0]
        .days
        .iter()
        .find(|d| d.date == date(2027, 1, 7))
        .unwrap();
    let s = &day.sessions[0];
    assert_eq!(s.template_id, "bike.endurance.steady");
    assert_eq!(s.duration_min, 60);
    assert!(!s.hard);
    // Handrechnung (Faustregel, rules.default.json): IF leicht 0,65, Last je Minute
    // 100 / 60 * 0,65^2 = 0,70417, für 60 Minuten 42,25, auf eine Stelle gerundet 42,2 (derselbe
    // Wert steht im Basisplan für diese Vorlage und Dauer am 10. Januar).
    assert_eq!(s.planned_load, 42.2);
    // Woche: Basis 236,8 minus 21,1 (Erholungsfahrt 30 Minuten) plus 42,2 = 257,9.
    assert_eq!(base.weeks[0].planned_load, Some(236.8));
    assert_eq!(plan.weeks[0].planned_load, Some(257.9));
    // Das Wochenziel ist kein Rechenergebnis des Modells und bleibt.
    assert_eq!(plan.weeks[0].target_load, base.weeks[0].target_load);
    // Alles andere bleibt, auch die zweite Woche.
    assert_eq!(plan.weeks[1], base.weeks[1]);
    assert_schema_valid(&plan);
    let ctx = context();
    assert!(
        !validate_plan(&plan, &ctx, &rules())
            .iter()
            .any(|v| v.hardness == Hardness::Hard)
    );
}

#[test]
fn empty_edit_list_keeps_the_plan_but_marks_the_source() {
    let client = FakeClient::with_replies([reply("", "Der Plan passt.")]);
    let (outcome, base) = run(&client);
    let RefineOutcome::Applied { plan, .. } = outcome else {
        panic!("Applied erwartet");
    };
    assert_eq!(plan.source, PlanSource::Llm);
    assert_eq!(plan.weeks, base.weeks);
    assert_eq!(plan.warnings, base.warnings);
}

#[test]
fn move_and_remove_are_applied() {
    // Lauf vom 9. Januar auf den Nachmittag des 8. Januar, Krafteinheit am 6. Januar streichen.
    let edits = r#"{"op":"move","from_date":"2027-01-09","from_slot":"am","to_date":"2027-01-08","to_slot":"pm"},
        {"op":"remove","date":"2027-01-06","slot":"pm"}"#;
    let client = FakeClient::with_replies([reply(edits, "Umgestellt.")]);
    let (outcome, base) = run(&client);
    let RefineOutcome::Applied { plan, attempts, .. } = outcome else {
        panic!("Applied erwartet: {outcome:?}");
    };
    assert_eq!(attempts, 1);
    let day = |d| {
        plan.weeks[0]
            .days
            .iter()
            .find(|x| x.date == d)
            .unwrap()
            .sessions
            .clone()
    };
    let moved = day(date(2027, 1, 8));
    assert_eq!(moved.len(), 2);
    assert_eq!(
        (moved[0].slot, moved[0].template_id.as_str()),
        (Slot::Am, "bike.recovery.spin")
    );
    assert_eq!(
        (moved[1].slot, moved[1].template_id.as_str()),
        (Slot::Pm, "run.easy.steady")
    );
    assert!(day(date(2027, 1, 9)).is_empty());
    assert_eq!(day(date(2027, 1, 6)).len(), 1);
    // Die Lasten der Einheiten bleiben, die Woche verliert die Kraft: 236,8 - 27,0 = 209,8.
    assert_eq!(moved[1].planned_load, 17.6);
    assert_eq!(base.weeks[0].planned_load, Some(236.8));
    assert_eq!(plan.weeks[0].planned_load, Some(209.8));
    assert_schema_valid(&plan);
}

#[test]
fn invalid_json_gets_one_correction_with_codes_only() {
    let wrong = "Gerne! Hier ist mein Vorschlag: SECRET-FREITEXT";
    let client = FakeClient::with_replies([
        wrong.to_string(),
        reply(REPLACE_SPIN_BY_ENDURANCE, "Jetzt gültig."),
    ]);
    let (outcome, _) = run(&client);
    let RefineOutcome::Applied { attempts, .. } = outcome else {
        panic!("Applied erwartet");
    };
    assert_eq!(attempts, 2);
    let requests = client.requests();
    assert_eq!(requests.len(), 2);
    // Erster Aufruf: Systemanweisung und Eingang. Zweiter: dazu die frühere Antwort und die
    // Fehler als Codes.
    assert_eq!(requests[0].messages.len(), 2);
    assert_eq!(requests[1].messages.len(), 4);
    let roles: Vec<Role> = requests[1].messages.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        [Role::System, Role::User, Role::Assistant, Role::User]
    );
    let correction = &requests[1].messages[3].content;
    let json_start = correction.find('{').unwrap();
    let list: Value = serde_json::from_str(&correction[json_start..]).unwrap();
    assert_eq!(list["violations"][0]["code"], "NOT_JSON");
    // Der Korrekturtext nennt den Freitext der Antwort nicht.
    assert!(!correction.contains("SECRET-FREITEXT"));
}

#[test]
fn unknown_template_is_corrected() {
    let bad = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"bike.fantasy.x","duration_min":60}"#;
    let client =
        FakeClient::with_replies([reply(bad, "x"), reply(REPLACE_SPIN_BY_ENDURANCE, "ok")]);
    let (outcome, _) = run(&client);
    assert!(matches!(
        outcome,
        RefineOutcome::Applied { attempts: 2, .. }
    ));
    let correction = &client.requests()[1].messages[3].content;
    assert!(correction.contains("UNKNOWN_TEMPLATE"), "{correction}");
    assert!(correction.contains("2027-01-07"), "{correction}");
    assert!(!correction.contains("fantasy"), "{correction}");
}

#[test]
fn template_of_wrong_sport_is_rejected() {
    // Lauf-Vorlage auf einer Radeinheit.
    let bad = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"run.easy.steady","duration_min":30}"#;
    let client = FakeClient::with_replies([reply(bad, "x"), reply(bad, "x"), reply(bad, "x")]);
    let (outcome, base) = run(&client);
    let (reason, attempts) = fallback(outcome, &base);
    assert_eq!(attempts, 3);
    let FallbackReason::InvalidOutput(issues) = reason else {
        panic!("InvalidOutput erwartet");
    };
    assert_eq!(issues[0].code, "SPORT_MISMATCH");
    assert_eq!(issues[0].date, Some(date(2027, 1, 7)));
}

#[test]
fn duration_outside_the_template_window_is_rejected() {
    // Die Erholungsfahrt erlaubt 30 bis 60 Minuten.
    let bad = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"bike.recovery.spin","duration_min":90}"#;
    let client = FakeClient::with_replies([reply(bad, "x"), reply(bad, "x"), reply(bad, "x")]);
    let (outcome, base) = run(&client);
    let (reason, _) = fallback(outcome, &base);
    let FallbackReason::InvalidOutput(issues) = reason else {
        panic!("InvalidOutput erwartet");
    };
    assert_eq!(issues[0].code, "DURATION_OUT_OF_WINDOW");
}

#[test]
fn edits_on_missing_sessions_or_occupied_slots_are_rejected() {
    let cases = [
        // Kein Eintrag im Slot, Datum nicht im Plan, Zielslot belegt.
        (
            r#"{"op":"remove","date":"2027-01-07","slot":"pm"}"#,
            "NO_SESSION",
        ),
        (
            r#"{"op":"remove","date":"2027-03-01","slot":"am"}"#,
            "UNKNOWN_DATE",
        ),
        (
            r#"{"op":"move","from_date":"2027-01-09","from_slot":"am","to_date":"2027-01-08","to_slot":"am"}"#,
            "SLOT_OCCUPIED",
        ),
    ];
    for (edit, code) in cases {
        let client =
            FakeClient::with_replies([reply(edit, "x"), reply(edit, "x"), reply(edit, "x")]);
        let (outcome, base) = run(&client);
        let (reason, _) = fallback(outcome, &base);
        let FallbackReason::InvalidOutput(issues) = reason else {
            panic!("InvalidOutput erwartet");
        };
        assert_eq!(issues[0].code, code, "{edit}");
    }
}

#[test]
fn two_hard_days_in_a_row_violate_a_hard_rule_and_are_corrected() {
    // Am 5. Januar liegt die Schwelleneinheit. Eine harte Laufeinheit am 6. Januar verletzt R04.
    let hard_next_day = r#"{"op":"replace","date":"2027-01-06","slot":"am","template_id":"run.tempo.continuous","duration_min":40}"#;
    let client = FakeClient::with_replies([
        reply(hard_next_day, "zu hart"),
        reply(REPLACE_SPIN_BY_ENDURANCE, "besser"),
    ]);
    let (outcome, _) = run(&client);
    let RefineOutcome::Applied { attempts, plan, .. } = outcome else {
        panic!("Applied erwartet");
    };
    assert_eq!(attempts, 2);
    assert_eq!(plan.explanation.as_deref(), Some("besser"));
    let correction = &client.requests()[1].messages[3].content;
    assert!(correction.contains("\"R04\""), "{correction}");
    assert!(correction.contains("2027-01-06"), "{correction}");
}

#[test]
fn a_hard_violation_is_never_returned_as_a_plan() {
    let hard_next_day = r#"{"op":"replace","date":"2027-01-06","slot":"am","template_id":"run.tempo.continuous","duration_min":40}"#;
    let client = FakeClient::with_replies([
        reply(hard_next_day, "a"),
        reply(hard_next_day, "b"),
        reply(hard_next_day, "c"),
    ]);
    let (outcome, base) = run(&client);
    let (reason, attempts) = fallback(outcome, &base);
    assert_eq!(attempts, 3);
    assert_eq!(reason.code(), "invalid_output");
    let FallbackReason::InvalidOutput(issues) = reason else {
        panic!();
    };
    assert!(issues.iter().any(|i| i.code == "R04"));
}

#[test]
fn availability_overrun_is_a_hard_violation() {
    // 120 Minuten Ausdauer, aber der Tag erlaubt 90 (R11).
    let edit = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"bike.endurance.steady","duration_min":120}"#;
    let client = FakeClient::with_replies([reply(edit, "x"), reply(edit, "x"), reply(edit, "x")]);
    let (outcome, base) = run(&client);
    let (reason, _) = fallback(outcome, &base);
    let FallbackReason::InvalidOutput(issues) = reason else {
        panic!();
    };
    assert!(issues.iter().any(|i| i.code == "R11"), "{issues:?}");
}

#[test]
fn at_most_two_corrections_then_fallback_after_exactly_three_calls() {
    let client = FakeClient::with_replies(["kein json", "{}", "[]", "noch eins"]);
    let (outcome, base) = run(&client);
    let (reason, attempts) = fallback(outcome, &base);
    assert_eq!(attempts, 3);
    assert_eq!(client.calls(), 3);
    assert_eq!(reason.code(), "invalid_output");
}

#[test]
fn the_limit_of_two_corrections_cannot_be_raised() {
    let client = FakeClient::with_replies(["a", "b", "c", "d", "e"]);
    let cfg = RefineConfig {
        max_corrections: 200,
        timeout_s: 5,
    };
    let (outcome, base) = run_with(&client, &cfg);
    let (_, attempts) = fallback(outcome, &base);
    assert_eq!(attempts, 3);
    assert_eq!(client.calls(), 3);
}

#[test]
fn zero_corrections_means_one_call() {
    let client = FakeClient::with_replies(["a", "b"]);
    let cfg = RefineConfig {
        max_corrections: 0,
        timeout_s: 5,
    };
    let (outcome, base) = run_with(&client, &cfg);
    let (_, attempts) = fallback(outcome, &base);
    assert_eq!(attempts, 1);
    assert_eq!(client.calls(), 1);
}

#[test]
fn the_timeout_setting_goes_to_every_call() {
    let client = FakeClient::with_replies(["a", "b", "c"]);
    let cfg = RefineConfig {
        max_corrections: 2,
        timeout_s: 7,
    };
    let _ = run_with(&client, &cfg);
    assert!(client.requests().iter().all(|r| r.timeout_s == 7));
    // Das Schema der Antwort geht mit.
    assert!(
        client
            .requests()
            .iter()
            .all(|r| r.schema["properties"]["edits"]["maxItems"] == 14)
    );
}

#[test]
fn timeout_gives_a_fallback_with_the_reason() {
    let client = FakeClient::new(vec![Err(LlmError::Timeout)]);
    let (outcome, base) = run(&client);
    let (reason, attempts) = fallback(outcome, &base);
    assert_eq!(reason, FallbackReason::Timeout);
    assert_eq!(attempts, 1);
}

#[test]
fn transport_error_in_a_correction_call_gives_a_fallback() {
    let client = FakeClient::new(vec![
        Ok("kein json".into()),
        Err(LlmError::Transport("refused".into())),
    ]);
    let (outcome, base) = run(&client);
    let (reason, attempts) = fallback(outcome, &base);
    assert_eq!(attempts, 2);
    assert_eq!(reason.code(), "transport");
    assert!(matches!(
        reason,
        FallbackReason::Transport(LlmError::Transport(_))
    ));

    let client = FakeClient::new(vec![Err(LlmError::Status(500))]);
    let (outcome, base) = run(&client);
    let (reason, _) = fallback(outcome, &base);
    assert_eq!(reason, FallbackReason::Transport(LlmError::Status(500)));
}

#[test]
fn injection_in_the_explanation_stays_plain_text() {
    let nasty = "<script>alert(1)</script> [Klick hier](http://evil.example/x) ![img](http://evil.example/p.png) **fett** <img src=x onerror=alert(1)>";
    let client = FakeClient::with_replies([reply("", nasty)]);
    let (outcome, _) = run(&client);
    let RefineOutcome::Applied {
        plan, explanation, ..
    } = outcome
    else {
        panic!("Applied erwartet");
    };
    // Unverändert als String, nichts wird entfernt, ersetzt oder gerendert.
    assert_eq!(explanation, nasty);
    assert_eq!(plan.explanation.as_deref(), Some(nasty));
    let json = serde_json::to_string(&plan).unwrap();
    let back: Plan = serde_json::from_str(&json).unwrap();
    assert_eq!(back.explanation.as_deref(), Some(nasty));
    assert_schema_valid(&plan);
}

#[test]
fn explanation_is_cut_to_the_schema_length() {
    let long = "x".repeat(5000);
    let client = FakeClient::with_replies([reply("", &long)]);
    let (outcome, _) = run(&client);
    let RefineOutcome::Applied {
        plan, explanation, ..
    } = outcome
    else {
        panic!("Applied erwartet");
    };
    assert_eq!(explanation.chars().count(), 1200);
    assert_eq!(
        plan.explanation.as_deref().map(|e| e.chars().count()),
        Some(1200)
    );
    assert_schema_valid(&plan);
}

#[test]
fn the_reply_is_not_trusted_for_loads() {
    // Felder wie planned_load gehören nicht in eine Änderung (additionalProperties false).
    let edit = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"bike.endurance.steady","duration_min":60,"planned_load":1}"#;
    let client =
        FakeClient::with_replies([reply(edit, "x"), reply(REPLACE_SPIN_BY_ENDURANCE, "ok")]);
    let (outcome, _) = run(&client);
    let RefineOutcome::Applied { plan, .. } = outcome else {
        panic!();
    };
    let s = &plan.weeks[0].days[3].sessions[0];
    assert_eq!(s.planned_load, 42.2);
    let correction = &client.requests()[1].messages[3].content;
    assert!(correction.contains("\"SCHEMA\""), "{correction}");
}

#[test]
fn a_base_plan_with_a_hard_violation_is_not_refined() {
    let ctx = context();
    let mut base = base_plan(&ctx);
    // Zwei harte Tage in Folge im Basisplan: 5. und 6. Januar.
    let template = ctx.catalog.get("run.tempo.continuous").unwrap();
    let s = &mut base.weeks[0].days[2].sessions[0];
    s.template_id = template.id.clone();
    s.hard = true;
    assert!(
        validate_plan(&base, &ctx, &rules())
            .iter()
            .any(|v| v.hardness == Hardness::Hard)
    );
    let client = FakeClient::with_replies([reply("", "x")]);
    let outcome = refine_plan(&base, &ctx, &rules(), &client, &RefineConfig::default());
    let (reason, attempts) = fallback(outcome, &base);
    assert_eq!(reason, FallbackReason::BaseInvalid);
    assert_eq!(attempts, 0);
    assert_eq!(client.calls(), 0);
}

#[test]
fn soft_violations_become_warnings_and_are_not_suppressed() {
    // Zwei mittlere Einheiten à 90 Minuten statt der Erholungsfahrten: keine harten Einheiten,
    // kein Nachbartag hart, Verfügbarkeit eingehalten, aber der Anteil lockerer Zeit (R08)
    // sinkt unter die Grenze. Das ist eine weiche Verletzung und erscheint als Warnung.
    let edits = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"bike.tempo.blocks","duration_min":90},
        {"op":"replace","date":"2027-01-08","slot":"am","template_id":"bike.tempo.blocks","duration_min":90}"#;
    let client = FakeClient::with_replies([reply(edits, "Mehr Tempo.")]);
    let ctx = context();
    let (outcome, _) = run(&client);
    let RefineOutcome::Applied { plan, .. } = outcome else {
        panic!("Applied erwartet");
    };
    // Die Warnungen des Plans sind genau die weichen Verletzungen der Prüfung (plus Hinweise
    // des Basisplans, hier keine).
    let soft: Vec<_> = validate_plan(&plan, &ctx, &rules())
        .into_iter()
        .filter(|v| v.hardness == Hardness::Soft)
        .collect();
    assert_eq!(plan.warnings, violations_to_warnings(&soft));
    assert!(
        !plan.warnings.is_empty(),
        "ein Fall ohne weiche Verletzung taugt nicht"
    );
    assert_eq!(
        plan.weeks[0].days[3].sessions[0].template_id,
        "bike.tempo.blocks"
    );
}
