//! `pull_model` gegen einen Attrappen-Server auf Loopback: Anfrage, Fortschritt, Abbruch,
//! Fehlerzeilen, keine Weiterleitung, Stillstand.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use lastwerk_coach::pull::pull_with_idle;
use lastwerk_coach::{BaseUrl, PullError, PullPhase, PullProgress, pull_model};

/// Liest Kopf und Rumpf einer Anfrage und liefert beides als Text.
fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
            let len: usize = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .map_or(0, |v| v.trim().parse().unwrap());
            if buf.len() >= pos + 4 + len {
                break;
            }
        }
        let n = stream.read(&mut chunk).unwrap();
        assert!(n > 0, "Verbindung zu früh geschlossen");
        buf.extend_from_slice(&chunk[..n]);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn write_chunk(stream: &mut TcpStream, data: &str) -> std::io::Result<()> {
    write!(stream, "{:x}\r\n{}\r\n", data.len(), data)?;
    stream.flush()
}

const HEAD: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";

/// Antwortet auf genau eine Anfrage mit den Zeilen als Strom (chunked, wie Ollama) und meldet
/// die Anfrage.
fn serve_lines(lines: Vec<String>) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        tx.send(read_request(&mut stream)).unwrap();
        stream.write_all(HEAD.as_bytes()).unwrap();
        for line in lines {
            // Zwei Zeilen in einem Stück und halbe Zeilen kommen vor; hier eine Zeile je Stück.
            write_chunk(&mut stream, &format!("{line}\n")).unwrap();
        }
        stream.write_all(b"0\r\n\r\n").unwrap();
    });
    (port, rx)
}

fn url(port: u16) -> BaseUrl {
    BaseUrl::parse(&format!("http://127.0.0.1:{port}"), false).unwrap()
}

fn run(port: u16, name: &str) -> (Result<(), PullError>, Vec<PullProgress>) {
    let mut seen = Vec::new();
    let cancel = AtomicBool::new(false);
    let r = pull_model(&url(port), name, &mut |p| seen.push(p), &cancel);
    (r, seen)
}

#[test]
fn a_pull_reports_progress_and_ends_with_success() {
    let lines = [
        r#"{"status":"pulling manifest"}"#,
        r#"{"status":"pulling 6e9f","digest":"sha256:6e9f","total":1000,"completed":0}"#,
        r#"{"status":"pulling 6e9f","digest":"sha256:6e9f","total":1000,"completed":400}"#,
        r#"{"status":"pulling 6e9f","digest":"sha256:6e9f","total":1000,"completed":1000}"#,
        r#"{"status":"verifying sha256 digest"}"#,
        r#"{"status":"writing manifest"}"#,
        r#"{"status":"success"}"#,
    ]
    .map(String::from)
    .to_vec();
    let (port, rx) = serve_lines(lines);
    let (result, seen) = run(port, "qwen3:4b");
    assert_eq!(result, Ok(()));

    let request = rx.try_recv().unwrap();
    assert!(
        request.starts_with("POST /api/pull HTTP/1.1\r\n"),
        "{request}"
    );
    let lower = request.to_ascii_lowercase();
    assert!(lower.contains("host: 127.0.0.1:"), "{request}");
    assert!(!lower.contains("authorization"), "{request}");
    let body = request.split("\r\n\r\n").nth(1).unwrap();
    let json: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"model": "qwen3:4b", "stream": true})
    );

    let figures: Vec<(PullPhase, u64, u64)> = seen
        .iter()
        .map(|p| (p.phase, p.completed, p.total))
        .collect();
    assert_eq!(
        figures,
        [
            (PullPhase::Manifest, 0, 0),
            (PullPhase::Download, 0, 1000),
            (PullPhase::Download, 400, 1000),
            (PullPhase::Download, 1000, 1000),
            (PullPhase::Verify, 1000, 1000),
            (PullPhase::Write, 1000, 1000),
            (PullPhase::Done, 1000, 1000),
        ]
    );
}

#[test]
fn an_error_line_becomes_a_code_and_the_text_does_not_travel() {
    let lines = vec![
        r#"{"status":"pulling manifest"}"#.to_string(),
        r#"{"error":"pull model manifest: file does not exist /home/secret/path"}"#.to_string(),
    ];
    let (port, _rx) = serve_lines(lines);
    let (result, _seen) = run(port, "qwen3:4b");
    let err = result.unwrap_err();
    assert_eq!(err, PullError::ModelNotFound);
    assert_eq!(err.code(), "model_not_found");
    assert!(!format!("{err} {err:?}").contains("secret"));
}

#[test]
fn an_unknown_error_is_a_generic_code() {
    let (port, _rx) = serve_lines(vec![r#"{"error":"boom"}"#.to_string()]);
    let (result, _) = run(port, "qwen3:4b");
    assert_eq!(result.unwrap_err().code(), "pull_failed");
}

#[test]
fn a_stream_without_success_is_an_error() {
    let (port, _rx) = serve_lines(vec![r#"{"status":"pulling manifest"}"#.to_string()]);
    let (result, _) = run(port, "qwen3:4b");
    assert_eq!(result.unwrap_err().code(), "bad_response");
}

#[test]
fn a_line_that_is_not_json_is_an_error() {
    let (port, _rx) = serve_lines(vec!["das ist kein json".to_string()]);
    let (result, _) = run(port, "qwen3:4b");
    assert_eq!(result.unwrap_err().code(), "bad_response");
}

#[test]
fn an_invalid_name_is_never_sent() {
    // Kein Server nötig: Es darf nicht einmal verbunden werden.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    for bad in ["", "Qwen3", "a b", "x;y", "../x", "a\nb", "hf.co/u/m"] {
        let (result, seen) = run(port, bad);
        assert_eq!(result, Err(PullError::InvalidModel), "{bad:?}");
        assert!(seen.is_empty());
    }
    assert!(listener.accept().is_err(), "es wurde verbunden");
}

#[test]
fn a_redirect_is_not_followed() {
    let other = TcpListener::bind("127.0.0.1:0").unwrap();
    other.set_nonblocking(true).unwrap();
    let other_port = other.local_addr().unwrap().port();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        let text = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:{other_port}/api/pull\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(text.as_bytes()).unwrap();
    });
    let (result, _) = run(port, "qwen3:4b");
    assert_eq!(result, Err(PullError::Status(307)));
    assert!(
        other.accept().is_err(),
        "der zweite Server wurde angesprochen"
    );
}

#[test]
fn a_status_error_is_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let (result, _) = run(port, "qwen3:4b");
    assert_eq!(result, Err(PullError::Status(500)));
}

#[test]
fn nobody_listening_is_a_transport_error() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let (result, _) = run(port, "qwen3:4b");
    assert_eq!(result.unwrap_err().code(), "transport");
}

/// Der Server sendet eine Zeile und dann alle 50 ms weitere, bis ein Schreiben scheitert, weil
/// der Client die Verbindung geschlossen hat. Das Abbruchflag, nach der ersten Zeile gesetzt,
/// beendet den Aufruf sofort mit `Cancelled`, und der Hilfsthread schließt die Verbindung bei
/// der nächsten Zeile.
#[test]
fn cancel_ends_the_call_at_once_and_closes_the_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (closed_tx, closed_rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        stream.write_all(HEAD.as_bytes()).unwrap();
        let line = r#"{"status":"pulling aaa","digest":"aaa","total":1000,"completed":10}"#;
        for _ in 0..200 {
            if write_chunk(&mut stream, &format!("{line}\n")).is_err() {
                closed_tx.send(true).unwrap();
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        closed_tx.send(false).unwrap();
    });

    let cancel = AtomicBool::new(false);
    let mut count = 0;
    let started = Instant::now();
    let result = pull_model(
        &url(port),
        "qwen3:4b",
        &mut |_| {
            count += 1;
            cancel.store(true, Ordering::SeqCst);
        },
        &cancel,
    );
    assert_eq!(result, Err(PullError::Cancelled));
    assert_eq!(count, 1);
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(
        closed_rx.recv_timeout(Duration::from_secs(8)),
        Ok(true),
        "der Server sah die Verbindung nicht schließen"
    );
}

/// Ein Abbruch vor der ersten Zeile wirkt auch, wenn der Server gar nicht antwortet.
#[test]
fn cancel_works_before_any_answer() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        // Keine Antwort; die Verbindung bleibt offen, bis der Client sie schließt.
        let mut b = [0u8; 1];
        let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
        let _ = stream.read(&mut b);
    });
    let cancel = AtomicBool::new(false);
    thread::scope(|s| {
        s.spawn(|| {
            thread::sleep(Duration::from_millis(300));
            cancel.store(true, Ordering::SeqCst);
        });
        let started = Instant::now();
        let result = pull_model(&url(port), "qwen3:4b", &mut |_| {}, &cancel);
        assert_eq!(result, Err(PullError::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(3));
    });
}

#[test]
fn silence_longer_than_the_idle_limit_is_a_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        stream.write_all(HEAD.as_bytes()).unwrap();
        write_chunk(&mut stream, "{\"status\":\"pulling manifest\"}\n").unwrap();
        thread::sleep(Duration::from_secs(5));
    });
    let cancel = AtomicBool::new(false);
    let result = pull_with_idle(
        &url(port),
        "qwen3:4b",
        &mut |_| {},
        &cancel,
        Duration::from_millis(400),
    );
    assert_eq!(result, Err(PullError::Timeout));
}
