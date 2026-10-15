//! Kennzahlen für `lastwerk plausibility` (SPEC 11.5).
//!
//! Die Zusammenfassung enthält nur Zahlen: Anzahlen, Spannweiten, Anteile. Keine Pfade,
//! Daten, Seriennummern oder Namen, damit die Ausgabe auf echten Daten nichts Privates
//! preisgibt. Sie wird nicht gespeichert.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, NaiveDate};

use crate::error::Result;
use crate::rows::{LoadStatus, SourceStatus, sport_to_str};
use crate::store::Store;

/// Verhältnis NP zu Durchschnittsleistung, ab dem eine Aktivität auffällt. Faustregel:
/// Wechselnde Fahrten liegen meist zwischen 1,0 und 1,3, über 1,5 deutet auf Spitzen,
/// Messfehler oder falsche Pausen hin. Keine Quelle, nur zum Hinschauen.
pub const NP_RATIO_FLAG: f64 = 1.5;

/// Kleinster und größter Wert einer Kennzahl.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

impl Range {
    fn of(values: impl Iterator<Item = f64>) -> Option<Self> {
        values.fold(None, |acc, v| match acc {
            None => Some(Range { min: v, max: v }),
            Some(r) => Some(Range {
                min: r.min.min(v),
                max: r.max.max(v),
            }),
        })
    }
}

/// Kennzahlen eines Importlaufs.
#[derive(Debug, Clone, PartialEq)]
pub struct PlausibilitySummary {
    pub files_total: usize,
    pub files_ok: usize,
    pub files_not_activity: usize,
    pub files_error: usize,
    /// Fehlercodes mit Häufigkeit, nach Code sortiert.
    pub error_codes: Vec<(String, usize)>,
    pub activities: usize,
    pub primary: usize,
    /// Aktivitäten, die als Duplikat einer anderen gelten. Jede bildet mit ihrer primären
    /// Aktivität ein Paar.
    pub duplicate_pairs: usize,
    /// Größter Startunterschied innerhalb der Paare in Sekunden.
    pub duplicate_max_start_diff_s: Option<i64>,
    /// Größter Dauerunterschied innerhalb der Paare in Prozent der längeren Dauer.
    pub duplicate_max_duration_diff_pct: Option<f64>,
    /// Primäre Aktivitäten je Sportart, nach Name sortiert.
    pub by_sport: Vec<(String, usize)>,
    /// Spannweiten über die primären Aktivitäten (Sekunden, Meter, bpm, Watt).
    pub duration_s: Option<Range>,
    pub distance_m: Option<Range>,
    pub avg_hr: Option<Range>,
    pub avg_power_w: Option<Range>,
    pub np_w: Option<Range>,
    /// Primäre Aktivitäten mit einer Belastung im Status `ok`.
    pub load_computable: usize,
    /// Primäre Aktivitäten ohne berechenbare Belastung, mit Status und Häufigkeit.
    pub load_not_computable: Vec<(String, usize)>,
    pub load_value: Option<Range>,
    /// Summe der Tageslasten je Kalenderjahr.
    pub load_per_year: Vec<(i32, f64)>,
    /// Kleinste und größte Wochenlast (ISO-Wochen mit mindestens einer Belastung).
    pub load_per_week: Option<Range>,
    /// Spannweite von NP geteilt durch Durchschnittsleistung.
    pub np_ratio: Option<Range>,
    /// Aktivitäten mit einem Verhältnis über [`NP_RATIO_FLAG`].
    pub np_ratio_flagged: usize,
}

impl PlausibilitySummary {
    /// Anteil der primären Aktivitäten ohne berechenbare Belastung, 0 bis 1. `None` ohne
    /// primäre Aktivität.
    pub fn share_not_computable(&self) -> Option<f64> {
        (self.primary > 0).then(|| 1.0 - self.load_computable as f64 / self.primary as f64)
    }
}

impl Store {
    /// Kennzahlen über den Inhalt der Datenbank (SPEC 11.5).
    pub fn plausibility_summary(&self) -> Result<PlausibilitySummary> {
        let files = self.source_files()?;
        let activities = self.activities()?;
        let loads = self.activity_loads()?;

        let mut error_codes: BTreeMap<String, usize> = BTreeMap::new();
        for f in files.iter().filter(|f| f.row.status == SourceStatus::Error) {
            *error_codes
                .entry(f.row.error_code.clone().unwrap_or_default())
                .or_default() += 1;
        }

        let by_id: HashMap<i64, &crate::rows::ActivityRecord> =
            activities.iter().map(|a| (a.id, a)).collect();
        let primary: Vec<_> = activities.iter().filter(|a| a.is_primary).collect();

        let mut pairs = 0;
        let mut max_start: Option<i64> = None;
        let mut max_dur: Option<f64> = None;
        for d in activities.iter().filter(|a| !a.is_primary) {
            let Some(p) = d.duplicate_of.and_then(|id| by_id.get(&id)) else {
                continue;
            };
            pairs += 1;
            let start = (d.row.start_utc - p.row.start_utc).num_seconds().abs();
            max_start = Some(max_start.map_or(start, |m| m.max(start)));
            let longer = d.row.duration_s.max(p.row.duration_s);
            if longer > 0 {
                let pct =
                    100.0 * (d.row.duration_s - p.row.duration_s).abs() as f64 / longer as f64;
                max_dur = Some(max_dur.map_or(pct, |m| m.max(pct)));
            }
        }

        let mut by_sport: BTreeMap<&str, usize> = BTreeMap::new();
        for a in &primary {
            *by_sport.entry(sport_to_str(a.row.sport)).or_default() += 1;
        }

        // Je primärer Aktivität zählt die Belastung mit Status ok, sonst der Status der
        // vorhandenen Zeile.
        let mut load_of: HashMap<i64, (LoadStatus, Option<f64>)> = HashMap::new();
        for l in &loads {
            let e = load_of.entry(l.activity_id).or_insert((l.status, l.value));
            if l.status == LoadStatus::Ok {
                *e = (l.status, l.value);
            }
        }
        let mut computable = 0;
        let mut not_computable: BTreeMap<&str, usize> = BTreeMap::new();
        let mut load_values = Vec::new();
        for a in &primary {
            match load_of.get(&a.id) {
                Some((LoadStatus::Ok, v)) => {
                    computable += 1;
                    load_values.extend(*v);
                }
                Some((s, _)) => *not_computable.entry(s.as_str()).or_default() += 1,
                None => *not_computable.entry("missing_row").or_default() += 1,
            }
        }

        let daily = self.daily_loads(
            NaiveDate::from_ymd_opt(1900, 1, 1).unwrap_or(NaiveDate::MIN),
            NaiveDate::from_ymd_opt(2200, 1, 1).unwrap_or(NaiveDate::MAX),
        )?;
        let mut per_year: BTreeMap<i32, f64> = BTreeMap::new();
        let mut per_week: BTreeMap<(i32, u32), f64> = BTreeMap::new();
        for d in &daily {
            *per_year.entry(d.date.year()).or_default() += f64::from(d.load);
            let w = d.date.iso_week();
            *per_week.entry((w.year(), w.week())).or_default() += f64::from(d.load);
        }

        let ratios: Vec<f64> = primary
            .iter()
            .filter_map(|a| match (a.row.np_w, a.row.avg_power_w) {
                (Some(np), Some(avg)) if avg > 0.0 => Some(np / avg),
                _ => None,
            })
            .collect();

        let range_of = |pick: fn(&crate::rows::ActivityRow) -> Option<f64>| {
            Range::of(primary.iter().filter_map(|a| pick(&a.row)))
        };

        Ok(PlausibilitySummary {
            files_total: files.len(),
            files_ok: files
                .iter()
                .filter(|f| f.row.status == SourceStatus::Ok)
                .count(),
            files_not_activity: files
                .iter()
                .filter(|f| f.row.status == SourceStatus::NotActivity)
                .count(),
            files_error: files
                .iter()
                .filter(|f| f.row.status == SourceStatus::Error)
                .count(),
            error_codes: error_codes.into_iter().collect(),
            activities: activities.len(),
            primary: primary.len(),
            duplicate_pairs: pairs,
            duplicate_max_start_diff_s: max_start,
            duplicate_max_duration_diff_pct: max_dur,
            by_sport: by_sport
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
            duration_s: range_of(|r| Some(r.duration_s as f64)),
            distance_m: range_of(|r| r.distance_m),
            avg_hr: range_of(|r| r.avg_hr),
            avg_power_w: range_of(|r| r.avg_power_w),
            np_w: range_of(|r| r.np_w),
            load_computable: computable,
            load_not_computable: not_computable
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
            load_value: Range::of(load_values.into_iter()),
            load_per_year: per_year.into_iter().collect(),
            load_per_week: Range::of(per_week.into_values()),
            np_ratio: Range::of(ratios.iter().copied()),
            np_ratio_flagged: ratios.iter().filter(|r| **r > NP_RATIO_FLAG).count(),
        })
    }
}
