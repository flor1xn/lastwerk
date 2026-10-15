//! Aufrufprotokoll (SPEC 5.4, 9.6 Punkt 6): Werkzeug, Parameter, Zeilenzahl, Dauer, Ausgang,
//! keine Nutzdaten.

mod common;

use std::fs;

use common::*;
use lastwerk_mcp::LastwerkMcp;
use lastwerk_mcp::args::{
    ActivityArgs, FitnessArgs, ListActivitiesArgs, RecoveryArgs, RuleArgs, WeeklyLoadArgs,
};
use lastwerk_mcp::calllog::CallLog;
use lastwerk_mcp::startup::{log_path, open_server};
use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value;

fn lines(path: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn every_call_writes_one_line_with_tool_parameters_rows_and_outcome() {
    let dir = work_dir("calllog-lines");
    let db = build_marker_db(&dir);
    let log = dir.join("mcp-calls.log");
    let server = server_for(&db, &log);

    let fitness = server
        .get_fitness_status(Parameters(FitnessArgs {
            from: date(2026, 5, 1),
            to: date(2026, 5, 31),
        }))
        .ok()
        .unwrap()
        .0;
    let weekly = server
        .get_weekly_load(Parameters(WeeklyLoadArgs {
            weeks: 4,
            sport: None,
        }))
        .ok()
        .unwrap()
        .0;
    assert!(
        server
            .explain_rule(Parameters(RuleArgs { code: "R99".into() }))
            .is_err()
    );
    assert!(
        server
            .get_recovery_days(Parameters(RecoveryArgs {
                from: date(2026, 5, 1),
                to: date(2026, 5, 31),
            }))
            .is_err()
    );
    assert!(
        server
            .get_activity(Parameters(ActivityArgs { id: 424_242 }))
            .is_err()
    );
    assert!(
        server
            .list_activities(Parameters(ListActivitiesArgs {
                from: date(2026, 5, 2),
                to: date(2026, 5, 1),
                sport: None,
            }))
            .is_err()
    );

    let l = lines(&log);
    assert_eq!(l.len(), 6);
    for line in &l {
        let keys: Vec<&str> = line
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            ["duration_ms", "params", "rows", "status", "tool", "ts"],
            "fixed set of fields"
        );
        assert_eq!(line["ts"], "2026-05-25T09:30:00Z");
        assert!(line["duration_ms"].is_u64());
    }
    assert_eq!(l[0]["tool"], "get_fitness_status");
    assert_eq!(l[0]["params"]["from"], "2026-05-01");
    assert_eq!(l[0]["params"]["to"], "2026-05-31");
    assert_eq!(l[0]["rows"], fitness.days.len());
    assert!(fitness.days.len() > 1, "the call returned data");
    assert_eq!(l[0]["status"], "ok");
    assert_eq!(l[1]["tool"], "get_weekly_load");
    assert_eq!(l[1]["params"]["weeks"], 4);
    assert_eq!(l[1]["rows"], weekly.weeks.len());
    assert_eq!(l[2]["tool"], "explain_rule");
    assert_eq!(l[2]["params"]["code"], "R99");
    assert_eq!(l[2]["status"], "not_found");
    assert_eq!(l[2]["rows"], 0);
    assert_eq!(l[3]["status"], "disabled");
    assert_eq!(l[4]["params"]["id"], 424_242);
    assert_eq!(l[4]["status"], "not_found");
    assert_eq!(l[5]["status"], "invalid_params");
}

#[test]
fn the_log_holds_no_values_from_the_responses() {
    let dir = work_dir("calllog-nodata");
    let db = build_marker_db(&dir);
    let log = dir.join("mcp-calls.log");
    let server = server_for(&db, &log);
    let rule = server
        .explain_rule(Parameters(RuleArgs { code: "R03".into() }))
        .ok()
        .unwrap()
        .0;
    let fitness = server
        .get_fitness_status(Parameters(FitnessArgs {
            from: date(2026, 5, 1),
            to: date(2026, 5, 31),
        }))
        .ok()
        .unwrap()
        .0;
    let text = fs::read_to_string(&log).unwrap();
    // Weder der Text der Regel noch ein Messwert steht im Protokoll.
    assert!(!text.contains(&rule.title));
    assert!(!text.contains(&rule.explanation));
    assert!(!fitness.days.is_empty());
    // Keine Felder der Antwort, nur die Argumente.
    for field in ["ctl", "atl", "tsb", "hardness", "parameters", "source"] {
        assert!(!text.contains(field), "{field}");
    }
    for marker in markers() {
        assert!(!text.contains(&marker), "{marker}");
    }
    assert_eq!(text.lines().count(), 2);
}

#[test]
fn the_log_is_appended_across_server_runs() {
    let dir = work_dir("calllog-append");
    let log = dir.join("mcp-calls.log");
    for _ in 0..2 {
        let server = LastwerkMcp::with_clock(
            lastwerk_store::Store::open_in_memory().unwrap(),
            CallLog::open(&log).unwrap(),
            fixed_clock(),
        );
        let _ = server.explain_rule(Parameters(RuleArgs { code: "R01".into() }));
    }
    assert_eq!(lines(&log).len(), 2);
}

#[test]
fn the_default_log_sits_next_to_the_database() {
    let dir = work_dir("calllog-default");
    let db = build_marker_db(&dir);
    assert_eq!(log_path(&db, None), dir.join("mcp-calls.log"));
    assert_eq!(
        log_path(std::path::Path::new("m1.db"), None),
        std::path::Path::new("./mcp-calls.log")
    );
    let other = dir.join("elsewhere.log");
    assert_eq!(log_path(&db, Some(&other)), other);

    let server = open_server(&db, None).unwrap();
    drop(server);
    assert!(dir.join("mcp-calls.log").exists());
    let server = open_server(&db, Some(&other)).unwrap();
    drop(server);
    assert!(other.exists());
}

#[test]
fn startup_fails_cleanly_without_database_or_log() {
    let dir = work_dir("calllog-startup");
    let missing = dir.join("missing.db");
    let e = open_server(&missing, None).err().expect("no database");
    assert_eq!(e.exit_code(), 2);
    // Die Meldung nennt keinen Pfad.
    assert!(!e.message().contains("missing.db") && !e.message().contains(dir.to_str().unwrap()));
    assert!(!missing.exists(), "the server must not create a database");

    let db = build_marker_db(&dir);
    let bad_log = dir.join("no-such-folder").join("x.log");
    let e = open_server(&db, Some(&bad_log))
        .err()
        .expect("log not writable");
    assert_eq!(e.exit_code(), 2);
    assert!(!e.message().contains("no-such-folder"));
}
