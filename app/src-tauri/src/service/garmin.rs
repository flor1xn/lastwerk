//! Garmin in der App (T23, SPEC 5.6): Zustand, Abgleich, Trennen.
//!
//! Die Arbeit machen `lastwerk-garmin` (Client, Schlüsselbund, Drossel) und `lastwerk-cli`
//! (Abgleich, Ort der Ablage). Hier stehen nur die Übersetzung in die Typen der Oberfläche und
//! die Sperre gegen zwei gleichzeitige Abgleiche. Alles ist ohne Fenster und ohne Netz testbar:
//! Der Abgleich nimmt eine `GarminSource`, Zustand und Trennen einen `GarminClient` mit einer
//! Token-Ablage im Speicher.
//!
//! Was die Oberfläche nie bekommt: Token, Ticket, Anzeigename des Kontos, Pfade der Ablage. Der
//! einzige Pfad ist `Settings::fit_root`, der feste Stamm (T28), nur zur Anzeige.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use lastwerk_cli::garmin::{
    GarminSource, SyncError, SyncOptions, SyncProgress, SyncSummary, fit_location, sync_into,
};
use lastwerk_garmin::GarminClient;
use lastwerk_store::Store;

use crate::dto::{ErrorCount, GarminAbort, GarminStatus, GarminSyncProgress, GarminSyncSummary};
use crate::error::{AppError, Result};

/// Auslöser eines Abgleichs: Klick des Nutzers oder einmal beim Start der App.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Start,
}

impl Trigger {
    pub fn parse(text: &str) -> Result<Self> {
        match text {
            "manual" => Ok(Self::Manual),
            "start" => Ok(Self::Start),
            _ => Err(AppError::new("invalid_value")),
        }
    }
}

fn iso_utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn iso_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// Zustand ohne Netzzugriff: Token vorhanden, letzter Abgleich, Einstellung, Sperre.
pub fn status(store: &Store, client: &GarminClient) -> Result<GarminStatus> {
    let settings = store.garmin_settings()?;
    Ok(GarminStatus {
        connected: client.is_connected()?,
        last_sync: settings.last_sync.map(iso_date),
        sync_on_start: settings.sync_on_start,
        locked_until: client.active_lockout().map(iso_utc),
    })
}

/// "Trennen": löscht die Token aus dem Schlüsselbund. Der Zugriff bei Garmin bleibt, bis der
/// Nutzer ihn dort widerruft (SPEC 5.6); die Oberfläche sagt das.
pub fn logout(client: &GarminClient) -> Result<()> {
    client.disconnect()?;
    Ok(())
}

/// Ob der Abgleich beim Start laufen soll: nur wenn eingeschaltet und Token vorhanden. Ein nicht
/// erreichbarer Schlüsselbund zählt als "nein", beim Start soll keine Fehlermeldung aufgehen.
pub fn should_sync_on_start(store: &Store, client: &GarminClient) -> Result<bool> {
    if !store.garmin_settings()?.sync_on_start {
        return Ok(false);
    }
    Ok(client.is_connected().unwrap_or(false))
}

fn sync_error(e: SyncError) -> AppError {
    match e {
        SyncError::Store(e) => e.into(),
        SyncError::Io(_) => AppError::new("garmin_io"),
    }
}

/// Abgleich (SPEC 5.6, Entscheidung T23, T28): neue FIT-Dateien landen in `<Stamm>/garmin`,
/// danach wird der ganze feste FIT-Stamm `fit_root` (`<lokaler Datenordner>/Lastwerk/fit`)
/// eingelesen. Der Stamm wird bei Bedarf angelegt.
pub fn sync(
    store: &mut Store,
    source: &dyn GarminSource,
    fit_root: &Path,
    today: NaiveDate,
    now: DateTime<Utc>,
    progress: &mut dyn FnMut(GarminSyncProgress),
) -> Result<GarminSyncSummary> {
    let loc = fit_location(fit_root).map_err(sync_error)?;
    let summary = sync_into(
        store,
        source,
        &loc.download,
        &loc.root,
        &SyncOptions::new(today, now),
        &mut |p: SyncProgress| progress(progress_dto(p)),
    )
    .map_err(sync_error)?;
    Ok(summary_dto(&summary))
}

pub fn progress_dto(p: SyncProgress) -> GarminSyncProgress {
    GarminSyncProgress {
        phase: p.phase.as_str().to_owned(),
        done: p.done,
        total: p.total,
    }
}

pub fn summary_dto(s: &SyncSummary) -> GarminSyncSummary {
    let errors: &BTreeMap<String, u32> = &s.errors;
    GarminSyncSummary {
        window_start: iso_date(s.window_start),
        activities_listed: s.activities_listed,
        new_activities: s.new_activities,
        known_activities: s.known_activities,
        imported: s.imported,
        import_errors: s.import_errors,
        recovery_days: s.recovery_days,
        dropped_values: s.dropped_values,
        calls: s.calls,
        thresholds_changed: s.thresholds_changed,
        recomputed: s.recomputed,
        wellness_days: s.wellness_days,
        training_days: s.training_days,
        sensitive_days: s.sensitive_days,
        garmin_loads: s.garmin_loads,
        errors_by_code: errors
            .iter()
            .map(|(code, count)| ErrorCount {
                code: code.clone(),
                count: *count,
            })
            .collect(),
        aborted: s.aborted.as_ref().map(|a| GarminAbort {
            code: a.code.clone(),
            until: a.until.map(iso_utc),
        }),
        last_sync_updated: s.last_sync_updated,
    }
}

/// Sperre gegen zwei Abgleiche zur selben Zeit (Klick während des Abgleichs beim Start). Der
/// zweite Aufruf bekommt sofort `busy`, statt hinter dem ersten zu warten: Die Drossel hielte
/// ihn ohnehin Minuten fest, und doppelte Arbeit bringt nichts. Gibt die Sperre beim Fallenlassen
/// frei, auch nach einer Panik im Abgleich.
#[derive(Debug)]
pub struct SyncGuard(Arc<AtomicBool>);

impl SyncGuard {
    pub fn try_acquire(flag: &Arc<AtomicBool>) -> Result<Self> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self(Arc::clone(flag)))
            .map_err(|_| AppError::new("busy"))
    }
}

impl Drop for SyncGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Höchstens etwa 200 Fortschrittsereignisse beim Import, wie `import_folder` (Konvention).
/// Aktivitäten und Erholung melden jeden Schritt: Jeder ist ein gedrosselter Aufruf an Garmin
/// (mindestens 1,5 s), die Brücke zur Oberfläche wird davon nicht belastet.
pub fn should_emit(p: &GarminSyncProgress, last_import_done: &mut Option<u32>) -> bool {
    if p.phase != "import" {
        return true;
    }
    let total = p.total.unwrap_or(0);
    let step = (total / 200).max(1);
    if p.done == total || p.done.is_multiple_of(step) {
        if *last_import_done == Some(p.done) {
            return false;
        }
        *last_import_done = Some(p.done);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_accepts_only_two_words() {
        assert_eq!(Trigger::parse("manual").unwrap(), Trigger::Manual);
        assert_eq!(Trigger::parse("start").unwrap(), Trigger::Start);
        for bad in ["", "Manual", "start ", "auto", "1"] {
            assert_eq!(Trigger::parse(bad).unwrap_err().code, "invalid_value");
        }
    }

    #[test]
    fn a_second_sync_is_busy_until_the_first_ends() {
        let flag = Arc::new(AtomicBool::new(false));
        let first = SyncGuard::try_acquire(&flag).unwrap();
        assert_eq!(SyncGuard::try_acquire(&flag).unwrap_err().code, "busy");
        drop(first);
        let again = SyncGuard::try_acquire(&flag);
        assert!(again.is_ok());
    }

    #[test]
    fn the_guard_is_released_after_a_panic() {
        let flag = Arc::new(AtomicBool::new(false));
        let f = Arc::clone(&flag);
        let joined = std::thread::spawn(move || {
            let _guard = SyncGuard::try_acquire(&f).unwrap();
            panic!("Abgleich bricht ab");
        })
        .join();
        assert!(joined.is_err());
        assert!(!flag.load(Ordering::Acquire));
    }

    /// 1000 Dateien: Schritt 1000 / 200 = 5, also die Nummern 5, 10, ..., 1000, das sind 200
    /// Ereignisse; eine doppelte Meldung derselben Nummer zählt nur einmal. Andere Phasen melden
    /// jeden Schritt.
    #[test]
    fn import_progress_is_thinned_to_about_200_events() {
        let mut last = None;
        let mut n = 0;
        for done in 1..=1000 {
            let p = GarminSyncProgress {
                phase: "import".into(),
                done,
                total: Some(1000),
            };
            if should_emit(&p, &mut last) {
                n += 1;
            }
        }
        assert_eq!(n, 200);
        let p = GarminSyncProgress {
            phase: "import".into(),
            done: 1000,
            total: Some(1000),
        };
        assert!(!should_emit(&p, &mut last));
        let p = GarminSyncProgress {
            phase: "activities".into(),
            done: 3,
            total: None,
        };
        assert!(should_emit(&p, &mut last));
    }
}
