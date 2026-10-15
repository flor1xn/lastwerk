//! Kennzahlen je Runde (T31, SPEC 4.2 F38).
//!
//! Die Rundengrenzen kommen aus der FIT-Datei (`lap`), die Mittelwerte aus der Messreihe im
//! Sekundentakt. Mittel zählen nur vorhandene Werte (SPEC 6.5): Eine Sekunde ohne Leistung ist
//! keine Sekunde mit 0 W. Das ist hier anders als bei `best_power`, weil eine Runde ein festes
//! Zeitfenster ist und kein gesuchtes Maximum; ein Überspringen kann nichts aufblähen. Die
//! Konvention entspricht der Mittelung der Messreihe für die Darstellung (`downsample` der App).
//!
//! Geschwindigkeit: Nennt die Datei Strecke und Zeit bei laufendem Timer, ist das Mittel
//! `Strecke / Timerzeit` (so rechnen auch die Geräte, Pausen zählen nicht). Sonst das Mittel
//! der Geschwindigkeitswerte im Fenster. Reine Rechnung, keine Quelle nötig (Faustregel nur für
//! die Wahl der Variante).

use serde::{Deserialize, Serialize};

use crate::load::Sample;

/// Grenzen einer Runde relativ zum Ursprung der Messreihe (`Sample::t_s`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LapBounds {
    /// Beginn in Sekunden ab dem ersten Messpunkt.
    pub start_s: u32,
    /// Dauer inklusive Pausen in Sekunden, bestimmt das Fenster der Messreihe.
    pub elapsed_s: u32,
    /// Dauer bei laufendem Timer, falls die Datei sie nennt.
    pub timer_s: Option<f64>,
    /// Strecke laut Datei.
    pub distance_m: Option<f64>,
}

/// Kennzahlen einer Runde.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LapStats {
    pub start_s: u32,
    pub elapsed_s: u32,
    pub timer_s: Option<f64>,
    pub distance_m: Option<f64>,
    pub avg_power_w: Option<f32>,
    pub avg_hr_bpm: Option<f32>,
    pub avg_speed_mps: Option<f32>,
}

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values
        .filter(|v| v.is_finite())
        .fold((0.0_f64, 0_u32), |(s, n), v| (s + f64::from(v), n + 1));
    (n > 0).then(|| (sum / f64::from(n)) as f32)
}

/// Kennzahlen je Runde in der Reihenfolge von `laps`. Ein Messpunkt gehört zur Runde, wenn
/// `start_s <= t_s < start_s + elapsed_s`. Runden ohne Messpunkte behalten Dauer und Strecke,
/// ihre Mittel sind `None`.
pub fn lap_stats(samples: &[Sample], laps: &[LapBounds]) -> Vec<LapStats> {
    laps.iter()
        .map(|lap| {
            let end = lap.start_s.saturating_add(lap.elapsed_s);
            let inside = || {
                samples
                    .iter()
                    .filter(move |s| s.t_s >= lap.start_s && s.t_s < end)
            };
            let from_file = match (lap.distance_m, lap.timer_s) {
                (Some(d), Some(t)) if d.is_finite() && d > 0.0 && t.is_finite() && t > 0.0 => {
                    Some((d / t) as f32)
                }
                _ => None,
            };
            LapStats {
                start_s: lap.start_s,
                elapsed_s: lap.elapsed_s,
                timer_s: lap.timer_s,
                distance_m: lap.distance_m,
                avg_power_w: mean(inside().filter_map(|s| s.power_w)),
                avg_hr_bpm: mean(inside().filter_map(|s| s.hr_bpm)),
                avg_speed_mps: from_file.or_else(|| mean(inside().filter_map(|s| s.speed_mps))),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    fn s(t: u32, p: Option<f32>, h: Option<f32>, v: Option<f32>) -> Sample {
        Sample {
            t_s: t,
            power_w: p,
            hr_bpm: h,
            speed_mps: v,
        }
    }

    // Referenz (Handrechnung): Messpunkte t = 0..5.
    //   t:   0    1    2    3    4    5
    //   P: 100  200    -  300  400    -
    //   HF: 120  130  140    -  150  160
    //   v:  2,0  4,0    -    -  3,0  5,0
    // Runde 1 = [0, 3): P (100 + 200) / 2 = 150, HF (120 + 130 + 140) / 3 = 130,
    //   ohne Strecke aus der Datei: v (2 + 4) / 2 = 3,0.
    // Runde 2 = [3, 6): P (300 + 400) / 2 = 350, HF (150 + 160) / 2 = 155,
    //   Strecke 1000 m bei Timer 250 s: v = 1000 / 250 = 4,0 (statt Mittel der Werte 4,0).
    const LAP1_POWER: f32 = 150.0;
    const LAP1_HR: f32 = 130.0;
    const LAP1_SPEED: f32 = 3.0;
    const LAP2_POWER: f32 = 350.0;
    const LAP2_HR: f32 = 155.0;
    const LAP2_SPEED: f32 = 4.0;

    fn reference() -> Vec<Sample> {
        vec![
            s(0, Some(100.0), Some(120.0), Some(2.0)),
            s(1, Some(200.0), Some(130.0), Some(4.0)),
            s(2, None, Some(140.0), None),
            s(3, Some(300.0), None, None),
            s(4, Some(400.0), Some(150.0), Some(3.0)),
            s(5, None, Some(160.0), Some(5.0)),
        ]
    }

    #[test]
    fn means_per_lap_match_hand_calculation() {
        let laps = [
            LapBounds {
                start_s: 0,
                elapsed_s: 3,
                timer_s: None,
                distance_m: None,
            },
            LapBounds {
                start_s: 3,
                elapsed_s: 3,
                timer_s: Some(250.0),
                distance_m: Some(1000.0),
            },
        ];
        let out = lap_stats(&reference(), &laps);
        assert_eq!(out.len(), 2);
        assert!((out[0].avg_power_w.unwrap() - LAP1_POWER).abs() < EPS);
        assert!((out[0].avg_hr_bpm.unwrap() - LAP1_HR).abs() < EPS);
        assert!((out[0].avg_speed_mps.unwrap() - LAP1_SPEED).abs() < EPS);
        assert!((out[1].avg_power_w.unwrap() - LAP2_POWER).abs() < EPS);
        assert!((out[1].avg_hr_bpm.unwrap() - LAP2_HR).abs() < EPS);
        assert!((out[1].avg_speed_mps.unwrap() - LAP2_SPEED).abs() < EPS);
        assert_eq!(out[1].distance_m, Some(1000.0));
    }

    #[test]
    fn lap_without_samples_keeps_bounds_and_has_no_means() {
        let laps = [LapBounds {
            start_s: 100,
            elapsed_s: 60,
            timer_s: Some(60.0),
            distance_m: None,
        }];
        let out = lap_stats(&reference(), &laps);
        assert_eq!(out[0].elapsed_s, 60);
        assert_eq!(out[0].avg_power_w, None);
        assert_eq!(out[0].avg_hr_bpm, None);
        assert_eq!(out[0].avg_speed_mps, None);
    }

    #[test]
    fn zero_distance_falls_back_to_sample_mean() {
        let laps = [LapBounds {
            start_s: 0,
            elapsed_s: 2,
            timer_s: Some(2.0),
            distance_m: Some(0.0),
        }];
        let out = lap_stats(&reference(), &laps);
        assert!((out[0].avg_speed_mps.unwrap() - LAP1_SPEED).abs() < EPS);
    }

    #[test]
    fn huge_start_does_not_overflow() {
        let laps = [LapBounds {
            start_s: u32::MAX - 1,
            elapsed_s: 10,
            timer_s: None,
            distance_m: None,
        }];
        assert_eq!(lap_stats(&reference(), &laps)[0].avg_power_w, None);
    }
}
