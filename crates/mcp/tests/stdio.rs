//! Rauchtest über stdio (SPEC 11.4, M7): Der Server läuft als Kindprozess, `initialize`,
//! `tools/list` und `tools/call` gehen als JSON-RPC über stdin, die Antworten kommen über
//! stdout. Jeder Lesevorgang hat ein Zeitlimit, damit ein hängender Server den Test nicht
//! blockiert.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_lastwerk-mcp");
const LIMIT: Duration = Duration::from_secs(30);

/// Die Tabelle aus SPEC 5.4.
const SPEC_TOOLS: [&str; 9] = [
    "get_fitness_status",
    "get_weekly_load",
    "list_activities",
    "get_activity",
    "get_current_plan",
    "get_plan_week",
    "get_thresholds",
    "explain_rule",
    "get_recovery_days",
];

struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    next_id: u64,
}

impl Session {
    fn start(db: &Path, log: &Path) -> Self {
        let mut child = Command::new(BIN)
            .arg("--db")
            .arg(db)
            .arg("--log")
            .arg(log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the server binary starts");
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        let stdin = child.stdin.take();
        Self {
            child,
            stdin,
            lines,
            next_id: 1,
        }
    }

    fn send(&mut self, v: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap();
    }

    /// Sendet eine Anfrage und wartet auf die Antwort mit derselben ID.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let deadline = Instant::now() + LIMIT;
        loop {
            let left = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| panic!("no answer to {method} within {LIMIT:?}"));
            let line = self
                .lines
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("no answer to {method} within {LIMIT:?}"));
            let v: Value = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("stdout must carry only JSON-RPC, got {line:?}: {e}"));
            if v["id"] == id {
                return v;
            }
        }
    }

    fn initialize(&mut self) -> Value {
        let answer = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "lastwerk-test", "version": "0.0.1"}
            }),
        );
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        answer
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": tool, "arguments": arguments}))
    }

    /// Schließt stdin und wartet, dass der Server von selbst endet.
    fn finish(mut self) -> std::process::ExitStatus {
        drop(self.stdin.take());
        let deadline = Instant::now() + LIMIT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            if Instant::now() > deadline {
                let _ = self.child.kill();
                panic!("the server did not end after stdin was closed");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
fn initialize_list_and_call_over_stdio() {
    let dir = work_dir("stdio-session");
    let db = build_marker_db(&dir);
    let log = dir.join("mcp-calls.log");
    let mut s = Session::start(&db, &log);

    let init = s.initialize();
    assert_eq!(init["result"]["serverInfo"]["name"], "lastwerk-mcp");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    assert!(init["result"]["capabilities"].get("resources").is_none());
    assert!(init["result"]["capabilities"].get("prompts").is_none());

    let list = s.request("tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    let mut want = SPEC_TOOLS.to_vec();
    want.sort_unstable();
    assert_eq!(names, want);
    for t in tools {
        assert_eq!(t["annotations"]["readOnlyHint"], true, "{}", t["name"]);
        assert_eq!(t["annotations"]["openWorldHint"], false, "{}", t["name"]);
    }

    // Schwellenwerte: strukturierte Antwort mit dem Wert aus der Datenbank.
    let th = s.call("get_thresholds", json!({"on_date": "2026-05-15"}));
    assert_ne!(th["result"]["isError"], true, "{th}");
    let sports = th["result"]["structuredContent"]["sports"]
        .as_array()
        .unwrap();
    let cycling = sports.iter().find(|x| x["sport"] == "cycling").unwrap();
    let ftp = cycling["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["kind"] == "ftp_w")
        .unwrap();
    assert_eq!(ftp["value"], 260.0);

    // Form: die Frage "Wie ist meine Form diese Woche?" beantwortet dieses Werkzeug.
    let fit = s.call(
        "get_fitness_status",
        json!({"from": "2026-05-01", "to": "2026-05-31"}),
    );
    let days = fit["result"]["structuredContent"]["days"]
        .as_array()
        .unwrap();
    assert!(!days.is_empty());
    assert!(days[0]["tsb"].is_number() && days[0]["ctl"].is_number());

    let plan = s.call("get_current_plan", json!({}));
    assert_eq!(plan["result"]["structuredContent"]["plan_id"], PLAN_ID);
    assert!(
        plan["result"]["structuredContent"]
            .get("explanation")
            .is_none()
    );

    let rule = s.call("explain_rule", json!({"code": "R03"}));
    assert_eq!(
        rule["result"]["structuredContent"]["parameters"]["max_hard_per_week"],
        2
    );

    // Erholungswerte ohne Freigabe: Fehler im Ergebnis.
    let rec = s.call(
        "get_recovery_days",
        json!({"from": "2026-05-01", "to": "2026-05-31"}),
    );
    assert_eq!(rec["result"]["isError"], true, "{rec}");

    // Ungültige Argumente und unbekannte Werkzeuge enden als Fehler, nie als Ergebnis.
    for (tool, args) in [
        ("explain_rule", json!({"code": "R03; DROP TABLE"})),
        ("get_weekly_load", json!({"weeks": 500})),
        ("get_weekly_load", json!({"weeks": 4, "extra": "x"})),
        ("get_activity", json!({"id": "1 OR 1=1"})),
        (
            "get_fitness_status",
            json!({"from": "gestern", "to": "heute"}),
        ),
        ("set_setting", json!({"key": "a", "value": "b"})),
        ("read_file", json!({"path": "/etc/passwd"})),
    ] {
        let answer = s.call(tool, args.clone());
        let failed = answer.get("error").is_some() || answer["result"]["isError"] == true;
        assert!(failed, "{tool} {args} must fail: {answer}");
    }

    let status = s.finish();
    assert!(status.success(), "{status:?}");

    // Das Protokoll hat die gültigen Aufrufe, aber keinen Marker und keine Antwortwerte.
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.lines().count() >= 5, "{text}");
    for marker in markers() {
        assert!(!text.contains(&marker), "{marker}");
    }
}

#[test]
fn the_database_is_not_changed_by_a_session() {
    let dir = work_dir("stdio-unchanged");
    let db = build_marker_db(&dir);
    let before = std::fs::read(&db).unwrap();
    let mut s = Session::start(&db, &dir.join("mcp-calls.log"));
    s.initialize();
    s.call("get_current_plan", json!({}));
    s.call("get_weekly_load", json!({"weeks": 52}));
    assert!(s.finish().success());
    assert_eq!(std::fs::read(&db).unwrap(), before);
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("the server binary runs")
}

#[test]
fn list_tools_prints_json_and_exits_without_starting_the_transport() {
    let dir = work_dir("stdio-list");
    // Die Datenbank muss dafür nicht existieren.
    let db = dir.join("not-there.db");
    let out = run(&["--db", db.to_str().unwrap(), "--list-tools"]);
    assert_eq!(out.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let names: Vec<&str> = v["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, SPEC_TOOLS);
    assert!(
        v["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["annotations"]["readOnlyHint"] == true)
    );
    assert!(!db.exists());
    assert!(!dir.join("mcp-calls.log").exists());
}

#[test]
fn missing_arguments_and_databases_end_with_exit_code_2() {
    let dir = work_dir("stdio-exit");
    assert_eq!(run(&[]).status.code(), Some(2), "--db is required");
    assert_eq!(run(&["--list-tools"]).status.code(), Some(2));
    let missing = dir.join("missing.db");
    let out = run(&["--db", missing.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "stdout belongs to the protocol");
    assert!(!missing.exists(), "the server must not create the database");

    let not_a_db = dir.join("text.db");
    std::fs::write(&not_a_db, "this is not a database").unwrap();
    assert_eq!(
        run(&["--db", not_a_db.to_str().unwrap()]).status.code(),
        Some(2)
    );
    assert_eq!(run(&["--unknown"]).status.code(), Some(2));
}
