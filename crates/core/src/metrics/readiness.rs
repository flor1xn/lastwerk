//! Erholungsabweichung: HRV, Ruhepuls und Schlafdauer gegen das eigene Mittel (SPEC 4.2 F36,
//! 8.4 R12, Anhang A Q6).
//!
//! Die Basislogik gehört zu R12 und wird hier nur aufgerufen (`plan::recovery::recovery_drops`):
//! Basislinie ist das Mittel der `baseline_days` Tage vor dem jüngsten Erholungstag, gefordert
//! sind mindestens `min_baseline_days` Tage mit Wert, ein Tag ist veraltet nach `max_age_days`.
//! Alle diese Werte stehen in `R12` der Regeln (Faustregeln). Q6 (Plews u. a.) belegt die
//! HRV-Überwachung, das 7-Tage-Mittel als Basislinie stammt nur aus einem Sekundärbericht (O8).
//!
//! Das Dashboard und der Planer kommen damit bei gleichen Daten zum selben Urteil: Die Deutung
//! [`ReadinessState::Low`] gilt genau dann, wenn R12 auslöst (HRV oder Schlafdauer um die
//! Anteile aus den Regeln unter der Basislinie). Der Ruhepuls wird angezeigt, löst aber nichts
//! aus: R12 kennt ihn nicht, und eine Schwelle dafür ließe sich nicht belegen.
//!
//! Die Werte beschreiben Trainingsdaten und sagen nichts über Gesundheit (SPEC 2.3).

use chrono::{NaiveDate, TimeDelta};
use serde::{Deserialize, Serialize};

use crate::plan::recovery::{baselines, param, recovery_drops};
use crate::plan::{RecoveryDay, Rules};

/// Abweichung der Werte des jüngsten Erholungstags von der eigenen Basislinie in Prozent der
/// Basislinie. Negativ heißt unter der Basislinie. `None` heißt nicht berechenbar (Wert fehlt
/// oder die Basislinie hat zu wenig Tage).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReadinessDeviation {
    /// Jüngster Erholungstag, auf den sich die Abweichungen beziehen.
    pub date: NaiveDate,
    pub hrv_percent: Option<f64>,
    pub resting_hr_percent: Option<f64>,
    pub sleep_percent: Option<f64>,
}

/// Abweichungen des jüngsten Erholungstags bis einschließlich `today` gegen die eigene Basislinie.
/// `None`, wenn es keinen Erholungstag gibt oder der jüngste älter als `R12.max_age_days` ist.
pub fn recovery_deviation(
    days: &[RecoveryDay],
    today: NaiveDate,
    rules: &Rules,
) -> Option<ReadinessDeviation> {
    let after_today = today.checked_add_signed(TimeDelta::days(1))?;
    let d = recovery_drops(days, after_today, today, &rules.r12)?;
    // Anteil darunter (positiv) wird zur Abweichung (negativ = darunter).
    let pct = |drop: Option<f64>| drop.map(|x| -x * 100.0);
    Some(ReadinessDeviation {
        date: d.date,
        hrv_percent: pct(d.hrv),
        resting_hr_percent: pct(d.resting_hr),
        sleep_percent: pct(d.sleep),
    })
}

/// Basislinie und Untergrenze eines Tages für die Seite "Erholung" (T34). Die Untergrenzen sind
/// die Werte, ab denen R12 auslöst: Basislinie mal (1 minus `hrv_drop_fraction`) bzw.
/// (1 minus `sleep_deficit_fraction`), beide aus den Regeln (Faustregeln). Der Ruhepuls hat keine
/// Untergrenze (R12 kennt ihn nicht). Schlaf in Sekunden.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BaselinePoint {
    pub date: NaiveDate,
    pub hrv_base: Option<f64>,
    pub hrv_floor: Option<f64>,
    pub resting_hr_base: Option<f64>,
    pub sleep_base_s: Option<f64>,
    pub sleep_floor_s: Option<f64>,
    /// HRV oder Schlafdauer des Tages liegen auf oder unter ihrer Untergrenze; dieselbe Regel wie
    /// [`readiness_state`] (`Low`), nur für jeden Tag statt für den jüngsten.
    pub low: bool,
}

/// Basislinien und Untergrenzen für jeden Tag von `from` bis `to`.
pub fn baseline_series(
    days: &[RecoveryDay],
    from: NaiveDate,
    to: NaiveDate,
    rules: &Rules,
) -> Vec<BaselinePoint> {
    let r = &rules.r12;
    let hrv_keep = 1.0 - param(r.hrv_drop_fraction);
    let sleep_keep = 1.0 - param(r.sleep_deficit_fraction);
    let by_date: std::collections::BTreeMap<NaiveDate, &RecoveryDay> =
        days.iter().map(|d| (d.date, d)).collect();
    baselines(days, from, to, r)
        .into_iter()
        .map(|(date, b)| {
            let day = by_date.get(&date);
            let hrv = day
                .and_then(|d| d.hrv_overnight_ms)
                .map(f64::from)
                .filter(|v| v.is_finite() && *v > 0.0);
            let sleep = day
                .and_then(|d| d.sleep_duration_s)
                .filter(|s| *s > 0)
                .map(f64::from);
            let hrv_floor = b.hrv.map(|m| m * hrv_keep);
            let sleep_floor = b.sleep.map(|m| m * sleep_keep);
            // "Ab" der Schwelle wie R12: Wert auf oder unter der Untergrenze (kleine Toleranz für
            // die Rundung, wie in `readiness_state`).
            let under = |v: Option<f64>, floor: Option<f64>| match (v, floor) {
                (Some(v), Some(f)) => v <= f + 1e-9 * f.abs().max(1.0),
                _ => false,
            };
            BaselinePoint {
                date,
                hrv_base: b.hrv,
                hrv_floor,
                resting_hr_base: b.resting_hr,
                sleep_base_s: b.sleep,
                sleep_floor_s: sleep_floor,
                low: under(hrv, hrv_floor) || under(sleep, sleep_floor),
            }
        })
        .collect()
}

/// Deutung der Erholungsabweichung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    /// Weder HRV noch Schlafdauer lassen sich gegen eine Basislinie messen.
    NoData,
    /// Weder HRV noch Schlafdauer liegen um die Schwelle von R12 unter der Basislinie.
    Normal,
    /// HRV oder Schlafdauer liegen mindestens um die Schwelle von R12 unter der Basislinie.
    Low,
}

impl ReadinessState {
    /// Stabiler Code, Schlüssel für Texte in `catalog/locales`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoData => "readiness.no_data",
            Self::Normal => "readiness.normal",
            Self::Low => "readiness.low",
        }
    }
}

/// Deutet die Abweichungen mit den Schwellen von R12 (`hrv_drop_fraction`,
/// `sleep_deficit_fraction`, beide Faustregeln). Die Grenzen gelten "ab" und mit derselben
/// Rundung der Parameter wie in R12, damit beide an derselben Stelle umschlagen.
pub fn readiness_state(dev: Option<&ReadinessDeviation>, rules: &Rules) -> ReadinessState {
    let Some(d) = dev else {
        return ReadinessState::NoData;
    };
    if d.hrv_percent.is_none() && d.sleep_percent.is_none() {
        return ReadinessState::NoData;
    }
    let below = |percent: Option<f64>, fraction: f32| {
        percent.is_some_and(|p| -p / 100.0 >= param(fraction) - 1e-9)
    };
    if below(d.hrv_percent, rules.r12.hrv_drop_fraction)
        || below(d.sleep_percent, rules.r12.sleep_deficit_fraction)
    {
        ReadinessState::Low
    } else {
        ReadinessState::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::default_rules;
    use crate::plan::recovery_signal;

    fn day(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 2, 1).unwrap() + TimeDelta::days(n)
    }

    fn rd(n: i64, hrv: Option<f32>, rhr: Option<f32>, sleep_s: Option<u32>) -> RecoveryDay {
        RecoveryDay {
            date: day(n),
            hrv_overnight_ms: hrv,
            resting_hr_bpm: rhr,
            sleep_duration_s: sleep_s,
            sleep_score: None,
        }
    }

    /// Sieben Tage Basislinie (Tag 0 bis 6): HRV 60 ms, Ruhepuls 50 bpm, Schlaf 25 200 s
    /// (7 h), dann der Tag `latest` (Tag 7) mit den übergebenen Werten.
    fn series(hrv: f32, rhr: f32, sleep_s: u32) -> Vec<RecoveryDay> {
        let mut v: Vec<RecoveryDay> = (0..7)
            .map(|n| rd(n, Some(60.0), Some(50.0), Some(25_200)))
            .collect();
        v.push(rd(7, Some(hrv), Some(rhr), Some(sleep_s)));
        v
    }

    /// T34, Handrechnung mit den Regeln (R12: 7 Tage Basis, mindestens 4, HRV ab 15 Prozent,
    /// Schlaf ab 20 Prozent darunter): Tag 7 hat Basis HRV 60 ms, Untergrenze 60 * 0,85 = 51 ms,
    /// Schlaf 25 200 s, Untergrenze 25 200 * 0,8 = 20 160 s, Ruhepuls 50 bpm. Tag 3 hat erst
    /// drei Tage davor, also keine Basis; Tag 4 die ersten vier.
    const BASE_HRV: f64 = 60.0;
    const FLOOR_HRV: f64 = 51.0;
    const FLOOR_SLEEP_S: f64 = 20_160.0;

    #[test]
    fn baseline_series_matches_hand_calculation_and_r12() {
        let rules = default_rules();
        let days = series(51.0, 55.0, 25_200);
        let s = baseline_series(&days, day(3), day(8), &rules);
        assert_eq!(s.len(), 6);
        assert_eq!(s[0].hrv_base, None);
        assert_eq!(s[1].hrv_base, Some(BASE_HRV));
        let d7 = s.iter().find(|p| p.date == day(7)).unwrap();
        assert!((d7.hrv_floor.unwrap() - FLOOR_HRV).abs() < 1e-6);
        assert!((d7.sleep_floor_s.unwrap() - FLOOR_SLEEP_S).abs() < 1e-6);
        assert_eq!(d7.resting_hr_base, Some(50.0));
        // HRV genau auf der Untergrenze: wie R12 "ab 15 Prozent", also niedrig.
        assert!(d7.low);
        assert!(!s.iter().find(|p| p.date == day(6)).unwrap().low);
        // Gleiches Urteil wie die Deutung des jüngsten Tages.
        let dev = recovery_deviation(&days, day(7), &rules);
        assert_eq!(readiness_state(dev.as_ref(), &rules), ReadinessState::Low);
        // Tag 8 hat keinen eigenen Wert: Basis ja, niedrig nein.
        let d8 = s.iter().find(|p| p.date == day(8)).unwrap();
        assert!(d8.hrv_base.is_some() && !d8.low);
    }

    #[test]
    fn deviations_are_percent_of_the_own_baseline() {
        let rules = default_rules();
        // HRV 51 gegen 60: (51 - 60) / 60 = -0,15, also -15 Prozent.
        // Ruhepuls 55 gegen 50: (55 - 50) / 50 = +0,10, also +10 Prozent.
        // Schlaf 20 160 s gegen 25 200 s: (20 160 - 25 200) / 25 200 = -0,2, also -20 Prozent.
        let dev = recovery_deviation(&series(51.0, 55.0, 20_160), day(7), &rules).unwrap();
        assert_eq!(dev.date, day(7));
        assert!((dev.hrv_percent.unwrap() + 15.0).abs() < 1e-6);
        assert!((dev.resting_hr_percent.unwrap() - 10.0).abs() < 1e-6);
        assert!((dev.sleep_percent.unwrap() + 20.0).abs() < 1e-6);
    }

    #[test]
    fn the_state_flips_exactly_where_r12_does() {
        let rules = default_rules();
        // Wie die R12-Tests (SPEC 8.4): 51 ms gegen 60 ms löst aus, 52 ms nicht.
        let low = recovery_deviation(&series(51.0, 50.0, 25_200), day(7), &rules);
        assert_eq!(readiness_state(low.as_ref(), &rules), ReadinessState::Low);
        let ok = recovery_deviation(&series(52.0, 50.0, 25_200), day(7), &rules);
        assert_eq!(readiness_state(ok.as_ref(), &rules), ReadinessState::Normal);
        // 20 160 s gegen 25 200 s löst aus, 20 400 s nicht.
        let low = recovery_deviation(&series(60.0, 50.0, 20_160), day(7), &rules);
        assert_eq!(readiness_state(low.as_ref(), &rules), ReadinessState::Low);
        let ok = recovery_deviation(&series(60.0, 50.0, 20_400), day(7), &rules);
        assert_eq!(readiness_state(ok.as_ref(), &rules), ReadinessState::Normal);
    }

    #[test]
    fn dashboard_state_and_r12_agree_on_many_inputs() {
        let rules = default_rules();
        for hrv in [40.0, 50.0, 51.0, 51.5, 52.0, 60.0, 75.0] {
            for sleep in [10_000, 20_160, 20_161, 20_400, 25_200, 30_000] {
                let days = series(hrv, 50.0, sleep);
                let signal = recovery_signal(&days, day(8), &rules.r12).is_some();
                let dev = recovery_deviation(&days, day(7), &rules);
                let state = readiness_state(dev.as_ref(), &rules);
                assert_eq!(
                    state == ReadinessState::Low,
                    signal,
                    "hrv {hrv}, sleep {sleep}"
                );
            }
        }
    }

    #[test]
    fn resting_hr_alone_triggers_nothing_and_is_no_data_for_the_state() {
        let rules = default_rules();
        let mut days: Vec<RecoveryDay> = (0..7).map(|n| rd(n, None, Some(50.0), None)).collect();
        days.push(rd(7, None, Some(70.0), None));
        let dev = recovery_deviation(&days, day(7), &rules).unwrap();
        assert!((dev.resting_hr_percent.unwrap() - 40.0).abs() < 1e-6);
        assert_eq!(dev.hrv_percent, None);
        assert_eq!(dev.sleep_percent, None);
        assert_eq!(readiness_state(Some(&dev), &rules), ReadinessState::NoData);
    }

    #[test]
    fn missing_stale_or_thin_data_gives_no_deviation() {
        let rules = default_rules();
        assert_eq!(recovery_deviation(&[], day(7), &rules), None);
        assert_eq!(readiness_state(None, &rules), ReadinessState::NoData);
        // Älter als max_age_days (3): Tag 7 ist der jüngste, heute ist Tag 11.
        assert_eq!(
            recovery_deviation(&series(51.0, 50.0, 25_200), day(11), &rules),
            None
        );
        // Genau drei Tage alt zählt noch.
        assert!(recovery_deviation(&series(51.0, 50.0, 25_200), day(10), &rules).is_some());
        // Tage nach "heute" zählen nicht.
        let future = recovery_deviation(&series(51.0, 50.0, 25_200), day(6), &rules).unwrap();
        assert_eq!(future.date, day(6));
        // Basislinie mit nur drei Tagen (Minimum 4): keine Abweichung.
        let thin = vec![
            rd(4, Some(60.0), None, None),
            rd(5, Some(60.0), None, None),
            rd(6, Some(60.0), None, None),
            rd(7, Some(30.0), None, None),
        ];
        let dev = recovery_deviation(&thin, day(7), &rules).unwrap();
        assert_eq!(dev.hrv_percent, None);
        assert_eq!(readiness_state(Some(&dev), &rules), ReadinessState::NoData);
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(ReadinessState::Low.code(), "readiness.low");
        assert_eq!(ReadinessState::Normal.code(), "readiness.normal");
        assert_eq!(ReadinessState::NoData.code(), "readiness.no_data");
    }
}
