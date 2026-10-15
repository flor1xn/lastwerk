//! Seite "Leistung" (T33, SPEC 4.3 F35, 4.2 F40): Leistungskurve von 1 s bis 2 h für einen
//! Zeitraum gegen den gleich langen Zeitraum davor, mit Datum und Aktivität jedes Bestwerts.
//!
//! Die Bestwerte je Aktivität stehen im Cache (`activity_best_power`, Raster
//! `POWER_CURVE_DURATIONS_S`). Diese Schicht nimmt je Dauer das Maximum über die Radeinheiten des
//! Zeitraums und vergleicht über `core::metrics::power_curve`. Nur Rad: Leistung beim Laufen ist
//! eine andere Größe, die Kurve würde sonst vermischt (wie im Dashboard).
//!
//! Bestzeiten im Laufen (T37) kommen aus `activity_best_effort`: je Strecke die kürzeste Zeit
//! der Läufe im Zeitraum, dazu die kürzeste im Vorzeitraum.
//!
//! Schwellen (T36): die gesetzten Werte bis heute und je Kalendermonat des Zeitraums der
//! FTP-Vorschlag aus der besten 20-Minuten-Leistung (`suggest_ftp`, SPEC 8.6). Übernommen wird
//! hier nichts; das bleibt eine ausdrückliche Handlung in den Einstellungen.

use std::collections::HashMap;

use chrono::{NaiveDate, TimeDelta};
use lastwerk_core::metrics::change_percent;
use lastwerk_core::metrics::power_curve::{compare_power_curves_over, power_curve_trend_code};
use lastwerk_core::suggest_ftp;
use lastwerk_core::{BEST_EFFORT_DISTANCES_M, BestPower, POWER_CURVE_DURATIONS_S, Sport};
use lastwerk_store::{Store, ThresholdKind};

use crate::dto::{PerfBest, PerfFtpSuggestion, PerfPower, PerfRunBest, PerfThreshold, Performance};
use crate::error::{AppError, Result};
use crate::service::parse_date;

/// Zeiträume der Seite in Tagen; `all` heißt alles bis heute, ohne Vorzeitraum.
pub const RANGES: [&str; 4] = ["42", "90", "365", "all"];

fn round1(v: f64) -> Option<f64> {
    v.is_finite().then(|| (v * 10.0).round() / 10.0)
}

/// Bestwert einer Dauer mit Herkunft.
#[derive(Clone, Copy)]
struct Best {
    watts: f64,
    date: NaiveDate,
    activity_id: i64,
}

/// Zeitraum aus der Auswahl: (erster Tag des Zeitraums, erster Tag des Vorzeitraums).
fn bounds(today: NaiveDate, range: &str) -> Result<Option<(NaiveDate, NaiveDate)>> {
    if range == "all" {
        return Ok(None);
    }
    if !RANGES.contains(&range) {
        return Err(AppError::new("invalid_value"));
    }
    let days: i64 = range.parse().map_err(|_| AppError::new("invalid_value"))?;
    let from = today - TimeDelta::days(days - 1);
    Ok(Some((from, from - TimeDelta::days(days))))
}

pub fn performance(store: &Store, today: &str, range: &str) -> Result<Performance> {
    let today = parse_date(today)?;
    let window = bounds(today, range)?;

    // Radeinheiten, die zählen (SPEC 6.4), mit ihrem Tag.
    let rides: HashMap<i64, NaiveDate> = store
        .activities()?
        .into_iter()
        .filter(|a| a.is_primary && a.row.sport == Sport::Cycling && a.row.local_date <= today)
        .map(|a| (a.id, a.row.local_date))
        .collect();

    let mut current: HashMap<u32, Best> = HashMap::new();
    let mut previous: Vec<BestPower> = Vec::new();
    for b in store.best_powers()? {
        let Some(&date) = rides.get(&b.activity_id) else {
            continue;
        };
        if !(b.watts.is_finite() && b.watts > 0.0) {
            continue;
        }
        let in_current = window.is_none_or(|(from, _)| date >= from);
        if in_current {
            let candidate = Best {
                watts: b.watts,
                date,
                activity_id: b.activity_id,
            };
            // Bei Gleichstand gewinnt der frühere Tag: Er hat den Wert zuerst erreicht.
            current
                .entry(b.duration_s)
                .and_modify(|e| {
                    if candidate.watts > e.watts
                        || (candidate.watts == e.watts && candidate.date < e.date)
                    {
                        *e = candidate;
                    }
                })
                .or_insert(candidate);
        } else if let Some((from, previous_from)) = window
            && date >= previous_from
            && date < from
        {
            previous.push(BestPower {
                duration_s: b.duration_s,
                watts: b.watts as f32,
            });
        }
    }

    let current_list: Vec<BestPower> = current
        .iter()
        .map(|(&duration_s, b)| BestPower {
            duration_s,
            watts: b.watts as f32,
        })
        .collect();
    let power = compare_power_curves_over(&POWER_CURVE_DURATIONS_S, &current_list, &previous)
        .into_iter()
        .map(|c| PerfPower {
            duration_s: c.duration_s,
            current: current.get(&c.duration_s).and_then(|b| {
                round1(b.watts).map(|watts| PerfBest {
                    value: watts,
                    date: b.date.to_string(),
                    activity_id: b.activity_id,
                })
            }),
            previous_w: c.previous_w.and_then(|w| round1(f64::from(w))),
            change_percent: c.change_percent.and_then(round1),
            trend: power_curve_trend_code(c.trend).to_owned(),
        })
        .collect();

    let run_bests = run_bests(store, today, window)?;
    let thresholds = threshold_history(store, today)?;
    let ftp_suggestions = ftp_suggestions(store, &rides, window)?;

    Ok(Performance {
        today: today.to_string(),
        range: range.to_owned(),
        from: window.map(|(from, _)| from.to_string()),
        previous_from: window.map(|(_, p)| p.to_string()),
        power,
        run_bests,
        thresholds,
        ftp_suggestions,
    })
}

/// Arten im Verlauf: die Schwellen, aus denen Belastung und Zonen folgen.
const HISTORY_KINDS: [ThresholdKind; 4] = [
    ThresholdKind::FtpW,
    ThresholdKind::LthrBpm,
    ThresholdKind::RunSpeedMps,
    ThresholdKind::CssMps,
];

fn threshold_history(store: &Store, today: NaiveDate) -> Result<Vec<PerfThreshold>> {
    let mut rows: Vec<_> = store
        .list_thresholds()?
        .into_iter()
        .filter(|t| HISTORY_KINDS.contains(&t.kind) && t.valid_from <= today)
        .collect();
    rows.sort_by(|a, b| {
        (a.kind.as_str(), a.valid_from, a.id).cmp(&(b.kind.as_str(), b.valid_from, b.id))
    });
    Ok(rows
        .into_iter()
        .map(|t| PerfThreshold {
            kind: t.kind,
            sport: t.sport,
            value: t.value,
            valid_from: t.valid_from.to_string(),
            origin: t.origin.as_str().to_owned(),
        })
        .collect())
}

/// Dauer des Tests für den FTP-Vorschlag: 20 Minuten (SPEC 8.6).
const FTP_TEST_S: u32 = 1200;

fn ftp_suggestions(
    store: &Store,
    rides: &HashMap<i64, NaiveDate>,
    window: Option<(NaiveDate, NaiveDate)>,
) -> Result<Vec<PerfFtpSuggestion>> {
    // Je Monat die beste 20-Minuten-Leistung; bei Gleichstand der frühere Tag.
    let mut best: std::collections::BTreeMap<String, (f64, NaiveDate, i64)> =
        std::collections::BTreeMap::new();
    for b in store.best_powers()? {
        if b.duration_s != FTP_TEST_S || !(b.watts.is_finite() && b.watts > 0.0) {
            continue;
        }
        let Some(&date) = rides.get(&b.activity_id) else {
            continue;
        };
        if window.is_some_and(|(from, _)| date < from) {
            continue;
        }
        let month = date.format("%Y-%m").to_string();
        let better =
            |old: &(f64, NaiveDate, i64)| b.watts > old.0 || (b.watts == old.0 && date < old.1);
        if best.get(&month).is_none_or(better) {
            best.insert(month, (b.watts, date, b.activity_id));
        }
    }
    Ok(best
        .into_iter()
        .filter_map(|(month, (watts, date, activity_id))| {
            let ftp = suggest_ftp(watts as f32)?;
            Some(PerfFtpSuggestion {
                month,
                value_w: round1(f64::from(ftp))?,
                best_20min_w: round1(watts)?,
                date: date.to_string(),
                activity_id,
            })
        })
        .collect())
}

/// Kürzeste Zeit je Strecke im Zeitraum (mit Herkunft) und im Vorzeitraum.
fn run_bests(
    store: &Store,
    today: NaiveDate,
    window: Option<(NaiveDate, NaiveDate)>,
) -> Result<Vec<PerfRunBest>> {
    let runs: HashMap<i64, NaiveDate> = store
        .activities()?
        .into_iter()
        .filter(|a| a.is_primary && a.row.sport == Sport::Running && a.row.local_date <= today)
        .map(|a| (a.id, a.row.local_date))
        .collect();
    let mut current: HashMap<u32, (u32, NaiveDate, i64)> = HashMap::new();
    let mut previous: HashMap<u32, u32> = HashMap::new();
    for e in store.best_efforts()? {
        let Some(&date) = runs.get(&e.activity_id) else {
            continue;
        };
        if e.seconds == 0 {
            continue;
        }
        if window.is_none_or(|(from, _)| date >= from) {
            let better = |old: &(u32, NaiveDate, i64)| {
                e.seconds < old.0 || (e.seconds == old.0 && date < old.1)
            };
            if current.get(&e.distance_m).is_none_or(better) {
                current.insert(e.distance_m, (e.seconds, date, e.activity_id));
            }
        } else if let Some((from, previous_from)) = window
            && date >= previous_from
            && date < from
        {
            let slot = previous.entry(e.distance_m).or_insert(e.seconds);
            *slot = (*slot).min(e.seconds);
        }
    }
    Ok(BEST_EFFORT_DISTANCES_M
        .iter()
        .map(|&d| {
            let cur = current.get(&d);
            let prev = previous.get(&d).map(|&s| f64::from(s));
            PerfRunBest {
                distance_m: d,
                current: cur.map(|&(seconds, date, activity_id)| PerfBest {
                    value: f64::from(seconds),
                    date: date.to_string(),
                    activity_id,
                }),
                previous_s: prev,
                change_percent: change_percent(cur.map(|c| f64::from(c.0)), prev).and_then(round1),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn ranges_cover_whole_days_back_from_today() {
        // 90 Tage bis einschließlich 15. Oktober: ab 18. Juli, Vorzeitraum ab 19. April.
        assert_eq!(
            bounds(d(2026, 10, 15), "90").unwrap(),
            Some((d(2026, 7, 18), d(2026, 4, 19)))
        );
        assert_eq!(bounds(d(2026, 10, 15), "all").unwrap(), None);
        assert!(bounds(d(2026, 10, 15), "7").is_err());
        assert!(bounds(d(2026, 10, 15), "").is_err());
    }
}
