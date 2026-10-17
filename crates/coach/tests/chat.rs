//! Attrappen-Fälle für `chat_turn` (SPEC 9.7, T26 Teil A): Antwort ohne Vorschlag, gültiger
//! `plan_edit`, ungültiges JSON mit Korrektur, Regelverletzung mit Korrektur und ohne, Grenzen
//! von `planning_change`, Verlauf, Anzeigetext unverändert, Zeitlimit und Netzfehler.

mod common;

use common::{base_plan, context, rules};
use lastwerk_coach::chat::{
    CHAT_SCHEMA, MAX_HISTORY_MESSAGES, MAX_MESSAGE_CHARS, MAX_REPLY_CHARS, prepare_user_text,
};
use lastwerk_coach::{
    ChatError, ChatFocus, ChatHistory, ChatInput, ChatOutcome, ChatRole, ChatTopic, ChatView,
    CheckedProposal, FakeClient, LlmError, MetricValue, RefineConfig, RefineInput, Role,
    chat_system_prompt, chat_turn,
};
use lastwerk_core::plan::{
    DayAvailability, Hardness, Locale, PlanSource, PlanSport, validate_plan,
};
use serde_json::{Value, json};

/// Gültig im Basisplan der Tests (wie in `refine.rs`): Erholungsfahrt am 7. Januar wird eine
/// lockere Grundlagenfahrt.
const REPLACE_SPIN_BY_ENDURANCE: &str = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"bike.endurance.steady","duration_min":60}"#;

/// Drei harte Einheiten in der ersten Woche: Schwelle am 5. (Basisplan), VO2 am 7. und Tempo
/// am 9. Januar. Keine zwei harten Tage in Folge, aber R03 erlaubt höchstens zwei je Woche.
const THREE_HARD: &str = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"bike.vo2.5x4","duration_min":60},{"op":"replace","date":"2027-01-09","slot":"am","template_id":"run.tempo.continuous","duration_min":40}"#;

const USER_TEXT: &str = "Dienstag habe ich keine Zeit, und bitte mehr Laufen.";

fn input() -> ChatInput {
    let ctx = context();
    let base = base_plan(&ctx);
    let metrics = vec![
        MetricValue::new("ramp_ctl_7d", 3.13).unwrap(),
        MetricValue::new("share_z12_4w", 81.0).unwrap(),
    ];
    ChatInput::build(&base, &ctx, &rules(), metrics, None).unwrap()
}

fn answer(reply: &str, proposal: Option<Value>) -> String {
    let mut v = json!({ "schema_version": 1, "reply": reply });
    if let Some(p) = proposal {
        v["proposal"] = p;
    }
    v.to_string()
}

fn plan_edit(edits: &str) -> Option<Value> {
    let edits: Value = serde_json::from_str(&format!("[{edits}]")).unwrap();
    Some(json!({
        "kind": "plan_edit",
        "edit": { "schema_version": 1, "edits": edits, "explanation": "Kurz erklärt." }
    }))
}

fn turn(client: &FakeClient) -> Result<ChatOutcome, ChatError> {
    turn_with(client, &ChatHistory::new(), &RefineConfig::default())
}

fn turn_with(
    client: &FakeClient,
    history: &ChatHistory,
    cfg: &RefineConfig,
) -> Result<ChatOutcome, ChatError> {
    chat_turn(client, &input(), history, USER_TEXT, Locale::De, cfg)
}

/// Fehlerliste aus der Korrekturnachricht (fester Satz, dann JSON).
fn violations(correction: &str) -> Value {
    let start = correction.find('{').unwrap();
    serde_json::from_str(&correction[start..]).unwrap()
}

#[test]
fn a_reply_without_proposal_is_returned_as_is() {
    let client = FakeClient::with_replies([answer("Deine Form liegt bei -5.", None)]);
    let outcome = turn(&client).unwrap();
    assert_eq!(outcome.reply, "Deine Form liegt bei -5.");
    assert_eq!(outcome.proposal, None);
    assert!(outcome.rejected_codes.is_empty());
    assert_eq!(outcome.calls, 1);

    let req = &client.requests()[0];
    // Das Schema der Antwort ist plan-chat, nicht plan-edit.
    assert_eq!(
        req.schema,
        serde_json::from_str::<Value>(CHAT_SCHEMA).unwrap()
    );
    // Systemanweisung mit dem Eingang dahinter, dann die Nachricht des Nutzers.
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].role, Role::System);
    let system = &req.messages[0].content;
    assert!(system.starts_with(chat_system_prompt(Locale::De).trim_end()));
    assert!(system.ends_with(&input().to_json()));
    assert_eq!(req.messages[1].role, Role::User);
    assert_eq!(req.messages[1].content, USER_TEXT);
}

#[test]
fn the_input_is_the_refine_input_plus_rounded_metrics_only() {
    let ctx = context();
    let base = base_plan(&ctx);
    let mut chat: Value = serde_json::from_str(&input().to_json()).unwrap();
    let metrics = chat.as_object_mut().unwrap().remove("metrics").unwrap();
    // 3,13 auf eine Stelle: 31,3 gerundet 31, also 3,1.
    assert_eq!(
        metrics,
        json!([{"code":"ramp_ctl_7d","value":3.1},{"code":"share_z12_4w","value":81.0}])
    );
    let refine: Value =
        serde_json::from_str(&RefineInput::build(&base, &ctx, &rules()).to_json()).unwrap();
    assert_eq!(chat, refine);
}

/// Der Fokus steht als zwei Codes im Eingang (SPEC 9.7), sonst ändert sich nichts. Auch bei
/// einer offenen Aktivität kommt nur die Art der Ansicht mit, keine ID und kein Datum.
#[test]
fn the_focus_is_two_codes_in_the_input() {
    let ctx = context();
    let base = base_plan(&ctx);
    let focus = ChatFocus {
        view: ChatView::HistoryActivity,
        topic: Some(ChatTopic::RampSteep),
    };
    let with = ChatInput::build(&base, &ctx, &rules(), Vec::new(), Some(focus)).unwrap();
    assert_eq!(with.focus(), Some(focus));
    let mut json: Value = serde_json::from_str(&with.to_json()).unwrap();
    let f = json.as_object_mut().unwrap().remove("focus").unwrap();
    assert_eq!(
        f,
        json!({"view": "history_activity", "topic": "ramp.steep"})
    );
    let without = ChatInput::build(&base, &ctx, &rules(), Vec::new(), None).unwrap();
    let plain: Value = serde_json::from_str(&without.to_json()).unwrap();
    assert!(plain.get("focus").is_none());
    assert_eq!(json, plain);
    // Die Systemanweisung endet mit genau diesem Eingang.
    let client = FakeClient::with_replies([answer("ok", None)]);
    chat_turn(
        &client,
        &with,
        &ChatHistory::new(),
        USER_TEXT,
        Locale::De,
        &RefineConfig::default(),
    )
    .unwrap();
    let system = &client.requests()[0].messages[0].content;
    assert!(system.ends_with(&with.to_json()));
    assert!(system.contains(r#""focus":{"view":"history_activity","topic":"ramp.steep"}"#));
}

#[test]
fn metrics_are_limited_and_unique() {
    let ctx = context();
    let base = base_plan(&ctx);
    let twice = vec![
        MetricValue::new("a", 1.0).unwrap(),
        MetricValue::new("a", 2.0).unwrap(),
    ];
    assert_eq!(
        ChatInput::build(&base, &ctx, &rules(), twice, None),
        Err(ChatError::InvalidMetric)
    );
    let many: Vec<_> = (0..41)
        .map(|i| MetricValue::new(&format!("m{i}"), 1.0).unwrap())
        .collect();
    assert!(matches!(
        ChatInput::build(&base, &ctx, &rules(), many, None),
        Err(ChatError::TooManyMetrics { .. })
    ));
}

#[test]
fn a_valid_plan_edit_comes_back_applied_and_checked() {
    let client = FakeClient::with_replies([answer(
        "Ich mache die Erholungsfahrt am Donnerstag länger und locker.",
        plan_edit(REPLACE_SPIN_BY_ENDURANCE),
    )]);
    let outcome = turn(&client).unwrap();
    assert_eq!(outcome.calls, 1);
    assert!(outcome.rejected_codes.is_empty());
    let Some(CheckedProposal::PlanEdit {
        edits,
        explanation,
        plan,
    }) = outcome.proposal
    else {
        panic!("plan_edit erwartet: {:?}", outcome.proposal);
    };
    assert_eq!(edits.len(), 1);
    assert_eq!(explanation, "Kurz erklärt.");
    assert_eq!(plan.source, PlanSource::Llm);
    let ctx = context();
    let session = &plan.weeks[0].days[3].sessions[0];
    assert_eq!(session.template_id, "bike.endurance.steady");
    assert_eq!(session.duration_min, 60);
    // Die Last rechnet core, nicht das Modell.
    let template = ctx.catalog.get("bike.endurance.steady").unwrap();
    assert_eq!(
        session.planned_load,
        rules().load_model.session_load(template, 60)
    );
    assert!(
        !validate_plan(&plan, &ctx, &rules())
            .iter()
            .any(|v| v.hardness == Hardness::Hard)
    );
}

#[test]
fn invalid_json_is_corrected_with_codes_only() {
    let client = FakeClient::with_replies([
        "Gerne! SECRET-FREITEXT".to_string(),
        answer("Jetzt im Format.", None),
    ]);
    let outcome = turn(&client).unwrap();
    assert_eq!(outcome.reply, "Jetzt im Format.");
    assert_eq!(outcome.calls, 2);
    let second = &client.requests()[1].messages;
    let roles: Vec<Role> = second.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        [Role::System, Role::User, Role::Assistant, Role::User]
    );
    let correction = &second[3].content;
    assert_eq!(violations(correction)["violations"][0]["code"], "NOT_JSON");
    assert!(!correction.contains("SECRET-FREITEXT"));
}

#[test]
fn a_proposal_violating_r03_is_corrected() {
    let client = FakeClient::with_replies([
        answer("Mehr Intensität.", plan_edit(THREE_HARD)),
        answer(
            "Dann nur länger und locker.",
            plan_edit(REPLACE_SPIN_BY_ENDURANCE),
        ),
    ]);
    let outcome = turn(&client).unwrap();
    assert_eq!(outcome.calls, 2);
    assert_eq!(outcome.reply, "Dann nur länger und locker.");
    assert!(matches!(
        outcome.proposal,
        Some(CheckedProposal::PlanEdit { .. })
    ));
    let correction = &client.requests()[1].messages[3].content;
    let list = violations(correction);
    assert!(
        list["violations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["code"] == "R03"),
        "{correction}"
    );
}

#[test]
fn a_proposal_that_stays_invalid_gives_the_reply_without_proposal_and_the_codes() {
    let bad = answer("Drei harte Tage.", plan_edit(THREE_HARD));
    let client = FakeClient::with_replies([bad.clone(), bad.clone(), bad]);
    let outcome = turn(&client).unwrap();
    assert_eq!(outcome.calls, 3);
    assert_eq!(client.calls(), 3);
    assert_eq!(outcome.reply, "Drei harte Tage.");
    assert_eq!(outcome.proposal, None);
    assert!(outcome.rejected_codes.contains(&"R03".to_string()));

    // Mit einer erlaubten Korrektur endet es nach zwei Aufrufen.
    let bad = answer("Drei harte Tage.", plan_edit(THREE_HARD));
    let client = FakeClient::with_replies([bad.clone(), bad]);
    let cfg = RefineConfig {
        max_corrections: 1,
        ..RefineConfig::default()
    };
    let outcome = turn_with(&client, &ChatHistory::new(), &cfg).unwrap();
    assert_eq!(outcome.calls, 2);
    assert_eq!(outcome.proposal, None);
    assert!(outcome.rejected_codes.contains(&"R03".to_string()));
}

#[test]
fn the_limit_of_two_corrections_cannot_be_raised() {
    let client = FakeClient::with_replies(["x", "y", "z", "w", "v"]);
    let cfg = RefineConfig {
        max_corrections: 9,
        ..RefineConfig::default()
    };
    let err = turn_with(&client, &ChatHistory::new(), &cfg).unwrap_err();
    let ChatError::InvalidOutput { issues, calls } = err else {
        panic!("InvalidOutput erwartet: {err:?}");
    };
    assert_eq!(calls, 3);
    assert_eq!(client.calls(), 3);
    assert_eq!(issues[0].code, "NOT_JSON");
}

#[test]
fn an_unreadable_envelope_with_a_proposal_is_not_half_accepted() {
    // Ein Vorschlag ohne `reply` hat keinen Anzeigetext: Fehler, kein halber Erfolg.
    let no_reply = json!({ "schema_version": 1, "proposal": plan_edit(REPLACE_SPIN_BY_ENDURANCE) })
        .to_string();
    let client = FakeClient::with_replies([no_reply.clone(), no_reply.clone(), no_reply]);
    let err = turn(&client).unwrap_err();
    assert_eq!(err.code(), "invalid_output");
}

#[test]
fn a_valid_planning_change_is_returned_typed() {
    let mut days = vec![json!({"max_minutes": 60, "allow_hard": true, "fixed_rest": false}); 7];
    days[1] = json!({"max_minutes": 0, "allow_hard": false, "fixed_rest": true});
    let proposal = json!({
        "kind": "planning_change",
        "availability": days,
        "sports": ["running", "cycling"],
        "strength_per_week": 0,
        "weeks": 12
    });
    let client = FakeClient::with_replies([answer("Dienstag ist frei.", Some(proposal))]);
    let outcome = turn(&client).unwrap();
    let Some(CheckedProposal::PlanningChange(change)) = outcome.proposal else {
        panic!("planning_change erwartet");
    };
    let availability = change.availability.unwrap();
    assert_eq!(
        availability[1],
        DayAvailability {
            max_minutes: 0,
            allow_hard: false,
            fixed_rest: true
        }
    );
    assert_eq!(availability[0].max_minutes, 60);
    assert_eq!(
        change.sports,
        Some(vec![PlanSport::Running, PlanSport::Cycling])
    );
    assert_eq!(change.strength_per_week, Some(0));
    assert_eq!(change.weeks, Some(12));

    // Ein einzelnes Feld genügt.
    let client = FakeClient::with_replies([answer(
        "Kürzer.",
        Some(json!({"kind": "planning_change", "weeks": 4})),
    )]);
    let Some(CheckedProposal::PlanningChange(change)) = turn(&client).unwrap().proposal else {
        panic!("planning_change erwartet");
    };
    assert_eq!(change.weeks, Some(4));
    assert_eq!(change.availability, None);
}

#[test]
fn a_planning_change_out_of_bounds_is_rejected_with_paths() {
    let day = json!({"max_minutes": 60, "allow_hard": true, "fixed_rest": false});
    let mut too_long = vec![day.clone(); 7];
    too_long[3] = json!({"max_minutes": 2000, "allow_hard": true, "fixed_rest": false});
    let cases = [
        (
            json!({"kind": "planning_change", "weeks": 30}),
            "/proposal/weeks",
        ),
        (
            json!({"kind": "planning_change", "weeks": 0}),
            "/proposal/weeks",
        ),
        (
            json!({"kind": "planning_change", "strength_per_week": 3}),
            "/proposal/strength_per_week",
        ),
        (
            json!({"kind": "planning_change", "sports": ["strength"]}),
            "/proposal/sports/0",
        ),
        (
            json!({"kind": "planning_change", "sports": ["running", "running"]}),
            "/proposal/sports",
        ),
        (
            json!({"kind": "planning_change", "availability": vec![day.clone(); 6]}),
            "/proposal/availability",
        ),
        (
            json!({"kind": "planning_change", "availability": too_long}),
            "/proposal/availability/3/max_minutes",
        ),
        (json!({"kind": "planning_change"}), "/proposal"),
        (
            json!({"kind": "planning_change", "weeks": 4, "goal_label": "x"}),
            "/proposal",
        ),
    ];
    for (proposal, path) in cases {
        let bad = answer("Vorschlag.", Some(proposal.clone()));
        let client = FakeClient::with_replies([bad.clone(), bad.clone(), bad]);
        let outcome = turn(&client).unwrap();
        assert_eq!(outcome.proposal, None, "{proposal}");
        assert_eq!(outcome.rejected_codes, ["SCHEMA"], "{proposal}");
        assert_eq!(outcome.reply, "Vorschlag.");
        assert_eq!(outcome.calls, 3);
        let correction = &client.requests()[1].messages[3].content;
        let list = violations(correction);
        assert!(
            list["violations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["path"] == path),
            "{proposal}: {correction}"
        );
    }
}

#[test]
fn the_history_keeps_the_newest_twelve_messages_and_cuts_long_ones() {
    let mut history = ChatHistory::new();
    for i in 0..20 {
        let role = if i % 2 == 0 {
            ChatRole::User
        } else {
            ChatRole::Assistant
        };
        history.push(role, &format!("Nachricht {i}"));
    }
    assert_eq!(history.len(), MAX_HISTORY_MESSAGES);
    let texts: Vec<&str> = history.messages().map(|m| m.text.as_str()).collect();
    assert_eq!(texts.first(), Some(&"Nachricht 8"));
    assert_eq!(texts.last(), Some(&"Nachricht 19"));

    history.push(ChatRole::User, &"ä".repeat(MAX_MESSAGE_CHARS + 300));
    let last = history.messages().last().unwrap();
    assert_eq!(last.text.chars().count(), MAX_MESSAGE_CHARS);
    assert_eq!(history.len(), MAX_HISTORY_MESSAGES);
    // Leere Nachrichten zählen nicht.
    history.push(ChatRole::User, " \u{7} ");
    assert_eq!(history.len(), MAX_HISTORY_MESSAGES);

    // Gesendet werden Anweisung, der ganze Verlauf und die neue Nachricht. Antworten des Modells
    // stehen im Antwortformat, ohne Vorschlag.
    let client = FakeClient::with_replies([answer("ok", None)]);
    turn_with(&client, &history, &RefineConfig::default()).unwrap();
    let sent = &client.requests()[0].messages;
    assert_eq!(sent.len(), 1 + MAX_HISTORY_MESSAGES + 1);
    let first_assistant = sent.iter().find(|m| m.role == Role::Assistant).unwrap();
    let echoed: Value = serde_json::from_str(&first_assistant.content).unwrap();
    assert_eq!(echoed, json!({"schema_version": 1, "reply": "Nachricht 9"}));
    assert_eq!(sent.last().unwrap().content, USER_TEXT);
}

#[test]
fn record_appends_the_user_text_and_the_reply() {
    let client = FakeClient::with_replies([answer("Antwort eins.", None)]);
    let mut history = ChatHistory::new();
    let outcome = turn_with(&client, &history, &RefineConfig::default()).unwrap();
    history.record(USER_TEXT, &outcome);
    let all: Vec<_> = history.messages().cloned().collect();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].role, ChatRole::User);
    assert_eq!(all[0].text, USER_TEXT);
    assert_eq!(all[1].role, ChatRole::Assistant);
    assert_eq!(all[1].text, "Antwort eins.");
}

#[test]
fn markup_in_the_reply_stays_verbatim_text() {
    let nasty = "<script>alert(1)</script> [Klick](http://evil.example/x) **fett** <img src=x onerror=alert(2)>";
    let client = FakeClient::with_replies([answer(nasty, None)]);
    let outcome = turn(&client).unwrap();
    assert_eq!(outcome.reply, nasty);
}

#[test]
fn a_long_reply_is_cut_not_rejected() {
    let long = "ü".repeat(MAX_REPLY_CHARS + 50);
    let client = FakeClient::with_replies([answer(&long, None)]);
    let outcome = turn(&client).unwrap();
    assert_eq!(outcome.calls, 1);
    assert_eq!(outcome.reply.chars().count(), MAX_REPLY_CHARS);
}

#[test]
fn timeout_and_transport_errors_end_the_turn_without_corrections() {
    let client = FakeClient::new(vec![Err(LlmError::Timeout)]);
    assert_eq!(turn(&client), Err(ChatError::Timeout { calls: 1 }));
    assert_eq!(client.calls(), 1);

    let client = FakeClient::new(vec![Err(LlmError::Status(503))]);
    assert_eq!(
        turn(&client),
        Err(ChatError::Transport {
            error: LlmError::Status(503),
            calls: 1
        })
    );

    // Ein Netzfehler im Korrekturaufruf beendet ebenfalls sofort.
    let client = FakeClient::new(vec![
        Ok("kein json".into()),
        Err(LlmError::Transport("refused".into())),
    ]);
    let err = turn(&client).unwrap_err();
    assert_eq!(err.code(), "transport");
    assert_eq!(client.calls(), 2);
}

#[test]
fn the_timeout_setting_goes_to_every_call() {
    let client = FakeClient::with_replies(["x".to_string(), answer("ok", None)]);
    let cfg = RefineConfig {
        timeout_s: 7,
        ..RefineConfig::default()
    };
    turn_with(&client, &ChatHistory::new(), &cfg).unwrap();
    assert!(client.requests().iter().all(|r| r.timeout_s == 7));
}

#[test]
fn a_plan_edit_on_an_invalid_base_plan_is_rejected_without_corrections() {
    let ctx = context();
    let mut base = base_plan(&ctx);
    // Zwei harte Tage in Folge im Basisplan: 5. und 6. Januar (wie in refine.rs).
    let s = &mut base.weeks[0].days[2].sessions[0];
    s.template_id = "run.tempo.continuous".into();
    s.hard = true;
    let input = ChatInput::build(&base, &ctx, &rules(), Vec::new(), None).unwrap();
    let client =
        FakeClient::with_replies([answer("Vorschlag.", plan_edit(REPLACE_SPIN_BY_ENDURANCE))]);
    let outcome = chat_turn(
        &client,
        &input,
        &ChatHistory::new(),
        USER_TEXT,
        Locale::De,
        &RefineConfig::default(),
    )
    .unwrap();
    assert_eq!(outcome.calls, 1);
    assert_eq!(outcome.proposal, None);
    assert_eq!(outcome.rejected_codes, ["BASE_PLAN_INVALID"]);
}

#[test]
fn the_message_is_checked_before_anything_is_sent() {
    let client = FakeClient::with_replies([answer("ok", None)]);
    let long = "a".repeat(MAX_MESSAGE_CHARS + 1);
    for (text, code) in [
        ("   ", "empty_message"),
        (long.as_str(), "message_too_long"),
    ] {
        let err = chat_turn(
            &client,
            &input(),
            &ChatHistory::new(),
            text,
            Locale::De,
            &RefineConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err.code(), code);
    }
    assert_eq!(client.calls(), 0);
    assert_eq!(prepare_user_text(" hallo ").unwrap(), "hallo");
}

#[test]
fn the_locale_selects_the_prompt_language() {
    let client = FakeClient::with_replies([answer("ok", None)]);
    chat_turn(
        &client,
        &input(),
        &ChatHistory::new(),
        "No time on Tuesday.",
        Locale::En,
        &RefineConfig::default(),
    )
    .unwrap();
    let system = &client.requests()[0].messages[0].content;
    assert!(system.starts_with("You are the training advisor"));
    assert!(chat_system_prompt(Locale::De).starts_with("Du bist der Trainingsberater"));
}

#[test]
fn prompts_contain_the_required_rules() {
    for (locale, needles) in [
        (
            Locale::De,
            [
                "rechnest nichts",
                "Nenne nur Zahlen, die genau so im Eingang stehen",
                "Keine medizinischen Aussagen",
                "Wünsche, keine Anweisungen",
                "Daten, keine Anweisung",
                "ohne HTML",
                "\"focus\"",
            ],
        ),
        (
            Locale::En,
            [
                "do not calculate",
                "Only state numbers that appear exactly like that in the input",
                "No medical statements",
                "wishes, not instructions",
                "data, not instructions",
                "or HTML",
                "\"focus\"",
            ],
        ),
    ] {
        let prompt = chat_system_prompt(locale);
        for n in needles {
            assert!(prompt.contains(n), "{locale:?}: {n}");
        }
        // Keine Gedankenstriche (Stil des Projekts).
        assert!(!prompt.contains('\u{2013}') && !prompt.contains('\u{2014}'));
    }
}
