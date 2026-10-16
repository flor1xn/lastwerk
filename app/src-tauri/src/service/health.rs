//! Seite "Gesundheit" (T39, SPEC 5.6): Tages- und Trainingswerte aus Garmin über 30, 90 oder
//! 365 Tage, je Tag eine Zeile. Hier wird nur gelesen, zusammengeführt und gerundet. Garmins
//! akute Last steht nur zur Anzeige da (Entscheidung E2), die Werte beschreiben Trainingsdaten,
//! keine Gesundheit im medizinischen Sinn (SPEC 2.3).

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use lastwerk_store::{
    BodyDay, BpDay, DayRecord, NutritionDay, SensitiveCategory, SensitiveDay, Store, TrainingDay,
    WellnessDay,
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
    let mut released = Vec::new();
    for c in SensitiveCategory::ALL {
        if store.sensitive_released(c)? {
            released.push(c);
        }
    }
    let wellness = by_date::<WellnessDay>(store, from, today, true)?;
    let training = by_date::<TrainingDay>(store, from, today, true)?;
    // S3 erscheint nur, solange die Freigabe des Bereichs gilt, auch wenn noch Zeilen da sind.
    let on = |c| released.contains(&c);
    let sensitive = by_date::<SensitiveDay>(store, from, today, on(SensitiveCategory::Hydration))?;
    let body = by_date::<BodyDay>(store, from, today, on(SensitiveCategory::Body))?;
    let bp = by_date::<BpDay>(store, from, today, on(SensitiveCategory::BloodPressure))?;
    let food = by_date::<NutritionDay>(store, from, today, on(SensitiveCategory::Nutrition))?;
    let mut dates: Vec<NaiveDate> = wellness
        .keys()
        .chain(training.keys())
        .chain(sensitive.keys())
        .chain(body.keys())
        .chain(bp.keys())
        .chain(food.keys())
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
            let b = body.get(&date);
            let p = bp.get(&date);
            let n = food.get(&date);
            let km = |m: Option<f64>| m.map(|m| m / 1000.0);
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
                floors_up: round(w.and_then(|w| w.floors_up), 0),
                floors_down: round(w.and_then(|w| w.floors_down), 0),
                training_status: t.and_then(|t| t.training_status.clone()),
                garmin_chronic_load: round(t.and_then(|t| t.garmin_chronic_load), 0),
                garmin_acwr: round(t.and_then(|t| t.garmin_acwr), 2),
                load_aerobic_low: round(t.and_then(|t| t.load_aerobic_low), 0),
                load_aerobic_low_min: round(t.and_then(|t| t.load_aerobic_low_min), 0),
                load_aerobic_low_max: round(t.and_then(|t| t.load_aerobic_low_max), 0),
                load_aerobic_high: round(t.and_then(|t| t.load_aerobic_high), 0),
                load_aerobic_high_min: round(t.and_then(|t| t.load_aerobic_high_min), 0),
                load_aerobic_high_max: round(t.and_then(|t| t.load_aerobic_high_max), 0),
                load_anaerobic: round(t.and_then(|t| t.load_anaerobic), 0),
                load_anaerobic_min: round(t.and_then(|t| t.load_anaerobic_min), 0),
                load_anaerobic_max: round(t.and_then(|t| t.load_anaerobic_max), 0),
                load_balance: t.and_then(|t| t.load_balance.clone()),
                race_5k_s: round(t.and_then(|t| t.race_5k_s), 0),
                race_10k_s: round(t.and_then(|t| t.race_10k_s), 0),
                race_half_s: round(t.and_then(|t| t.race_half_s), 0),
                race_marathon_s: round(t.and_then(|t| t.race_marathon_s), 0),
                run_acute_distance_km: round(km(t.and_then(|t| t.run_acute_distance_m)), 1),
                run_impact_load: round(t.and_then(|t| t.run_impact_load), 0),
                run_tolerance_km: round(km(t.and_then(|t| t.run_tolerance_m)), 1),
                run_tolerance_level: t.and_then(|t| t.run_tolerance_level.clone()),
                weight_kg: round(b.and_then(|b| b.weight_kg), 1),
                bmi: round(b.and_then(|b| b.bmi), 1),
                body_fat_pct: round(b.and_then(|b| b.body_fat_pct), 1),
                body_water_pct: round(b.and_then(|b| b.body_water_pct), 1),
                bone_kg: round(b.and_then(|b| b.bone_kg), 1),
                muscle_kg: round(b.and_then(|b| b.muscle_kg), 1),
                systolic_mmhg: round(p.and_then(|p| p.systolic_mmhg), 0),
                diastolic_mmhg: round(p.and_then(|p| p.diastolic_mmhg), 0),
                pulse_bpm: round(p.and_then(|p| p.pulse_bpm), 0),
                calories_kcal: round(n.and_then(|n| n.calories_kcal), 0),
                carbs_g: round(n.and_then(|n| n.carbs_g), 0),
                fat_g: round(n.and_then(|n| n.fat_g), 0),
                protein_g: round(n.and_then(|n| n.protein_g), 0),
                calories_goal_kcal: round(n.and_then(|n| n.calories_goal_kcal), 0),
            }
        })
        .collect();
    Ok(HealthPage {
        days,
        points,
        sensitive_released: !released.is_empty(),
        released: released.iter().map(|c| c.as_str().to_owned()).collect(),
    })
}

/// Zeilen einer Tabelle nach Datum, leer, wenn `read` nicht gesetzt ist.
fn by_date<T: DayRecord + Dated>(
    store: &Store,
    from: NaiveDate,
    to: NaiveDate,
    read: bool,
) -> Result<BTreeMap<NaiveDate, T>> {
    if !read {
        return Ok(BTreeMap::new());
    }
    Ok(store
        .garmin_rows::<T>(from, to)?
        .into_iter()
        .map(|r| (r.date(), r))
        .collect())
}

/// Datum einer Tageszeile.
trait Dated {
    fn date(&self) -> NaiveDate;
}

macro_rules! dated {
    ($($t:ty),*) => {
        $(impl Dated for $t {
            fn date(&self) -> NaiveDate {
                self.date
            }
        })*
    };
}
dated!(
    WellnessDay,
    TrainingDay,
    SensitiveDay,
    BodyDay,
    BpDay,
    NutritionDay
);

/// "Löschen" der S3-Werte in den Einstellungen: ein Bereich (`hydration`, `body`,
/// `blood_pressure`, `nutrition`) oder ohne Angabe alle. Gibt die Zahl der gelöschten Tage
/// zurück.
pub fn delete_sensitive(store: &mut Store, category: Option<&str>) -> Result<u32> {
    match category {
        None => Ok(store.delete_sensitive_days()?),
        Some(c) => {
            let c = SensitiveCategory::parse(c).ok_or_else(|| AppError::new("invalid_value"))?;
            Ok(store.delete_sensitive(c)?)
        }
    }
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
            .set_setting(SensitiveCategory::Hydration.setting(), "true")
            .unwrap();
        let page = health(&store, "2026-10-16", 30).unwrap();
        assert!(page.sensitive_released);
        assert_eq!(page.released, ["hydration"]);
        assert_eq!(page.points.len(), 3);
        assert_eq!(page.points[1].hydration_ml, Some(1500.0));

        // Ein Gewicht erscheint erst mit der Freigabe "Körper", nicht mit der der Hydration.
        let mut b = BodyDay::empty(d("2026-10-11"), GARMIN_APP_SOURCE, at());
        b.weight_kg = Some(72.44);
        store.merge_garmin_rows(&[b]).unwrap();
        assert_eq!(
            health(&store, "2026-10-16", 30).unwrap().points[1].weight_kg,
            None
        );
        store
            .set_setting(SensitiveCategory::Body.setting(), "true")
            .unwrap();
        let page = health(&store, "2026-10-16", 30).unwrap();
        assert_eq!(page.points[1].weight_kg, Some(72.4));
        assert_eq!(page.released, ["hydration", "body"]);

        assert!(delete_sensitive(&mut store, Some("menstrual")).is_err());
        assert_eq!(delete_sensitive(&mut store, Some("body")).unwrap(), 1);
        assert_eq!(delete_sensitive(&mut store, None).unwrap(), 1);
        assert_eq!(health(&store, "2026-10-16", 30).unwrap().points.len(), 2);
        assert!(health(&store, "2026-10-16", 31).is_err());
    }
}
