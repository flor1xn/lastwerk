//! Einrichtung des lokalen Coachs: Ollama erkennen, installieren, Modell laden (T29, SPEC 9.2).
//!
//! Die Schicht verbindet `lastwerk-coach` (Katalog, Empfehlung, Laden) mit dem System: Arbeits-
//! und Grafikspeicher messen, das gebündelte Skript `ollama-setup.ps1` (Teil A) unter Windows
//! starten und seine Zeilen lesen, die Zustandsdatei des Skripts lesen. Alles, was sich ohne
//! Windows prüfen lässt, ist eine reine Funktion mit Test: der Zeilenparser, der Leser der
//! Zustandsdatei, die Positivliste der Modellnamen, die Ausdünnung der Fortschrittsereignisse.
//!
//! Was die Oberfläche nie bekommt: einen Pfad (Skript, Zustandsdatei, Ollama), den Fremdtext einer
//! Fehlermeldung von Ollama, PowerShell oder winget. Fehler gehen als Codes (`ui.error.*`).
//!
//! Die Verbindung zu Ollama läuft nur im Rust-Teil, nur an die Loopback-Adresse (`BaseUrl`).

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, SecondsFormat, Utc};
use lastwerk_coach::catalog::{CatalogModel, embedded_catalog, is_valid_model_name, starter};
use lastwerk_coach::{
    BaseUrl, Fit, Hardware, OLLAMA_DEFAULT_URL, PullError, PullPhase, PullProgress, ollama_models,
    pick_model, pull_model, recommend,
};
use lastwerk_store::Store;
use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, ERROR_CODES, Result};
use crate::service::coach::{KEY_MODEL, STATUS_TIMEOUT_S, read_config};

/// Name der Zustandsdatei neben der Datenbank (Vertrag mit `ollama-setup.ps1`, T29 Teil A).
pub const STATE_FILE: &str = "ollama-setup.json";

/// Pfad des Skripts unter den Ressourcen der App (Vertrag, T29 Teil A).
pub const SCRIPT_RESOURCE: &str = "resources/ollama-setup.ps1";

/// Längste Laufzeit des Skripts in Sekunden (Faustregel): winget oder Download der rund 1 GB
/// großen Installation, Start des Dienstes (bis 60 s) und eine langsame Leitung. Danach wird
/// der Prozess beendet.
pub const INSTALL_TIMEOUT_S: u64 = 30 * 60;

/// Zeitlimit für `nvidia-smi` in Sekunden (Faustregel): Das Programm antwortet in unter einer
/// Sekunde; wer länger braucht, hat einen hängenden Treiber, und die Einrichtung soll nicht
/// warten.
const NVIDIA_SMI_TIMEOUT_S: u64 = 3;

/// Größte gelesene Zustandsdatei in Bytes (Konvention): Sie hat unter 500 Byte.
const MAX_STATE_BYTES: u64 = 16 * 1024;

/// Fehlercodes des Skripts (Vertrag, T29 Teil A).
const SCRIPT_ERROR_CODES: [&str; 8] = [
    "no_windows",
    "winget_failed",
    "download_failed",
    "signature_invalid",
    "install_failed",
    "start_timeout",
    "pull_failed",
    "bad_args",
];

// --- Zeilen des Skripts ------------------------------------------------------------------

/// Eine Zeile `LASTWERK:<phase>:<wert>` des Skripts. Phase und Wert stammen aus festen Listen,
/// fremder Text gelangt nicht in die Oberfläche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetupProgress {
    /// `detect`, `install`, `start`, `pull`, `done` oder `error`.
    pub phase: &'static str,
    /// Fester Wert der Phase; bei `pull` mit Prozentzahl `progress`, bei `error` der Code oder
    /// `unknown`.
    pub value: String,
    /// Nur bei `pull` mit Prozentzahl.
    pub percent: Option<u8>,
}

/// Liest eine Zeile der Standardausgabe des Skripts. Alles, was nicht dem Vertrag entspricht,
/// ist `None` und wird ignoriert (Vertrag: "Alles andere auf der Standardausgabe ignoriert die
/// App").
pub fn parse_setup_line(line: &str) -> Option<SetupProgress> {
    // Windows PowerShell schreibt mitunter eine Bytefolge-Marke und Wagenrücklauf.
    let line = line.trim_start_matches('\u{feff}').trim();
    if line.len() > 200 {
        return None;
    }
    let rest = line.strip_prefix("LASTWERK:")?;
    let (phase, value) = rest.split_once(':')?;
    let value = value.trim();
    let fixed = |phase: &'static str, allowed: &[&str]| -> Option<SetupProgress> {
        allowed.contains(&value).then(|| SetupProgress {
            phase,
            value: value.to_owned(),
            percent: None,
        })
    };
    match phase {
        "detect" => fixed("detect", &["found", "missing", "running"]),
        "install" => fixed("install", &["winget", "download", "verify_ok", "skipped"]),
        "start" => fixed("start", &["ok", "timeout"]),
        "done" => fixed("done", &["ok"]),
        "pull" => {
            if let Some(p) = fixed("pull", &["start", "done", "skipped"]) {
                return Some(p);
            }
            let digits = value.strip_suffix('%').unwrap_or(value);
            let n: u8 = digits.parse().ok().filter(|n| *n <= 100)?;
            Some(SetupProgress {
                phase: "pull",
                value: "progress".to_owned(),
                percent: Some(n),
            })
        }
        "error" => Some(SetupProgress {
            phase: "error",
            value: if SCRIPT_ERROR_CODES.contains(&value) {
                value.to_owned()
            } else {
                "unknown".to_owned()
            },
            percent: None,
        }),
        _ => None,
    }
}

/// Der Fehlercode des Skripts als Fehler der App (`coach_setup_<code>`). Unbekanntes ist
/// `coach_setup_failed`.
fn setup_error(code: &str) -> AppError {
    let full = format!("coach_setup_{code}");
    if ERROR_CODES.contains(&full.as_str()) {
        AppError::new(&full)
    } else {
        AppError::new("coach_setup_failed")
    }
}

/// Entfernt das Präfix `\\?\` eines Windows-Pfads (verbatim), den manche Auflösung liefert. Die
/// Windows PowerShell kommt mit ihm bei `-File` nicht zurecht. `\\?\UNC\` und andere Formen
/// bleiben unverändert.
pub fn strip_verbatim_prefix(path: &str) -> &str {
    match path.strip_prefix(r"\\?\") {
        Some(rest)
            if rest.len() >= 2
                && rest.as_bytes()[0].is_ascii_alphabetic()
                && rest.as_bytes()[1] == b':' =>
        {
            rest
        }
        _ => path,
    }
}

/// Unter Windows ohne sichtbares Konsolenfenster starten (`CREATE_NO_WINDOW`).
#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_window(_cmd: &mut Command) {}

/// Befehl für das Skript: `powershell -NoProfile -ExecutionPolicy Bypass -File <skript> -Mode
/// app -StatePath <datei>`, ohne Fenster. Ohne `-Model`: Das Modell lädt `coach_pull_model` mit
/// Fortschritt, das Skript richtet nur Ollama ein (Vertrag). `Bypass` gilt nur für diesen
/// Prozess und für das mitgelieferte Skript (SPEC 10.2).
pub fn script_command(script: &Path, state_path: &Path) -> Command {
    let script = script.to_string_lossy();
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(strip_verbatim_prefix(&script))
        .args(["-Mode", "app", "-StatePath"])
        .arg(state_path);
    hide_window(&mut cmd);
    cmd
}

/// Startet `cmd`, liest die Zeilen der Standardausgabe, meldet jede erkannte Zeile an `emit` und
/// wartet auf das Ende, höchstens `timeout`. Fehler als Code des Skripts (`winget_failed` und
/// so weiter) oder `failed`, `timeout`.
pub fn run_script(
    mut cmd: Command,
    timeout: Duration,
    emit: &mut dyn FnMut(SetupProgress),
) -> std::result::Result<(), String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(|_| "failed".to_owned())?;
    let stdout = child.stdout.take().ok_or_else(|| "failed".to_owned())?;
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        // Höchstens 8 MiB insgesamt (Konvention): Das Skript schreibt wenige Zeilen.
        let reader = BufReader::new(stdout.take(8 * 1024 * 1024));
        for line in reader.split(b'\n').map_while(std::result::Result::ok) {
            if tx
                .send(String::from_utf8_lossy(&line).into_owned())
                .is_err()
            {
                break;
            }
        }
    });
    let deadline = Instant::now() + timeout;
    let mut last_error: Option<String> = None;
    let mut timed_out = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                if let Some(p) = parse_setup_line(&line) {
                    if p.phase == "error" {
                        last_error = Some(p.value.clone());
                    }
                    emit(p);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = child.wait().map_err(|_| "failed".to_owned())?;
    if timed_out {
        return Err("timeout".to_owned());
    }
    if status.success() {
        return Ok(());
    }
    // Exit-Code 2: falsche Parameter (Vertrag); sonst die letzte Fehlerzeile.
    Err(match (last_error, status.code()) {
        (Some(code), _) if code != "unknown" => code,
        (_, Some(2)) => "bad_args".to_owned(),
        _ => "failed".to_owned(),
    })
}

// --- Zustandsdatei des Skripts -------------------------------------------------------

/// Inhalt der Zustandsdatei (Vertrag, T29 Teil A) in der Form für die Oberfläche: nur geprüfte
/// Felder, kein Pfad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetupFileState {
    /// `yes`, `no` oder `null` (nicht entschieden).
    pub decision: Option<String>,
    pub ollama_found: bool,
    /// `existing`, `winget` oder `download`.
    pub installed_via: Option<String>,
    pub model: Option<String>,
    pub model_ready: bool,
    /// Code des Skripts oder `unknown`.
    pub error_code: Option<String>,
    /// ISO 8601 UTC.
    pub updated_at: Option<String>,
}

/// Liest den Text der Zustandsdatei vorsichtig: Was fehlt, ist falsch typisiert oder unbekannt,
/// wird zu "nicht gesetzt". Ohne Objekt oder mit anderer `schema_version` als 1: `None`.
pub fn parse_state_file(text: &str) -> Option<SetupFileState> {
    let v: Value = serde_json::from_str(text).ok()?;
    let obj = v.as_object()?;
    if obj.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let text_in = |key: &str, allowed: &[&str]| -> Option<String> {
        let s = obj.get(key)?.as_str()?;
        allowed.contains(&s).then(|| s.to_owned())
    };
    let flag = |key: &str| obj.get(key).and_then(Value::as_bool).unwrap_or(false);
    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .filter(|m| is_valid_model_name(m))
        .map(str::to_owned);
    let error_code = obj.get("error_code").and_then(Value::as_str).map(|c| {
        if SCRIPT_ERROR_CODES.contains(&c) {
            c.to_owned()
        } else {
            "unknown".to_owned()
        }
    });
    let updated_at = obj
        .get("updated_at")
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 40)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| {
            d.with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        });
    Some(SetupFileState {
        decision: text_in("decision", &["yes", "no"]),
        ollama_found: flag("ollama_found"),
        installed_via: text_in("installed_via", &["existing", "winget", "download"]),
        model,
        model_ready: flag("model_ready"),
        error_code,
        updated_at,
    })
}

/// Liest die Zustandsdatei. Fehlt sie oder ist sie unlesbar, gibt es `None`; die Einrichtung
/// funktioniert auch ohne sie.
pub fn read_state_file(path: &Path) -> Option<SetupFileState> {
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    file.take(MAX_STATE_BYTES).read_to_string(&mut text).ok()?;
    parse_state_file(&text)
}

// --- Hardware --------------------------------------------------------------------------

/// Größter Wert aus der Ausgabe von `nvidia-smi --query-gpu=memory.total
/// --format=csv,noheader,nounits` (eine Zeile je Karte, Mebibyte) in GB (Dezimal). `None`, wenn
/// keine Zeile eine Zahl ist.
pub fn parse_nvidia_smi(output: &str) -> Option<f64> {
    output
        .lines()
        .filter_map(|l| l.trim().parse::<u64>().ok())
        .max()
        .filter(|mib| *mib > 0)
        .map(|mib| mib as f64 * 1_048_576.0 / 1e9)
}

/// Führt `cmd` aus und liefert die Standardausgabe, wenn es innerhalb von `timeout` mit Erfolg
/// endet.
fn output_within(mut cmd: Command, timeout: Duration) -> Option<String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_window(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut text = String::new();
        let _ = (&mut stdout).take(64 * 1024).read_to_string(&mut text);
        let _ = tx.send(text);
    });
    match rx.recv_timeout(timeout) {
        Ok(text) => child
            .wait()
            .ok()
            .filter(std::process::ExitStatus::success)
            .map(|_| text),
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

/// Grafikspeicher der größten NVIDIA-Karte über `nvidia-smi`, `None` ohne Programm, ohne Karte
/// oder bei Zeitüberschreitung. Andere Hersteller werden nicht erkannt (Faustregel: Ollama
/// beschleunigt unter Windows vor allem mit NVIDIA; AMD und Intel gelten als "keine Karte",
/// die Empfehlung fällt dann vorsichtig aus).
fn detect_vram_gb() -> Option<f64> {
    let mut cmd = Command::new("nvidia-smi");
    cmd.args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"]);
    parse_nvidia_smi(&output_within(
        cmd,
        Duration::from_secs(NVIDIA_SMI_TIMEOUT_S),
    )?)
}

/// Gesamter Arbeitsspeicher in GB (Dezimal) über `sysinfo`.
fn detect_ram_gb() -> f64 {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    system.total_memory() as f64 / 1e9
}

/// Misst die Hardware, auf eine Nachkommastelle gerundet.
pub fn detect_hardware() -> Hardware {
    let round1 = |v: f64| (v * 10.0).round() / 10.0;
    Hardware {
        ram_gb: round1(detect_ram_gb()),
        vram_gb: detect_vram_gb().map(round1),
    }
}

// --- Zustand der App ---------------------------------------------------------------------

/// Art des laufenden Auftrags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Install,
    Pull,
}

/// Was gerade läuft, für `CoachSetupStatus::busy`. Damit findet die Oberfläche einen laufenden
/// Auftrag nach einem Wechsel der Ansicht oder einem Neuladen wieder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobInfo {
    pub kind: JobKind,
    /// Name des Modells bei `pull`.
    pub name: Option<String>,
    pub phase: Option<String>,
    pub completed: u64,
    pub total: u64,
}

#[derive(Debug, Default)]
struct Shared {
    job: Mutex<Option<JobInfo>>,
    cancel: AtomicBool,
    hardware: Mutex<Option<Hardware>>,
}

impl Shared {
    fn job(&self) -> MutexGuard<'_, Option<JobInfo>> {
        self.job.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Zustand der Einrichtung, billig zu kopieren (für `spawn_blocking`).
#[derive(Debug, Clone, Default)]
pub struct CoachSetupState(Arc<Shared>);

/// Hält die Sperre für einen laufenden Auftrag und gibt sie beim Verlassen frei, auch nach
/// einer Panik.
#[derive(Debug)]
pub struct JobGuard(Arc<Shared>);

impl Drop for JobGuard {
    fn drop(&mut self) {
        *self.0.job() = None;
    }
}

impl JobGuard {
    fn set(&self, f: impl FnOnce(&mut JobInfo)) {
        if let Some(job) = self.0.job().as_mut() {
            f(job);
        }
    }
}

impl CoachSetupState {
    /// Beginnt einen Auftrag. Läuft schon einer (Installation oder Laden), ist das
    /// `coach_setup_busy`.
    pub fn begin(&self, kind: JobKind, name: Option<String>) -> Result<JobGuard> {
        let mut job = self.0.job();
        if job.is_some() {
            return Err(AppError::new("coach_setup_busy"));
        }
        *job = Some(JobInfo {
            kind,
            name,
            phase: None,
            completed: 0,
            total: 0,
        });
        self.0.cancel.store(false, Ordering::SeqCst);
        Ok(JobGuard(Arc::clone(&self.0)))
    }

    /// Bricht ein laufendes Laden ab. Ohne laufendes Laden passiert nichts.
    pub fn cancel_pull(&self) {
        if self
            .0
            .job()
            .as_ref()
            .is_some_and(|j| j.kind == JobKind::Pull)
        {
            self.0.cancel.store(true, Ordering::SeqCst);
        }
    }

    pub fn busy(&self) -> Option<JobInfo> {
        self.0.job().clone()
    }

    /// Die Hardware, beim ersten Aufruf gemessen und danach gemerkt (`nvidia-smi` startet einen
    /// Prozess). `measure` ersetzt die Messung in Tests.
    pub fn hardware_with(&self, measure: impl FnOnce() -> Hardware) -> Hardware {
        let mut slot = self
            .0
            .hardware
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *slot.get_or_insert_with(measure)
    }

    pub fn hardware(&self) -> Hardware {
        self.hardware_with(detect_hardware)
    }
}

// --- Fortschritt des Ladens ----------------------------------------------------------

/// Ereignis `coach-pull-progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PullEvent {
    pub name: String,
    /// `manifest`, `download`, `verify`, `write`, `done` oder `other`.
    pub phase: PullPhase,
    pub completed: u64,
    pub total: u64,
}

/// Dünnt die Ereignisse aus: Ollama meldet mehrmals je Sekunde, die Brücke zur Oberfläche und
/// die Anzeige brauchen höchstens vier je Sekunde (Konvention). Ein Wechsel der Phase und das
/// Ende gehen immer durch.
#[derive(Debug, Default)]
pub struct Throttle {
    last: Option<(Instant, PullPhase)>,
}

impl Throttle {
    const MIN_GAP: Duration = Duration::from_millis(250);

    pub fn allow(&mut self, now: Instant, phase: PullPhase) -> bool {
        let pass = match self.last {
            None => true,
            Some((at, last_phase)) => {
                phase != last_phase
                    || phase == PullPhase::Done
                    || now.saturating_duration_since(at) >= Self::MIN_GAP
            }
        };
        if pass {
            self.last = Some((now, phase));
        }
        pass
    }
}

// --- Modellnamen -----------------------------------------------------------------------

/// Ob `name` geladen werden darf: ein Name aus dem Katalog oder ein schon installierter, jeweils
/// im einfachen Muster. Die Positivliste verhindert, dass die Oberfläche beliebige Namen (und
/// damit beliebige Downloads) auslöst.
pub fn is_allowed_pull_name(name: &str, catalog: &[CatalogModel], installed: &[String]) -> bool {
    is_valid_model_name(name)
        && (catalog.iter().any(|m| m.name == name) || installed.iter().any(|m| m == name))
}

/// Fehler des Ladens als Fehler der App (`coach_pull_<code>`).
pub fn pull_error(e: &PullError) -> AppError {
    AppError::new(match e {
        PullError::InvalidModel => "coach_pull_invalid_model",
        PullError::Cancelled => "coach_pull_cancelled",
        PullError::Timeout => "coach_pull_timeout",
        PullError::Transport(_) => "coach_pull_unreachable",
        PullError::Status(_) | PullError::BadResponse(_) => "coach_pull_bad_response",
        PullError::ModelNotFound => "coach_pull_model_not_found",
        PullError::DiskFull => "coach_pull_disk_full",
        PullError::Network => "coach_pull_network",
        PullError::Failed => "coach_pull_failed",
    })
}

// --- Status ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HardwareDto {
    pub ram_gb: f64,
    pub vram_gb: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogEntryDto {
    pub name: String,
    pub size_gb: f64,
    pub label_key: String,
    pub starter: bool,
    /// `fits`, `slow` oder `too_big`.
    pub fit: Fit,
    /// Die Empfehlung für diesen Rechner.
    pub recommended: bool,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoachSetupStatus {
    /// `running` (antwortet), `installed` (gefunden, antwortet nicht) oder `missing`.
    pub ollama: &'static str,
    /// Das gebündelte Skript ist da und das System unterstützt es (Windows). Sonst zeigt die
    /// Oberfläche die manuelle Anleitung.
    pub can_install: bool,
    /// Modelle, die Ollama lokal hat (nur bei `running`).
    pub models: Vec<String>,
    /// Das Modell, mit dem das Gespräch liefe.
    pub model: Option<String>,
    pub hardware: HardwareDto,
    pub catalog: Vec<CatalogEntryDto>,
    /// Name der Empfehlung, `None` bei leerem Katalog.
    pub recommended: Option<String>,
    pub state: Option<SetupFileState>,
    pub busy: Option<JobInfo>,
}

/// Eingaben des Status, getrennt vom Messen (Tests).
pub struct StatusInputs<'a> {
    /// Modelle von Ollama, `None`, wenn der Server nicht antwortet.
    pub models: Option<Vec<String>>,
    /// Das gespeicherte Modell (`coach.model`).
    pub configured_model: Option<&'a str>,
    /// Die ausführbare Datei von Ollama liegt am üblichen Ort.
    pub exe_present: bool,
    pub can_install: bool,
    pub hardware: Hardware,
    pub state: Option<SetupFileState>,
    pub busy: Option<JobInfo>,
}

pub fn build_status(inputs: StatusInputs<'_>, catalog: &[CatalogModel]) -> CoachSetupStatus {
    let running = inputs.models.is_some();
    let models = inputs.models.unwrap_or_default();
    let recommendation = recommend(&inputs.hardware, catalog);
    let entries = catalog
        .iter()
        .zip(&recommendation.models)
        .map(|(m, f)| CatalogEntryDto {
            name: m.name.clone(),
            size_gb: m.size_gb,
            label_key: m.label_key.clone(),
            starter: m.starter,
            fit: f.fit,
            recommended: recommendation.recommended.as_deref() == Some(m.name.as_str()),
            installed: models.contains(&m.name),
        })
        .collect();
    let ollama = if running {
        "running"
    } else if inputs.exe_present || inputs.state.as_ref().is_some_and(|s| s.ollama_found) {
        "installed"
    } else {
        "missing"
    };
    // Wie `coach::status`: das gespeicherte Modell nur, wenn der Server es hat.
    let model = inputs
        .configured_model
        .filter(|m| models.iter().any(|x| x == m))
        .map(str::to_owned)
        .or_else(|| pick_model(&models));
    CoachSetupStatus {
        ollama,
        can_install: inputs.can_install,
        models,
        model,
        hardware: HardwareDto {
            ram_gb: inputs.hardware.ram_gb,
            vram_gb: inputs.hardware.vram_gb,
        },
        catalog: entries,
        recommended: recommendation.recommended,
        state: inputs.state,
        busy: inputs.busy,
    }
}

/// Basisadresse von Ollama: die gespeicherte Adresse, solange der Anbieter Ollama ist, sonst die
/// Standardadresse. Eine ungültige gespeicherte Adresse fällt auf die Standardadresse zurück.
fn ollama_base(store: &Store) -> Result<BaseUrl> {
    let cfg = read_config(store)?;
    let text = if cfg.openai_compatible {
        None
    } else {
        cfg.base_url
    };
    Ok(text
        .as_deref()
        .and_then(|t| BaseUrl::parse(t, false).ok())
        .unwrap_or_else(|| {
            BaseUrl::parse(OLLAMA_DEFAULT_URL, false).expect("die Standardadresse ist gültig")
        }))
}

/// Äußere Umstände des Status, die der Befehl in `lib.rs` kennt (Pfade, Skript).
pub struct Environment {
    pub state_path: PathBuf,
    pub exe_present: bool,
    pub script_present: bool,
}

/// Status der Einrichtung. Fragt Ollama kurz auf Loopback ab, misst beim ersten Aufruf die
/// Hardware.
pub fn status(
    store: &Store,
    setup: &CoachSetupState,
    env: &Environment,
) -> Result<CoachSetupStatus> {
    let base = ollama_base(store)?;
    let models = ollama_models(&base, STATUS_TIMEOUT_S).ok();
    let cfg = read_config(store)?;
    let catalog = embedded_catalog();
    Ok(build_status(
        StatusInputs {
            models,
            configured_model: cfg.model.as_deref().filter(|_| !cfg.openai_compatible),
            exe_present: env.exe_present,
            can_install: cfg!(windows) && env.script_present,
            hardware: setup.hardware(),
            state: read_state_file(&env.state_path),
            busy: setup.busy(),
        },
        &catalog,
    ))
}

/// Richtet Ollama über das gebündelte Skript ein (nur Windows, T29 Teil A). Der Aufrufer prüft
/// Plattform, Skript und Sperre vorher.
pub fn install_ollama(
    script: &Path,
    state_path: &Path,
    emit: &mut dyn FnMut(SetupProgress),
) -> Result<()> {
    run_script(
        script_command(script, state_path),
        Duration::from_secs(INSTALL_TIMEOUT_S),
        emit,
    )
    .map_err(|code| setup_error(&code))
}

/// Lädt ein Modell und merkt es danach als Modell des Coachs (`coach.model`), außer ein anderer
/// Anbieter als Ollama ist eingestellt. `guard` ist die Sperre des Auftrags. `emit` bekommt die
/// ausgedünnten Fortschrittsereignisse.
pub fn pull(
    store: &mut Store,
    setup: &CoachSetupState,
    guard: &JobGuard,
    name: &str,
    emit: &mut dyn FnMut(PullEvent),
) -> Result<()> {
    let base = ollama_base(store)?;
    let catalog = embedded_catalog();
    let mut installed = Vec::new();
    if !catalog.iter().any(|m| m.name == name) {
        // Nur für Namen außerhalb des Katalogs braucht die Positivliste den Server.
        installed = ollama_models(&base, STATUS_TIMEOUT_S).unwrap_or_default();
    }
    if !is_allowed_pull_name(name, &catalog, &installed) {
        return Err(AppError::new("coach_pull_invalid_model"));
    }
    let mut throttle = Throttle::default();
    let mut on_progress = |p: PullProgress| {
        guard.set(|job| {
            job.phase = Some(phase_name(p.phase).to_owned());
            job.completed = p.completed;
            job.total = p.total;
        });
        if throttle.allow(Instant::now(), p.phase) {
            emit(PullEvent {
                name: name.to_owned(),
                phase: p.phase,
                completed: p.completed,
                total: p.total,
            });
        }
    };
    pull_model(&base, name, &mut on_progress, &setup.0.cancel).map_err(|e| pull_error(&e))?;
    if !read_config(store)?.openai_compatible {
        store.set_setting(KEY_MODEL, name)?;
    }
    Ok(())
}

fn phase_name(phase: PullPhase) -> &'static str {
    match phase {
        PullPhase::Manifest => "manifest",
        PullPhase::Download => "download",
        PullPhase::Verify => "verify",
        PullPhase::Write => "write",
        PullPhase::Done => "done",
        PullPhase::Other => "other",
    }
}

/// Das Startmodell des Katalogs (für Tests und Hinweise).
pub fn starter_name() -> Option<String> {
    starter(&embedded_catalog()).map(|m| m.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> Option<(&'static str, String, Option<u8>)> {
        parse_setup_line(text).map(|p| (p.phase, p.value, p.percent))
    }

    #[test]
    fn contract_lines_are_parsed() {
        for (text, phase, value) in [
            ("LASTWERK:detect:found", "detect", "found"),
            ("LASTWERK:detect:missing", "detect", "missing"),
            ("LASTWERK:detect:running", "detect", "running"),
            ("LASTWERK:install:winget", "install", "winget"),
            ("LASTWERK:install:download", "install", "download"),
            ("LASTWERK:install:verify_ok", "install", "verify_ok"),
            ("LASTWERK:install:skipped", "install", "skipped"),
            ("LASTWERK:start:ok", "start", "ok"),
            ("LASTWERK:start:timeout", "start", "timeout"),
            ("LASTWERK:pull:start", "pull", "start"),
            ("LASTWERK:pull:done", "pull", "done"),
            ("LASTWERK:pull:skipped", "pull", "skipped"),
            ("LASTWERK:done:ok", "done", "ok"),
            ("LASTWERK:error:winget_failed", "error", "winget_failed"),
        ] {
            assert_eq!(line(text), Some((phase, value.to_owned(), None)), "{text}");
        }
        for code in SCRIPT_ERROR_CODES {
            let p = parse_setup_line(&format!("LASTWERK:error:{code}")).unwrap();
            assert_eq!(p.value, code);
        }
    }

    #[test]
    fn pull_percentages_become_numbers() {
        assert_eq!(
            line("LASTWERK:pull:0"),
            Some(("pull", "progress".into(), Some(0)))
        );
        assert_eq!(
            line("LASTWERK:pull:37"),
            Some(("pull", "progress".into(), Some(37)))
        );
        assert_eq!(
            line("LASTWERK:pull:100%"),
            Some(("pull", "progress".into(), Some(100)))
        );
        for bad in ["101", "-1", "3.5", "abc", "", "999999999999"] {
            assert_eq!(line(&format!("LASTWERK:pull:{bad}")), None, "{bad}");
        }
    }

    #[test]
    fn noise_and_unknown_values_are_ignored() {
        for text in [
            "",
            "Starting download of Ollama ...",
            "lastwerk:detect:found",
            "LASTWERK",
            "LASTWERK:",
            "LASTWERK:detect",
            "LASTWERK:detect:other",
            "LASTWERK:unknown:ok",
            "LASTWERK:done:fail",
            "  xLASTWERK:detect:found",
            "LASTWERK:install:C:\\Users\\x\\ollama.exe",
        ] {
            assert_eq!(line(text), None, "{text:?}");
        }
        // Zu lange Zeilen zählen nicht.
        assert_eq!(
            line(&format!("LASTWERK:detect:{}", "found".repeat(100))),
            None
        );
    }

    #[test]
    fn whitespace_carriage_returns_and_bom_are_tolerated() {
        assert!(parse_setup_line("LASTWERK:detect:found\r").is_some());
        assert!(parse_setup_line("\u{feff}LASTWERK:detect:found").is_some());
        assert!(parse_setup_line("LASTWERK:detect: found ").is_some());
    }

    #[test]
    fn an_unknown_error_code_is_not_passed_on() {
        let p = parse_setup_line("LASTWERK:error:C:\\Users\\geheim\\x").unwrap();
        assert_eq!(p.value, "unknown");
        let p = parse_setup_line("LASTWERK:error:disk exploded").unwrap();
        assert_eq!(p.value, "unknown");
    }

    #[test]
    fn script_errors_map_to_app_errors() {
        assert_eq!(
            setup_error("winget_failed").code,
            "coach_setup_winget_failed"
        );
        assert_eq!(
            setup_error("signature_invalid").code,
            "coach_setup_signature_invalid"
        );
        assert_eq!(setup_error("timeout").code, "coach_setup_timeout");
        assert_eq!(setup_error("unknown").code, "coach_setup_failed");
        assert_eq!(setup_error("etwas anderes").code, "coach_setup_failed");
        for code in SCRIPT_ERROR_CODES {
            assert_eq!(setup_error(code).code, format!("coach_setup_{code}"));
        }
    }

    #[test]
    fn the_verbatim_prefix_is_removed_only_for_drive_paths() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\C:\Program Files\Lastwerk\resources\ollama-setup.ps1"),
            r"C:\Program Files\Lastwerk\resources\ollama-setup.ps1"
        );
        assert_eq!(strip_verbatim_prefix(r"C:\x\y.ps1"), r"C:\x\y.ps1");
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\x"),
            r"\\?\UNC\server\x"
        );
        assert_eq!(
            strip_verbatim_prefix("/usr/share/x.ps1"),
            "/usr/share/x.ps1"
        );
        assert_eq!(strip_verbatim_prefix(r"\\?\"), r"\\?\");
    }

    #[test]
    fn the_script_command_has_the_contract_arguments() {
        let cmd = script_command(
            Path::new(r"\\?\C:\Apps\Lastwerk\resources\ollama-setup.ps1"),
            Path::new("state.json"),
        );
        assert_eq!(cmd.get_program(), "powershell");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                r"C:\Apps\Lastwerk\resources\ollama-setup.ps1",
                "-Mode",
                "app",
                "-StatePath",
                "state.json",
            ]
        );
        // Kein -Model: Das Modell lädt `coach_pull_model`.
        assert!(!args.iter().any(|a| a == "-Model"));
    }

    // --- Zustandsdatei -------------------------------------------------------------------

    const GOOD_STATE: &str = r#"{
        "schema_version": 1, "decision": "yes", "ollama_found": true,
        "installed_via": "winget", "model": "qwen3:4b", "model_ready": true,
        "error_code": null, "updated_at": "2026-10-15T08:30:00Z"
    }"#;

    #[test]
    fn a_good_state_file_is_read() {
        let s = parse_state_file(GOOD_STATE).unwrap();
        assert_eq!(s.decision.as_deref(), Some("yes"));
        assert!(s.ollama_found && s.model_ready);
        assert_eq!(s.installed_via.as_deref(), Some("winget"));
        assert_eq!(s.model.as_deref(), Some("qwen3:4b"));
        assert_eq!(s.error_code, None);
        assert_eq!(s.updated_at.as_deref(), Some("2026-10-15T08:30:00Z"));
    }

    #[test]
    fn a_state_file_with_offset_time_is_normalised_to_utc() {
        let text = GOOD_STATE.replace("2026-10-15T08:30:00Z", "2026-10-15T10:30:00+02:00");
        assert_eq!(
            parse_state_file(&text).unwrap().updated_at.as_deref(),
            Some("2026-10-15T08:30:00Z")
        );
    }

    #[test]
    fn a_bad_state_file_gives_nothing_or_safe_defaults() {
        for bad in [
            "",
            "kein json",
            "[]",
            "null",
            "{}",
            r#"{"schema_version": 2}"#,
            r#"{"schema_version": "1"}"#,
        ] {
            assert_eq!(parse_state_file(bad), None, "{bad:?}");
        }
        // Falsche Typen und unbekannte Werte werden zu "nicht gesetzt".
        let s = parse_state_file(
            r#"{"schema_version":1,"decision":"maybe","ollama_found":"ja","installed_via":"magic",
                "model":"C:\\Windows\\x; rm","model_ready":1,"error_code":"C:\\Users\\geheim",
                "updated_at":"gestern"}"#,
        )
        .unwrap();
        assert_eq!(
            s,
            SetupFileState {
                decision: None,
                ollama_found: false,
                installed_via: None,
                model: None,
                model_ready: false,
                error_code: Some("unknown".into()),
                updated_at: None,
            }
        );
    }

    #[test]
    fn an_unknown_error_code_in_the_state_file_is_not_passed_on() {
        let s = parse_state_file(r#"{"schema_version":1,"error_code":"start_timeout"}"#).unwrap();
        assert_eq!(s.error_code.as_deref(), Some("start_timeout"));
        let s = parse_state_file(r#"{"schema_version":1,"error_code":"/etc/passwd"}"#).unwrap();
        assert_eq!(s.error_code.as_deref(), Some("unknown"));
    }

    #[test]
    fn the_state_file_is_read_from_disk_with_a_size_limit() {
        let dir = std::env::temp_dir().join(format!("lastwerk-setup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(STATE_FILE);
        assert_eq!(read_state_file(&path), None);
        std::fs::write(&path, GOOD_STATE).unwrap();
        assert!(read_state_file(&path).is_some());
        // Eine riesige Datei wird nach 16 KiB abgeschnitten und ist dann kein gültiges JSON.
        std::fs::write(&path, vec![b'{'; 40_000]).unwrap();
        assert_eq!(read_state_file(&path), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // --- Hardware ----------------------------------------------------------------------

    /// 8192 MiB * 1 048 576 = 8 589 934 592 Byte = 8,59 GB. Zwei Karten: der größere Wert
    /// zählt, 24 576 MiB = 25,77 GB.
    #[test]
    fn nvidia_smi_output_is_read_in_gb() {
        let gb = parse_nvidia_smi("8192\n").unwrap();
        assert!((gb - 8.589_934_592).abs() < 1e-9, "{gb}");
        let gb = parse_nvidia_smi("8192\r\n24576\r\n").unwrap();
        assert!((gb - 25.769_803_776).abs() < 1e-9, "{gb}");
    }

    #[test]
    fn nvidia_smi_noise_gives_nothing() {
        for bad in [
            "",
            "\n",
            "[N/A]",
            "0",
            "NVIDIA-SMI has failed because it couldn't communicate with the NVIDIA driver.",
            "-5",
        ] {
            assert_eq!(parse_nvidia_smi(bad), None, "{bad:?}");
        }
        // Eine gültige Zeile neben Müll zählt.
        assert!(parse_nvidia_smi("[N/A]\n4096\n").is_some());
    }

    #[test]
    fn the_hardware_is_measured_once() {
        let state = CoachSetupState::default();
        let first = state.hardware_with(|| Hardware {
            ram_gb: 16.0,
            vram_gb: Some(8.0),
        });
        let second = state.hardware_with(|| panic!("nicht noch einmal messen"));
        assert_eq!(first, second);
    }

    // --- Sperre, Abbruch, Ausdünnung -----------------------------------------------------

    #[test]
    fn a_second_job_is_busy_until_the_first_ends() {
        let state = CoachSetupState::default();
        let first = state.begin(JobKind::Install, None).unwrap();
        assert_eq!(
            state
                .begin(JobKind::Pull, Some("qwen3:4b".into()))
                .unwrap_err()
                .code,
            "coach_setup_busy"
        );
        assert_eq!(state.busy().unwrap().kind, JobKind::Install);
        drop(first);
        assert_eq!(state.busy(), None);
        let again = state.begin(JobKind::Pull, Some("qwen3:4b".into()));
        assert!(again.is_ok());
        assert_eq!(state.busy().unwrap().name.as_deref(), Some("qwen3:4b"));
    }

    #[test]
    fn the_guard_is_released_after_a_panic() {
        let state = CoachSetupState::default();
        let s = state.clone();
        let joined = thread::spawn(move || {
            let _guard = s.begin(JobKind::Install, None).unwrap();
            panic!("Skript bricht ab");
        })
        .join();
        assert!(joined.is_err());
        assert_eq!(state.busy(), None);
        assert!(state.begin(JobKind::Install, None).is_ok());
    }

    #[test]
    fn cancel_only_applies_to_a_running_pull() {
        let state = CoachSetupState::default();
        state.cancel_pull();
        let guard = state.begin(JobKind::Install, None).unwrap();
        state.cancel_pull();
        assert!(!state.0.cancel.load(Ordering::SeqCst));
        drop(guard);
        let _pull = state.begin(JobKind::Pull, Some("qwen3:4b".into())).unwrap();
        state.cancel_pull();
        assert!(state.0.cancel.load(Ordering::SeqCst));
    }

    #[test]
    fn a_new_job_clears_an_old_cancel() {
        let state = CoachSetupState::default();
        let guard = state.begin(JobKind::Pull, Some("qwen3:4b".into())).unwrap();
        state.cancel_pull();
        drop(guard);
        let _again = state.begin(JobKind::Pull, Some("qwen3:4b".into())).unwrap();
        assert!(!state.0.cancel.load(Ordering::SeqCst));
    }

    /// Erste Meldung geht durch. Nach 100 ms noch nicht (Abstand 250 ms), nach 250 ms ja. Ein
    /// Wechsel der Phase und das Ende gehen sofort durch.
    #[test]
    fn progress_events_are_thinned() {
        let mut t = Throttle::default();
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        assert!(t.allow(t0, PullPhase::Download));
        assert!(!t.allow(t0 + ms(100), PullPhase::Download));
        assert!(!t.allow(t0 + ms(240), PullPhase::Download));
        assert!(t.allow(t0 + ms(250), PullPhase::Download));
        assert!(!t.allow(t0 + ms(300), PullPhase::Download));
        assert!(t.allow(t0 + ms(310), PullPhase::Verify));
        assert!(!t.allow(t0 + ms(320), PullPhase::Verify));
        assert!(t.allow(t0 + ms(330), PullPhase::Done));
        assert!(t.allow(t0 + ms(331), PullPhase::Done));
    }

    // --- Positivliste --------------------------------------------------------------------

    #[test]
    fn only_catalog_or_installed_names_may_be_pulled() {
        let catalog = embedded_catalog();
        let installed = vec!["llama3.2:3b".to_owned(), "hf.co/u/Model:Q4".to_owned()];
        for ok in [
            "qwen3:1.7b",
            "qwen3:4b",
            "qwen3:8b",
            "qwen3:14b",
            "llama3.2:3b",
        ] {
            assert!(is_allowed_pull_name(ok, &catalog, &installed), "{ok}");
        }
        for bad in [
            "",
            "qwen3",
            "qwen3:32b",
            "evil:latest",
            "qwen3:4b; rm -rf /",
            "../qwen3:4b",
            "QWEN3:4B",
            "qwen3:4b ",
            // Auch ein installierter Name muss dem einfachen Muster entsprechen.
            "hf.co/u/Model:Q4",
        ] {
            assert!(!is_allowed_pull_name(bad, &catalog, &installed), "{bad:?}");
        }
        // Ohne installierte Modelle gilt nur der Katalog.
        assert!(!is_allowed_pull_name("llama3.2:3b", &catalog, &[]));
    }

    #[test]
    fn pull_errors_map_to_distinct_app_codes() {
        for (e, code) in [
            (PullError::InvalidModel, "coach_pull_invalid_model"),
            (PullError::Cancelled, "coach_pull_cancelled"),
            (PullError::Timeout, "coach_pull_timeout"),
            (PullError::Transport("x".into()), "coach_pull_unreachable"),
            (PullError::Status(500), "coach_pull_bad_response"),
            (
                PullError::BadResponse("x".into()),
                "coach_pull_bad_response",
            ),
            (PullError::ModelNotFound, "coach_pull_model_not_found"),
            (PullError::DiskFull, "coach_pull_disk_full"),
            (PullError::Network, "coach_pull_network"),
            (PullError::Failed, "coach_pull_failed"),
        ] {
            assert_eq!(pull_error(&e).code, code);
        }
    }

    // --- Status ------------------------------------------------------------------------

    fn inputs(models: Option<Vec<&str>>, hw: Hardware) -> StatusInputs<'static> {
        StatusInputs {
            models: models.map(|m| m.into_iter().map(str::to_owned).collect()),
            configured_model: None,
            exe_present: false,
            can_install: true,
            hardware: hw,
            state: None,
            busy: None,
        }
    }

    fn hw(ram: f64, vram: Option<f64>) -> Hardware {
        Hardware {
            ram_gb: ram,
            vram_gb: vram,
        }
    }

    #[test]
    fn ollama_is_missing_installed_or_running() {
        let catalog = embedded_catalog();
        let s = build_status(inputs(None, hw(16.0, None)), &catalog);
        assert_eq!(s.ollama, "missing");
        assert!(s.models.is_empty() && s.model.is_none());

        let mut i = inputs(None, hw(16.0, None));
        i.exe_present = true;
        assert_eq!(build_status(i, &catalog).ollama, "installed");

        let mut i = inputs(None, hw(16.0, None));
        i.state = parse_state_file(GOOD_STATE);
        assert_eq!(build_status(i, &catalog).ollama, "installed");

        // Antwortet der Server, zählt er als laufend, auch ohne Modelle.
        let s = build_status(inputs(Some(vec![]), hw(16.0, None)), &catalog);
        assert_eq!(s.ollama, "running");
        assert_eq!(s.model, None);
    }

    #[test]
    fn the_catalog_carries_fit_recommendation_and_installed_flags() {
        let catalog = embedded_catalog();
        let s = build_status(
            inputs(Some(vec!["qwen3:4b", "llama3.2:3b"]), hw(16.0, Some(8.0))),
            &catalog,
        );
        let row = |n: &str| s.catalog.iter().find(|e| e.name == n).unwrap().clone();
        assert_eq!(s.recommended.as_deref(), Some("qwen3:8b"));
        assert!(row("qwen3:8b").recommended && row("qwen3:8b").fit == Fit::Fits);
        assert!(!row("qwen3:4b").recommended && row("qwen3:4b").installed);
        assert_eq!(row("qwen3:14b").fit, Fit::TooBig);
        assert!(row("qwen3:4b").starter && !row("qwen3:8b").starter);
        assert_eq!(s.model.as_deref(), Some("qwen3:4b"));
        assert_eq!(
            s.hardware,
            HardwareDto {
                ram_gb: 16.0,
                vram_gb: Some(8.0)
            }
        );
    }

    #[test]
    fn the_configured_model_counts_only_if_the_server_has_it() {
        let catalog = embedded_catalog();
        let mut i = inputs(Some(vec!["qwen3:4b", "llama3.2:3b"]), hw(16.0, None));
        i.configured_model = Some("llama3.2:3b");
        assert_eq!(
            build_status(i, &catalog).model.as_deref(),
            Some("llama3.2:3b")
        );
        let mut i = inputs(Some(vec!["qwen3:4b"]), hw(16.0, None));
        i.configured_model = Some("geloescht:1b");
        assert_eq!(build_status(i, &catalog).model.as_deref(), Some("qwen3:4b"));
    }

    #[test]
    fn the_status_json_has_no_paths() {
        let catalog = embedded_catalog();
        let mut i = inputs(Some(vec!["qwen3:4b"]), hw(8.0, None));
        i.state = parse_state_file(GOOD_STATE);
        let text = serde_json::to_string(&build_status(i, &catalog)).unwrap();
        for forbidden in ["\\\\", "C:", "/home", "ps1", "AppData", "LOCALAPPDATA"] {
            assert!(!text.contains(forbidden), "{forbidden}: {text}");
        }
    }

    // --- Skript als Kindprozess (nur Unix: `sh` ersetzt PowerShell) ----------------------

    #[cfg(unix)]
    fn sh(script: &str) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", script]);
        cmd
    }

    #[cfg(unix)]
    fn run(script: &str, timeout: Duration) -> (std::result::Result<(), String>, Vec<String>) {
        let mut seen = Vec::new();
        let r = run_script(sh(script), timeout, &mut |p| {
            seen.push(format!("{}:{}:{:?}", p.phase, p.value, p.percent));
        });
        (r, seen)
    }

    /// Synthetische Ausgabe eines erfolgreichen Laufs mit Rauschen dazwischen.
    #[cfg(unix)]
    #[test]
    fn a_successful_run_reports_the_known_lines() {
        let (r, seen) = run(
            "printf 'Hallo\\nLASTWERK:detect:missing\\nLASTWERK:install:winget\\nLASTWERK:install:verify_ok\\nLASTWERK:start:ok\\nLASTWERK:pull:skipped\\nrauschen\\nLASTWERK:done:ok\\n'",
            Duration::from_secs(10),
        );
        assert_eq!(r, Ok(()));
        assert_eq!(
            seen,
            [
                "detect:missing:None",
                "install:winget:None",
                "install:verify_ok:None",
                "start:ok:None",
                "pull:skipped:None",
                "done:ok:None",
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_run_returns_the_code_of_the_last_error_line() {
        let (r, seen) = run(
            "printf 'LASTWERK:detect:missing\\nLASTWERK:install:winget\\nLASTWERK:error:winget_failed\\n'; exit 1",
            Duration::from_secs(10),
        );
        assert_eq!(r, Err("winget_failed".to_owned()));
        assert_eq!(seen.last().unwrap(), "error:winget_failed:None");
    }

    #[cfg(unix)]
    #[test]
    fn exit_code_2_without_a_line_is_bad_args_and_other_exits_are_failed() {
        assert_eq!(
            run("exit 2", Duration::from_secs(10)).0,
            Err("bad_args".into())
        );
        assert_eq!(
            run("exit 1", Duration::from_secs(10)).0,
            Err("failed".into())
        );
        assert_eq!(
            run(
                "printf 'LASTWERK:error:komisch\\n'; exit 1",
                Duration::from_secs(10)
            )
            .0,
            Err("failed".into())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_run_that_takes_too_long_is_killed() {
        let started = Instant::now();
        let (r, _) = run(
            "printf 'LASTWERK:detect:missing\\n'; sleep 30",
            Duration::from_millis(500),
        );
        assert_eq!(r, Err("timeout".into()));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_missing_program_is_failed() {
        let cmd = Command::new("lastwerk-programm-gibt-es-nicht");
        assert_eq!(
            run_script(cmd, Duration::from_secs(5), &mut |_| {}),
            Err("failed".into())
        );
    }

    /// Das Startmodell des Katalogs und des Installer-Hooks (Teil A) sind dasselbe. Der Hook
    /// liegt erst nach dem Zusammenführen von Teil A im Zweig; bis dahin prüft der Test nichts.
    #[test]
    fn the_starter_model_matches_the_installer_hook_if_present() {
        let starter = starter_name().unwrap();
        assert_eq!(starter, "qwen3:4b");
        let hook = Path::new(env!("CARGO_MANIFEST_DIR")).join("windows/hooks.nsh");
        if let Ok(text) = std::fs::read_to_string(hook) {
            assert!(text.contains(&starter), "hooks.nsh nennt {starter} nicht");
        }
    }
}
