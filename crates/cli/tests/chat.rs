//! `chat` (SPEC 9.7, T26 Teil A): Markertest wie in T15 (Zielbezeichnung, Pfade, Namen und
//! Notizen erreichen das Modell nie, der eigene Text des Nutzers schon), angezeigter Eingang,
//! aktiver Plan, nichts gespeichert, Host-Prüfung und der Weg durch das Binary gegen einen
//! Attrappen-Server auf Loopback. Alle Daten sind synthetisch.

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

use common::{marker_plan, markers};
use lastwerk_cli::chat::{ChatRequest, run_chat};
use lastwerk_cli::error::CliError;
use lastwerk_cli::refine::ProviderChoice;
use lastwerk_coach::{FakeClient, LlmError, Provider, Role};
use lastwerk_store::{PlanStatus, Store};
use serde_json::{Value, json};

/// Eigener Text des Nutzers mit einer erkennbaren Kennung: Er darf zum Modell (SPEC 9.7).
const USER_TEXT: &str = "Dienstag habe ich keine Zeit, NUTZERTEXT-4711.";

fn work_dir(name: &str) -> PathBuf {
    common::work_dir("cli-chat-tests", name)
}

fn request(plan: Option<&Path>, show_input: bool) -> ChatRequest {
    ChatRequest {
        plan: plan.map(Path::to_path_buf),
        message: USER_TEXT.into(),
        provider: ProviderChoice::Ollama,
        model: Some("testmodell".into()),
        base_url: None,
        allow_lan: false,
        show_input,
        timeout_s: 30,
    }
}

struct Run {
    result: Result<(), CliError>,
    out: String,
    err: String,
}

fn chat_with(db: &Path, req: &ChatRequest, client: &FakeClient) -> Run {
    let store = Store::open(db).unwrap();
    let provider = Provider::ollama(None, "testmodell", false).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let result = run_chat(&store, req, &provider, client, &mut out, &mut err);
    Run {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn answer(reply: &str, proposal: Option<Value>) -> String {
    let mut v = json!({ "schema_version": 1, "reply": reply });
    if let Some(p) = proposal {
        v["proposal"] = p;
    }
    v.to_string()
}

#[test]
fn markers_never_reach_the_model_but_the_users_own_text_does() {
    let dir = work_dir("markers");
    let (db, plan) = marker_plan(&dir);

    // Gegenprobe: Die Marker stehen wirklich in der Datenbank.
    let raw = std::fs::read(&db).unwrap();
    let in_db = markers()
        .iter()
        .filter(|m| raw.windows(m.len()).any(|w| w == m.as_bytes()))
        .count();
    assert!(in_db >= 8, "zu wenige Marker in der Datenbank: {in_db}");

    // Ein ungültiger plan_edit, damit auch Korrekturaufrufe mit Fehlercodes geprüft werden.
    let bad_edit = answer(
        "Vorschlag.",
        Some(json!({
            "kind": "plan_edit",
            "edit": {
                "schema_version": 1,
                "edits": [{"op": "remove", "date": "2030-01-01", "slot": "pm"}],
                "explanation": "x"
            }
        })),
    );
    for (name, replies) in [
        ("gültig", vec![answer("Alles klar.", None)]),
        (
            "unlesbar",
            vec!["x".to_string(), "y".to_string(), "z".to_string()],
        ),
        (
            "vorschlag abgelehnt",
            vec![bad_edit.clone(), bad_edit.clone(), bad_edit],
        ),
    ] {
        let client = FakeClient::with_replies(replies);
        let run = chat_with(&db, &request(Some(&plan), true), &client);
        run.result
            .unwrap_or_else(|e| panic!("{name}: {}", e.message()));
        let prompt = client.all_prompt_text();
        assert!(prompt.len() > 500, "{name}: Prompt zu kurz");
        for m in markers() {
            assert!(!prompt.contains(&m), "{name}: Marker {m} im Prompt");
            assert!(!run.out.contains(&m), "{name}: Marker {m} in der Ausgabe");
            assert!(
                !run.err.contains(&m),
                "{name}: Marker {m} in der Fehlerausgabe"
            );
        }
        assert!(prompt.contains(USER_TEXT), "{name}: Text des Nutzers fehlt");

        // Der Eingang steht hinter der Anweisung und trägt die aggregierten Werte und die
        // (noch leere) Liste der Kennzahlen.
        let system = &client.requests()[0].messages[0].content;
        let start = system.rfind("\n{").unwrap() + 1;
        let input: Value = serde_json::from_str(&system[start..]).unwrap();
        assert_eq!(input["recovery_signal"], true, "{name}");
        assert_eq!(input["goal"]["kind"], "event", "{name}");
        assert_eq!(input["metrics"], json!([]), "{name}");
    }
}

#[test]
fn the_shown_input_is_exactly_the_text_sent_to_the_model() {
    let dir = work_dir("shown");
    let (db, plan) = marker_plan(&dir);
    let client = FakeClient::with_replies([answer("ok", None)]);
    let run = chat_with(&db, &request(Some(&plan), true), &client);
    run.result.unwrap();
    let sent = &client.requests()[0].messages;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].role, Role::User);
    for m in sent {
        assert!(
            run.out
                .contains(&format!("--- {} ---\n{}", m.role.as_str(), m.content)),
            "{}",
            m.role.as_str()
        );
    }

    // Ohne --show-input steht der Eingang nicht in der Ausgabe.
    let client = FakeClient::with_replies([answer("ok", None)]);
    let run = chat_with(&db, &request(Some(&plan), false), &client);
    run.result.unwrap();
    assert!(!run.out.contains("--- system ---"), "{}", run.out);
}

#[test]
fn without_a_plan_file_the_active_plan_is_used() {
    let dir = work_dir("active");
    let (db, _plan) = marker_plan(&dir);
    let client = FakeClient::with_replies([answer("ok", None)]);
    let run = chat_with(&db, &request(None, false), &client);
    let err = run.result.unwrap_err();
    assert_eq!(err.exit_code(), 2, "{}", err.message());
    assert!(err.message().contains("aktiven Plan"));
    assert_eq!(client.calls(), 0);

    let mut store = Store::open(&db).unwrap();
    let id = store.list_plans().unwrap()[0].id.clone();
    store.set_plan_status(&id, PlanStatus::Active).unwrap();
    let run = chat_with(&db, &request(None, false), &client);
    run.result.unwrap();
    assert_eq!(client.calls(), 1);
}

#[test]
fn a_proposal_is_shown_but_nothing_is_saved() {
    let dir = work_dir("nothing_saved");
    let (db, plan) = marker_plan(&dir);
    let before = Store::open(&db).unwrap().list_plans().unwrap();
    let client = FakeClient::with_replies([answer(
        "Kürzer planen.",
        Some(json!({"kind": "planning_change", "weeks": 4, "sports": ["running"]})),
    )]);
    let run = chat_with(&db, &request(Some(&plan), false), &client);
    run.result.unwrap();
    assert!(run.out.contains("Kürzer planen."), "{}", run.out);
    assert!(run.out.contains("Wochen: 4"), "{}", run.out);
    assert!(run.out.contains("Sportarten: running"), "{}", run.out);
    assert!(run.out.contains("Nichts wurde übernommen"), "{}", run.out);
    assert_eq!(Store::open(&db).unwrap().list_plans().unwrap(), before);
}

#[test]
fn rejected_proposals_and_model_errors_are_reported_with_codes() {
    let dir = work_dir("rejected");
    let (db, plan) = marker_plan(&dir);
    let bad = answer(
        "Zu viel.",
        Some(json!({"kind": "planning_change", "weeks": 99})),
    );
    let client = FakeClient::with_replies([bad.clone(), bad.clone(), bad]);
    let run = chat_with(&db, &request(Some(&plan), false), &client);
    run.result.unwrap();
    assert!(run.out.contains("Zu viel."), "{}", run.out);
    assert!(run.out.contains("verworfen: SCHEMA"), "{}", run.out);
    assert!(run.out.contains("3 Aufrufe"), "{}", run.out);

    for (error, code) in [
        (LlmError::Timeout, "timeout"),
        (LlmError::Status(503), "transport"),
    ] {
        let client = FakeClient::new(vec![Err(error)]);
        let run = chat_with(&db, &request(Some(&plan), false), &client);
        run.result.unwrap();
        assert!(
            run.out.contains(&format!("Grund: {code}, 1 Aufrufe")),
            "{}",
            run.out
        );
    }
}

#[test]
fn the_reply_is_printed_as_plain_text() {
    let dir = work_dir("plain");
    let (db, plan) = marker_plan(&dir);
    let nasty = "<script>alert(1)</script> [Klick](http://evil.example/x) \u{1b}[31mrot";
    let client = FakeClient::with_replies([answer(nasty, None)]);
    let run = chat_with(&db, &request(Some(&plan), false), &client);
    run.result.unwrap();
    assert!(
        run.out
            .contains("<script>alert(1)</script> [Klick](http://evil.example/x) ?[31mrot"),
        "{}",
        run.out
    );
    assert!(!run.out.contains('\u{1b}'));
}

#[test]
fn an_empty_or_too_long_message_is_an_input_error() {
    let dir = work_dir("message");
    let (db, plan) = marker_plan(&dir);
    for text in ["   ".to_string(), "a".repeat(1001)] {
        let client = FakeClient::with_replies([answer("ok", None)]);
        let mut req = request(Some(&plan), false);
        req.message = text;
        let err = chat_with(&db, &req, &client).result.unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert_eq!(client.calls(), 0);
    }
}

// Prozess-Tests mit dem Binary.

fn lastwerk(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lastwerk"));
    for a in args {
        cmd.arg(a.as_ref());
    }
    cmd.env_remove("XDG_DATA_HOME")
        .env_remove("LOCALAPPDATA")
        .env_remove("LASTWERK_DB");
    cmd.output().expect("lastwerk startet")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).unwrap();
        assert!(n > 0);
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
            let length: usize = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .map(|v| v.trim().parse().unwrap())
                .unwrap_or(0);
            if buf.len() >= pos + 4 + length {
                break;
            }
        }
    }
    String::from_utf8(buf).unwrap()
}

/// Beantwortet nacheinander so viele Anfragen, wie `bodies` Einträge hat.
fn serve(bodies: Vec<String>) -> (u16, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let mut seen = Vec::new();
        for body in bodies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            seen.push(read_request(&mut stream));
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
        seen
    });
    (port, handle)
}

#[test]
fn the_binary_picks_a_model_talks_to_the_fake_server_and_prints_text() {
    let dir = work_dir("binary_e2e");
    let (db, plan) = marker_plan(&dir);
    let nasty = "<script>alert(1)</script> Dienstag ist frei.";
    let tags =
        json!({"models": [{"name": "nomic-embed-text:latest"}, {"name": "qwen3:8b"}]}).to_string();
    let chat =
        json!({"message": {"role": "assistant", "content": answer(nasty, None)}}).to_string();
    let (port, server) = serve(vec![tags, chat]);
    let url = format!("http://127.0.0.1:{port}");
    let out = lastwerk(&[
        &"chat",
        &"--db",
        &db,
        &"--plan",
        &plan,
        &"--message",
        &USER_TEXT,
        &"--base-url",
        &url,
    ]);
    let stdout = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{stdout}\n{}",
        text(&out.stderr)
    );
    assert!(stdout.contains(nasty), "{stdout}");
    let seen = server.join().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen[0].starts_with("GET /api/tags HTTP/1.1"), "{}", seen[0]);
    assert!(
        seen[1].starts_with("POST /api/chat HTTP/1.1"),
        "{}",
        seen[1]
    );
    let body: Value =
        serde_json::from_str(&seen[1][seen[1].find("\r\n\r\n").unwrap() + 4..]).unwrap();
    // Gewählt nach der Vorzugsliste, nicht das Einbettungsmodell.
    assert_eq!(body["model"], "qwen3:8b");
    assert_eq!(body["format"]["properties"]["reply"]["maxLength"], 1200);
    for m in markers() {
        assert!(!seen[1].contains(&m), "Marker {m} in der Anfrage");
    }
}

#[test]
fn rejected_hosts_end_with_exit_code_two_before_any_request() {
    let dir = work_dir("hosts");
    let (db, plan) = marker_plan(&dir);
    for extra in [
        vec!["--base-url", "https://api.openai.com"],
        vec!["--base-url", "http://8.8.8.8:11434", "--allow-lan"],
        vec!["--base-url", "http://192.168.1.20:11434"],
        vec!["--base-url", "http://192.168.1.20:11434", "--model", "m"],
        vec!["--provider", "openai-compatible", "--model", "m"],
        vec![
            "--provider",
            "openai-compatible",
            "--base-url",
            "http://127.0.0.1:1",
        ],
    ] {
        let mut args: Vec<&dyn AsRef<std::ffi::OsStr>> = vec![
            &"chat",
            &"--db",
            &db,
            &"--plan",
            &plan,
            &"--message",
            &USER_TEXT,
        ];
        for e in &extra {
            args.push(e);
        }
        let out = lastwerk(&args);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{extra:?}: {}",
            text(&out.stderr)
        );
        assert!(!text(&out.stdout).contains("Antwort"), "{extra:?}");
    }
}
