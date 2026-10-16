//! Seite "Gesundheit" (T39, SPEC 5.6): Tages- und Trainingswerte aus Garmin über 30, 90 oder
//! 365 Tage, je Tag eine Zeile. Hier wird nur gelesen, zusammengeführt und gerundet. Garmins
//! akute Last steht nur zur Anzeige da (Entscheidung E2), die Werte beschreiben Trainingsdaten,
//! keine Gesundheit im medizinischen Sinn (SPEC 2.3).

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use lastwerk_store::{
    SETTING_GARMIN_SHARE_HYDRATION, SensitiveDay, Store, TrainingDay, WellnessDay,
};

use crate::dto::{HealthPage, HealthPoint};
use crate::error::{AppError, Result};
use crate::service::parse_date;

/// Zeiträume der Seite in Tagen (wie die Seite "Erholung").
pub const HEALTH_RANGES: [u32; 3] = [30, 90, 365];

fn round(v: Option<f64>, digits: i32) -> Option<f64> {
    v.filter(|v| v.is_finite()).map(|v| {
        let f = 10f64.powi(digits);
        (v * f).round() / f
    })
}

pub fn health(store: &Store, today: &str, days: u32) -> Result<HealthPage> {
    let today = parse_date(today)?;
    if !HEALTH_RANGES.contains(&days) {
        return Err(AppError::new("invalid_value"));
    }
    let from = today - TimeDelta::days(i64::from(days) - 1);
    let released = store
        .get_setting(SETTING_GARMIN_SHARE_HYDRATION)?
        .as_deref()
        == Some("true");
    let wellness: BTreeMap<NaiveDate, WellnessDay> = store
        .garmin_rows::<WellnessDay>(from, today)?
        .into_iter()
        .map(|r| (r.date, r))
        .collect();
    let training: BTreeMap<NaiveDate, TrainingDay> = store
        .garmin_rows::<TrainingDay>(from, today)?
        .into_iter()
        .map(|r| (r.date, r))
        .collect();
    // S3 erscheint nur, solange die Freigabe gilt, auch wenn noch Zeilen da sind.
    let sensitive: BTreeMap<NaiveDate, SensitiveDay> = if released {
        store
            .garmin_rows::<SensitiveDay>(from, today)?
            .into_iter()
            .map(|r| (r.date, r))
            .collect()
    } else {
        BTreeMap::new()
    };
    let mut dates: Vec<NaiveDate> = wellness
        .keys()
        .chain(training.keys())
        .chain(sensitive.keys())
        .copied()
        .collect();
    dates.sort();
    dates.dedup();
    let points = dates
        .into_iter()
        .map(|date| {
            let w = wellness.get(&date);
            let t = training.get(&date);
            let h = sensitive.get(&date);
            HealthPoint {
                date: date.to_string(),
                steps: round(w.and_then(|w| w.steps), 0),
                step_distance_km: round(w.and_then(|w| w.step_distance_m).map(|m| m / 1000.0), 2),
                hr_min_bpm: round(w.and_then(|w| w.hr_min_bpm), 0),
                hr_max_bpm: round(w.and_then(|w| w.hr_max_bpm), 0),
                respiration_waking_brpm: round(w.and_then(|w| w.respiration_waking_brpm), 1),
                respiration_sleep_brpm: round(w.and_then(|w| w.respiration_sleep_brpm), 1),
                spo2_avg_pct: round(w.and_then(|w| w.spo2_avg_pct), 0),
                spo2_low_pct: round(w.and_then(|w| w.spo2_low_pct), 0),
                intensity_moderate_min: round(w.and_then(|w| w.intensity_moderate_min), 0),
                intensity_vigorous_min: round(w.and_then(|w| w.intensity_vigorous_min), 0),
                stress_avg: round(w.and_then(|w| w.stress_avg), 0),
                stress_max: round(w.and_then(|w| w.stress_max), 0),
                readiness_score: round(t.and_then(|t| t.readiness_score), 0),
                readiness_level: t.and_then(|t| t.readiness_level.clone()),
                garmin_acute_load: round(t.and_then(|t| t.garmin_acute_load), 0),
                fitness_age: round(t.and_then(|t| t.fitness_age), 1),
                vo2max_running: round(t.and_then(|t| t.vo2max_running), 1),
                vo2max_cycling: round(t.and_then(|t| t.vo2max_cycling), 1),
                endurance_score: round(t.and_then(|t| t.endurance_score), 0),
                hill_score: round(t.and_then(|t| t.hill_score), 0),
                hydration_ml: round(h.and_then(|h| h.hydration_ml), 0),
            }
        })
        .collect();
    Ok(HealthPage {
        days,
        points,
        sensitive_released: released,
    })
}

/// "S3-Werte löschen" in den Einstellungen. Gibt die Zahl der gelöschten Tage zurück.
pub fn delete_sensitive(store: &mut Store) -> Result<u32> {
    Ok(store.delete_sensitive_days()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use lastwerk_store::GARMIN_APP_SOURCE;

    fn at() -> DateTime<Utc> {
        DateTime::from_timestamp(1_790_000_000, 0).unwrap()
    }

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn days_are_merged_rounded_and_s3_only_with_release() {
        let mut store = Store::open_in_memory().unwrap();
        let mut w = WellnessDay::empty(d("2026-10-10"), GARMIN_APP_SOURCE, at());
        w.steps = Some(8123.0);
        w.step_distance_m = Some(6512.0);
        w.respiration_sleep_brpm = Some(12.46);
        let mut t = TrainingDay::empty(d("2026-10-12"), GARMIN_APP_SOURCE, at());
        t.vo2max_running = Some(52.04);
        let mut h = SensitiveDay::empty(d("2026-10-11"), GARMIN_APP_SOURCE, at());
        h.hydration_ml = Some(1500.0);
        store.merge_garmin_rows(&[w]).unwrap();
        store.merge_garmin_rows(&[t]).unwrap();
        store.merge_garmin_rows(&[h]).unwrap();

        let page = health(&store, "2026-10-16", 30).unwrap();
        assert!(!page.sensitive_released);
        let dates: Vec<&str> = page.points.iter().map(|p| p.date.as_str()).collect();
        assert_eq!(dates, ["2026-10-10", "2026-10-12"]);
        assert_eq!(page.points[0].step_distance_km, Some(6.51));
        assert_eq!(page.points[0].respiration_sleep_brpm, Some(12.5));
        assert_eq!(page.points[0].hr_min_bpm, None);
        assert_eq!(page.points[1].vo2max_running, Some(52.0));

        store
            .set_setting(SETTING_GARMIN_SHARE_HYDRATION, "true")
            .unwrap();
        let page = health(&store, "2026-10-16", 30).unwrap();
        assert!(page.sensitive_released);
        assert_eq!(page.points.len(), 3);
        assert_eq!(page.points[1].hydration_ml, Some(1500.0));

        assert_eq!(delete_sensitive(&mut store).unwrap(), 1);
        assert_eq!(health(&store, "2026-10-16", 30).unwrap().points.len(), 2);
        assert!(health(&store, "2026-10-16", 31).is_err());
    }
}
