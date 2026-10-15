//! Gemeinsame Hilfen der Integrationstests. Alle Dateien liegen unter `target/`, es gibt
//! keine echten Daten und nichts außerhalb des Build-Ordners.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use lastwerk_core::Sport;
use lastwerk_store::{
    ImportOptions, ImportReport, NewThreshold, Store, ThresholdKind, ThresholdOrigin,
};
use lastwerk_testkit::fixtures;

/// Fester Importzeitpunkt, damit `imported_at` und `computed_at` deterministisch sind.
pub fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap()
}

pub fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

/// Leerer Arbeitsordner unter `target/tmp` für einen Test.
pub fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("store-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Schreibt alle Fixtures in `dir/fixtures` und gibt den Ordner zurück.
pub fn write_fixtures(dir: &Path) -> PathBuf {
    let out = dir.join("fixtures");
    fs::create_dir_all(&out).unwrap();
    for f in fixtures::all() {
        fs::write(out.join(f.file_name()), &f.bytes).unwrap();
    }
    out
}

pub fn import(store: &mut Store, dir: &Path) -> ImportReport {
    store
        .import_dir(dir, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap()
}

pub fn threshold(
    sport: Option<Sport>,
    kind: ThresholdKind,
    value: f64,
    from: NaiveDate,
) -> NewThreshold {
    NewThreshold {
        sport,
        kind,
        value,
        valid_from: from,
        origin: ThresholdOrigin::Manual,
        note: None,
    }
}

/// Schwellen, mit denen alle Methoden der Fixtures rechnen können: FTP 250 und LTHR 160 für
/// Rad, Laufschwelle 3,5 m/s, CSS 1,2 m/s, Ruhepuls 50 und Maximalpuls 190 für alle, alle ab
/// 1. Januar 2026. Dazu das Geschlecht für hrTSS.
pub fn add_thresholds(store: &mut Store) {
    let from = date(2026, 1, 1);
    let cyc = Some(Sport::Cycling);
    for t in [
        threshold(cyc, ThresholdKind::FtpW, 250.0, from),
        threshold(cyc, ThresholdKind::LthrBpm, 160.0, from),
        threshold(Some(Sport::Running), ThresholdKind::RunSpeedMps, 3.5, from),
        threshold(Some(Sport::Swimming), ThresholdKind::CssMps, 1.2, from),
        threshold(None, ThresholdKind::HrRestBpm, 50.0, from),
        threshold(None, ThresholdKind::HrMaxBpm, 190.0, from),
        threshold(None, ThresholdKind::LthrBpm, 160.0, from),
    ] {
        store.insert_threshold(&t).unwrap();
    }
    store
        .set_setting(lastwerk_store::SETTING_ATHLETE_SEX, "male")
        .unwrap();
}
