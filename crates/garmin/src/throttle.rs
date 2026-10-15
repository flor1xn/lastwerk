//! Drosselung und Sperre (SPEC 5.6, Abrufregel 4, O24).
//!
//! Alle Zahlen sind vorläufige Startwerte (Faustregel, O24) und stammen aus dem Zusatzprogramm
//! (Zusatzprogramm `bridge/`, Datei `throttle.py`). Gelockert wird erst nach dem Livetest (T23).
//!
//! - Mindestabstand zwischen zwei Anfragen an Garmin: 1,5 s plus Zufall bis 0,5 s.
//! - Nach einer Antwort 429 oder 403 wartet die nächste Anfrage zusätzlich 30 s.
//! - Die dritte Antwort 429 oder 403 in Folge schreibt eine Sperrdatei mit Ende in 6 Stunden.
//!   Solange sie gilt, endet jede Anfrage sofort mit `Locked`, auch nach einem Neustart der App.
//!
//! Die Drossel kennt weder HTTP noch Token. Sie bekommt vor jeder Anfrage `before_request` und
//! nach jeder Antwort `after_response`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::clock::{Clock, Jitter};
use crate::error::{GarminError, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThrottleConfig {
    pub min_interval: Duration,
    pub jitter: Duration,
    pub block_pause: Duration,
    pub lockout_threshold: u32,
    pub lockout_hold: Duration,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            min_interval: Duration::from_millis(1500),
            jitter: Duration::from_millis(500),
            block_pause: Duration::from_secs(30),
            lockout_threshold: 3,
            lockout_hold: Duration::from_secs(6 * 3600),
        }
    }
}

/// Inhalt der Sperrdatei. Enthält keine Token und keine Adressen.
#[derive(Debug, Serialize, Deserialize)]
struct LockoutRecord {
    since_epoch_s: i64,
    until_epoch_s: i64,
    http_status: u16,
    consecutive: u32,
}

#[derive(Debug, Default)]
struct State {
    last_call_ms: Option<i64>,
    pause_until_ms: i64,
    consecutive_blocks: u32,
    /// Spiegel der Sperre im Speicher, falls die Datei nicht geschrieben werden konnte.
    locked_until_ms: Option<i64>,
}

pub struct Throttle {
    config: ThrottleConfig,
    clock: Arc<dyn Clock>,
    jitter: Arc<dyn Jitter>,
    lockout_file: PathBuf,
    state: Mutex<State>,
}

impl std::fmt::Debug for Throttle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Throttle")
            .field("config", &self.config)
            .field("lockout_file", &self.lockout_file)
            .finish_non_exhaustive()
    }
}

fn until_to_datetime(epoch_s: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(epoch_s, 0).unwrap_or(DateTime::<Utc>::MAX_UTC)
}

impl Throttle {
    /// `lockout_file` bestimmt der Aufrufer (SPEC 5.6: Datenordner der App).
    pub fn new(
        lockout_file: impl Into<PathBuf>,
        config: ThrottleConfig,
        clock: Arc<dyn Clock>,
        jitter: Arc<dyn Jitter>,
    ) -> Self {
        Self {
            config,
            clock,
            jitter,
            lockout_file: lockout_file.into(),
            state: Mutex::new(State::default()),
        }
    }

    pub fn lockout_file(&self) -> &Path {
        &self.lockout_file
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        // Ein vergifteter Mutex hieße, dass ein anderer Thread beim Zählen abstürzte. Die Zähler
        // sind einfache Zahlen und bleiben brauchbar, also weiterarbeiten statt selbst abstürzen.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Ende der Sperre, solange sie gilt.
    pub fn active_lockout(&self) -> Option<DateTime<Utc>> {
        let now_ms = self.clock.now_ms();
        let mut until_ms = self.state().locked_until_ms;
        if let Some(record) = self.read_lockout_file() {
            let file_ms = record.until_epoch_s.saturating_mul(1000);
            until_ms = Some(until_ms.map_or(file_ms, |m| m.max(file_ms)));
        }
        until_ms
            .filter(|&ms| ms > now_ms)
            .map(|ms| until_to_datetime(ms.div_euclid(1000)))
    }

    /// Prüft die Sperre und wartet bis zum nächsten erlaubten Zeitpunkt. Danach darf genau eine
    /// Anfrage folgen.
    pub fn before_request(&self) -> Result<()> {
        if let Some(until) = self.active_lockout() {
            return Err(GarminError::Locked { until });
        }
        let jitter_ms = (self.config.jitter.as_millis() as f64 * self.jitter.fraction()) as i64;
        let spacing_ms = self.config.min_interval.as_millis() as i64 + jitter_ms;
        // Obergrenze der Wartezeit: Springt die Wanduhr zurück, soll die App nicht Stunden schlafen.
        let cap_ms = self
            .config
            .block_pause
            .max(self.config.min_interval + self.config.jitter)
            .as_millis() as i64;

        let wait_ms = {
            let state = self.state();
            let now_ms = self.clock.now_ms();
            let earliest = state.last_call_ms.map_or(state.pause_until_ms, |last| {
                state.pause_until_ms.max(last.saturating_add(spacing_ms))
            });
            (earliest - now_ms).clamp(0, cap_ms)
        };
        if wait_ms > 0 {
            self.clock.sleep(Duration::from_millis(wait_ms as u64));
        }
        self.state().last_call_ms = Some(self.clock.now_ms());
        Ok(())
    }

    /// Meldet die Antwort. `status` ist `None`, wenn keine Antwort kam (Netzfehler); das ändert
    /// den Zähler nicht. Gibt `Err(Locked)` zurück, wenn diese Antwort die Sperre auslöst.
    pub fn after_response(&self, status: Option<u16>) -> Result<()> {
        let Some(status) = status else {
            return Ok(());
        };
        let mut state = self.state();
        if !matches!(status, 429 | 403) {
            state.consecutive_blocks = 0;
            return Ok(());
        }
        let now_ms = self.clock.now_ms();
        state.consecutive_blocks += 1;
        if state.consecutive_blocks < self.config.lockout_threshold {
            state.pause_until_ms =
                now_ms.saturating_add(self.config.block_pause.as_millis() as i64);
            return Ok(());
        }
        let hold_s = self.config.lockout_hold.as_secs() as i64;
        let since_s = now_ms.div_euclid(1000);
        let until_s = since_s.saturating_add(hold_s);
        let record = LockoutRecord {
            since_epoch_s: since_s,
            until_epoch_s: until_s,
            http_status: status,
            consecutive: state.consecutive_blocks,
        };
        state.locked_until_ms = Some(until_s.saturating_mul(1000));
        state.consecutive_blocks = 0;
        drop(state);
        // Schlägt das Schreiben fehl, bleibt die Sperre im Speicher bis zum Programmende. Das ist
        // besser als ein Fehler, der den Abbruch verdeckt.
        let _ = self.write_lockout_file(&record);
        Err(GarminError::Locked {
            until: until_to_datetime(until_s),
        })
    }

    fn read_lockout_file(&self) -> Option<LockoutRecord> {
        // Eine fehlende oder unlesbare Datei gilt als keine Sperre: Sie stammt aus unserer
        // eigenen Schreibfunktion, ein Eingriff von außen ist hier kein Angriffsziel.
        let text = std::fs::read_to_string(&self.lockout_file).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write_lockout_file(&self, record: &LockoutRecord) -> std::io::Result<()> {
        if let Some(parent) = self.lockout_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut tmp = self.lockout_file.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        let json = serde_json::to_string_pretty(record).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, json)?;
        // Umbenennen ersetzt atomar, ein Absturz hinterlässt keine halbe Datei.
        std::fs::rename(&tmp, &self.lockout_file)
    }
}
