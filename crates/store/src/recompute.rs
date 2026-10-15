//! Neuberechnung von Belastung und Zeit in Zonen nach einer geänderten Schwelle (SPEC 3, 6.3).
//!
//! Der Cache speichert keine Messpunkte (SPEC 6.1). Eine Neuberechnung braucht deshalb die
//! FIT-Dateien. [`Store::recompute_loads`] liest sie aus dem Ordner, prüft über die
//! SHA-256-Summe, dass die Datei noch die ist, aus der die Aktivität stammt, und rechnet
//! nur die Aktivitäten im betroffenen Zeitraum neu. Alles andere bleibt unberührt, eine
//! Schwelle mit späterem Datum ändert also keine früheren Belastungen.

use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use lastwerk_core::Sport;
use lastwerk_fit::{parse_activity, read_fit_file};
use rusqlite::params;

use crate::derived::{delete_threshold_dependent, prepare, write_load, write_zone_times};
use crate::error::{Result, StoreError};
use crate::import::{ImportOptions, activity_input, sha256_hex};
use crate::rows::{LoadStatus, fmt_date, parse_date, sport_to_str};
use crate::statements::Db;
use crate::store::Store;

/// Ergebnis einer Neuberechnung.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecomputeReport {
    /// Aktivitäten im betroffenen Zeitraum.
    pub candidates: usize,
    /// Davon neu berechnet.
    pub recomputed: usize,
    /// Dateien, die im Ordner fehlen oder sich nicht lesen lassen. Ihre Aktivitäten bleiben
    /// unverändert.
    pub file_missing: usize,
    /// Dateien, deren Inhalt sich seit dem Import geändert hat. Ihre Aktivitäten bleiben
    /// unverändert, ein neuer Import ersetzt sie.
    pub file_changed: usize,
    pub loads_ok: usize,
    pub loads_missing_threshold: usize,
    pub loads_missing_channel: usize,
    pub loads_implausible: usize,
}

impl RecomputeReport {
    fn count_load(&mut self, s: LoadStatus) {
        match s {
            LoadStatus::Ok => self.loads_ok += 1,
            LoadStatus::MissingThreshold => self.loads_missing_threshold += 1,
            LoadStatus::MissingChannel => self.loads_missing_channel += 1,
            LoadStatus::Implausible => self.loads_implausible += 1,
        }
    }
}

/// Pfad einer Quelldatei unter `dir`. `rel_path` ist mit `/` getrennt und relativ. Pfade mit
/// `..`, Laufwerken oder Wurzel werden abgelehnt, auch wenn sie nur durch eine fremde
/// Datenbank hineingeraten sind.
fn source_path(dir: &Path, rel_path: &str) -> Option<PathBuf> {
    let mut path = dir.to_path_buf();
    for part in rel_path.split('/') {
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(c)), None) => path.push(c),
            _ => return None,
        }
    }
    Some(path)
}

impl Store {
    /// Rechnet Belastung und Zeit in Zonen der Aktivitäten neu, deren lokales Datum in
    /// `from..=to` liegt (`to = None` heißt ohne Ende) und deren Sportart zu `sport` passt
    /// (`None` heißt alle). Die Schwellenwerte kommen aus der Datenbank, jede Aktivität
    /// bekommt die, die an ihrem Tag gelten (SPEC 5.4). Die Messpunkte liest der Speicher aus
    /// den Dateien in `dir`. Alles geschieht in einer Transaktion.
    ///
    /// Entscheidung in T05 (SPEC 5.4): Der Cache hält keine Messpunkte, deshalb braucht die
    /// Neuberechnung den Ordner. Ohne Ordner empfiehlt die CLI `rebuild`.
    pub fn recompute_loads(
        &mut self,
        dir: &Path,
        sport: Option<Sport>,
        from: NaiveDate,
        to: Option<NaiveDate>,
        now: DateTime<Utc>,
        opts: &ImportOptions,
    ) -> Result<RecomputeReport> {
        if !dir.is_dir() {
            return Err(StoreError::NotADirectory);
        }
        let tx = self.conn.transaction()?;
        let candidates: Vec<(i64, String, NaiveDate, String)> = {
            let mut stmt = tx.prepare(
                "SELECT a.id, f.rel_path, a.local_date, f.sha256 \
                 FROM activity a JOIN source_file f ON f.id = a.source_file_id \
                 WHERE a.local_date >= ?1 AND (?2 IS NULL OR a.local_date <= ?2) \
                   AND (?3 IS NULL OR a.sport = ?3) \
                 ORDER BY a.id",
            )?;
            stmt.query_map(
                params![fmt_date(from), to.map(fmt_date), sport.map(sport_to_str)],
                |r| {
                    let date: String = r.get(2)?;
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        crate::error::conversion(2, parse_date(&date))?,
                        r.get(3)?,
                    ))
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut report = RecomputeReport {
            candidates: candidates.len(),
            ..RecomputeReport::default()
        };
        // Vorbereitete Anweisungen für alle Aktivitäten des Laufs (T38, Anhang Messung).
        let db = Db::new(&tx);
        for (activity_id, rel_path, local_date, sha256) in candidates {
            let Some(path) = source_path(dir, &rel_path) else {
                report.file_missing += 1;
                continue;
            };
            let Ok(bytes) = read_fit_file(&path) else {
                report.file_missing += 1;
                continue;
            };
            if sha256_hex(&bytes) != sha256 {
                report.file_changed += 1;
                continue;
            }
            let Ok(raw) = parse_activity(&bytes) else {
                report.file_changed += 1;
                continue;
            };
            let input = activity_input(&raw);
            delete_threshold_dependent(&db, activity_id)?;
            let prepared = prepare(&input, opts);
            let status = write_load(
                &db,
                activity_id,
                &input,
                prepared.as_ref(),
                local_date,
                now,
                opts,
            )?;
            report.count_load(status);
            write_zone_times(
                &db,
                activity_id,
                &input,
                prepared.as_ref(),
                local_date,
                opts,
            )?;
            report.recomputed += 1;
        }
        drop(db);
        tx.commit()?;
        Ok(report)
    }
}
