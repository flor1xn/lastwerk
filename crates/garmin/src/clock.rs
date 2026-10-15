//! Uhr, Schlafen und Zufall als austauschbare Teile, damit Tests nicht wirklich warten und die
//! Drossel deterministisch prüfbar ist. Die Fassung für den Betrieb nutzt Systemzeit und den
//! Zufall der Standardbibliothek.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    /// Millisekunden seit der Unix-Epoche (Wanduhr).
    fn now_ms(&self) -> i64;
    fn sleep(&self, duration: Duration);
}

/// Zufallsanteil für den Mindestabstand.
pub trait Jitter: Send + Sync {
    /// Gleichverteilt in `[0, 1)`.
    fn fraction(&self) -> f64;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        // Eine Uhr vor 1970 ist defekt und liefert hier 0 statt eines Absturzes. Eine Sperre
        // bleibt dann aktiv, weil ihr Ende in der Datei in der Zukunft liegt.
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0)
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Zufall ohne Zusatzabhängigkeit: `RandomState` der Standardbibliothek wird vom Betriebssystem
/// zufällig gesät. Reicht für Abstände zwischen Aufrufen (kein Sicherheitszweck, Faustregel).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemJitter;

impl Jitter for SystemJitter {
    fn fraction(&self) -> f64 {
        let bits = RandomState::new().build_hasher().finish();
        // Obere 53 Bit ergeben eine gleichmäßig verteilte Zahl in [0, 1).
        (bits >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_jitter_stays_in_unit_interval() {
        for _ in 0..1000 {
            let f = SystemJitter.fraction();
            assert!((0.0..1.0).contains(&f), "{f}");
        }
    }

    #[test]
    fn system_clock_is_after_2020() {
        // 1_577_836_800_000 ms = 1. Januar 2020, 00:00 UTC.
        assert!(SystemClock.now_ms() > 1_577_836_800_000);
    }
}
