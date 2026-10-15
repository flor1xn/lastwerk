//! Fitness, Ermüdung und Form als Verlauf (SPEC 8.2).
//!
//! Modellgrundlage ist das Impuls-Antwort-Modell von Banister und Kollegen (Q1, Q2, Q3),
//! in der Praxis als exponentiell gewichtete Mittelwerte der Tageslast angewandt:
//!
//! ```text
//! CTL_t = CTL_{t-1} + (L_t - CTL_{t-1}) * (1 - exp(-1 / tau_ctl))
//! ATL_t = ATL_{t-1} + (L_t - ATL_{t-1}) * (1 - exp(-1 / tau_atl))
//! TSB_t = CTL_{t-1} - ATL_{t-1}
//! ```
//!
//! Die Zeitkonstanten 42 und 7 Tage sind Konvention der Praxis (Sekundärquellen). Das
//! Originalmodell berichtet 50 und 15 Tage (Q2, Seitenangabe nicht geprüft). Beide
//! Parametersätze sind einstellbar, der Standard ist 42 und 7. `TSB` aus den Werten des
//! Vortags ist Konvention ("Form am Morgen").
//!
//! Das Modul ist rein: Die Tageslasten kommen als Daten, das Crate liest nichts selbst.

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use serde::{Deserialize, Serialize};

/// Belastung eines Tages, Summe über alle zählenden Aktivitäten (SPEC 5.4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DailyLoad {
    pub date: NaiveDate,
    pub load: f32,
}

/// Fitness (CTL), Ermüdung (ATL) und Form (TSB) am Ende eines Tages.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PmcPoint {
    pub date: NaiveDate,
    pub ctl: f32,
    pub atl: f32,
    /// Form am Morgen: CTL minus ATL des Vortags (Konvention, SPEC 8.2).
    pub tsb: f32,
}

/// Parameter des Verlaufs. Alle Werte sind einstellbar (SPEC 8.2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PmcParams {
    /// Zeitkonstante der Fitness in Tagen.
    pub tau_ctl_d: f32,
    /// Zeitkonstante der Ermüdung in Tagen.
    pub tau_atl_d: f32,
    /// Startwert der Fitness vor dem ersten Tag.
    pub ctl0: f32,
    /// Startwert der Ermüdung vor dem ersten Tag.
    pub atl0: f32,
}

impl Default for PmcParams {
    fn default() -> Self {
        Self {
            // Konvention der Praxis (Sekundärquellen). Das Originalmodell nennt 50 und 15
            // Tage (Q2, laut Suchtreffer, Seitenangabe nicht geprüft), SPEC 8.2.
            tau_ctl_d: 42.0,
            tau_atl_d: 7.0,
            // SPEC 8.2: Standard 0 am ersten Tag mit Daten.
            ctl0: 0.0,
            atl0: 0.0,
        }
    }
}

/// Gewicht eines Tages im exponentiellen Mittel: `1 - exp(-1 / tau)`. Eine Zeitkonstante
/// von null oder weniger (oder nicht endlich) gibt das Gewicht 1, der Wert folgt dann der
/// Tageslast ohne Gedächtnis. So bleibt die Funktion total und liefert nie `NaN`.
fn smoothing_weight(tau_d: f32) -> f64 {
    let tau = f64::from(tau_d);
    if tau.is_finite() && tau > 0.0 {
        1.0 - (-1.0 / tau).exp()
    } else {
        1.0
    }
}

/// Verlauf von CTL, ATL und TSB, lückenlos je Tag von `from` bis `to` einschließlich.
///
/// - Tage ohne Eintrag in `daily` zählen als Last 0 (Ruhetag). Mehrere Einträge für denselben
///   Tag werden addiert. Einträge außerhalb von `from..=to` bleiben unberücksichtigt, die
///   Startwerte in `p` gelten also vor `from`. Wer eine Vorgeschichte mitrechnen will,
///   beginnt `from` früher und schneidet das Ergebnis selbst.
/// - Der TSB des ersten Tages ist `ctl0 - atl0`.
/// - Ist `from` nach `to`, ist das Ergebnis leer.
///
/// Abweichung von der Skizze in SPEC 5.4: zusätzlich `from` und `to`, weil `daily` nur Tage
/// mit Last enthält (die Speicherschicht lässt Tage ohne zählende Belastung aus) und der
/// Verlauf trotzdem je Tag gebraucht wird.
///
/// Gerechnet wird in `f64`, damit sich Rundung über lange Verläufe nicht aufsummiert.
pub fn pmc(daily: &[DailyLoad], p: &PmcParams, from: NaiveDate, to: NaiveDate) -> Vec<PmcPoint> {
    let mut loads: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for d in daily.iter().filter(|d| d.date >= from && d.date <= to) {
        *loads.entry(d.date).or_insert(0.0) += f64::from(d.load);
    }

    let k_ctl = smoothing_weight(p.tau_ctl_d);
    let k_atl = smoothing_weight(p.tau_atl_d);
    let mut ctl = f64::from(p.ctl0);
    let mut atl = f64::from(p.atl0);
    let mut out = Vec::new();
    let mut date = from;
    while date <= to {
        let load = loads.get(&date).copied().unwrap_or(0.0);
        let tsb = ctl - atl;
        ctl += (load - ctl) * k_ctl;
        atl += (load - atl) * k_atl;
        out.push(PmcPoint {
            date,
            ctl: ctl as f32,
            atl: atl as f32,
            tsb: tsb as f32,
        });
        let Some(next) = date.checked_add_signed(TimeDelta::days(1)) else {
            break;
        };
        date = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Toleranz für handgerechnete Werte (Rundung der Handrechnung, SPEC 8.2).
    const EPS: f32 = 1e-3;

    fn day(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + TimeDelta::days(n)
    }

    fn constant(load: f32, days: i64) -> Vec<DailyLoad> {
        (0..days)
            .map(|n| DailyLoad { date: day(n), load })
            .collect()
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= EPS,
            "erwartet {expected}, erhalten {actual}"
        );
    }

    /// T-PMC-1, Tag 1 und Tag 2 bei konstanter Last 100 und Start 0 (SPEC 8.2, 11.2).
    ///
    /// Nebenrechnung: `1 - exp(-1/42) = 0,023528`, `1 - exp(-1/7) = 0,133122`.
    /// Tag 1: CTL = 0 + 100 * 0,023528 = 2,3528, ATL = 0 + 100 * 0,133122 = 13,3122,
    /// TSB = 0 - 0 = 0.
    /// Tag 2: CTL = 2,3528 + (100 - 2,3528) * 0,023528 = 4,6503,
    /// ATL = 13,3122 + (100 - 13,3122) * 0,133122 = 24,8523,
    /// TSB = 2,3528 - 13,3122 = -10,9594.
    #[test]
    fn t_pmc_1_constant_load_day_1_and_2() {
        const CTL_1: f32 = 2.3528;
        const ATL_1: f32 = 13.3122;
        const TSB_1: f32 = 0.0;
        const CTL_2: f32 = 4.6503;
        const ATL_2: f32 = 24.8523;
        const TSB_2: f32 = -10.9594;

        let points = pmc(&constant(100.0, 2), &PmcParams::default(), day(0), day(1));
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].date, day(0));
        assert_close(points[0].ctl, CTL_1);
        assert_close(points[0].atl, ATL_1);
        assert_close(points[0].tsb, TSB_1);
        assert_eq!(points[1].date, day(1));
        assert_close(points[1].ctl, CTL_2);
        assert_close(points[1].atl, ATL_2);
        assert_close(points[1].tsb, TSB_2);
    }

    /// T-PMC-2: Last 0 ab CTL0 = ATL0 = 50. ATL klingt mit tau 7 ab, nach 7 Tagen gilt
    /// `ATL = 50 * exp(-7/7) = 50 * 0,367879 = 18,3940`. Jeder Tag multipliziert mit
    /// `exp(-1/7)`, sieben Tage ergeben `exp(-1)`. CTL nach 7 Tagen:
    /// `50 * exp(-7/42) = 50 * 0,846482 = 42,3241`.
    #[test]
    fn t_pmc_2_decay_without_load() {
        const ATL_AFTER_7_DAYS: f32 = 18.3940;
        const CTL_AFTER_7_DAYS: f32 = 42.3241;

        let p = PmcParams {
            ctl0: 50.0,
            atl0: 50.0,
            ..PmcParams::default()
        };
        let points = pmc(&[], &p, day(0), day(6));
        assert_eq!(points.len(), 7);
        let last = points[6];
        assert_close(last.atl, ATL_AFTER_7_DAYS);
        assert_close(last.ctl, CTL_AFTER_7_DAYS);
        // Der TSB des ersten Tages ist ctl0 - atl0 = 0.
        assert_close(points[0].tsb, 0.0);
    }

    /// Ruhetage ohne Eintrag zählen als Last 0: Last 100 an Tag 1, dann zwei Tage ohne Eintrag.
    /// Tag 2: CTL = 2,3528 * (1 - 0,023528) = 2,2975,
    /// ATL = 13,3122 * (1 - 0,133122) = 11,5400.
    #[test]
    fn missing_days_count_as_zero_load() {
        const CTL_2: f32 = 2.2975;
        const ATL_2: f32 = 11.5400;

        let daily = [DailyLoad {
            date: day(0),
            load: 100.0,
        }];
        let points = pmc(&daily, &PmcParams::default(), day(0), day(2));
        assert_eq!(points.len(), 3);
        assert_close(points[1].ctl, CTL_2);
        assert_close(points[1].atl, ATL_2);
        // TSB am Morgen von Tag 2 ist CTL minus ATL von Tag 1: 2,3528 - 13,3122 = -10,9594.
        assert_close(points[1].tsb, -10.9594);
    }

    #[test]
    fn same_day_entries_are_summed_and_outside_entries_ignored() {
        let daily = [
            DailyLoad {
                date: day(0),
                load: 60.0,
            },
            DailyLoad {
                date: day(0),
                load: 40.0,
            },
            DailyLoad {
                date: day(-5),
                load: 500.0,
            },
            DailyLoad {
                date: day(9),
                load: 500.0,
            },
        ];
        let summed = pmc(&daily, &PmcParams::default(), day(0), day(1));
        let single = pmc(&constant(100.0, 1), &PmcParams::default(), day(0), day(1));
        assert_eq!(summed[0], single[0]);
        assert_eq!(summed.len(), 2);
    }

    #[test]
    fn empty_range_gives_no_points() {
        assert!(pmc(&[], &PmcParams::default(), day(3), day(2)).is_empty());
    }

    #[test]
    fn invalid_time_constant_never_gives_nan() {
        let p = PmcParams {
            tau_ctl_d: 0.0,
            tau_atl_d: f32::NAN,
            ..PmcParams::default()
        };
        let points = pmc(&constant(80.0, 3), &p, day(0), day(2));
        // Gewicht 1: der Wert folgt der Tageslast.
        for pt in &points {
            assert_close(pt.ctl, 80.0);
            assert_close(pt.atl, 80.0);
        }
    }

    /// Eigenschaft: Bei konstanter Last `L` laufen CTL und ATL monoton gegen `L`, von unten
    /// steigend ohne Überschwingen, von oben fallend. Folgt aus dem exponentiellen Mittel mit
    /// Gewicht zwischen 0 und 1.
    fn assert_monotone_towards(values: &[f32], start: f32, target: f32) {
        let tol = 1e-3 * target.max(start).max(1.0);
        let mut prev = start;
        for &v in values {
            if target >= start {
                assert!(
                    v + tol >= prev && v <= target + tol,
                    "{prev} -> {v} ({target})"
                );
            } else {
                assert!(
                    v <= prev + tol && v + tol >= target,
                    "{prev} -> {v} ({target})"
                );
            }
            prev = v;
        }
    }

    proptest! {
        #[test]
        fn constant_load_converges_monotonically(
            load in 0.0_f32..600.0,
            start in 0.0_f32..600.0,
            days in 2_i64..120,
        ) {
            let p = PmcParams { ctl0: start, atl0: start, ..PmcParams::default() };
            let points = pmc(&constant(load, days), &p, day(0), day(days - 1));
            prop_assert_eq!(points.len() as i64, days);
            let ctl: Vec<f32> = points.iter().map(|x| x.ctl).collect();
            let atl: Vec<f32> = points.iter().map(|x| x.atl).collect();
            assert_monotone_towards(&ctl, start, load);
            assert_monotone_towards(&atl, start, load);
        }
    }

    #[test]
    fn long_constant_load_reaches_the_load() {
        // Nach 400 Tagen (rund 10 Zeitkonstanten) liegt CTL nahe bei L:
        // 100 * exp(-400/42) = 0,0073, also Abstand unter 0,01.
        let points = pmc(
            &constant(100.0, 400),
            &PmcParams::default(),
            day(0),
            day(399),
        );
        let last = points.last().unwrap();
        assert!((last.ctl - 100.0).abs() < 0.01);
        assert!((last.atl - 100.0).abs() < 1e-3);
        assert!(last.tsb.abs() < 0.01);
    }
}
