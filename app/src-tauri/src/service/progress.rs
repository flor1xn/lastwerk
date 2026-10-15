//! Fortschritt nach Woche, Monat und Jahr (T32, SPEC 4.1 F7, 4.2 F39).
//!
//! Ein Aufruf liefert je Tag und Sportart die Summen der zählenden Aktivitäten (SPEC 6.4):
//! Zahl, Dauer, Strecke, Last und die Zahl der Einheiten ohne berechenbare Last. Der Zeitraum
//! reicht vom früheren der beiden Tage "1. Januar des Vorjahres" und "Monatserster 23 Monate
//! vor heute" bis heute, damit die Oberfläche 52 Wochen, 24 Monate und den Vergleich mit dem
//! Vorjahr bilden kann. Hier wird nur summiert und gruppiert, keine Formel; Wochen, Monate und
//! Summenkurven bildet die Oberfläche aus diesen Tageszeilen (`lib/progress.ts`).

use std::collections::BTreeMap;

use chrono::{Datelike, Months, NaiveDate};
use lastwerk_store::{LoadRow, Store};

use crate::dto::{Progress, ProgressEntry};
use crate::error::Result;
use crate::service::{load_info, parse_date, sport_rank};

/// Monate, die die Monatstabelle zurückreicht (einschließlich des laufenden).
pub const PROGRESS_MONTHS: u32 = 24;

/// Erster Tag des Zeitraums für `today` (siehe Modulkopf).
pub fn progress_from(today: NaiveDate) -> NaiveDate {
    let previous_year_start =
        NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap_or(NaiveDate::MIN);
    let month_start = today.with_day(1).unwrap_or(today);
    let months_back = month_start
        .checked_sub_months(Months::new(PROGRESS_MONTHS - 1))
        .unwrap_or(NaiveDate::MIN);
    previous_year_start.min(months_back)
}

#[derive(Default)]
struct Acc {
    count: u32,
    duration_s: i64,
    distance_m: f64,
    load: f64,
    load_missing: u32,
}

/// Tagessummen je Sportart von [`progress_from`] bis `today`.
pub fn progress(store: &Store, today: &str) -> Result<Progress> {
    let today = parse_date(today)?;
    let from = progress_from(today);
    let activities = store.activities_between(from, today)?;
    let loads = store.loads_between(from, today)?;
    let mut by_activity: BTreeMap<i64, Vec<&LoadRow>> = BTreeMap::new();
    for l in &loads {
        by_activity.entry(l.activity_id).or_default().push(l);
    }

    let mut sums: BTreeMap<(NaiveDate, usize), Acc> = BTreeMap::new();
    for a in activities.iter().filter(|a| a.is_primary) {
        let acc = sums
            .entry((a.row.local_date, sport_rank(a.row.sport)))
            .or_default();
        acc.count += 1;
        acc.duration_s += a.row.duration_s.max(0);
        if let Some(d) = a.row.distance_m.filter(|d| d.is_finite() && *d >= 0.0) {
            acc.distance_m += d;
        }
        let info = load_info(by_activity.get(&a.id).map_or(&[][..], Vec::as_slice));
        match (info.status.as_str(), info.value) {
            ("ok", Some(v)) if v.is_finite() => acc.load += v,
            _ => acc.load_missing += 1,
        }
    }

    let entries = sums
        .into_iter()
        .map(|((date, rank), acc)| ProgressEntry {
            date: date.to_string(),
            sport: super::SPORT_ORDER
                .get(rank)
                .copied()
                .unwrap_or(lastwerk_core::Sport::Other),
            count: acc.count,
            duration_s: acc.duration_s,
            distance_m: (acc.distance_m * 10.0).round() / 10.0,
            load: (acc.load * 10.0).round() / 10.0,
            load_missing: acc.load_missing,
        })
        .collect();

    Ok(Progress {
        today: today.to_string(),
        from: from.to_string(),
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn range_starts_at_the_earlier_of_previous_year_and_24_months() {
        // 15. Oktober 2026: Vorjahr ab 1. Januar 2025, 24 Monate ab 1. November 2024.
        assert_eq!(progress_from(d(2026, 10, 15)), d(2024, 11, 1));
        // 31. Dezember 2026: 24 Monate ab 1. Januar 2025, gleich dem Vorjahresbeginn.
        assert_eq!(progress_from(d(2026, 12, 31)), d(2025, 1, 1));
        // 1. Januar 2026: 24 Monate ab 1. Februar 2024, früher als der 1. Januar 2025.
        assert_eq!(progress_from(d(2026, 1, 1)), d(2024, 2, 1));
    }
}
