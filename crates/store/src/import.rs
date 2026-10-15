//! Import eines Ordners mit FIT-Dateien und Wiederaufbau des Caches (SPEC 6.3, 6.4, 12 M1).
//!
//! Ablauf je Datei: lesen, SHA-256 bilden, mit `source_file` vergleichen. Gleicher Pfad und
//! gleiche Prüfsumme heißt unverändert, die Datei wird nicht dekodiert. Sonst wird sie
//! dekodiert, die Aktivität gespeichert und die Belastung mit den Schwellenwerten gerechnet,
//! die am Tag der Aktivität gelten. Eine fehlerhafte Datei erscheint im Bericht und als
//! `source_file`-Zeile mit Status `error`, der Import läuft weiter. Nach der letzten Datei
//! läuft die Duplikatregel über alle Aktivitäten.
//!
//! Der ganze Lauf ist eine Transaktion. Bricht die Datenbank ab, bleibt der alte Stand.

use std::path::Path;

use chrono::{DateTime, Utc};
use lastwerk_core::{
    ActivityInput, LoadConfig, PreparedSamples, Sample, ZoneCatalog, normalized_power,
};
use lastwerk_fit::{FitError, RawActivity, list_fit_files, parse_activity, read_fit_file};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::catalog::embedded_zone_catalog;
use crate::derived::{prepare, write_best_effort, write_best_power, write_load, write_zone_times};
use crate::duplicates::{self, DuplicateRule};
use crate::error::{Result, StoreError};
use crate::rows::{ActivityRow, LoadStatus, SourceFileRow, SourceStatus};
use crate::statements::Db;
use crate::store::{
    Store, clear_cache, delete_activity_children, delete_activity_of_source, upsert_activity,
    upsert_source_file,
};

/// Stand der abgeleiteten Werte (Belastung, Zonen, Bestleistungen). Erhöhen, wenn sich die
/// Berechnung beim Import so ändert, dass vorhandene Zeilen veraltet sind; der nächste Import
/// liest dann jede bekannte Datei neu ein (Tabelle `cache_meta`, Migration 0005).
///
/// - 1: Stand bis T32 (stillschweigend, ohne Zeile in `cache_meta` ist es 0).
/// - 2: T33, Bestleistungen im Raster `POWER_CURVE_DURATIONS_S` statt fünf Dauern.
/// - 3: T37, Bestzeiten im Laufen (`activity_best_effort`).
pub const DERIVED_CACHE_VERSION: u32 = 3;

const CACHE_META_DERIVED: &str = "derived_version";

/// Gespeicherter Stand der abgeleiteten Werte, 0 ohne Eintrag oder bei unlesbarem Wert.
pub(crate) fn derived_version(conn: &Connection) -> Result<u32> {
    use rusqlite::OptionalExtension;
    let v: Option<String> = conn
        .query_row(
            "SELECT value FROM cache_meta WHERE key = ?1",
            [CACHE_META_DERIVED],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0))
}

fn set_derived_version(conn: &Connection, v: u32) -> Result<()> {
    conn.execute(
        "INSERT INTO cache_meta (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![CACHE_META_DERIVED, v.to_string()],
    )?;
    Ok(())
}

/// Einstellungen des Imports.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Parameter der Belastungsberechnung und der Plausibilitätsregeln (SPEC 6.5).
    pub load: LoadConfig,
    /// Schwellen der Duplikatregel (SPEC 6.4).
    pub duplicates: DuplicateRule,
    /// Zonenschemata (SPEC 8.5). Standard ist der eingebettete Katalog `catalog/zones.json`.
    pub zones: ZoneCatalog,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            load: LoadConfig::default(),
            duplicates: DuplicateRule::default(),
            zones: embedded_zone_catalog(),
        }
    }
}

/// Fortschritt für die Anzeige: Datei `done` von `total` wird gerade bearbeitet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub rel_path: String,
}

/// Eine Datei, die nicht als Aktivität gelesen werden konnte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIssue {
    /// Pfad relativ zum Importordner, `/` als Trenner. Leer, wenn der Ordner selbst nicht
    /// lesbar war.
    pub rel_path: String,
    /// Fehlercode, siehe `FitError::code`.
    pub code: String,
}

/// Ergebnis eines Imports. Jede gefundene Datei zählt in genau einem der Felder `new`,
/// `updated`, `unchanged`, `not_activity` oder `errors`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Zahl der gefundenen Dateien (Endung `.fit`), auch der nicht lesbaren.
    pub files_found: usize,
    /// Neue Dateien, aus denen eine Aktivität wurde.
    pub new: usize,
    /// Geänderte Dateien (gleicher Pfad, andere Prüfsumme), aus denen eine Aktivität wurde.
    pub updated: usize,
    /// Dateien mit gleichem Pfad und gleicher Prüfsumme wie beim letzten Import.
    pub unchanged: usize,
    /// Gültige FIT-Dateien ohne Aktivität, zum Beispiel Monitoring (`no_activity`).
    pub not_activity: usize,
    /// Fehlerhafte Dateien mit Code, in Reihenfolge der Pfade.
    pub errors: Vec<FileIssue>,
    /// Aktivitäten in der Datenbank nach dem Lauf, die als Duplikat gelten (SPEC 6.4).
    pub duplicates: usize,
    /// Aktivitäten in der Datenbank nach dem Lauf, die zählen (`is_primary = 1`).
    pub primary: usize,
    /// In diesem Lauf berechnete Belastungen mit Status `ok`.
    pub loads_ok: usize,
    /// ... mit Status `missing_threshold`.
    pub loads_missing_threshold: usize,
    /// ... mit Status `missing_channel`.
    pub loads_missing_channel: usize,
    /// ... mit Status `implausible`.
    pub loads_implausible: usize,
}

impl ImportReport {
    /// Zahl der fehlerhaften Dateien.
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub(crate) fn count_load(&mut self, s: LoadStatus) {
        match s {
            LoadStatus::Ok => self.loads_ok += 1,
            LoadStatus::MissingThreshold => self.loads_missing_threshold += 1,
            LoadStatus::MissingChannel => self.loads_missing_channel += 1,
            LoadStatus::Implausible => self.loads_implausible += 1,
        }
    }
}

impl Store {
    /// Die abgeleiteten Werte stammen von einem älteren Programmstand, und es gibt eingelesene
    /// Dateien: Der nächste Import rechnet alles neu (siehe [`DERIVED_CACHE_VERSION`]). Eine
    /// leere Datenbank ist nie veraltet.
    pub fn cache_outdated(&self) -> Result<bool> {
        if derived_version(&self.conn)? >= DERIVED_CACHE_VERSION {
            return Ok(false);
        }
        let files: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM source_file WHERE status = 'ok'",
            [],
            |r| r.get(0),
        )?;
        Ok(files > 0)
    }

    /// Liest einen Ordner rekursiv ein (SPEC 5.4, 12 M1). Bereits bekannte, unveränderte
    /// Dateien werden übersprungen. `now` ist der Zeitpunkt, der als `imported_at` und
    /// `computed_at` gespeichert wird.
    pub fn import_dir(
        &mut self,
        dir: &Path,
        now: DateTime<Utc>,
        opts: &ImportOptions,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<ImportReport> {
        if !dir.is_dir() {
            return Err(StoreError::NotADirectory);
        }
        let tx = self.conn.transaction()?;
        let report = run(&tx, dir, now, opts, progress)?;
        tx.commit()?;
        Ok(report)
    }

    /// Wiederaufbau des Caches (SPEC 6.3): leert `source_file`, `activity`, `activity_load`,
    /// `activity_zone_time` und `activity_best_power`, liest den Ordner neu ein und rechnet neu.
    /// Nutzertabellen bleiben unberührt. Abweichung von der Skizze in SPEC 5.4: zusätzlich
    /// `now` und `opts`, damit der Lauf deterministisch und einstellbar ist, und der Bericht
    /// als Ergebnis.
    pub fn rebuild_cache(
        &mut self,
        dir: &Path,
        now: DateTime<Utc>,
        opts: &ImportOptions,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<ImportReport> {
        if !dir.is_dir() {
            return Err(StoreError::NotADirectory);
        }
        let tx = self.conn.transaction()?;
        clear_cache(&tx)?;
        let report = run(&tx, dir, now, opts, progress)?;
        tx.commit()?;
        Ok(report)
    }
}

/// SHA-256 als Kleinbuchstaben-Hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn run(
    conn: &Connection,
    dir: &Path,
    now: DateTime<Utc>,
    opts: &ImportOptions,
    progress: &mut dyn FnMut(Progress),
) -> Result<ImportReport> {
    // Veraltete abgeleitete Werte: Jede bekannte Datei wird neu gelesen, auch unverändert.
    let stale = derived_version(conn)? < DERIVED_CACHE_VERSION;
    // Vorbereitete Anweisungen für alle Dateien des Laufs (T38, Anhang Messung).
    let db = Db::new(conn);
    let listing = list_fit_files(dir);
    let total = listing.len();
    let mut report = ImportReport {
        files_found: total,
        ..ImportReport::default()
    };

    for (i, item) in listing.into_iter().enumerate() {
        let file = match item {
            Ok(f) => f,
            Err(e) => {
                progress(Progress {
                    done: i,
                    total,
                    rel_path: e.rel_path.clone(),
                });
                report.errors.push(FileIssue {
                    rel_path: e.rel_path,
                    code: e.error.code().to_owned(),
                });
                continue;
            }
        };
        progress(Progress {
            done: i,
            total,
            rel_path: file.rel_path.clone(),
        });

        // Lesefehler und Größenlimit: keine Prüfsumme, also keine Zeile in source_file.
        let bytes = match read_fit_file(&file.path) {
            Ok(b) => b,
            Err(e) => {
                report.errors.push(FileIssue {
                    rel_path: file.rel_path,
                    code: e.code().to_owned(),
                });
                continue;
            }
        };
        let sha256 = sha256_hex(&bytes);

        let known: Option<(i64, String)> = db.with(
            "SELECT id, sha256 FROM source_file WHERE rel_path = ?1",
            |s| {
                use rusqlite::OptionalExtension;
                s.query_row([&file.rel_path], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()
            },
        )?;
        if !stale && known.as_ref().is_some_and(|(_, old)| *old == sha256) {
            report.unchanged += 1;
            continue;
        }

        let mtime = std::fs::metadata(&file.path)
            .and_then(|m| m.modified())
            .map_or(now, DateTime::<Utc>::from);
        let mut source = SourceFileRow {
            rel_path: file.rel_path.clone(),
            sha256,
            size_bytes: i64::try_from(bytes.len()).unwrap_or(i64::MAX),
            mtime_utc: mtime,
            imported_at: now,
            status: SourceStatus::Ok,
            error_code: None,
        };

        match parse_activity(&bytes) {
            Ok(raw) => {
                let source_id = upsert_source_file(&db, &source)?;
                store_activity(&db, source_id, &raw, now, opts, &mut report)?;
                if known.is_some() {
                    report.updated += 1;
                } else {
                    report.new += 1;
                }
            }
            Err(e) => {
                let is_monitoring = matches!(e, FitError::NoActivity);
                source.status = if is_monitoring {
                    SourceStatus::NotActivity
                } else {
                    SourceStatus::Error
                };
                source.error_code = Some(e.code().to_owned());
                let source_id = upsert_source_file(&db, &source)?;
                // Eine Datei, die früher eine Aktivität war, jetzt aber nicht mehr lesbar
                // ist, hinterlässt keine alte Aktivität.
                delete_activity_of_source(&db, source_id)?;
                if is_monitoring {
                    report.not_activity += 1;
                } else {
                    report.errors.push(FileIssue {
                        rel_path: file.rel_path,
                        code: e.code().to_owned(),
                    });
                }
            }
        }
    }

    report.duplicates = duplicates::apply(conn, &opts.duplicates)?;
    set_derived_version(conn, DERIVED_CACHE_VERSION)?;
    let activities: i64 = conn.query_row("SELECT COUNT(*) FROM activity", [], |r| r.get(0))?;
    report.primary = usize::try_from(activities).unwrap_or(0) - report.duplicates;
    Ok(report)
}

/// Eingang der Berechnungen aus den Rohdaten einer Datei.
pub(crate) fn activity_input(raw: &RawActivity) -> ActivityInput {
    ActivityInput {
        sport: raw.sport,
        start_utc: raw.start_utc,
        local_offset_s: raw.local_offset_s,
        samples: raw.samples.clone(),
        // Die RPE kommt von Hand (manual_session), nicht aus der FIT-Datei.
        rpe: None,
        pool_length_m: raw.session.pool_length_m,
    }
}

/// Speichert Aktivität, Belastung, Zeit in Zonen und Bestleistungen einer gelesenen Datei.
fn store_activity(
    db: &Db<'_>,
    source_file_id: i64,
    raw: &RawActivity,
    now: DateTime<Utc>,
    opts: &ImportOptions,
    report: &mut ImportReport,
) -> Result<()> {
    let input = activity_input(raw);
    // Einmal geprüft für Kennzahlen, Belastung, Zonen und Bestleistung (T38).
    let prepared = prepare(&input, opts);
    let row = activity_row(source_file_id, raw, &input, prepared.as_ref());
    // Alte Belastungen einer geänderten Datei fallen weg, die Aktivität wird aktualisiert.
    let activity_id = upsert_activity(db, &row)?;
    delete_activity_children(db, activity_id)?;

    let prepared = prepared.as_ref();
    let status = write_load(db, activity_id, &input, prepared, row.local_date, now, opts)?;
    report.count_load(status);
    write_zone_times(db, activity_id, &input, prepared, row.local_date, opts)?;
    write_best_power(db, activity_id, &input, prepared, opts)?;
    write_best_effort(db, activity_id, &input, prepared, opts)?;
    Ok(())
}

/// Kennzahlen der Aktivität aus den Rohdaten. Statistiken über Leistung und Herzfrequenz
/// zählen nur Werte, die die Regeln aus SPEC 6.5 behalten. `prepared` ist das Ergebnis von
/// `check_plausibility` für die Samples (`None` bei gescheiterter Prüfung, dann gibt es
/// keine Statistiken).
fn activity_row(
    source_file_id: i64,
    raw: &RawActivity,
    input: &ActivityInput,
    prepared: Option<&PreparedSamples>,
) -> ActivityRow {
    let segments: &[Vec<Sample>] = prepared.map_or(&[], |p| p.segments.as_slice());
    let valid = |pick: fn(&Sample) -> Option<f32>| -> Vec<f32> {
        segments.iter().flatten().filter_map(pick).collect()
    };
    let power = valid(|s| s.power_w);
    let hr = valid(|s| s.hr_bpm);

    ActivityRow {
        source_file_id,
        sport: raw.sport,
        sub_sport: raw.sub_sport.clone(),
        start_utc: raw.start_utc,
        local_offset_s: raw.local_offset_s,
        local_date: input.local_date(),
        duration_s: raw.duration_s().map_or(0, |d| d.round() as i64),
        moving_s: raw.session.total_timer_time_s.map(|d| d.round() as i64),
        distance_m: raw.session.total_distance_m,
        // Höhenmeter liest der Parser noch nicht (nicht Teil der Belastung).
        ascent_m: None,
        avg_hr: mean(&hr),
        max_hr: hr.iter().copied().reduce(f32::max).map(f64::from),
        avg_power_w: mean(&power),
        np_w: np_over_segments(segments).map(f64::from),
        device_serial: raw.device_serial,
        device_product: raw.device_product.clone(),
        has_power: !power.is_empty(),
        has_hr: !hr.is_empty(),
    }
}

fn mean(v: &[f32]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    Some(v.iter().map(|x| f64::from(*x)).sum::<f64>() / v.len() as f64)
}

/// Normalisierte Leistung über alle Abschnitte (Q16, Sekundärquelle): Summe der vierten
/// Potenzen aller vollen 30-Sekunden-Fenster geteilt durch ihre Anzahl, dann vierte Wurzel.
/// Dieselbe Zusammenfassung wie in `lastwerk_core::compute_load`, damit Fenster nicht über
/// Pausen laufen. Je Abschnitt liefert `normalized_power` die NP, das Gewicht ist die Zahl
/// der Fenster (`n - 29`).
fn np_over_segments(segments: &[Vec<Sample>]) -> Option<f32> {
    let mut sum4 = 0.0_f64;
    let mut windows = 0_usize;
    for seg in segments {
        let p: Vec<f32> = seg.iter().filter_map(|s| s.power_w).collect();
        let Some(np) = normalized_power(&p) else {
            continue;
        };
        let w = p.len() - (lastwerk_core::NP_WINDOW_S - 1);
        sum4 += w as f64 * f64::from(np).powi(4);
        windows += w;
    }
    (windows > 0).then(|| (sum4 / windows as f64).powf(0.25) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_known_strings() {
        // Prüfwerte aus FIPS 180-4 (Anhang B der Beispiele): "abc" und die leere Eingabe.
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    fn seg(range: std::ops::Range<u32>, w: f32) -> Vec<Sample> {
        range
            .map(|t| Sample {
                power_w: Some(w),
                ..Sample::at(t)
            })
            .collect()
    }

    /// Zwei Abschnitte konstanter Leistung: NP ist das Leistungsmittel der Fenster. Abschnitt
    /// A: 60 Werte mit 100 W, 31 Fenster. Abschnitt B: 40 Werte mit 200 W, 11 Fenster.
    /// Summe der vierten Potenzen: 31 * 100^4 + 11 * 200^4 = 3,1e9 + 1,76e10 = 2,07e10,
    /// geteilt durch 42 Fenster = 4,92857e8, vierte Wurzel = 149,0 (gerundet).
    #[test]
    fn np_combines_segments_by_window_count() {
        let segments = vec![seg(0..60, 100.0), seg(100..140, 200.0)];
        let np = np_over_segments(&segments).unwrap();
        let expected = (2.07e10_f64 / 42.0).powf(0.25) as f32;
        assert!((np - expected).abs() < 0.01, "{np} gegen {expected}");
        assert!((np - 149.0).abs() < 0.2);
    }

    #[test]
    fn np_of_short_segments_is_none() {
        assert_eq!(np_over_segments(&[seg(0..29, 100.0)]), None);
    }

    /// T33: Eine Datenbank mit älteren abgeleiteten Werten liest beim nächsten Import jede
    /// bekannte Datei neu ein, danach gilt sie als aktuell und überspringt wieder.
    #[test]
    fn stale_derived_version_reimports_known_files_once() {
        let dir =
            std::env::temp_dir().join(format!("lastwerk-cache-version-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["power_constant_200w_3600s", "no_power"] {
            let f = lastwerk_testkit::fixtures::by_name(name).unwrap();
            std::fs::write(dir.join(f.file_name()), &f.bytes).unwrap();
        }
        let now = DateTime::from_timestamp(1_790_000_000, 0).unwrap();
        let opts = ImportOptions::default();
        let mut store = Store::open_in_memory().unwrap();
        assert!(
            !store.cache_outdated().unwrap(),
            "leere Datenbank ist nie veraltet"
        );

        let first = store.import_dir(&dir, now, &opts, &mut |_| {}).unwrap();
        assert_eq!(first.new, 2);
        assert!(!store.cache_outdated().unwrap());
        assert_eq!(derived_version(&store.conn).unwrap(), DERIVED_CACHE_VERSION);
        let again = store.import_dir(&dir, now, &opts, &mut |_| {}).unwrap();
        assert_eq!(again.unchanged, 2);

        // Stand vor T33: keine Zeile in cache_meta, nur die fünf Eckdauern gespeichert.
        store.conn.execute("DELETE FROM cache_meta", []).unwrap();
        store
            .conn
            .execute(
                "DELETE FROM activity_best_power WHERE duration_s NOT IN (5, 60, 300, 1200, 3600)",
                [],
            )
            .unwrap();
        assert!(store.cache_outdated().unwrap());
        let ids_before: Vec<i64> = store.activities().unwrap().iter().map(|a| a.id).collect();
        let upgrade = store.import_dir(&dir, now, &opts, &mut |_| {}).unwrap();
        assert_eq!(upgrade.updated, 2);
        assert!(!store.cache_outdated().unwrap());
        // Gleiche Aktivitäten (gleiche IDs), jetzt mit dem ganzen Raster bis 3600 s.
        let ids_after: Vec<i64> = store.activities().unwrap().iter().map(|a| a.id).collect();
        assert_eq!(ids_before, ids_after);
        assert_eq!(store.best_powers().unwrap().len(), 20);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
