//! Rampe: Anstieg der Fitness (CTL) über sieben Tage (SPEC 4.2 F20, 8.2, 8.4 R1).
//!
//! Die CTL kommt aus [`crate::pmc`] (Impuls-Antwort-Modell, Q1 bis Q3). Die Rampe ist die
//! Differenz `CTL(Tag) - CTL(Tag - 7)`, dieselbe Messgröße, die R1 für Pläne begrenzt (SPEC 8.4,
//! "CTL am letzten Tag der Woche minus CTL am Ende der Vorwoche"). Die Grenze selbst ist die
//! Faustregel aus R1 (`max_ctl_rise_per_week`, Standard 5, Einstellung bis 8) und kommt aus den
//! Regeln, nicht aus diesem Modul.

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use serde::{Deserialize, Serialize};

use crate::plan::Rules;
use crate::pmc::PmcPoint;

/// Abstand der beiden CTL-Werte in Tagen (R1 misst über eine Woche).
pub const RAMP_DAYS: i64 = 7;

/// Unterhalb dieses Betrags in CTL-Punkten je Woche gilt die Fitness als gleichbleibend
/// (Faustregel). Ein Punkt je Woche entspricht einer Tageslast, die nur wenige Punkte von der
/// CTL abweicht, und liegt damit unter dem, was ein einzelner Ruhetag verschiebt.
pub const STEADY_BAND_CTL_PER_WEEK: f32 = 1.0;

/// Toleranz der Grenzprüfung, wie bei der Prüfung von R1 im Planer (`validate`, 1e-3).
const LIMIT_EPS: f32 = 1e-3;

/// Rampe an einem Tag.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RampPoint {
    pub date: NaiveDate,
    /// CTL-Punkte je Woche (positiv heißt Fitness wächst).
    pub ramp: f32,
}

/// Rampe am Tag `date`: CTL an diesem Tag minus CTL sieben Tage davor. `None`, wenn einer der
/// beiden Tage im Verlauf fehlt (zum Beispiel in den ersten sieben Tagen des Verlaufs).
/// Der Verlauf darf unsortiert sein.
pub fn ctl_ramp(series: &[PmcPoint], date: NaiveDate) -> Option<f32> {
    let before = date.checked_sub_signed(TimeDelta::days(RAMP_DAYS))?;
    let now = series.iter().find(|p| p.date == date)?;
    let then = series.iter().find(|p| p.date == before)?;
    Some(now.ctl - then.ctl)
}

/// Rampe für jeden Tag von `from` bis `to` einschließlich, für den beide Werte vorliegen,
/// aufsteigend nach Datum. Tage ohne Vergleichswert fehlen in der Antwort.
pub fn ramp_series(series: &[PmcPoint], from: NaiveDate, to: NaiveDate) -> Vec<RampPoint> {
    if from > to {
        // `BTreeMap::range` würde bei umgekehrter Spanne abbrechen.
        return Vec::new();
    }
    let ctl: BTreeMap<NaiveDate, f32> = series.iter().map(|p| (p.date, p.ctl)).collect();
    ctl.range(from..=to)
        .filter_map(|(&date, &now)| {
            let before = date.checked_sub_signed(TimeDelta::days(RAMP_DAYS))?;
            Some(RampPoint {
                date,
                ramp: now - ctl.get(&before)?,
            })
        })
        .collect()
}

/// Deutung der Rampe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RampState {
    /// Keine Rampe berechenbar (Verlauf zu kurz).
    NoData,
    /// Fitness sinkt (Rampe höchstens `-STEADY_BAND_CTL_PER_WEEK`).
    Falling,
    /// Fitness bleibt (Betrag unter dem Band).
    Steady,
    /// Fitness wächst, höchstens bis zur Grenze aus R1.
    Building,
    /// Wächst schneller als die Grenze aus R1 erlaubt.
    Steep,
}

impl RampState {
    /// Stabiler Code, Schlüssel für Texte in `catalog/locales`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoData => "ramp.no_data",
            Self::Falling => "ramp.falling",
            Self::Steady => "ramp.steady",
            Self::Building => "ramp.building",
            Self::Steep => "ramp.steep",
        }
    }
}

/// Deutet eine Rampe. Die obere Grenze ist `R01.max_ctl_rise_per_week` der Regeln (Faustregel,
/// SPEC 8.4). Genau auf der Grenze ist die Rampe noch `Building`, wie R1 sie noch zulässt;
/// erst darüber (mit der Toleranz 1e-3 des Planers) ist sie `Steep`. Die Untergrenze des
/// Bands `STEADY_BAND_CTL_PER_WEEK` ist eine Faustregel dieses Moduls und gilt "ab".
pub fn ramp_state(ramp: Option<f32>, rules: &Rules) -> RampState {
    let Some(r) = ramp.filter(|r| r.is_finite()) else {
        return RampState::NoData;
    };
    if r > rules.r01.max_ctl_rise_per_week + LIMIT_EPS {
        RampState::Steep
    } else if r >= STEADY_BAND_CTL_PER_WEEK {
        RampState::Building
    } else if r <= -STEADY_BAND_CTL_PER_WEEK {
        RampState::Falling
    } else {
        RampState::Steady
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::default_rules;
    use crate::pmc::{DailyLoad, PmcParams, pmc};

    fn day(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + TimeDelta::days(n)
    }

    fn point(n: i64, ctl: f32) -> PmcPoint {
        PmcPoint {
            date: day(n),
            ctl,
            atl: 0.0,
            tsb: 0.0,
        }
    }

    /// Konstante Last 100, Start 0, `1 - exp(-1/42) = 0,023528` (SPEC 8.2).
    /// Nach n Tagen gilt `CTL_n = 100 * (1 - exp(-n/42))`.
    /// Tag 7 (Index 6): hier zählt der Tag n = 7, `exp(-7/42) = exp(-1/6) = 0,846482`,
    /// `CTL_7 = 15,3518`. Tag 14: `exp(-14/42) = exp(-1/3) = 0,716531`, `CTL_14 = 28,3469`.
    /// Rampe = 28,3469 - 15,3518 = 12,9951.
    const RAMP_CONSTANT_100_AFTER_14_DAYS: f32 = 12.9951;

    #[test]
    fn ramp_of_constant_load_matches_the_hand_calculation() {
        let daily: Vec<DailyLoad> = (0..14)
            .map(|n| DailyLoad {
                date: day(n),
                load: 100.0,
            })
            .collect();
        let series = pmc(&daily, &PmcParams::default(), day(0), day(13));
        let ramp = ctl_ramp(&series, day(13)).expect("beide Tage vorhanden");
        assert!(
            (ramp - RAMP_CONSTANT_100_AFTER_14_DAYS).abs() < 1e-3,
            "erwartet {RAMP_CONSTANT_100_AFTER_14_DAYS}, erhalten {ramp}"
        );
    }

    #[test]
    fn ramp_needs_both_days_and_ignores_the_order() {
        let series = vec![point(7, 30.0), point(0, 22.5), point(3, 99.0)];
        // 30 - 22,5 = 7,5
        assert_eq!(ctl_ramp(&series, day(7)), Some(7.5));
        // Tag 6 hat keinen Vergleichstag (Tag -1) und Tag 3 keinen (Tag -4).
        assert_eq!(ctl_ramp(&series, day(3)), None);
        assert_eq!(ctl_ramp(&[], day(7)), None);
    }

    #[test]
    fn series_lists_only_days_with_a_comparison_value() {
        let series: Vec<PmcPoint> = (0..10).map(|n| point(n, n as f32 * 2.0)).collect();
        let r = ramp_series(&series, day(0), day(9));
        // Vergleich ab Tag 7: Tage 7, 8, 9, je 7 Tage * 2 Punkte = 14.
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].date, day(7));
        assert!(r.iter().all(|p| (p.ramp - 14.0).abs() < 1e-6));
        assert!(ramp_series(&series, day(9), day(0)).is_empty());
    }

    #[test]
    fn states_follow_the_r1_limit_from_the_rules() {
        let mut rules = default_rules();
        assert_eq!(rules.r01.max_ctl_rise_per_week, 5.0);
        assert_eq!(ramp_state(None, &rules), RampState::NoData);
        assert_eq!(ramp_state(Some(f32::NAN), &rules), RampState::NoData);
        assert_eq!(ramp_state(Some(-3.0), &rules), RampState::Falling);
        assert_eq!(ramp_state(Some(-1.0), &rules), RampState::Falling);
        assert_eq!(ramp_state(Some(-0.5), &rules), RampState::Steady);
        assert_eq!(ramp_state(Some(0.99), &rules), RampState::Steady);
        assert_eq!(ramp_state(Some(1.0), &rules), RampState::Building);
        // Genau auf der Grenze ist noch erlaubt, wie in R1.
        assert_eq!(ramp_state(Some(5.0), &rules), RampState::Building);
        assert_eq!(ramp_state(Some(5.01), &rules), RampState::Steep);
        // Die Einstellung des Nutzers (bis 8) verschiebt die Grenze.
        rules.r01.max_ctl_rise_per_week = 8.0;
        assert_eq!(ramp_state(Some(5.01), &rules), RampState::Building);
        assert_eq!(ramp_state(Some(8.5), &rules), RampState::Steep);
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(RampState::Steep.code(), "ramp.steep");
        assert_eq!(RampState::Building.code(), "ramp.building");
        assert_eq!(RampState::Steady.code(), "ramp.steady");
        assert_eq!(RampState::Falling.code(), "ramp.falling");
        assert_eq!(RampState::NoData.code(), "ramp.no_data");
    }
}
