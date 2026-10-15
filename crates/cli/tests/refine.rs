//! `refine` (SPEC 5.4, 9, 12 M5, Task T15): Eingang ohne Freitext (Markertest), Bestätigung,
//! Ausgabe der Erklärung als Text, Fallback, Host-Prüfung und der Weg durch das Binary gegen
//! einen Attrappen-Server auf Loopback. Alle Daten sind synthetisch, alle Dateien liegen unter
//! `target/`.

mod common;

use std::fs;
use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

use common::{marker_plan, markers, now};
use lastwerk_cli::error::CliError;
use lastwerk_cli::planning;
use lastwerk_cli::refine::{
    ProviderChoice, RefineRequest, default_out_path, plain_text, run_refine,
};
use lastwerk_coach::{FakeClient, LlmError, Provider};
use lastwerk_store::{PlanStatus, Store};
use serde_json::Value;

fn work_dir(name: &str) -> PathBuf {
    common::work_dir("cli-refine-tests", name)
}

fn request(plan: &Path, yes: bool) -> RefineRequest {
    RefineRequest {
        plan: plan.to_path_buf(),
        provider: ProviderChoice::Ollama,
        model: "testmodell".into(),
        base_url: None,
        allow_lan: false,
        out: None,
        yes,
        timeout_s: 30,
    }
}

struct Run {
    result: Result<(), CliError>,
    out: String,
    err: String,
}

#[allow(clippy::too_many_arguments)]
fn refine_with(
    db: &Path,
    plan: &Path,
    client: &FakeClient,
    yes: bool,
    interactive: bool,
    answer: &str,
) -> Run {
    let mut store = Store::open(db).unwrap();
    let req = request(plan, yes);
    let provider = Provider::ollama(None, "testmodell", false).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let result = run_refine(
        &mut store,
        &req,
        &provider,
        client,
        "11111111-1111-4111-8111-111111111111".into(),
        now(),
        &mut Cursor::new(answer.as_bytes().to_vec()),
        interactive,
        &mut out,
        &mut err,
    );
    Run {
        result,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn reply(explanation: &str) -> String {
    format!(
        r#"{{"schema_version":1,"edits":[],"explanation":{}}}"#,
        serde_json::to_string(explanation).unwrap()
    )
}

#[test]
fn markers_in_the_database_never_reach_the_model_or_the_output() {
    let dir = work_dir("markers");
    let (db, plan) = marker_plan(&dir);

    // Gegenprobe: Die Marker stehen wirklich in der Datenbank, der Test ist nicht leer.
    let raw = fs::read(&db).unwrap();
    let in_db = markers()
        .iter()
        .filter(|m| raw.windows(m.len()).any(|w| w == m.as_bytes()))
        .count();
    assert!(in_db >= 8, "zu wenige Marker in der Datenbank: {in_db}");
    // Auch die Plandatei und der Plan im Speicher tragen keinen Marker.
    let plan_text = fs::read_to_string(&plan).unwrap();
    for m in markers() {
        assert!(!plan_text.contains(&m), "{m} in der Plandatei");
    }

    // Erster Lauf: gültige Antwort. Zweiter: ungültig bis zum Fallback, damit auch die
    // Korrekturaufrufe geprüft werden.
    for (name, replies) in [
        ("gültig", vec![reply("Alles gut.")]),
        (
            "fallback",
            vec!["x".to_string(), "y".to_string(), "z".to_string()],
        ),
    ] {
        let client = FakeClient::with_replies(replies);
        let run = refine_with(&db, &plan, &client, true, false, "");
        run.result
            .unwrap_or_else(|e| panic!("{name}: {}", e.message()));
        let prompt = client.all_prompt_text();
        assert!(
            prompt.len() > 500,
            "{name}: Prompt zu kurz, nichts gesendet?"
        );
        for m in markers() {
            assert!(!prompt.contains(&m), "{name}: Marker {m} im Prompt");
            assert!(!run.out.contains(&m), "{name}: Marker {m} in der Ausgabe");
            assert!(
                !run.err.contains(&m),
                "{name}: Marker {m} in der Fehlerausgabe"
            );
        }
        // Der Eingang trägt die aggregierten Werte aus der Datenbank: Historie und Signal.
        let input = &client.requests()[0].messages[1].content;
        let value: Value = serde_json::from_str(input).unwrap();
        assert_eq!(value["recovery_signal"], true, "{name}");
        let weekly = value["weekly_load"].as_array().unwrap();
        assert!(
            weekly[0]["load_by_sport"]
                .as_object()
                .is_some_and(|m| !m.is_empty()),
            "{name}: {weekly:?}"
        );
        assert_eq!(value["goal"]["kind"], "event");
    }
}

#[test]
fn the_shown_input_is_exactly_the_text_sent_to_the_model() {
    let dir = work_dir("shown");
    let (db, plan) = marker_plan(&dir);
    let client = FakeClient::with_replies([reply("ok")]);
    let run = refine_with(&db, &plan, &client, true, false, "");
    run.result.unwrap();
    let sent = &client.requests()[0].messages;
    assert!(
        run.out.contains(&sent[1].content),
        "Eingang fehlt in der Ausgabe"
    );
    assert!(
        run.out.contains(sent[0].content.trim_end()),
        "Anweisung fehlt"
    );
    assert!(run.out.contains("--- Eingang (JSON) ---"));
}

#[test]
fn without_a_terminal_and_without_yes_nothing_is_sent() {
    let dir = work_dir("no_tty");
    let (db, plan) = marker_plan(&dir);
    let client = FakeClient::with_replies([reply("ok")]);
    let run = refine_with(&db, &plan, &client, false, false, "j\n");
    let err = run.result.unwrap_err();
    assert_eq!(err.exit_code(), 2);
    assert!(err.message().contains("--yes"));
    assert_eq!(client.calls(), 0);
}

#[test]
fn the_confirmation_decides_whether_anything_is_sent() {
    let dir = work_dir("confirm");
    let (db, plan) = marker_plan(&dir);
    for answer in ["n\n", "\n", "nein\n", "irgendwas\n", ""] {
        let client = FakeClient::with_replies([reply("ok")]);
        let run = refine_with(&db, &plan, &client, false, true, answer);
        run.result.unwrap();
        assert_eq!(client.calls(), 0, "{answer:?}");
        assert!(run.out.contains("Abgebrochen"), "{answer:?}");
        assert!(run.out.contains("Senden? [j/N]"));
    }
    for answer in ["j\n", "J\n", "ja\n", "y\n", "yes\n"] {
        let client = FakeClient::with_replies([reply("ok")]);
        let run = refine_with(&db, &plan, &client, false, true, answer);
        run.result.unwrap();
        assert_eq!(client.calls(), 1, "{answer:?}");
    }
}

#[test]
fn an_applied_plan_is_a_new_draft_next_to_the_input() {
    let dir = work_dir("applied");
    let (db, plan_path) = marker_plan(&dir);
    let before = fs::read_to_string(&plan_path).unwrap();
    let client = FakeClient::with_replies([reply("Passt so.")]);
    let run = refine_with(&db, &plan_path, &client, true, false, "");
    run.result.unwrap();
    // Eingabe unverändert, Ausgabe neben der Eingabe mit Suffix.
    assert_eq!(fs::read_to_string(&plan_path).unwrap(), before);
    let out_path = default_out_path(&plan_path);
    assert_eq!(out_path.file_name().unwrap(), "plan.refined.json");
    let written: Value = serde_json::from_str(&fs::read_to_string(&out_path).unwrap()).unwrap();
    assert_eq!(written["source"], "llm");
    assert_eq!(written["plan_id"], "11111111-1111-4111-8111-111111111111");
    assert_eq!(written["explanation"], "Passt so.");
    assert_eq!(written["created_at"], "2026-10-14T09:00:00Z");
    assert!(planning::read_plan(&out_path).is_ok());
    assert!(run.out.contains("0 Korrekturen"), "{}", run.out);
    assert!(run.out.contains("Passt so."));
    // Meldungen nennen nie den Pfad einer Datei (SPEC 5.4), nur ihre Art.
    assert!(run.out.contains("als Datei geschrieben"), "{}", run.out);
    assert!(
        !run.out.contains("refined.json") && !run.out.contains(dir.to_str().unwrap()),
        "{}",
        run.out
    );

    // Als Entwurf gespeichert, der Regelplan bleibt daneben.
    let store = Store::open(&db).unwrap();
    let plans = store.list_plans().unwrap();
    assert_eq!(plans.len(), 2);
    let refined = plans
        .iter()
        .find(|p| p.id == "11111111-1111-4111-8111-111111111111")
        .unwrap();
    assert_eq!(refined.status, PlanStatus::Draft);
    assert!(refined.goal_id.is_some());
    assert!(
        plans
            .iter()
            .any(|p| p.id == "00000000-0000-4000-8000-000000000001")
    );
}

#[test]
fn an_explicit_out_path_is_used() {
    let dir = work_dir("out_path");
    let (db, plan_path) = marker_plan(&dir);
    let client = FakeClient::with_replies([reply("ok")]);
    let mut store = Store::open(&db).unwrap();
    let mut req = request(&plan_path, true);
    req.out = Some(dir.join("mein-plan.json"));
    let provider = Provider::ollama(None, "m", false).unwrap();
    let mut out = Vec::new();
    run_refine(
        &mut store,
        &req,
        &provider,
        &client,
        "22222222-2222-4222-8222-222222222222".into(),
        now(),
        &mut Cursor::new(Vec::new()),
        false,
        &mut out,
        &mut Vec::new(),
    )
    .unwrap();
    assert!(dir.join("mein-plan.json").is_file());
    let message = String::from_utf8_lossy(&out);
    assert!(
        !message.contains("mein-plan") && !message.contains(dir.to_str().unwrap()),
        "{message}"
    );
    assert!(!default_out_path(&plan_path).exists());
}

#[test]
fn the_explanation_is_printed_as_plain_text_and_stored_unchanged() {
    let dir = work_dir("injection");
    let (db, plan_path) = marker_plan(&dir);
    let nasty = "<script>alert(1)</script> [Klick](http://evil.example/x) **fett** \u{1b}[31mrot\u{1b}]0;Titel\u{7}";
    let client = FakeClient::with_replies([reply(nasty)]);
    let run = refine_with(&db, &plan_path, &client, true, false, "");
    run.result.unwrap();
    // Ausgabe: der Text, ohne Steuerzeichen, sonst nichts verändert (kein Rendern, kein Entfernen).
    let shown = plain_text(nasty);
    assert!(run.out.contains(&shown), "{}", run.out);
    assert!(
        run.out
            .contains("<script>alert(1)</script> [Klick](http://evil.example/x) **fett**")
    );
    assert!(!run.out.contains('\u{1b}'));
    assert!(!run.out.contains('\u{7}'));
    // Datei: der Text unverändert.
    let written: Value =
        serde_json::from_str(&fs::read_to_string(default_out_path(&plan_path)).unwrap()).unwrap();
    assert_eq!(written["explanation"], nasty);
}

#[test]
fn plain_text_keeps_newlines_and_tabs_only() {
    assert_eq!(plain_text("a\nb\tc"), "a\nb\tc");
    assert_eq!(plain_text("a\rb\u{0}c\u{9b}d"), "a?b?c?d");
    assert_eq!(plain_text("Ärger äöü"), "Ärger äöü");
}

#[test]
fn a_fallback_prints_the_reason_counts_corrections_and_writes_nothing() {
    let dir = work_dir("fallback");
    let (db, plan_path) = marker_plan(&dir);
    let client = FakeClient::with_replies(["kein json", "{}", "[]"]);
    let run = refine_with(&db, &plan_path, &client, true, false, "");
    run.result.unwrap();
    assert_eq!(client.calls(), 3);
    assert!(run.out.contains("Der Regelplan bleibt"), "{}", run.out);
    assert!(run.out.contains("invalid_output"));
    assert!(run.out.contains("3 Aufrufe, 2 Korrekturen"), "{}", run.out);
    assert!(!default_out_path(&plan_path).exists());
    assert_eq!(Store::open(&db).unwrap().list_plans().unwrap().len(), 1);
}

#[test]
fn timeout_and_transport_errors_are_fallbacks_with_exit_zero() {
    let dir = work_dir("net");
    let (db, plan_path) = marker_plan(&dir);
    for (error, code) in [
        (LlmError::Timeout, "timeout"),
        (LlmError::Transport("refused".into()), "transport"),
        (LlmError::Status(503), "transport"),
    ] {
        let client = FakeClient::new(vec![Err(error)]);
        let run = refine_with(&db, &plan_path, &client, true, false, "");
        run.result.unwrap();
        assert!(run.out.contains("Der Regelplan bleibt"), "{code}");
        assert!(run.out.contains(&format!("Grund: {code}")), "{}", run.out);
        assert!(run.out.contains("1 Aufrufe, 0 Korrekturen"), "{}", run.out);
    }
    assert!(!default_out_path(&plan_path).exists());
}

#[test]
fn a_plan_with_a_hard_violation_is_not_sent() {
    let dir = work_dir("invalid_plan");
    let (db, plan_path) = marker_plan(&dir);
    let mut value: Value = serde_json::from_str(&fs::read_to_string(&plan_path).unwrap()).unwrap();
    // Zwei Tage in Folge mit harten Einheiten: die Einheit des ersten Trainingstags und die
    // des folgenden werden hart gesetzt.
    let mut changed = 0;
    for day in value["weeks"][0]["days"].as_array_mut().unwrap() {
        for s in day["sessions"].as_array_mut().unwrap() {
            if changed < 2 {
                s["hard"] = true.into();
                changed += 1;
            }
        }
    }
    // Sicherstellen, dass die beiden Einheiten an aufeinanderfolgenden Tagen liegen: sonst
    // schlägt der Test mit einer klaren Meldung fehl.
    fs::write(&plan_path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    let client = FakeClient::with_replies([reply("ok")]);
    let run = refine_with(&db, &plan_path, &client, true, false, "");
    match run.result {
        Err(e) => {
            assert_eq!(e.exit_code(), 1, "{}", e.message());
            assert!(e.message().contains("harte Regeln"), "{}", e.message());
            assert_eq!(client.calls(), 0);
        }
        Ok(()) => panic!("ein Plan mit harter Verletzung darf nicht zum Modell gehen"),
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
    // Auch eine Umgebung mit Proxy darf den Aufruf nicht umleiten.
    cmd.output().expect("lastwerk startet")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn rejected_hosts_end_with_exit_code_two() {
    let dir = work_dir("hosts");
    let (db, plan) = marker_plan(&dir);
    let cases: Vec<(&str, Vec<&str>)> = vec![
        ("ollama", vec!["--base-url", "https://api.openai.com"]),
        (
            "ollama",
            vec!["--base-url", "https://api.anthropic.com", "--allow-lan"],
        ),
        (
            "ollama",
            vec!["--base-url", "http://8.8.8.8:11434", "--allow-lan"],
        ),
        ("ollama", vec!["--base-url", "http://example.com"]),
        (
            "openai-compatible",
            vec!["--base-url", "https://api.openai.com/v1"],
        ),
        // Privat, aber ohne Bestätigung.
        ("ollama", vec!["--base-url", "http://192.168.1.20:11434"]),
        (
            "openai-compatible",
            vec!["--base-url", "http://10.0.0.5:1234"],
        ),
        // Kein Standardwert für OpenAI-kompatible Server.
        ("openai-compatible", vec![]),
        (
            "ollama",
            vec!["--base-url", "http://user:pw@127.0.0.1:11434"],
        ),
    ];
    for (provider, extra) in cases {
        let mut args: Vec<&dyn AsRef<std::ffi::OsStr>> = vec![
            &"refine",
            &plan,
            &"--provider",
            &provider,
            &"--model",
            &"m",
            &"--db",
            &db,
            &"--yes",
        ];
        for e in &extra {
            args.push(e);
        }
        let out = lastwerk(&args);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{provider} {extra:?}: {}",
            text(&out.stderr)
        );
        // Nichts wurde gesendet oder angezeigt.
        assert!(
            !text(&out.stdout).contains("Eingang"),
            "{provider} {extra:?}"
        );
    }
    let out = lastwerk(&[
        &"refine",
        &plan,
        &"--provider",
        &"ollama",
        &"--model",
        &"m",
        &"--db",
        &db,
        &"--yes",
        &"--base-url",
        &"http://192.168.1.20:11434",
    ]);
    assert!(
        text(&out.stderr).contains("--allow-lan"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn without_a_terminal_the_binary_needs_yes() {
    let dir = work_dir("binary_tty");
    let (db, plan) = marker_plan(&dir);
    let out = lastwerk(&[
        &"refine",
        &plan,
        &"--provider",
        &"ollama",
        &"--model",
        &"m",
        &"--db",
        &db,
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("--yes"));
}

/// Einfacher HTTP-Antwortgeber: beantwortet `count` Anfragen mit dem Text der Liste.
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
            let request = read_request(&mut stream);
            seen.push(request);
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

#[test]
fn the_binary_talks_to_the_fake_server_and_prints_the_explanation_as_text() {
    let dir = work_dir("binary_e2e");
    let (db, plan) = marker_plan(&dir);
    let nasty = "<script>alert(1)</script> [Klick](http://evil.example/x)";
    let content = reply(nasty);
    let answer =
        serde_json::json!({"message": {"role": "assistant", "content": content}}).to_string();
    let (port, server) = serve(vec![answer]);
    let url = format!("http://127.0.0.1:{port}");
    let out = lastwerk(&[
        &"refine",
        &plan,
        &"--provider",
        &"ollama",
        &"--model",
        &"testmodell",
        &"--base-url",
        &url,
        &"--db",
        &db,
        &"--yes",
    ]);
    let stdout = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{stdout}\n{}",
        text(&out.stderr)
    );
    assert!(stdout.contains(nasty), "{stdout}");
    assert!(stdout.contains("0 Korrekturen"), "{stdout}");
    let seen = server.join().unwrap();
    assert_eq!(seen.len(), 1);
    assert!(
        seen[0].starts_with("POST /api/chat HTTP/1.1"),
        "{}",
        seen[0]
    );
    for m in markers() {
        assert!(!seen[0].contains(&m), "Marker {m} in der Anfrage");
    }
    let refined = dir.join("plan.refined.json");
    assert!(refined.is_file());
    // Der neue Plan besteht `validate`.
    let v = lastwerk(&[&"validate", &refined, &"--db", &db]);
    assert_eq!(v.status.code(), Some(0), "{}", text(&v.stdout));
}

#[test]
fn the_binary_falls_back_when_nobody_listens() {
    let dir = work_dir("binary_down");
    let (db, plan) = marker_plan(&dir);
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let url = format!("http://127.0.0.1:{port}");
    let out = lastwerk(&[
        &"refine",
        &plan,
        &"--provider",
        &"ollama",
        &"--model",
        &"m",
        &"--base-url",
        &url,
        &"--db",
        &db,
        &"--yes",
        &"--timeout",
        &"5",
    ]);
    let stdout = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{stdout}\n{}",
        text(&out.stderr)
    );
    assert!(stdout.contains("Der Regelplan bleibt"), "{stdout}");
    assert!(stdout.contains("Grund: transport"), "{stdout}");
    assert!(!dir.join("plan.refined.json").exists());
}
