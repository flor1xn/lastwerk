//! Seite "Erholung" (T34, SPEC 4.2 F20, F41): HRV, Ruhepuls, Schlaf und Body Battery über 30,
//! 90 oder 365 Tage, mit der eigenen Basislinie und der Untergrenze aus R12 je Tag, dazu die
//! Tageslast auf derselben Zeitachse.
//!
//! Basislinie, Untergrenze und das Urteil "niedrig" je Tag rechnet `core::metrics::readiness::
//! baseline_series` mit derselben Logik wie R12 und das Dashboard. Hier wird nur gelesen,
//! zusammengeführt, gemittelt (Mittel der letzten sieben Tage und des Zeitraums, beschreibend)
//! und gerundet. Die Werte beschreiben Trainingsdaten, keine Gesundheit (SPEC 2.3).

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use lastwerk_cli::planning;
use lastwerk_core::metrics::readiness::{baseline_series, readiness_state, recovery_deviation};
use lastwerk_core::plan::RecoveryDay;
use lastwerk_store::{RecoveryDayRecord, Store};

use crate::dto::{RecoveryPage, RecoveryPagePoint, RecoverySummary};
use crate::error::{AppError, Result};
use crate::service::parse_date;

/// Zeiträume der Seite in Tagen.
pub const RECOVERY_RANGES: [u32; 3] = [30, 90, 365];
/// Tage, über die das jüngere Mittel läuft (beschreibend, Konvention wie die Woche).
pub const RECENT_DAYS: i64 = 7;

fn round(v: f64, digits: i32) -> Option<f64> {
    v.is_finite().then(|| {
        let f = 10f64.powi(digits);
        (v * f).round() / f
    })
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values
        .filter(|v| v.is_finite())
        .fold((0.0, 0u32), |(s, n), v| (s + v, n + 1));
    (n > 0).then(|| sum / f64::from(n))
}

fn sleep_h(r: &RecoveryDayRecord) -> Option<f64> {
    r.sleep_duration_s
        .filter(|s| *s > 0)
        .map(|s| f64::from(s) / 3600.0)
}

pub fn recovery(store: &Store, today: &str, days: u32) -> Result<RecoveryPage> {
    let today = parse_date(today)?;
    if !RECOVERY_RANGES.contains(&days) {
        return Err(AppError::new("invalid_value"));
    }
    let (rules, _) = planning::load_rules().map_err(|_| AppError::new("internal"))?;
    let from = today - TimeDelta::days(i64::from(days) - 1);
    // Für die Basislinie des ersten Tages braucht es die `baseline_days` Tage davor.
    let lookback = from - TimeDelta::days(i64::from(rules.r12.baseline_days));
    let records = store.recovery_days(lookback, today)?;
    let core_days: Vec<RecoveryDay> = records.iter().map(RecoveryDay::from).collect();
    let bases = baseline_series(&core_days, from, today, &rules);
    let by_date: BTreeMap<NaiveDate, &RecoveryDayRecord> =
        records.iter().map(|r| (r.date, r)).collect();
    let loads: BTreeMap<NaiveDate, f32> = store
        .daily_loads(from, today)?
        .into_iter()
        .map(|d| (d.date, d.load))
        .collect();

    let points: Vec<RecoveryPagePoint> = bases
        .iter()
        .map(|b| {
            let r = by_date.get(&b.date);
            RecoveryPagePoint {
                date: b.date.to_string(),
                hrv_ms: r.and_then(|r| r.hrv_overnight_ms).and_then(|v| round(v, 1)),
                resting_hr_bpm: r.and_then(|r| r.resting_hr_bpm).and_then(|v| round(v, 1)),
                sleep_h: r.and_then(|r| sleep_h(r)).and_then(|v| round(v, 2)),
                sleep_score: r.and_then(|r| r.sleep_score),
                body_battery_high: r.and_then(|r| r.body_battery_high),
                body_battery_low: r.and_then(|r| r.body_battery_low),
                hrv_base: b.hrv_base.and_then(|v| round(v, 1)),
                hrv_floor: b.hrv_floor.and_then(|v| round(v, 1)),
                resting_hr_base: b.resting_hr_base.and_then(|v| round(v, 1)),
                sleep_base_h: b.sleep_base_s.and_then(|v| round(v / 3600.0, 2)),
                sleep_floor_h: b.sleep_floor_s.and_then(|v| round(v / 3600.0, 2)),
                low: b.low,
                load: loads
                    .get(&b.date)
                    .map(|l| f64::from(*l))
                    .filter(|l| *l > 0.0)
                    .and_then(|l| round(l, 1)),
            }
        })
        .collect();

    let in_period: Vec<&RecoveryDayRecord> = records.iter().filter(|r| r.date >= from).collect();
    let recent_from = today - TimeDelta::days(RECENT_DAYS - 1);
    let summary =
        |metric: &str, value: &dyn Fn(&RecoveryDayRecord) -> Option<f64>, digits| RecoverySummary {
            metric: metric.to_owned(),
            recent: mean(
                in_period
                    .iter()
                    .filter(|r| r.date >= recent_from)
                    .filter_map(|r| value(r)),
            )
            .and_then(|v| round(v, digits)),
            period: mean(in_period.iter().filter_map(|r| value(r))).and_then(|v| round(v, digits)),
        };
    let summaries = vec![
        summary("hrv", &|r| r.hrv_overnight_ms, 1),
        summary("resting_hr", &|r| r.resting_hr_bpm, 1),
        summary("sleep", &sleep_h, 2),
        summary("sleep_score", &|r| r.sleep_score.map(f64::from), 0),
        summary(
            "body_battery_high",
            &|r| r.body_battery_high.map(f64::from),
            0,
        ),
    ];

    let dev = recovery_deviation(&core_days, today, &rules);
    Ok(RecoveryPage {
        today: today.to_string(),
        days,
        state: readiness_state(dev.as_ref(), &rules).code().to_owned(),
        last_date: in_period.last().map(|r| r.date.to_string()),
        low_days: u32::try_from(points.iter().filter(|p| p.low).count()).unwrap_or(u32::MAX),
        hrv_drop_percent: round(f64::from(rules.r12.hrv_drop_fraction) * 100.0, 0),
        sleep_deficit_percent: round(f64::from(rules.r12.sleep_deficit_fraction) * 100.0, 0),
        points,
        summary: summaries,
    })
}
