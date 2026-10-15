//! Abgleich mit Garmin: Aktivitäten als FIT-Dateien und Erholungswerte (SPEC 5.6, T22).
//!
//! Die Bibliothek wird von `lastwerk garmin sync` und von der App gleich benutzt. Sie holt
//! über [`GarminSource`] (im Betrieb der `GarminClient` aus `lastwerk-garmin`, in Tests eine
//! Attrappe) und schreibt in den Speicher und in den FIT-Ordner:
//!
//! 1. **Aktivitäten.** Die Liste wird seitenweise gelesen, bis eine Aktivität vor dem Fenster
//!    liegt. Für jede, deren Datei `<Kennung>.fit` im FIT-Ordner fehlt, kommt das ZIP, daraus
//!    nur die FIT-Datei (siehe [`fit_zip`]). Der Dateiname entsteht allein aus der Kennung.
//! 2. **Erholung.** HRV, Ruhepuls und Body Battery über Bereichsabfragen, Schlaf je Tag nur für
//!    die letzten [`SLEEP_DAYS`] Tage des Fensters. Pro Tag wird gemischt: ein vorhandener Wert
//!    wird nie durch einen fehlenden ersetzt (Garmin liefert Werte nachträglich).
//! 3. **Import.** Danach der gewöhnliche Import des FIT-Ordners (Duplikatregel 6.4 und
//!    Belastung greifen wie bei jeder anderen Datei).
//!
//! Das Fenster beginnt ohne bisherigen Abgleich `backfill_days` vor heute (Standard 365,
//! Faustregel), sonst 7 Tage vor dem letzten Abgleich, weil Garmin Tageswerte nachträglich
//! ergänzt (wie `sync --refresh-days 7` des Zusatzprogramms, SPEC 5.5).
//!
//! Bricht Garmin den Lauf ab (Sperre, Drosselung, Anmeldung, Netz), ist das kein Fehler der
//! Funktion: Sie liefert, was bis dahin da war, im [`SyncSummary`] mit `aborted`. Nur lokale
//! Fehler (Datenbank, Dateisystem) sind ein [`SyncError`]. `garmin.last_sync` wird nur bei einem
//! Lauf ohne Abbruch gesetzt.
//!
//! Meldungen und Fehler enthalten nie einen Pfad, ein Token oder den Anzeigenamen des Kontos.

mod fit_zip;
mod parse;

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use lastwerk_garmin::{
    ActivitySummary, Endpoints, GarminClient, GarminError, KeyringStore, MAX_FIT_BYTES, Profile,
    TokenStore,
};
use lastwerk_store::{
    ImportOptions, MAX_BACKFILL_DAYS, MIN_BACKFILL_DAYS, RecoveryDayRecord, Store, StoreError,
    UpsertOutcome,
};
use serde_json::Value;

use crate::error::CliError;
use crate::paths;

pub use fit_zip::{ExtractError, extract_fit};

type GResult<T> = Result<T, GarminError>;

/// Quelle der Daten: genau die lesenden Aufrufe, die der Abgleich braucht. Der `GarminClient`
/// implementiert sie; Tests setzen eine Attrappe ein, ohne Netz und ohne Warten.
pub trait GarminSource {
    fn social_profile(&self) -> GResult<Profile>;
    fn activities(&self, start: u32, limit: u32) -> GResult<Vec<ActivitySummary>>;
    /// ZIP-Rohbytes des Originals einer Aktivität.
    fn activity_fit(&self, activity_id: u64) -> GResult<Vec<u8>>;
    fn hrv_range(&self, start: NaiveDate, end: NaiveDate) -> GResult<Value>;
    fn rhr_range(&self, display_name: &str, start: NaiveDate, end: NaiveDate) -> GResult<Value>;
    fn sleep_day(&self, display_name: &str, date: NaiveDate) -> GResult<Value>;
    fn body_battery(&self, start: NaiveDate, end: NaiveDate) -> GResult<Value>;
}

impl GarminSource for GarminClient {
    fn social_profile(&self) -> GResult<Profile> {
        GarminClient::social_profile(self)
    }
    fn activities(&self, start: u32, limit: u32) -> GResult<Vec<ActivitySummary>> {
        GarminClient::activities(self, start, limit)
    }
    fn activity_fit(&self, activity_id: u64) -> GResult<Vec<u8>> {
        GarminClient::activity_fit(self, activity_id)
    }
    fn hrv_range(&self, start: NaiveDate, end: NaiveDate) -> GResult<Value> {
        GarminClient::hrv_range(self, start, end)
    }
    fn rhr_range(&self, display_name: &str, start: NaiveDate, end: NaiveDate) -> GResult<Value> {
        GarminClient::rhr_range(self, display_name, start, end)
    }
    fn sleep_day(&self, display_name: &str, date: NaiveDate) -> GResult<Value> {
        GarminClient::sleep_day(self, display_name, date)
    }
    fn body_battery(&self, start: NaiveDate, end: NaiveDate) -> GResult<Value> {
        GarminClient::body_battery(self, start, end)
    }
}

/// Quelle der Erholungstage im Speicher (Feld `source`).
pub const RECOVERY_SOURCE: &str = "garmin-app";

/// Größe einer Seite der Aktivitätsliste (Faustregel, wie `ACTIVITY_PAGE_SIZE` des
/// Zusatzprogramms).
pub const ACTIVITY_PAGE_SIZE: u32 = 100;
/// So viele Seiten liest der Abgleich höchstens (Faustregel: 20 000 Aktivitäten, mehr als
/// 1825 Tage je zehn Einheiten; schützt vor einer Liste, die nie endet).
pub const MAX_ACTIVITY_PAGES: u32 = 200;
/// Tage, die ein Abgleich nach dem letzten zurückgeht (Garmin ergänzt Tageswerte nachträglich,
/// wie `--refresh-days 7` des Zusatzprogramms, SPEC 5.5).
pub const REFRESH_DAYS: i64 = 7;
/// Schlaf wird nur für so viele Tage am Ende des Fensters geholt (Faustregel: Die
/// 7-Tage-Basis und der 90-Tage-Trend der Regel R12 brauchen nicht mehr, und jeder Tag ist ein
/// Aufruf).
pub const SLEEP_DAYS: i64 = 90;
/// Längste Spanne (in Tagen, einschließlich beider Enden) je Aufruf. HRV und Body Battery nach
/// der Messung der Sonde (`bridge/registry.yaml`, `max_span_days`: 336 und 31, SPEC 14 O26),
/// Ruhepuls konservativ nach der Docstring-Angabe der Bibliothek ("bis etwa ein Jahr"), obwohl
/// die Sonde bis 1792 Tage ohne Fehler maß.
pub const HRV_SPAN_DAYS: i64 = 336;
pub const RHR_SPAN_DAYS: i64 = 365;
pub const BODY_BATTERY_SPAN_DAYS: i64 = 31;
/// Nach so vielen Fehlern in Folge (ohne "nicht verfügbar") bricht der Lauf ab (Faustregel:
/// weitere Aufrufe gegen einen kranken Server nützen nichts und kosten Drosselung).
pub const MAX_CONSECUTIVE_ERRORS: u32 = 5;

/// Phase des Abgleichs für die Anzeige.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPhase {
    Activities,
    Recovery,
    Import,
}

impl SyncPhase {
    /// Stabiler Name (`activities`, `recovery`, `import`) für Befehle der App.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Activities => "activities",
            Self::Recovery => "recovery",
            Self::Import => "import",
        }
    }
}

/// Fortschritt: `done` von `total`, soweit `total` schon bekannt ist (beim Lesen der
/// Aktivitätsliste zählt `done` die gefundenen Aktivitäten und `total` fehlt).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncProgress {
    pub phase: SyncPhase,
    pub done: u32,
    pub total: Option<u32>,
}

/// Warum der Lauf vorzeitig endete. `code` ist der stabile Fehlercode von `lastwerk-garmin`
/// (`locked`, `throttled`, `not_connected`, `auth_failed`, `network`, `store_failed`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncAbort {
    pub code: String,
    /// Ende der Sperre bei `locked`.
    pub until: Option<DateTime<Utc>>,
}

/// Ergebnis eines Laufs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSummary {
    /// Erster Tag des Fensters.
    pub window_start: NaiveDate,
    /// Aktivitäten der Liste im Fenster.
    pub activities_listed: u32,
    /// Neu geladene FIT-Dateien.
    pub new_activities: u32,
    /// Im Fenster, aber schon als Datei vorhanden (nicht erneut geladen).
    pub known_activities: u32,
    /// Vom Import neu aufgenommene oder geänderte Aktivitäten (`new + updated`).
    pub imported: u32,
    /// Dateien, die der Import nicht lesen konnte.
    pub import_errors: u32,
    /// Erholungstage, die neu angelegt oder geändert wurden.
    pub recovery_days: u32,
    /// Werte, die außerhalb der Plausibilitätsgrenzen lagen und verworfen wurden.
    pub dropped_values: u32,
    /// Aufrufe an Garmin (auch fehlgeschlagene).
    pub calls: u32,
    /// Fehler je Art (stabiler Code), ohne die, die den Lauf beendet haben.
    pub errors: BTreeMap<String, u32>,
    /// Gesetzt, wenn Garmin den Lauf beendet hat.
    pub aborted: Option<SyncAbort>,
    /// Ob `garmin.last_sync` gesetzt wurde (nur ohne Abbruch).
    pub last_sync_updated: bool,
}

/// Lokaler Fehler (Speicher oder Dateisystem). Nie mit Pfad.
#[derive(Debug)]
pub enum SyncError {
    Store(StoreError),
    Io(std::io::ErrorKind),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "{e}"),
            Self::Io(kind) => write!(f, "file system error ({kind})"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<StoreError> for SyncError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

impl From<std::io::Error> for SyncError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.kind())
    }
}

impl From<SyncError> for CliError {
    fn from(e: SyncError) -> Self {
        match e {
            SyncError::Store(e) => e.into(),
            SyncError::Io(kind) => Self::Internal(format!("Dateifehler beim Abgleich: {kind}")),
        }
    }
}

/// Eingaben eines Laufs, die nicht aus dem Speicher kommen.
#[derive(Debug, Clone, Copy)]
pub struct SyncOptions {
    /// Heute (lokales Datum des Nutzers), Ende des Fensters.
    pub today: NaiveDate,
    /// Jetzt, für `imported_at`. Der Speicher liest nie eine Uhr.
    pub now: DateTime<Utc>,
    /// Ersetzt das Fenster: so viele Tage vor heute, unabhängig vom letzten Abgleich.
    pub days: Option<u32>,
}

impl SyncOptions {
    pub fn new(today: NaiveDate, now: DateTime<Utc>) -> Self {
        Self {
            today,
            now,
            days: None,
        }
    }
}

/// Abgleich mit dem Client von `lastwerk-garmin`. Die Funktion, die die App aufruft.
pub fn sync_with_client(
    store: &mut Store,
    client: &GarminClient,
    fit_dir: &Path,
    today: NaiveDate,
    now: DateTime<Utc>,
    progress: &mut dyn FnMut(SyncProgress),
) -> Result<SyncSummary, SyncError> {
    sync(
        store,
        client,
        fit_dir,
        &SyncOptions::new(today, now),
        progress,
    )
}

/// Erster Tag des Fensters (siehe Moduldokumentation).
pub fn window_start(store: &Store, opts: &SyncOptions) -> Result<NaiveDate, StoreError> {
    let settings = store.garmin_settings()?;
    let start = match (opts.days, settings.last_sync) {
        (Some(days), _) => opts.today - TimeDelta::days(i64::from(days)),
        (None, Some(last)) => last - TimeDelta::days(REFRESH_DAYS),
        (None, None) => opts.today - TimeDelta::days(i64::from(settings.backfill_days)),
    };
    // Ein letzter Abgleich in der Zukunft (Uhr zurückgestellt) darf das Fenster nicht leeren.
    Ok(start.min(opts.today))
}

/// Unterordner des FIT-Stamms für Dateien aus dem Abgleich mit Garmin (SPEC 5.6, T23).
pub const GARMIN_SUBDIR: &str = "garmin";

/// Wo der Abgleich ablegt und was er danach einliest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FitLocation {
    /// Fester FIT-Stamm, wird als Ganzes eingelesen.
    pub root: PathBuf,
    /// `<root>/garmin`, dort landen die geholten Dateien.
    pub download: PathBuf,
}

/// Ort der Garmin-Dateien (SPEC 5.6, T28): der Unterordner `garmin/` im festen FIT-Stamm
/// (`paths::default_fit_root`). Der Nutzer wählt keinen Ordner mehr. Der Stamm wird angelegt, wenn
/// er fehlt: Er liegt im Datenordner der App, den Lastwerk besitzt, anders als der frühere
/// Ordner des Nutzers, den es nie neu anlegen durfte.
pub fn fit_location(root: &Path) -> Result<FitLocation, SyncError> {
    std::fs::create_dir_all(root)?;
    Ok(FitLocation {
        root: root.to_path_buf(),
        download: root.join(GARMIN_SUBDIR),
    })
}

/// Führt den Abgleich aus, Ablage und Import im selben Ordner `fit_dir` (so wie T22 ihn
/// gebaut hat, nur noch für Tests mit einem einzigen Ordner). Siehe Moduldokumentation und
/// [`sync_into`].
pub fn sync(
    store: &mut Store,
    source: &dyn GarminSource,
    fit_dir: &Path,
    opts: &SyncOptions,
    progress: &mut dyn FnMut(SyncProgress),
) -> Result<SyncSummary, SyncError> {
    sync_into(store, source, fit_dir, fit_dir, opts, progress)
}

/// Führt den Abgleich aus: neue FIT-Dateien landen in `download_dir`, danach wird
/// `import_root` eingelesen (rekursiv, also samt `download_dir`, wenn er darin liegt).
///
/// Die App und die Kommandozeile nehmen den festen FIT-Stamm als `import_root` und dessen
/// Unterordner `garmin/` als `download_dir` ([`fit_location`]).
/// Grund (T23): `source_file.rel_path` ist relativ zum eingelesenen Ordner. Mit nur einem
/// Wurzelordner tragen Garmin-Dateien den Pfad `garmin/<Kennung>.fit` und `rebuild_cache` mit
/// diesem Ordner verliert keine Aktivität (SPEC 5.4, Grenze der Ablage).
pub fn sync_into(
    store: &mut Store,
    source: &dyn GarminSource,
    download_dir: &Path,
    import_root: &Path,
    opts: &SyncOptions,
    progress: &mut dyn FnMut(SyncProgress),
) -> Result<SyncSummary, SyncError> {
    let fit_dir = download_dir;
    let start = window_start(store, opts)?;
    let mut run = Run {
        source,
        calls: 0,
        errors: BTreeMap::new(),
        consecutive_errors: 0,
        dropped: parse::Dropped::default(),
        found: Found::default(),
        days: BTreeMap::new(),
    };
    std::fs::create_dir_all(fit_dir)?;

    let mut aborted = None;
    let phases = run
        .activities(fit_dir, start, progress)
        .and_then(|()| run.recovery(start, opts.today, progress));
    match phases {
        Ok(()) => {}
        Err(Stop::Abort(abort)) => aborted = Some(abort),
        Err(Stop::Local(e)) => return Err(e),
    }

    // Was bis hierher gesammelt wurde, wird auch nach einem Abbruch geschrieben.
    let recovery_days = write_recovery(store, &run.days, start, opts)?;
    let report = store.import_dir(import_root, opts.now, &ImportOptions::default(), &mut |p| {
        progress(SyncProgress {
            phase: SyncPhase::Import,
            // Der Speicher meldet die 0-basierte Nummer der Datei in Arbeit.
            done: p.done as u32 + 1,
            total: Some(p.total as u32),
        });
    })?;

    let last_sync_updated = aborted.is_none();
    if last_sync_updated {
        store.set_garmin_last_sync(opts.today)?;
    }
    Ok(SyncSummary {
        window_start: start,
        activities_listed: run.found.listed,
        new_activities: run.found.downloaded,
        known_activities: run.found.known,
        imported: (report.new + report.updated) as u32,
        import_errors: report.error_count() as u32,
        recovery_days,
        dropped_values: run.dropped.0,
        calls: run.calls,
        errors: run.errors,
        aborted,
        last_sync_updated,
    })
}

#[derive(Default)]
struct Found {
    listed: u32,
    downloaded: u32,
    known: u32,
}

/// Werte eines Tages, wie sie aus den Antworten kommen (schon auf Plausibilität geprüft).
#[derive(Default, Clone, Copy)]
struct DayValues {
    hrv_overnight_ms: Option<f64>,
    resting_hr_bpm: Option<f64>,
    sleep_duration_s: Option<u32>,
    sleep_score: Option<u8>,
    body_battery_high: Option<u8>,
    body_battery_low: Option<u8>,
}

impl DayValues {
    fn is_empty(&self) -> bool {
        self.hrv_overnight_ms.is_none()
            && self.resting_hr_bpm.is_none()
            && self.sleep_duration_s.is_none()
            && self.sleep_score.is_none()
            && self.body_battery_high.is_none()
            && self.body_battery_low.is_none()
    }
}

/// Grund, den Lauf zu beenden.
enum Stop {
    Abort(SyncAbort),
    Local(SyncError),
}

impl From<SyncError> for Stop {
    fn from(e: SyncError) -> Self {
        Self::Local(e)
    }
}

impl From<std::io::Error> for Stop {
    fn from(e: std::io::Error) -> Self {
        Self::Local(e.into())
    }
}

/// Fehler, nach denen weitere Aufrufe nichts bringen: kein Zugang, Drosselung oder Sperre,
/// Netz oder Schlüsselbund gestört. Alles andere trifft nur den einen Aufruf.
fn ends_the_run(e: &GarminError) -> bool {
    matches!(
        e,
        GarminError::NotConnected
            | GarminError::AuthFailed { .. }
            | GarminError::Throttled
            | GarminError::Locked { .. }
            | GarminError::Network(_)
            | GarminError::Store(_)
    )
}

fn abort_of(e: &GarminError) -> SyncAbort {
    SyncAbort {
        code: e.code().to_owned(),
        until: match e {
            GarminError::Locked { until } => Some(*until),
            _ => None,
        },
    }
}

struct Run<'a> {
    source: &'a dyn GarminSource,
    calls: u32,
    errors: BTreeMap<String, u32>,
    consecutive_errors: u32,
    dropped: parse::Dropped,
    found: Found,
    days: BTreeMap<NaiveDate, DayValues>,
}

impl Run<'_> {
    /// Ein Aufruf an Garmin. `Ok(None)` heißt: dieser Aufruf scheiterte, der Lauf geht weiter
    /// (der Fehler ist gezählt). `Err` beendet den Lauf.
    fn call<T>(
        &mut self,
        f: impl FnOnce(&dyn GarminSource) -> GResult<T>,
    ) -> Result<Option<T>, Stop> {
        self.calls += 1;
        match f(self.source) {
            Ok(v) => {
                self.consecutive_errors = 0;
                Ok(Some(v))
            }
            Err(e) if ends_the_run(&e) => Err(Stop::Abort(abort_of(&e))),
            Err(e) => {
                *self.errors.entry(e.code().to_owned()).or_default() += 1;
                // "Nicht verfügbar" ist bei Schlaf und Aktivitäten normal (kein Gerät getragen)
                // und zählt weder als Fehler in Folge noch setzt es den Zähler zurück.
                if !matches!(e, GarminError::NotAvailable) {
                    self.consecutive_errors += 1;
                    if self.consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        return Err(Stop::Abort(abort_of(&e)));
                    }
                }
                Ok(None)
            }
        }
    }

    fn count(&mut self, code: &str) {
        *self.errors.entry(code.to_owned()).or_default() += 1;
    }

    // --- Aktivitäten ------------------------------------------------------------------

    fn activities(
        &mut self,
        fit_dir: &Path,
        start: NaiveDate,
        progress: &mut dyn FnMut(SyncProgress),
    ) -> Result<(), Stop> {
        let ids = self.list_activities(start, progress)?;
        self.found.listed = ids.len() as u32;

        let mut missing = Vec::new();
        for id in ids {
            if fit_path(fit_dir, id).exists() {
                self.found.known += 1;
            } else {
                missing.push(id);
            }
        }
        let total = missing.len() as u32;
        for (done, id) in missing.into_iter().enumerate() {
            progress(SyncProgress {
                phase: SyncPhase::Activities,
                done: done as u32,
                total: Some(total),
            });
            let Some(zip) = self.call(|s| s.activity_fit(id))? else {
                continue;
            };
            if zip.is_empty() {
                // 204: Garmin hat zu dieser Aktivität kein Original.
                self.count("empty_download");
                continue;
            }
            match extract_fit(&zip, &fit_path(fit_dir, id), MAX_FIT_BYTES) {
                Ok(_) => self.found.downloaded += 1,
                // Platte voll oder nicht beschreibbar: weitere Dateien scheitern ebenso.
                Err(ExtractError::Io(kind)) => return Err(Stop::Local(SyncError::Io(kind))),
                Err(e) => self.count(e.code()),
            }
        }
        progress(SyncProgress {
            phase: SyncPhase::Activities,
            done: total,
            total: Some(total),
        });
        Ok(())
    }

    /// Kennungen der Aktivitäten im Fenster, neueste zuerst, ohne Doppelte.
    fn list_activities(
        &mut self,
        start: NaiveDate,
        progress: &mut dyn FnMut(SyncProgress),
    ) -> Result<Vec<u64>, Stop> {
        let mut ids = Vec::new();
        let mut seen = HashSet::new();
        'pages: for page_no in 0..MAX_ACTIVITY_PAGES {
            let offset = page_no * ACTIVITY_PAGE_SIZE;
            let Some(page) = self.call(|s| s.activities(offset, ACTIVITY_PAGE_SIZE))? else {
                break;
            };
            let len = page.len();
            for activity in page {
                if activity.start_time_local.date() < start {
                    // Neueste zuerst: ab hier liegt alles vor dem Fenster.
                    break 'pages;
                }
                if seen.insert(activity.activity_id) {
                    ids.push(activity.activity_id);
                }
            }
            progress(SyncProgress {
                phase: SyncPhase::Activities,
                done: ids.len() as u32,
                total: None,
            });
            if len < ACTIVITY_PAGE_SIZE as usize {
                break;
            }
        }
        Ok(ids)
    }

    // --- Erholung ---------------------------------------------------------------------

    fn recovery(
        &mut self,
        start: NaiveDate,
        today: NaiveDate,
        progress: &mut dyn FnMut(SyncProgress),
    ) -> Result<(), Stop> {
        let sleep_start = start.max(today - TimeDelta::days(SLEEP_DAYS - 1));
        let hrv_chunks = chunks(start, today, HRV_SPAN_DAYS);
        let rhr_chunks = chunks(start, today, RHR_SPAN_DAYS);
        let bb_chunks = chunks(start, today, BODY_BATTERY_SPAN_DAYS);
        let sleep_days = (today - sleep_start).num_days() + 1;
        // Ein Aufruf für das Profil (Anzeigename für Ruhepuls und Schlaf).
        let total = (1 + hrv_chunks.len() + rhr_chunks.len() + bb_chunks.len()) as i64 + sleep_days;
        let total = total as u32;
        let mut done = 0u32;
        let tick = |done: &mut u32, progress: &mut dyn FnMut(SyncProgress)| {
            progress(SyncProgress {
                phase: SyncPhase::Recovery,
                done: *done,
                total: Some(total),
            });
            *done += 1;
        };

        for (from, to) in hrv_chunks {
            tick(&mut done, progress);
            if let Some(v) = self.call(|s| s.hrv_range(from, to))? {
                for (date, ms) in parse::hrv_days(&v, &mut self.dropped) {
                    if let Some(day) = self.day(date, from, to) {
                        day.hrv_overnight_ms = Some(ms);
                    }
                }
            }
        }
        for (from, to) in bb_chunks {
            tick(&mut done, progress);
            if let Some(v) = self.call(|s| s.body_battery(from, to))? {
                for (date, high, low) in parse::body_battery_days(&v, &mut self.dropped) {
                    if let Some(day) = self.day(date, from, to) {
                        day.body_battery_high = Some(high);
                        day.body_battery_low = Some(low);
                    }
                }
            }
        }

        // Ruhepuls und Schlaf brauchen den Anzeigenamen des Kontos. Er wird nur im Speicher des
        // Laufs gehalten, nirgends abgelegt und in keiner Meldung genannt.
        tick(&mut done, progress);
        let Some(profile) = self.call(|s| s.social_profile())? else {
            return Ok(());
        };
        let name = profile.display_name;
        for (from, to) in rhr_chunks {
            tick(&mut done, progress);
            if let Some(v) = self.call(|s| s.rhr_range(&name, from, to))? {
                for (date, bpm) in parse::rhr_days(&v, &mut self.dropped) {
                    if let Some(day) = self.day(date, from, to) {
                        day.resting_hr_bpm = Some(bpm);
                    }
                }
            }
        }
        let mut date = sleep_start;
        while date <= today {
            tick(&mut done, progress);
            if let Some(v) = self.call(|s| s.sleep_day(&name, date))? {
                let (duration, score) = parse::sleep(&v, &mut self.dropped);
                if let Some(day) = self.day(date, date, date) {
                    day.sleep_duration_s = duration;
                    day.sleep_score = score;
                }
            }
            date += TimeDelta::days(1);
        }
        progress(SyncProgress {
            phase: SyncPhase::Recovery,
            done: total,
            total: Some(total),
        });
        Ok(())
    }

    /// Eintrag eines Tages, nur für Tage, die die Anfrage `from..=to` gestellt hat. Antworten
    /// können Tage außerhalb enthalten (oder ein erfundenes Datum); die kommen nie in den
    /// Speicher.
    fn day(&mut self, date: NaiveDate, from: NaiveDate, to: NaiveDate) -> Option<&mut DayValues> {
        (from..=to)
            .contains(&date)
            .then(|| self.days.entry(date).or_default())
    }
}

/// Spannen von höchstens `span` Tagen (einschließlich beider Enden) von `first` bis `last`.
fn chunks(first: NaiveDate, last: NaiveDate, span: i64) -> Vec<(NaiveDate, NaiveDate)> {
    let mut out = Vec::new();
    let mut from = first;
    while from <= last {
        let to = (from + TimeDelta::days(span - 1)).min(last);
        out.push((from, to));
        from = to + TimeDelta::days(1);
    }
    out
}

fn fit_path(dir: &Path, activity_id: u64) -> PathBuf {
    // Der Name besteht nur aus der Kennung (eine Zahl), nie aus Daten von Garmin.
    dir.join(format!("{activity_id}.fit"))
}

/// Schreibt die gesammelten Tage. Pro Tag wird mit dem vorhandenen Eintrag gemischt: ein neuer
/// Wert ersetzt den alten, ein fehlender lässt den alten stehen.
fn write_recovery(
    store: &mut Store,
    days: &BTreeMap<NaiveDate, DayValues>,
    start: NaiveDate,
    opts: &SyncOptions,
) -> Result<u32, StoreError> {
    let existing: BTreeMap<NaiveDate, RecoveryDayRecord> = store
        .recovery_days(start, opts.today)?
        .into_iter()
        .map(|r| (r.date, r))
        .collect();
    let mut records = Vec::new();
    for (&date, new) in days {
        // Der Wegwerfeintrag und alles außerhalb des Fensters bleibt draußen.
        if date < start || date > opts.today || new.is_empty() {
            continue;
        }
        let old = existing.get(&date);
        records.push(RecoveryDayRecord {
            date,
            hrv_overnight_ms: new
                .hrv_overnight_ms
                .or_else(|| old.and_then(|o| o.hrv_overnight_ms)),
            resting_hr_bpm: new
                .resting_hr_bpm
                .or_else(|| old.and_then(|o| o.resting_hr_bpm)),
            sleep_duration_s: new
                .sleep_duration_s
                .or_else(|| old.and_then(|o| o.sleep_duration_s)),
            sleep_score: new.sleep_score.or_else(|| old.and_then(|o| o.sleep_score)),
            body_battery_high: new
                .body_battery_high
                .or_else(|| old.and_then(|o| o.body_battery_high)),
            body_battery_low: new
                .body_battery_low
                .or_else(|| old.and_then(|o| o.body_battery_low)),
            source: RECOVERY_SOURCE.to_owned(),
            imported_at: opts.now,
        });
    }
    let outcomes = store.upsert_recovery_days(&records)?;
    Ok(outcomes
        .iter()
        .filter(|o| matches!(o, UpsertOutcome::Inserted | UpsertOutcome::Updated))
        .count() as u32)
}

// --- Befehle `lastwerk garmin ...` ---------------------------------------------------

/// Fester deutscher Text zu einem Fehlercode von `lastwerk-garmin`. Nennt weder Pfade noch Token
/// noch den Anzeigenamen. Die App übersetzt über den Code, die Kommandozeile nimmt diese Texte.
pub fn abort_message(abort: &SyncAbort) -> String {
    match abort.code.as_str() {
        "not_connected" => "Nicht mit Garmin verbunden. Die Anmeldung läuft im Fenster der App \
                            (Einstellungen, Garmin verbinden), nicht in der Kommandozeile."
            .to_owned(),
        "auth_failed" => {
            "Garmin hat die Anmeldung abgelehnt. Bitte in der App neu verbinden.".to_owned()
        }
        "throttled" => "Garmin drosselt die Anfragen. Bitte später erneut versuchen.".to_owned(),
        "locked" => match abort.until {
            Some(until) => format!(
                "Anfragen an Garmin sind gesperrt bis {} (UTC), weil Garmin mehrfach gedrosselt \
                 hat. Bitte danach erneut versuchen.",
                until.format("%Y-%m-%d %H:%M")
            ),
            None => "Anfragen an Garmin sind vorübergehend gesperrt.".to_owned(),
        },
        "network" => {
            "Netzwerkfehler beim Abruf bei Garmin. Bitte die Verbindung prüfen.".to_owned()
        }
        "store_failed" => {
            "Der Schlüsselbund des Systems ist nicht erreichbar oder voll.".to_owned()
        }
        other => format!("Der Abgleich wurde abgebrochen ({other})."),
    }
}

/// `lastwerk garmin status`: verbunden ja oder nein, letzter Abgleich, Einstellungen. Ohne
/// Netzzugriff. Der Anzeigename des Kontos wird nicht abgelegt und deshalb nie ausgegeben
/// (er wird bei jedem Abgleich neu abgefragt).
pub fn run_status(
    client: &GarminClient,
    store: Option<&Store>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let connected = client.is_connected().map_err(|e| {
        abort_error(&SyncAbort {
            code: e.code().to_owned(),
            until: None,
        })
    })?;
    let say = |out: &mut dyn Write, text: String| {
        writeln!(out, "{text}")
            .map_err(|e| CliError::Internal(format!("Ausgabefehler: {}", e.kind())))
    };
    say(
        out,
        format!(
            "Garmin: {}",
            if connected {
                "verbunden"
            } else {
                "nicht verbunden (Anmeldung im Fenster der App)"
            }
        ),
    )?;
    let settings = match store {
        Some(s) => s.garmin_settings()?,
        None => lastwerk_store::GarminSettings {
            last_sync: None,
            sync_on_start: true,
            backfill_days: lastwerk_store::DEFAULT_BACKFILL_DAYS,
        },
    };
    say(
        out,
        match settings.last_sync {
            Some(d) => format!("Letzter Abgleich: {d}"),
            None => "Letzter Abgleich: noch keiner".to_owned(),
        },
    )?;
    say(
        out,
        format!(
            "Abgleich beim Start der App: {}",
            if settings.sync_on_start { "ja" } else { "nein" }
        ),
    )?;
    say(
        out,
        format!(
            "Rückschau beim ersten Abgleich: {} Tage (erlaubt {MIN_BACKFILL_DAYS} bis \
             {MAX_BACKFILL_DAYS})",
            settings.backfill_days
        ),
    )?;
    if let Some(until) = client.active_lockout() {
        say(
            out,
            format!("Gesperrt bis {} (UTC)", until.format("%Y-%m-%d %H:%M")),
        )?;
    }
    Ok(())
}

/// `lastwerk garmin logout`: löscht die Token aus dem Schlüsselbund. Der Zugriff bei Garmin
/// bleibt bestehen, bis der Nutzer ihn dort widerruft (SPEC 5.6); der Text sagt das.
pub fn run_logout(client: &GarminClient, out: &mut dyn Write) -> Result<(), CliError> {
    client.disconnect().map_err(|e| {
        abort_error(&SyncAbort {
            code: e.code().to_owned(),
            until: None,
        })
    })?;
    writeln!(
        out,
        "Die Verbindung zu Garmin ist getrennt, die Token sind gelöscht. Den Zugriff widerrufst \
         du zusätzlich in den Kontoeinstellungen bei Garmin."
    )
    .map_err(|e| CliError::Internal(format!("Ausgabefehler: {}", e.kind())))
}

fn abort_error(abort: &SyncAbort) -> CliError {
    CliError::Input(abort_message(abort))
}

/// `lastwerk garmin sync`: führt den Abgleich aus und schreibt den Bericht. Ein Abbruch durch
/// Garmin endet nach dem Bericht mit Exit-Code 2 (Eingabefehler: erst verbinden oder warten).
pub fn run_sync(
    store: &mut Store,
    source: &dyn GarminSource,
    download_dir: &Path,
    import_root: &Path,
    opts: &SyncOptions,
    out: &mut dyn Write,
    progress: &mut dyn FnMut(SyncProgress),
) -> Result<(), CliError> {
    let summary = sync_into(store, source, download_dir, import_root, opts, progress)?;
    write_summary(out, &summary)
        .map_err(|e| CliError::Internal(format!("Ausgabefehler: {}", e.kind())))?;
    match &summary.aborted {
        Some(abort) => Err(abort_error(abort)),
        None => Ok(()),
    }
}

fn write_summary(out: &mut dyn Write, s: &SyncSummary) -> std::io::Result<()> {
    writeln!(
        out,
        "{}",
        if s.aborted.is_some() {
            "Abgleich unvollständig beendet"
        } else {
            "Abgleich abgeschlossen"
        }
    )?;
    writeln!(out, "  Fenster ab: {}", s.window_start)?;
    writeln!(out, "  Aktivitäten im Fenster: {}", s.activities_listed)?;
    writeln!(out, "  Neu geladen: {}", s.new_activities)?;
    writeln!(out, "  Schon vorhanden: {}", s.known_activities)?;
    writeln!(out, "  Importiert: {}", s.imported)?;
    if s.import_errors > 0 {
        writeln!(out, "  Nicht lesbare Dateien: {}", s.import_errors)?;
    }
    writeln!(out, "  Erholungstage geschrieben: {}", s.recovery_days)?;
    if s.dropped_values > 0 {
        writeln!(out, "  Unplausible Werte verworfen: {}", s.dropped_values)?;
    }
    writeln!(out, "  Aufrufe an Garmin: {}", s.calls)?;
    for (code, n) in &s.errors {
        writeln!(out, "  Fehler {code}: {n}")?;
    }
    Ok(())
}

/// Der Client für die Kommandozeile: echte Adressen, Schlüsselbund, Sperrdatei im Datenordner.
pub fn default_client() -> Result<GarminClient, CliError> {
    let lockout = paths::default_lockout_file().ok_or_else(|| {
        CliError::Input("Kein Datenordner ermittelbar. Bitte LOCALAPPDATA oder HOME setzen.".into())
    })?;
    let store: Arc<dyn TokenStore> = Arc::new(KeyringStore::new());
    Ok(GarminClient::new(Endpoints::default(), store, lockout))
}

/// Fester FIT-Stamm (`<Datenordner>/Lastwerk/fit`, `LASTWERK_FIT` überschreibt ihn).
pub fn default_fit_root() -> Result<PathBuf, CliError> {
    paths::default_fit_root().ok_or_else(|| {
        CliError::Input("Kein Datenordner ermittelbar. Bitte LOCALAPPDATA oder HOME setzen.".into())
    })
}
