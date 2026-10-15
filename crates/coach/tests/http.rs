//! Tests der HTTP-Clients gegen einen Attrappen-Server auf Loopback (SPEC 11.4): Der Client
//! ruft nur die konfigurierte Adresse und den erwarteten Pfad auf, sendet `stream: false` und
//! das Schema, versteht die Antwortform, folgt keiner Weiterleitung und hält das Zeitlimit.

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::{base_plan, context, reply, rules};
use lastwerk_coach::edit::edit_schema;
use lastwerk_coach::{
    BaseUrl, LlmClient, LlmError, LlmRequest, Message, OllamaClient, OpenAiCompatibleClient,
    Provider, RefineConfig, RefineOutcome, Role, refine_plan,
};
use serde_json::{Value, json};

/// Eine aufgezeichnete Anfrage.
#[derive(Debug)]
struct Seen {
    request_line: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

fn read_request(stream: &mut TcpStream) -> Seen {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = stream.read(&mut chunk).unwrap();
        assert!(n > 0, "Verbindung zu früh geschlossen");
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let head = String::from_utf8(buf[..head_end].to_vec()).unwrap();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap().to_string();
    let headers: Vec<(String, String)> = lines
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (k, v) = l.split_once(':').unwrap();
            (k.trim().to_string(), v.trim().to_string())
        })
        .collect();
    let length: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map(|(_, v)| v.parse().unwrap())
        .unwrap_or(0);
    while buf.len() < head_end + length {
        let n = stream.read(&mut chunk).unwrap();
        assert!(n > 0, "Verbindung zu früh geschlossen");
        buf.extend_from_slice(&chunk[..n]);
    }
    Seen {
        request_line,
        headers,
        body: String::from_utf8(buf[head_end..head_end + length].to_vec()).unwrap(),
    }
}

fn respond(stream: &mut TcpStream, status: &str, extra: &str, body: &str) {
    let text = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
        body.len()
    );
    stream.write_all(text.as_bytes()).unwrap();
    stream.flush().unwrap();
}

/// Server auf Loopback, Port 0. Beantwortet nacheinander so viele Anfragen, wie `replies`
/// Einträge hat, und meldet jede gesehene Anfrage über den Kanal.
fn serve(replies: Vec<(&'static str, String, String)>) -> (u16, mpsc::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for (status, extra, body) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            let seen = read_request(&mut stream);
            tx.send(seen).unwrap();
            respond(&mut stream, status, &extra, &body);
        }
    });
    (port, rx)
}

fn request(timeout_s: u32) -> LlmRequest {
    LlmRequest {
        messages: vec![
            Message::new(Role::System, "System"),
            Message::new(Role::User, "{\"a\":1}"),
        ],
        schema: edit_schema(),
        timeout_s,
    }
}

fn url(port: u16) -> BaseUrl {
    BaseUrl::parse(&format!("http://127.0.0.1:{port}"), false).unwrap()
}

#[test]
fn ollama_client_calls_only_the_configured_address_and_path() {
    let answer = json!({
        "model": "m",
        "message": { "role": "assistant", "content": "{\"antwort\":true}" },
        "done": true
    })
    .to_string();
    let (port, rx) = serve(vec![("200 OK", String::new(), answer)]);
    let client = OllamaClient::new(url(port), "testmodell:1b".into());
    let response = client.complete(&request(10)).unwrap();
    assert_eq!(response.content, "{\"antwort\":true}");

    let seen = rx.try_recv().unwrap();
    assert_eq!(seen.request_line, "POST /api/chat HTTP/1.1");
    assert_eq!(
        seen.header("host"),
        Some(format!("127.0.0.1:{port}").as_str())
    );
    assert_eq!(seen.header("content-type"), Some("application/json"));
    // Keine Zugangsdaten in irgendeiner Kopfzeile.
    assert!(seen.header("authorization").is_none());
    let body = seen.json();
    assert_eq!(body["model"], "testmodell:1b");
    assert_eq!(body["stream"], false);
    assert_eq!(
        body["messages"][0],
        json!({"role":"system","content":"System"})
    );
    assert_eq!(body["messages"][1]["role"], "user");
    // `format` ist das Schema der Änderung, ohne die Kopfschlüssel `$schema` und `$id`.
    assert_eq!(body["format"]["properties"]["edits"]["maxItems"], 14);
    assert!(body["format"].get("$schema").is_none());
    assert!(body["format"].get("$id").is_none());
    // Genau eine Anfrage.
    assert!(rx.try_recv().is_err());
}

#[test]
fn openai_compatible_client_uses_chat_completions_with_json_schema() {
    let answer = json!({
        "id": "x",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": "{\"ok\":1}" } }]
    })
    .to_string();
    let (port, rx) = serve(vec![("200 OK", String::new(), answer)]);
    let client = OpenAiCompatibleClient::new(url(port), "lokal".into());
    let response = client.complete(&request(10)).unwrap();
    assert_eq!(response.content, "{\"ok\":1}");

    let seen = rx.try_recv().unwrap();
    assert_eq!(seen.request_line, "POST /v1/chat/completions HTTP/1.1");
    assert!(seen.header("authorization").is_none());
    let body = seen.json();
    assert_eq!(body["model"], "lokal");
    assert_eq!(body["stream"], false);
    assert_eq!(body["response_format"]["type"], "json_schema");
    assert_eq!(
        body["response_format"]["json_schema"]["schema"]["properties"]["explanation"]["maxLength"],
        1200
    );
    assert!(rx.try_recv().is_err());
}

#[test]
fn a_base_url_ending_in_v1_does_not_double_the_prefix() {
    let answer = json!({"choices":[{"message":{"content":"x"}}]}).to_string();
    let (port, rx) = serve(vec![("200 OK", String::new(), answer)]);
    let base = BaseUrl::parse(&format!("http://127.0.0.1:{port}/v1"), false).unwrap();
    OpenAiCompatibleClient::new(base, "m".into())
        .complete(&request(10))
        .unwrap();
    assert_eq!(
        rx.try_recv().unwrap().request_line,
        "POST /v1/chat/completions HTTP/1.1"
    );
}

#[test]
fn error_status_and_odd_bodies_are_errors() {
    let cases: Vec<(&'static str, &str, LlmError)> = vec![
        (
            "500 Internal Server Error",
            "{\"error\":\"x\"}",
            LlmError::Status(500),
        ),
        ("404 Not Found", "nicht da", LlmError::Status(404)),
        (
            "200 OK",
            "das ist kein JSON",
            LlmError::BadResponse("the body is not json".into()),
        ),
        (
            "200 OK",
            "{\"message\":{}}",
            LlmError::BadResponse("message.content is missing".into()),
        ),
    ];
    for (status, body, expected) in cases {
        let (port, _rx) = serve(vec![(status, String::new(), body.to_string())]);
        let client = OllamaClient::new(url(port), "m".into());
        assert_eq!(
            client.complete(&request(10)).unwrap_err(),
            expected,
            "{status}"
        );
    }
}

#[test]
fn a_redirect_is_not_followed() {
    // Ziel der Weiterleitung: ein zweiter Server, der keine Anfrage sehen darf.
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let target_port = target.local_addr().unwrap().port();
    let location = format!("Location: http://127.0.0.1:{target_port}/api/chat\r\n");
    let (port, _rx) = serve(vec![("307 Temporary Redirect", location, String::new())]);
    let client = OllamaClient::new(url(port), "m".into());
    assert!(client.complete(&request(10)).is_err());
    thread::sleep(Duration::from_millis(100));
    assert!(
        target.accept().is_err(),
        "der Client hat der Weiterleitung gefolgt"
    );
}

#[test]
fn a_silent_server_ends_in_a_timeout_within_the_limit() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = read_request(&mut stream);
        // Keine Antwort, bis der Client aufgegeben hat.
        thread::sleep(Duration::from_secs(4));
    });
    let client = OllamaClient::new(url(port), "m".into());
    let started = Instant::now();
    let err = client.complete(&request(1)).unwrap_err();
    assert_eq!(err, LlmError::Timeout);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    handle.join().unwrap();
}

#[test]
fn an_unreachable_server_is_a_transport_error() {
    // Port 0 gebunden und wieder freigegeben: dort lauscht niemand.
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let client = OllamaClient::new(url(port), "m".into());
    // Windows wiederholt den Verbindungsaufbau zu einem geschlossenen Port, bevor es ablehnt
    // (in der CI rund 2 s). Ein großzügiges Zeitlimit lässt die Ablehnung vor dem Zeitlimit
    // ankommen; unter Linux kommt sie sofort.
    let err = client.complete(&request(30)).unwrap_err();
    assert!(matches!(err, LlmError::Transport(_)), "{err:?}");
}

#[test]
fn refine_plan_works_end_to_end_against_the_fake_server() {
    let edit = r#"{"op":"replace","date":"2027-01-07","slot":"am","template_id":"bike.endurance.steady","duration_min":60}"#;
    let valid = reply(edit, "Länger locker.");
    let answer =
        |content: &str| json!({"message":{"role":"assistant","content":content}}).to_string();
    // Erste Antwort ungültig, zweite gültig: zwei Anfragen an dieselbe Adresse und denselben Pfad.
    let (port, rx) = serve(vec![
        ("200 OK", String::new(), answer("nein")),
        ("200 OK", String::new(), answer(&valid)),
    ]);
    let provider = Provider::ollama(Some(&format!("http://127.0.0.1:{port}")), "m", false).unwrap();
    let client = provider.client();
    let ctx = context();
    let base = base_plan(&ctx);
    let outcome = refine_plan(
        &base,
        &ctx,
        &rules(),
        client.as_ref(),
        &RefineConfig::default(),
    );
    let RefineOutcome::Applied {
        plan,
        explanation,
        attempts,
    } = outcome
    else {
        panic!("Applied erwartet: {outcome:?}");
    };
    assert_eq!(attempts, 2);
    assert_eq!(explanation, "Länger locker.");
    assert_eq!(plan.weeks[0].planned_load, Some(257.9));
    let first = rx.recv().unwrap();
    let second = rx.recv().unwrap();
    for seen in [&first, &second] {
        assert_eq!(seen.request_line, "POST /api/chat HTTP/1.1");
        assert_eq!(seen.json()["stream"], false);
    }
    assert_eq!(first.json()["messages"].as_array().unwrap().len(), 2);
    assert_eq!(second.json()["messages"].as_array().unwrap().len(), 4);
}
