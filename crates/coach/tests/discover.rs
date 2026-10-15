//! `ollama_models` gegen einen Attrappen-Server auf Loopback: richtiger Pfad, nur die geprüfte
//! Adresse, keine Weiterleitung, Fehler bei Status und Form, Namen bereinigt.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use lastwerk_coach::{BaseUrl, LlmError, ollama_models, pick_model};
use serde_json::json;

/// Beantwortet genau eine Anfrage mit `status`, Zusatzkopfzeilen `extra` und `body` und meldet
/// Anfragezeile und Kopf über den Kanal.
fn serve_once(
    status: &'static str,
    extra: &'static str,
    body: String,
) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut chunk).unwrap();
            assert!(n > 0, "Verbindung zu früh geschlossen");
            buf.extend_from_slice(&chunk[..n]);
        }
        tx.send(String::from_utf8_lossy(&buf).into_owned()).unwrap();
        let text = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
            body.len()
        );
        stream.write_all(text.as_bytes()).unwrap();
        stream.flush().unwrap();
    });
    (port, rx)
}

fn url(port: u16) -> BaseUrl {
    BaseUrl::parse(&format!("http://127.0.0.1:{port}"), false).unwrap()
}

#[test]
fn models_are_listed_in_server_order_without_doubles_or_bad_names() {
    let body = json!({
        "models": [
            { "name": "llama3.2:3b", "model": "llama3.2:3b", "size": 2019393189 },
            { "name": "qwen3:8b" },
            { "name": "qwen3:8b" },
            { "name": "" },
            { "name": " padded " },
            { "name": "a\nb" },
            { "name": "x".repeat(201) },
            { "model": "ohne-name" },
            { "name": 42 },
            { "name": "nomic-embed-text:latest" }
        ]
    })
    .to_string();
    let (port, rx) = serve_once("200 OK", "", body);
    let models = ollama_models(&url(port), 10).unwrap();
    assert_eq!(
        models,
        ["llama3.2:3b", "qwen3:8b", "nomic-embed-text:latest"]
    );
    assert_eq!(pick_model(&models).as_deref(), Some("qwen3:8b"));

    let seen = rx.try_recv().unwrap();
    assert!(seen.starts_with("GET /api/tags HTTP/1.1\r\n"), "{seen}");
    let lower = seen.to_ascii_lowercase();
    assert!(lower.contains(&format!("host: 127.0.0.1:{port}")), "{seen}");
    assert!(!lower.contains("authorization"), "{seen}");
}

#[test]
fn an_empty_list_is_no_error() {
    let (port, _rx) = serve_once("200 OK", "", json!({ "models": [] }).to_string());
    let models = ollama_models(&url(port), 10).unwrap();
    assert!(models.is_empty());
    assert_eq!(pick_model(&models), None);
}

#[test]
fn a_redirect_is_not_followed() {
    // Ein zweiter Server, der nie angesprochen werden darf.
    let other = TcpListener::bind("127.0.0.1:0").unwrap();
    other.set_nonblocking(true).unwrap();
    let other_port = other.local_addr().unwrap().port();
    let location: &'static str =
        Box::leak(format!("Location: http://127.0.0.1:{other_port}/api/tags\r\n").into_boxed_str());
    let (port, _rx) = serve_once("302 Found", location, String::new());
    assert_eq!(ollama_models(&url(port), 10), Err(LlmError::Status(302)));
    assert!(
        other.accept().is_err(),
        "der zweite Server wurde angesprochen"
    );
}

#[test]
fn status_and_shape_errors_are_reported() {
    let (port, _rx) = serve_once("500 Internal Server Error", "", "{}".into());
    assert_eq!(ollama_models(&url(port), 10), Err(LlmError::Status(500)));

    let (port, _rx) = serve_once("200 OK", "", "kein json".into());
    assert!(matches!(
        ollama_models(&url(port), 10),
        Err(LlmError::BadResponse(_))
    ));

    let (port, _rx) = serve_once("200 OK", "", json!({ "data": [] }).to_string());
    assert!(matches!(
        ollama_models(&url(port), 10),
        Err(LlmError::BadResponse(_))
    ));
}

#[test]
fn nobody_listening_is_a_transport_error() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let err = ollama_models(&url(port), 5).unwrap_err();
    assert_eq!(err.code(), "transport", "{err:?}");
}
