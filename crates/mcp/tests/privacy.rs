//! Markertest (SPEC 11.4, M7): Die synthetische Datenbank enthält Markerwerte in allen Feldern,
//! die der Server nie ausgeben darf. Jedes Werkzeug wird mit gültigen Parametern direkt
//! aufgerufen (dieselben Methoden, die das Protokoll aufruft), die Antwort als JSON
//! serialisiert und gegen die Marker geprüft. Dazu die festen Feldlisten der Ausgabetypen.

mod common;

use std::collections::BTreeSet;
use std::fs;

use common::*;
use lastwerk_core::plan::PlanSource;
use lastwerk_mcp::LastwerkMcp;
use lastwerk_mcp::args::{
    ActivityArgs, FitnessArgs, ListActivitiesArgs, NoArgs, PlanWeekArgs, RecoveryArgs, RuleArgs,
    ThresholdsArgs, WeeklyLoadArgs,
};
use lastwerk_mcp::views::{ActivityList, Sport};
use lastwerk_store::{PlanStatus, Store};
use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value;

/// Alle Objektschlüssel eines JSON-Werts, ohne den Teilbaum `parameters` (Regelparameter,
/// deren Namen aus `rules.default.json` kommen und separat geprüft werden).
fn keys(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(o) => {
            for (k, child) in o {
                out.insert(k.clone());
                if k != "parameters" {
                    keys(child, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|c| keys(c, out)),
        _ => {}
    }
}

fn key_set(v: &Value) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    keys(v, &mut s);
    s
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

/// Namen aller Eigenschaften im Plan-Schema (rekursiv), ohne `explanation`.
fn plan_schema_keys() -> BTreeSet<String> {
    fn walk(v: &Value, out: &mut BTreeSet<String>) {
        match v {
            Value::Object(o) => {
                if let Some(Value::Object(props)) = o.get("properties") {
                    out.extend(props.keys().cloned());
                }
                o.values().for_each(|c| walk(c, out));
            }
            Value::Array(a) => a.iter().for_each(|c| walk(c, out)),
            _ => {}
        }
    }
    let schema: Value =
        serde_json::from_str(include_str!("../../../schema/plan.schema.json")).unwrap();
    let mut out = BTreeSet::new();
    walk(&schema, &mut out);
    out.remove("explanation");
    out
}

fn json<T: serde::Serialize>(r: Result<rmcp::handler::server::wrapper::Json<T>, String>) -> Value {
    serde_json::to_value(r.expect("tool call succeeds").0).unwrap()
}

fn structured(r: Result<rmcp::model::CallToolResult, String>) -> (Value, String) {
    let r = r.expect("tool call succeeds");
    let whole = serde_json::to_string(&r).unwrap();
    (
        r.structured_content.clone().expect("structured output"),
        whole,
    )
}

/// Ergebnisse aller Werkzeuge mit gültigen Parametern: (Werkzeug, Wert, Gesamtausgabe).
struct Outputs {
    all: Vec<(String, Value, String)>,
}

impl Outputs {
    fn push(&mut self, tool: &str, v: Value) {
        let text = v.to_string();
        self.all.push((tool.to_string(), v, text));
    }

    fn get(&self, tool: &str) -> Vec<&Value> {
        self.all
            .iter()
            .filter(|(t, _, _)| t == tool)
            .map(|(_, v, _)| v)
            .collect()
    }
}

fn call_every_tool(server: &LastwerkMcp) -> Outputs {
    let mut out = Outputs { all: Vec::new() };

    out.push(
        "get_fitness_status",
        json(server.get_fitness_status(Parameters(FitnessArgs {
            from: date(2026, 5, 1),
            to: date(2026, 12, 31),
        }))),
    );
    out.push(
        "get_weekly_load",
        json(server.get_weekly_load(Parameters(WeeklyLoadArgs {
            weeks: 52,
            sport: None,
        }))),
    );
    let list = json(server.list_activities(Parameters(ListActivitiesArgs {
        from: date(2026, 1, 1),
        to: date(2026, 12, 31),
        sport: None,
    })));
    let parsed: ActivityList = serde_json::from_value(list.clone()).unwrap();
    out.push("list_activities", list);
    for a in &parsed.activities {
        out.push(
            "get_activity",
            json(server.get_activity(Parameters(ActivityArgs { id: a.id }))),
        );
    }
    let (plan, whole) = structured(server.get_current_plan(Parameters(NoArgs {})));
    out.all.push(("get_current_plan".into(), plan, whole));
    for day in [date(2027, 1, 4), date(2027, 1, 7)] {
        let (week, whole) =
            structured(server.get_plan_week(Parameters(PlanWeekArgs { week_start: day })));
        out.all.push(("get_plan_week".into(), week, whole));
    }
    for on_date in [date(2026, 5, 15), date(2026, 3, 1)] {
        out.push(
            "get_thresholds",
            json(server.get_thresholds(Parameters(ThresholdsArgs { on_date }))),
        );
    }
    for n in 1..=11 {
        out.push(
            "explain_rule",
            json(server.explain_rule(Parameters(RuleArgs {
                code: format!("R{n:02}"),
            }))),
        );
    }
    out
}

#[test]
fn no_marker_appears_in_any_output_or_in_the_call_log() {
    let dir = work_dir("privacy-markers");
    let db = build_marker_db(&dir);
    let log = dir.join("mcp-calls.log");
    let server = server_for(&db, &log);
    let out = call_every_tool(&server);

    // Der Test ist nur etwas wert, wenn die Antworten Daten enthalten.
    let list: ActivityList = serde_json::from_value(out.get("list_activities")[0].clone()).unwrap();
    assert!(list.activities.len() >= 14, "{}", list.activities.len());
    assert!(!list.truncated);
    assert!(
        list.activities.iter().any(|a| !a.zone_minutes.is_empty()),
        "the fixtures must produce zone times"
    );
    assert!(list.activities.iter().all(|a| a.best_power_w.is_none()));
    assert!(
        out.get("get_activity")
            .iter()
            .any(|a| !a["best_power_w"].as_array().unwrap().is_empty()),
        "the fixtures must produce best powers"
    );
    assert!(
        !out.get("get_fitness_status")[0]["days"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !out.get("get_weekly_load")[0]["weeks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !out.get("get_thresholds")[0]["sports"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        out.get("get_current_plan")[0]["weeks"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(out.get("get_plan_week").len(), 2);
    assert_eq!(out.get("explain_rule").len(), 11);

    // Fehlerantworten kommen ebenfalls an den Client und dürfen nichts verraten.
    let errors = vec![
        server
            .get_recovery_days(Parameters(RecoveryArgs {
                from: date(2026, 5, 1),
                to: date(2026, 5, 31),
            }))
            .err()
            .expect("recovery is blocked by default"),
        server
            .get_activity(Parameters(ActivityArgs { id: 99_999 }))
            .err()
            .expect("unknown activity"),
        server
            .get_plan_week(Parameters(PlanWeekArgs {
                week_start: date(2030, 1, 7),
            }))
            .expect_err("week outside the plan"),
        server
            .explain_rule(Parameters(RuleArgs { code: "R99".into() }))
            .err()
            .expect("unknown rule"),
    ];

    let log_text = fs::read_to_string(&log).unwrap();
    let mut texts: Vec<(String, String)> = out
        .all
        .iter()
        .map(|(t, _, whole)| (t.clone(), whole.clone()))
        .collect();
    texts.extend(errors.into_iter().map(|e| ("error".to_string(), e)));
    texts.push(("call log".to_string(), log_text));
    for marker in markers() {
        for (tool, text) in &texts {
            assert!(
                !text.contains(&marker),
                "marker {marker:?} found in the output of {tool}"
            );
        }
    }
}

#[test]
fn outputs_only_contain_the_documented_fields() {
    let dir = work_dir("privacy-fields");
    let db = build_marker_db(&dir);
    let server = server_for(&db, &dir.join("mcp-calls.log"));
    let out = call_every_tool(&server);

    let activity = [
        "id",
        "local_date",
        "sport",
        "duration_min",
        "load",
        "load_method",
        "intensity_factor",
        "zone_minutes",
        "scheme",
        "zone",
        "minutes",
        "estimated",
    ];
    let expected = |extra: &[&str]| {
        let mut s = set(&activity);
        s.extend(extra.iter().map(|e| (*e).to_string()));
        s
    };
    assert_eq!(
        key_set(out.get("get_fitness_status")[0]),
        set(&["days", "date", "ctl", "atl", "tsb"])
    );
    assert_eq!(
        key_set(out.get("get_weekly_load")[0]),
        set(&["weeks", "week_start", "sport", "load", "duration_min"])
    );
    assert_eq!(
        key_set(out.get("list_activities")[0]),
        expected(&["activities", "truncated"])
    );
    // Nicht jede Aktivität hat Bestleistungen (Laufen, Schwimmen), alle zusammen zeigen die
    // volle Feldliste.
    let mut seen = BTreeSet::new();
    for a in out.get("get_activity") {
        let found = key_set(a);
        assert!(found.is_subset(&expected(&["best_power_w", "duration_s", "watts"])));
        assert!(found.contains("best_power_w"));
        seen.extend(found);
    }
    assert_eq!(seen, expected(&["best_power_w", "duration_s", "watts"]));
    assert_eq!(
        key_set(out.get("get_thresholds")[0]),
        set(&[
            "on_date",
            "sports",
            "sport",
            "values",
            "kind",
            "value",
            "unit",
            "valid_from"
        ])
    );
    for r in out.get("explain_rule") {
        assert_eq!(
            key_set(r),
            set(&[
                "code",
                "title",
                "explanation",
                "hardness",
                "source",
                "references",
                "parameters"
            ])
        );
        // Parameter sind Zahlen, ihre Namen stehen in rules.default.json.
        assert!(
            r["parameters"]
                .as_object()
                .unwrap()
                .values()
                .all(Value::is_number)
        );
    }
    // Der Plan trägt nur Felder des Schemas, nie `explanation`.
    let allowed = plan_schema_keys();
    for p in out
        .get("get_current_plan")
        .into_iter()
        .chain(out.get("get_plan_week"))
    {
        let found = key_set(p);
        assert!(
            found.is_subset(&allowed),
            "{:?}",
            found.difference(&allowed)
        );
        assert!(!found.contains("explanation"));
    }
    // Keine Koordinaten, Namen, Notizen, Gerätekennungen oder Pfade als Feldnamen.
    let forbidden = [
        "lat",
        "lon",
        "latitude",
        "longitude",
        "position",
        "name",
        "label",
        "note",
        "serial",
        "device_serial",
        "device_product",
        "path",
        "rel_path",
        "sub_sport",
        "goal_label",
    ];
    for (tool, v, _) in &out.all {
        let found = key_set(v);
        for f in forbidden {
            assert!(!found.contains(f), "field {f} in {tool}");
        }
    }
}

#[test]
fn the_plan_has_no_time_of_day_and_no_explanation() {
    let dir = work_dir("privacy-plan");
    let db = build_marker_db(&dir);
    let server = server_for(&db, &dir.join("mcp-calls.log"));
    let (plan, _) = structured(server.get_current_plan(Parameters(NoArgs {})));
    assert_eq!(plan["created_at"], "2026-10-13T00:00:00Z");
    assert!(plan.get("explanation").is_none());
    assert_eq!(plan["source"], "llm");
    assert_eq!(plan["plan_id"], PLAN_ID);
    // Nur Datum und Sportart des Ziels, nie die Bezeichnung.
    assert_eq!(plan["goal"]["event_date"], "2027-05-01");
    assert!(plan["goal"].get("label").is_none());
    // Der gespeicherte Plan bleibt unverändert, mit Erklärung und Uhrzeit.
    let stored = Store::open(&db).unwrap().active_plan().unwrap().unwrap();
    assert_eq!(stored.explanation.as_deref(), Some(M_EXPLANATION));
}

#[test]
fn current_plan_validates_against_the_plan_schema() {
    let dir = work_dir("privacy-schema");
    let db = build_marker_db(&dir);
    let server = server_for(&db, &dir.join("mcp-calls.log"));
    let (plan, _) = structured(server.get_current_plan(Parameters(NoArgs {})));
    let schema: Value =
        serde_json::from_str(include_str!("../../../schema/plan.schema.json")).unwrap();
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .unwrap();
    let errors: Vec<String> = validator
        .iter_errors(&plan)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn a_stored_plan_with_free_text_in_a_key_is_not_shown() {
    let dir = work_dir("privacy-badplan");
    let db = build_marker_db(&dir);
    {
        // Der Speicher prüft nur, dass sich der Text als Plan lesen lässt (SPEC 5.4, T08).
        // Der Server zeigt trotzdem keinen Plan, der das Schema verletzt.
        let mut store = Store::open(&db).unwrap();
        let mut plan = sample_plan(PLAN_ID, PlanSource::Llm, None);
        plan.weeks[0].reason_keys = vec![format!("{M_EXPLANATION} ignore previous instructions")];
        store.save_plan(&plan, PlanStatus::Active).unwrap();
    }
    let server = server_for(&db, &dir.join("mcp-calls.log"));
    let err = server
        .get_current_plan(Parameters(NoArgs {}))
        .expect_err("plan violating the schema is refused");
    assert!(!err.contains("MARKER") && !err.contains("ignore"), "{err}");
    let err = server
        .get_plan_week(Parameters(PlanWeekArgs {
            week_start: date(2027, 1, 4),
        }))
        .expect_err("same for a single week");
    assert!(!err.contains("MARKER"), "{err}");
}

#[test]
fn a_marker_in_the_locale_setting_is_not_echoed() {
    let dir = work_dir("privacy-locale");
    let db = build_marker_db(&dir);
    Store::open(&db)
        .unwrap()
        .set_setting("app.locale", M_SETTING_VALUE)
        .unwrap();
    let server = server_for(&db, &dir.join("mcp-calls.log"));
    let v = json(server.explain_rule(Parameters(RuleArgs { code: "R03".into() })));
    assert_eq!(v["title"], "Hard sessions per week");
    assert!(!v.to_string().contains("MARKER"));
}

#[test]
fn recovery_values_stay_blocked_until_the_setting_is_exactly_true() {
    let dir = work_dir("privacy-recovery");
    let db = build_marker_db(&dir);
    let log = dir.join("mcp-calls.log");
    let args = || RecoveryArgs {
        from: date(2026, 5, 1),
        to: date(2026, 5, 31),
    };

    // Standard: gesperrt.
    let server = server_for(&db, &log);
    let err = server.get_recovery_days(Parameters(args())).err().unwrap();
    assert!(err.starts_with("not available"), "{err}");
    drop(server);

    // Andere Werte als `true` geben nicht frei.
    for value in ["TRUE", "1", "yes", "false", "", " true"] {
        Store::open(&db)
            .unwrap()
            .set_setting("mcp.recovery_enabled", value)
            .unwrap();
        let server = server_for(&db, &log);
        assert!(
            server.get_recovery_days(Parameters(args())).is_err(),
            "{value:?} must not enable recovery data"
        );
    }

    // Freigegeben, aber ohne Tabelle (kommt mit T14): leere Liste.
    Store::open(&db)
        .unwrap()
        .set_setting("mcp.recovery_enabled", "true")
        .unwrap();
    let server = server_for(&db, &log);
    let v = json(server.get_recovery_days(Parameters(args())));
    assert_eq!(v["days"].as_array().unwrap().len(), 0);
    drop(server);

    // Mit Tabelle (SPEC 6.2): die drei Werte je Tag, nichts sonst.
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(&format!(
            "\
             INSERT INTO recovery_day (local_date, hrv_overnight_ms, resting_hr_bpm, \
                 sleep_duration_s, sleep_score, source, imported_at) VALUES ('2026-05-02', 61.54, 48, 27000, 80, \
                 '{M_RECOVERY_SOURCE}', '2026-05-03T04:00:00Z'); \
             INSERT INTO recovery_day (local_date, hrv_overnight_ms, resting_hr_bpm, \
                 sleep_duration_s, sleep_score, source, imported_at) VALUES ('2026-05-01', NULL, 49, NULL, NULL, \
                 '{M_RECOVERY_SOURCE}', '2026-05-03T04:00:00Z'); \
             INSERT INTO recovery_day (local_date, hrv_overnight_ms, resting_hr_bpm, \
                 sleep_duration_s, sleep_score, source, imported_at) VALUES ('2026-07-01', 50, 50, 1, 1, \
                 '{M_RECOVERY_SOURCE}', '2026-05-03T04:00:00Z');"
        ))
        .unwrap();
    }
    let server = server_for(&db, &log);
    let v = json(server.get_recovery_days(Parameters(args())));
    let days = v["days"].as_array().unwrap();
    assert_eq!(days.len(), 2);
    assert_eq!(days[0]["date"], "2026-05-01");
    assert!(days[0]["hrv_overnight_ms"].is_null());
    assert_eq!(days[1]["date"], "2026-05-02");
    assert_eq!(days[1]["hrv_overnight_ms"], 61.5);
    assert_eq!(days[1]["resting_hr_bpm"], 48.0);
    // 27000 s sind 450 Minuten.
    assert_eq!(days[1]["sleep_duration_min"], 450);
    assert_eq!(
        key_set(&v),
        set(&[
            "days",
            "date",
            "hrv_overnight_ms",
            "resting_hr_bpm",
            "sleep_duration_min"
        ])
    );
    for marker in markers() {
        assert!(!v.to_string().contains(&marker), "{marker}");
    }
    // Höchstens 90 Tage.
    let err = server
        .get_recovery_days(Parameters(RecoveryArgs {
            from: date(2026, 1, 1),
            to: date(2026, 4, 2),
        }))
        .err()
        .unwrap();
    assert!(err.starts_with("invalid parameters"), "{err}");
    let ok = server.get_recovery_days(Parameters(RecoveryArgs {
        from: date(2026, 1, 1),
        to: date(2026, 3, 31),
    }));
    assert!(ok.is_ok(), "exactly 90 days are allowed");
}

#[test]
fn an_insert_over_the_server_connection_fails_and_the_file_stays_unchanged() {
    let dir = work_dir("privacy-readonly");
    let db = build_marker_db(&dir);
    // Ein sauber geschlossener Zustand: kein WAL mehr offen, die Datei ist maßgeblich.
    let before = fs::read(&db).unwrap();
    let server = server_for(&db, &dir.join("mcp-calls.log"));
    call_every_tool(&server);
    {
        // `set_setting` ist ein `INSERT INTO setting ... ON CONFLICT DO UPDATE`.
        let mut store = server.lock_store();
        let err = store.set_setting("x", "y").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("readonly"), "{err}");
        assert!(store.set_setting("app.locale", "en").is_err());
        assert!(
            store
                .insert_threshold(&threshold(
                    None,
                    lastwerk_store::ThresholdKind::HrMaxBpm,
                    200.0,
                    date(2026, 1, 1),
                    None
                ))
                .is_err()
        );
        assert!(
            store
                .save_plan(
                    &sample_plan(
                        "00000000-0000-4000-8000-00000000000b",
                        PlanSource::Rules,
                        None
                    ),
                    PlanStatus::Active
                )
                .is_err()
        );
    }
    drop(server);
    assert_eq!(fs::read(&db).unwrap(), before, "the database file changed");
    let store = Store::open(&db).unwrap();
    assert_eq!(store.get_setting("x").unwrap(), None);
    assert_eq!(
        store.get_setting("app.locale").unwrap().as_deref(),
        Some("de")
    );
    assert_eq!(store.list_plans().unwrap().len(), 1);
}

#[test]
fn only_cycling_filter_and_sport_values_are_typed() {
    let dir = work_dir("privacy-sport");
    let db = build_marker_db(&dir);
    let server = server_for(&db, &dir.join("mcp-calls.log"));
    let v = json(server.list_activities(Parameters(ListActivitiesArgs {
        from: date(2026, 1, 1),
        to: date(2026, 12, 31),
        sport: Some(Sport::Running),
    })));
    let list: ActivityList = serde_json::from_value(v).unwrap();
    assert!(!list.activities.is_empty());
    assert!(list.activities.iter().all(|a| a.sport == Sport::Running));
}

/// Gegenprobe: Die Marker stehen wirklich in der Datenbank. Sonst bewiese der Markertest
/// nichts.
#[test]
fn the_database_really_contains_the_markers() {
    let dir = work_dir("privacy-sanity");
    let db = build_marker_db(&dir);
    let store = Store::open(&db).unwrap();
    assert!(store.activities().unwrap().iter().any(|a| {
        a.row.device_serial == Some(M_SERIAL)
            && a.row
                .device_product
                .as_deref()
                .is_some_and(|p| p.contains(M_PRODUCT))
            && a.row
                .sub_sport
                .as_deref()
                .is_some_and(|p| p.contains(M_SUB_SPORT))
    }));
    assert!(
        store
            .source_files()
            .unwrap()
            .iter()
            .any(|f| f.row.rel_path.contains(M_DIR) && f.row.rel_path.contains(M_FILE))
    );
    assert!(
        store
            .list_thresholds()
            .unwrap()
            .iter()
            .any(|t| t.note.as_deref().is_some_and(|n| n.contains(M_NOTE)))
    );
    assert!(
        store
            .active_goal()
            .unwrap()
            .unwrap()
            .goal
            .label
            .is_some_and(|l| l.contains(M_LABEL) && l.contains(M_LAT))
    );
    assert_eq!(
        store.get_setting(M_SETTING_KEY).unwrap().as_deref(),
        Some(M_SETTING_VALUE)
    );
    assert!(
        store
            .get_setting("app.fit_dir")
            .unwrap()
            .unwrap()
            .contains(M_SETTING_VALUE)
    );
    assert_eq!(
        store.active_plan().unwrap().unwrap().explanation.as_deref(),
        Some(M_EXPLANATION)
    );
}
