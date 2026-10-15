//! Einrichtung des lokalen Coachs (T29) gegen einen Attrappen-Ollama auf Loopback: Status mit
//! Hardware und Katalog, Laden eines Modells mit Fortschritt, Positivliste, Sperre, Abbruch.
//! Keine echten Daten, kein Zugriff auf das Netz, kein Prozess außer dem Test selbst.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use lastwerk_app::service::coach::{self, KEY_BASE_URL, KEY_MODEL, KEY_PROVIDER};
use lastwerk_app::service::coach_setup::{
    self, CoachSetupState, Environment, JobKind, PullEvent, STATE_FILE,
};
use lastwerk_coach::{Fit, Hardware, PullPhase};
use lastwerk_store::Store;

fn work_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("app-coach-setup-tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn store(name: &str) -> (Store, PathBuf) {
    let dir = work_dir(name);
    (Store::open(&dir.join("lastwerk.db")).unwrap(), dir)
}

fn env(dir: &std::path::Path) -> Environment {
    Environment {
        state_path: dir.join(STATE_FILE),
        exe_present: false,
        script_present: true,
    }
}

fn setup_with(hw: Hardware) -> CoachSetupState {
    let s = CoachSetupState::default();
    s.hardware_with(|| hw);
    s
}

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
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Ein Attrappen-Ollama: `GET /api/tags` liefert `models`, `POST /api/pull` den Strom `lines`
/// (chunked). Gemerkt werden die Anfragezeilen. Läuft, bis der Test endet.
struct FakeOllama {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FakeOllama {
    fn start(models: &'static [&'static str], lines: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let request = read_request(&mut stream);
                let first = request.lines().next().unwrap_or("").to_owned();
                seen.lock().unwrap().push(first.clone());
                if first.starts_with("GET /api/tags") {
                    let body = serde_json::json!({
                        "models": models.iter().map(|m| serde_json::json!({"name": m})).collect::<Vec<_>>()
                    })
                    .to_string();
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                } else if first.starts_with("POST /api/pull") {
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                    );
                    for line in &lines {
                        let data = format!("{line}\n");
                        if write!(stream, "{:x}\r\n{data}\r\n", data.len()).is_err() {
                            return;
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                    let _ = stream.write_all(b"0\r\n\r\n");
                } else {
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
            }
        });
        Self { url, requests }
    }

    fn seen(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

fn success_stream() -> Vec<String> {
    [
        r#"{"status":"pulling manifest"}"#,
        r#"{"status":"pulling aaa","digest":"aaa","total":4000,"completed":1000}"#,
        r#"{"status":"pulling aaa","digest":"aaa","total":4000,"completed":4000}"#,
        r#"{"status":"verifying sha256 digest"}"#,
        r#"{"status":"writing manifest"}"#,
        r#"{"status":"success"}"#,
    ]
    .map(String::from)
    .to_vec()
}

fn hw(ram: f64, vram: Option<f64>) -> Hardware {
    Hardware {
        ram_gb: ram,
        vram_gb: vram,
    }
}

#[test]
fn status_without_a_server_says_missing_and_still_grades_the_catalog() {
    let (mut store, dir) = store("status-missing");
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    store
        .set_setting(KEY_BASE_URL, &format!("http://127.0.0.1:{port}"))
        .unwrap();
    let setup = setup_with(hw(16.0, None));
    let s = coach_setup::status(&store, &setup, &env(&dir)).unwrap();
    assert_eq!(s.ollama, "missing");
    assert!(s.models.is_empty() && s.model.is_none());
    assert!(s.busy.is_none() && s.state.is_none());
    assert!(s.can_install == cfg!(windows));
    let fits: Vec<(&str, Fit)> = s.catalog.iter().map(|e| (e.name.as_str(), e.fit)).collect();
    assert_eq!(
        fits,
        [
            ("qwen3:1.7b", Fit::Fits),
            ("qwen3:4b", Fit::Fits),
            ("qwen3:8b", Fit::Slow),
            ("qwen3:14b", Fit::TooBig),
        ]
    );
    assert_eq!(s.recommended.as_deref(), Some("qwen3:4b"));
}

#[test]
fn status_with_a_server_lists_models_and_reads_the_state_file() {
    let (store0, dir) = store("status-running");
    let mut store = store0;
    let fake = FakeOllama::start(&["qwen3:4b", "llama3.2:3b"], vec![]);
    store.set_setting(KEY_BASE_URL, &fake.url).unwrap();
    store.set_setting(KEY_MODEL, "llama3.2:3b").unwrap();
    std::fs::write(
        dir.join(STATE_FILE),
        r#"{"schema_version":1,"decision":"yes","ollama_found":true,"installed_via":"existing",
            "model":"qwen3:4b","model_ready":true,"error_code":null,"updated_at":"2026-10-15T08:00:00Z"}"#,
    )
    .unwrap();
    let setup = setup_with(hw(32.0, Some(12.0)));
    let s = coach_setup::status(&store, &setup, &env(&dir)).unwrap();
    assert_eq!(s.ollama, "running");
    assert_eq!(s.models, ["qwen3:4b", "llama3.2:3b"]);
    assert_eq!(s.model.as_deref(), Some("llama3.2:3b"));
    assert_eq!(s.recommended.as_deref(), Some("qwen3:14b"));
    let installed: Vec<&str> = s
        .catalog
        .iter()
        .filter(|e| e.installed)
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(installed, ["qwen3:4b"]);
    let state = s.state.unwrap();
    assert!(state.model_ready && state.ollama_found);
    assert_eq!(state.decision.as_deref(), Some("yes"));
    assert!(fake.seen().iter().any(|l| l.starts_with("GET /api/tags")));
    // Kein Pfad im Status.
    let json =
        serde_json::to_string(&coach_setup::status(&store, &setup, &env(&dir)).unwrap()).unwrap();
    assert!(!json.contains(dir.to_str().unwrap()));
}

#[test]
fn a_foreign_provider_does_not_move_the_setup_to_its_address() {
    let (mut store, dir) = store("status-openai");
    store
        .set_setting(KEY_PROVIDER, "openai_compatible")
        .unwrap();
    // Die Adresse gehört dem anderen Anbieter: Die Einrichtung fragt trotzdem Ollama (Standard).
    store
        .set_setting(KEY_BASE_URL, "http://127.0.0.1:1")
        .unwrap();
    let setup = setup_with(hw(8.0, None));
    let s = coach_setup::status(&store, &setup, &env(&dir)).unwrap();
    assert!(["running", "missing", "installed"].contains(&s.ollama));
}

#[test]
fn pulling_a_catalog_model_reports_progress_and_sets_the_coach_model() {
    let (mut store, _dir) = store("pull-ok");
    let fake = FakeOllama::start(&[], success_stream());
    store.set_setting(KEY_BASE_URL, &fake.url).unwrap();
    let setup = setup_with(hw(16.0, None));
    let guard = setup.begin(JobKind::Pull, Some("qwen3:4b".into())).unwrap();
    let mut events: Vec<PullEvent> = Vec::new();
    coach_setup::pull(&mut store, &setup, &guard, "qwen3:4b", &mut |e| {
        events.push(e)
    })
    .unwrap();

    assert_eq!(
        store.get_setting(KEY_MODEL).unwrap().as_deref(),
        Some("qwen3:4b")
    );
    // Alle Ereignisse tragen den Namen; das erste ist das Manifest, das letzte das Ende mit
    // vollem Fortschritt (die Ausdünnung lässt Phasenwechsel und das Ende immer durch).
    assert!(events.iter().all(|e| e.name == "qwen3:4b"));
    assert_eq!(events.first().unwrap().phase, PullPhase::Manifest);
    let last = events.last().unwrap();
    assert_eq!(
        (last.phase, last.completed, last.total),
        (PullPhase::Done, 4000, 4000)
    );
    let phases: Vec<PullPhase> = events.iter().map(|e| e.phase).collect();
    for p in [PullPhase::Verify, PullPhase::Write] {
        assert!(phases.contains(&p), "{p:?}");
    }
    // Während des Auftrags meldet der Status ihn als laufend.
    assert_eq!(setup.busy().unwrap().kind, JobKind::Pull);
    drop(guard);
    assert!(setup.busy().is_none());
    // Das Gespräch nutzt das Modell nach dem Laden, wenn der Server es hat. Hier sagt die
    // Attrappe `models: []`, also fällt der Status auf "kein Modell".
    let status = coach::status(&store).unwrap();
    assert_eq!(status.reason.as_deref(), Some("no_model"));
}

#[test]
fn a_model_outside_the_catalog_is_only_allowed_if_installed() {
    let (mut store, _dir) = store("pull-allowlist");
    let fake = FakeOllama::start(&["llama3.2:3b"], success_stream());
    store.set_setting(KEY_BASE_URL, &fake.url).unwrap();
    let setup = setup_with(hw(16.0, None));
    let guard = setup.begin(JobKind::Pull, None).unwrap();

    for bad in [
        "evil:latest",
        "qwen3:32b",
        "qwen3:4b; calc",
        "",
        "../x",
        "Qwen3:4B",
    ] {
        let err = coach_setup::pull(&mut store, &setup, &guard, bad, &mut |_| {}).unwrap_err();
        assert_eq!(err.code, "coach_pull_invalid_model", "{bad:?}");
    }
    assert_eq!(store.get_setting(KEY_MODEL).unwrap(), None);
    // Kein Aufruf von /api/pull für abgelehnte Namen.
    assert!(!fake.seen().iter().any(|l| l.starts_with("POST /api/pull")));

    // Ein installiertes Modell darf noch einmal geladen werden (Aktualisierung).
    coach_setup::pull(&mut store, &setup, &guard, "llama3.2:3b", &mut |_| {}).unwrap();
    assert_eq!(
        store.get_setting(KEY_MODEL).unwrap().as_deref(),
        Some("llama3.2:3b")
    );
    assert!(fake.seen().iter().any(|l| l.starts_with("POST /api/pull")));
}

#[test]
fn a_failed_pull_gives_a_code_and_keeps_the_model_setting() {
    let (mut store, _dir) = store("pull-error");
    store.set_setting(KEY_MODEL, "alt:1b").unwrap();
    let fake = FakeOllama::start(
        &[],
        vec![
            r#"{"status":"pulling manifest"}"#.to_string(),
            r#"{"error":"write /home/geheim/x: no space left on device"}"#.to_string(),
        ],
    );
    store.set_setting(KEY_BASE_URL, &fake.url).unwrap();
    let setup = setup_with(hw(16.0, None));
    let guard = setup.begin(JobKind::Pull, None).unwrap();
    let err = coach_setup::pull(&mut store, &setup, &guard, "qwen3:4b", &mut |_| {}).unwrap_err();
    assert_eq!(err.code, "coach_pull_disk_full");
    assert!(!format!("{err:?}").contains("geheim"));
    assert_eq!(
        store.get_setting(KEY_MODEL).unwrap().as_deref(),
        Some("alt:1b")
    );
}

#[test]
fn the_pull_does_not_change_the_model_of_another_provider() {
    let (mut store, _dir) = store("pull-openai");
    store
        .set_setting(KEY_PROVIDER, "openai_compatible")
        .unwrap();
    store.set_setting(KEY_MODEL, "lokal").unwrap();
    // Standardadresse von Ollama: Nichts läuft dort, also ein Fehler; entscheidend ist, dass
    // das Modell des anderen Anbieters unberührt bleibt.
    let setup = setup_with(hw(16.0, None));
    let guard = setup.begin(JobKind::Pull, None).unwrap();
    let _ = coach_setup::pull(&mut store, &setup, &guard, "qwen3:4b", &mut |_| {});
    assert_eq!(
        store.get_setting(KEY_MODEL).unwrap().as_deref(),
        Some("lokal")
    );
}

#[test]
fn a_second_job_is_busy_and_cancel_stops_a_running_pull() {
    let (mut store, _dir) = store("pull-cancel");
    // Ein Strom, der weiterläuft, bis der Test endet.
    let lines: Vec<String> = std::iter::repeat_n(
        r#"{"status":"pulling aaa","digest":"aaa","total":1000000,"completed":10}"#.to_string(),
        400,
    )
    .collect();
    let fake = FakeOllama::start(&[], lines);
    store.set_setting(KEY_BASE_URL, &fake.url).unwrap();
    let setup = setup_with(hw(16.0, None));
    let guard = setup.begin(JobKind::Pull, Some("qwen3:4b".into())).unwrap();

    assert_eq!(
        setup.begin(JobKind::Install, None).unwrap_err().code,
        "coach_setup_busy"
    );
    assert_eq!(
        setup
            .begin(JobKind::Pull, Some("qwen3:8b".into()))
            .unwrap_err()
            .code,
        "coach_setup_busy"
    );

    let canceller = setup.clone();
    let started = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&started);
    let err = thread::scope(|s| {
        s.spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(5));
            }
            canceller.cancel_pull();
        });
        coach_setup::pull(&mut store, &setup, &guard, "qwen3:4b", &mut |_| {
            started.store(true, Ordering::SeqCst);
        })
        .unwrap_err()
    });
    assert_eq!(err.code, "coach_pull_cancelled");
    assert_eq!(store.get_setting(KEY_MODEL).unwrap(), None);
}
