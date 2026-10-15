//! Gemeinsame Hilfen der Tests: Attrappen-Server auf Loopback, falsche Uhr, Token-Bau.
//!
//! Alle Werte sind synthetisch. Kein Test spricht mit Garmin: `Endpoints::for_tests` erlaubt nur
//! `http://127.0.0.1:<port>`.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use base64::Engine;
use lastwerk_garmin::{
    ClientConfig, Clock, Endpoints, GarminClient, Jitter, MemoryStore, ThrottleConfig, TokenSet,
};

/// 2026-09-13 12:26:40 UTC. Der genaue Tag ist ohne Bedeutung.
pub const START_S: i64 = 1_789_000_000;

/// Marker, der in keiner Ausgabe, keinem Fehler und keiner Datei außer dem Token selbst erscheinen darf.
pub const MARKER: &str = "TOKENMARKER7f3a9c";

// Falsche Uhr

#[derive(Debug)]
pub struct FakeClock {
    now_ms: Mutex<i64>,
    sleeps: Mutex<Vec<Duration>>,
}

impl FakeClock {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            now_ms: Mutex::new(START_S * 1000),
            sleeps: Mutex::new(Vec::new()),
        })
    }

    pub fn advance(&self, d: Duration) {
        *self.now_ms.lock().unwrap() += d.as_millis() as i64;
    }

    /// Setzt die Wanduhr (auch rückwärts, wie bei einer Zeitumstellung oder einem Uhrensprung).
    pub fn set_s(&self, seconds: i64) {
        *self.now_ms.lock().unwrap() = seconds * 1000;
    }

    pub fn sleeps(&self) -> Vec<Duration> {
        self.sleeps.lock().unwrap().clone()
    }

    pub fn total_slept(&self) -> Duration {
        self.sleeps().iter().sum()
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        *self.now_ms.lock().unwrap()
    }

    fn sleep(&self, duration: Duration) {
        // Schlafen lässt die falsche Uhr vorrücken und wartet nicht wirklich.
        self.sleeps.lock().unwrap().push(duration);
        self.advance(duration);
    }
}

pub struct FixedJitter(pub f64);

impl Jitter for FixedJitter {
    fn fraction(&self) -> f64 {
        self.0
    }
}

// Token

pub fn jwt(exp: i64, client_id: &str, marker: &str) -> String {
    let enc = |s: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s);
    let payload = format!(r#"{{"exp":{exp},"client_id":"{client_id}","m":"{marker}"}}"#);
    format!("{}.{}.sig", enc(r#"{"alg":"RS256"}"#), enc(&payload))
}

/// Token, das noch `valid_for_s` Sekunden gilt (gemessen an der falschen Uhr beim Start).
pub fn tokens(valid_for_s: i64, refresh: &str) -> TokenSet {
    TokenSet::for_tests(
        &jwt(
            START_S + valid_for_s,
            "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2",
            MARKER,
        ),
        Some(refresh),
        "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2",
    )
}

pub fn ticket() -> lastwerk_garmin::Ticket {
    lastwerk_garmin::ticket_from_redirect(&format!(
        "https://connect.garmin.com/app?ticket=ST-{MARKER}-cas"
    ))
    .unwrap()
}

// Attrappen-Server

#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    /// Pfad samt Query, wie gesendet.
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap()
    }

    pub fn query(&self) -> &str {
        self.target.split_once('?').map_or("", |(_, q)| q)
    }

    /// Wert eines Formularfelds im Rumpf (nur für unkodierte Werte der Tests).
    pub fn form(&self, key: &str) -> Option<String> {
        self.body.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == key).then(|| percent_decode(v))
        })
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
                out.push(u8::from_str_radix(hex, 16).unwrap());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap()
}

#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Verbindung offen lassen, ohne zu antworten (für Zeitlimits).
    pub hang_for: Option<Duration>,
}

impl Reply {
    pub fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            hang_for: None,
        }
    }

    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        let mut reply = Self::status(status);
        reply
            .headers
            .push(("Content-Type".into(), "application/json".into()));
        reply.body = serde_json::to_vec(value).unwrap();
        reply
    }

    pub fn bytes(status: u16, body: Vec<u8>) -> Self {
        let mut reply = Self::status(status);
        reply.body = body;
        reply
    }

    pub fn hang(duration: Duration) -> Self {
        let mut reply = Self::status(200);
        reply.hang_for = Some(duration);
        reply
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

type Handler = dyn Fn(&Seen) -> Reply + Send + Sync;

pub struct MockServer {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MockServer {
    pub fn start(handler: impl Fn(&Seen) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handler: Arc<Handler> = Arc::new(handler);
        let thread = {
            let (seen, stop) = (Arc::clone(&seen), Arc::clone(&stop));
            thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let (seen, handler) = (Arc::clone(&seen), Arc::clone(&handler));
                    thread::spawn(move || serve(stream, &seen, &*handler));
                }
            })
        };
        Self {
            addr,
            seen,
            stop,
            thread: Some(thread),
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn endpoints(&self) -> Endpoints {
        Endpoints::for_tests(&self.base_url())
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }

    pub fn count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Weckt die Annahmeschleife auf, damit der Thread endet.
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(mut stream: TcpStream, seen: &Mutex<Vec<Seen>>, handler: &Handler) {
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let Ok(n) = stream.read(&mut chunk) else {
            return;
        };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default().to_owned();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let headers: Vec<(String, String)> = lines
        .filter(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    while buf.len() < head_end + length {
        let Ok(n) = stream.read(&mut chunk) else {
            return;
        };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let request = Seen {
        method,
        target,
        headers,
        body: String::from_utf8_lossy(&buf[head_end..head_end + length]).into_owned(),
    };
    seen.lock().unwrap().push(request.clone());
    let reply = handler(&request);
    if let Some(hang) = reply.hang_for {
        thread::sleep(hang);
        return;
    }
    let mut out = format!("HTTP/1.1 {} X\r\nConnection: close\r\n", reply.status).into_bytes();
    for (k, v) in &reply.headers {
        out.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
    }
    out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", reply.body.len()).as_bytes());
    out.extend_from_slice(&reply.body);
    let _ = stream.write_all(&out);
    let _ = stream.flush();
}

/// Adresse, auf der garantiert niemand lauscht (Port nach dem Schließen wieder frei).
pub fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    url
}

// Prüfstand

static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "{name}-{}-{}",
        std::process::id(),
        DIR_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub struct Rig {
    pub server: MockServer,
    pub store: Arc<MemoryStore>,
    pub clock: Arc<FakeClock>,
    pub client: GarminClient,
    pub lockout_file: PathBuf,
}

impl Rig {
    pub fn new(handler: impl Fn(&Seen) -> Reply + Send + Sync + 'static) -> Self {
        Self::with(handler, None, ClientConfig::default())
    }

    pub fn connected(
        handler: impl Fn(&Seen) -> Reply + Send + Sync + 'static,
        tokens: TokenSet,
    ) -> Self {
        Self::with(handler, Some(tokens), ClientConfig::default())
    }

    pub fn with(
        handler: impl Fn(&Seen) -> Reply + Send + Sync + 'static,
        tokens: Option<TokenSet>,
        config: ClientConfig,
    ) -> Self {
        let server = MockServer::start(handler);
        let store = Arc::new(tokens.map_or_else(MemoryStore::new, MemoryStore::with_tokens));
        let clock = FakeClock::new();
        let lockout_file = scratch_dir("lockout").join("lockout.json");
        let client = build_client(&server, &store, &clock, &lockout_file, config);
        Self {
            server,
            store,
            clock,
            client,
            lockout_file,
        }
    }

    /// Neuer Client auf derselben Ablage, Uhr und Sperrdatei: simuliert einen Neustart der App.
    pub fn restart(&self) -> GarminClient {
        build_client(
            &self.server,
            &self.store,
            &self.clock,
            &self.lockout_file,
            ClientConfig::default(),
        )
    }

    pub fn gets(&self) -> Vec<Seen> {
        self.server
            .seen()
            .into_iter()
            .filter(|s| s.method == "GET")
            .collect()
    }

    pub fn posts(&self) -> Vec<Seen> {
        self.server
            .seen()
            .into_iter()
            .filter(|s| s.method == "POST")
            .collect()
    }
}

fn build_client(
    server: &MockServer,
    store: &Arc<MemoryStore>,
    clock: &Arc<FakeClock>,
    lockout_file: &PathBuf,
    config: ClientConfig,
) -> GarminClient {
    GarminClient::builder(server.endpoints(), store.clone(), lockout_file)
        .clock(clock.clone())
        .jitter(Arc::new(FixedJitter(0.0)))
        .throttle_config(ThrottleConfig::default())
        .client_config(config)
        .build()
}
