//! Schnellste Zeit über feste Strecken innerhalb einer Aktivität (T37, SPEC 4.2 F44).
//!
//! Die Strecke je Sekunde ist die Geschwindigkeit dieser Sekunde (1-Hz-Reihe, wie überall in
//! `core`). Konventionen, angelehnt an [`crate::best_power`]:
//!
//! - Ein Fenster liegt vollständig in einem zusammenhängenden Abschnitt (keine Pause über
//!   `max_gap_s`, SPEC 6.5).
//! - Eine Sekunde ohne Geschwindigkeit zählt als Stillstand (0 m). Das verlängert die Zeit und
//!   überschätzt nie das Tempo.
//! - Geschwindigkeiten über [`RUN_SPEED_MAX_MPS`] gelten beim Laufen als Messfehler (GPS-Sprung)
//!   und zählen ebenfalls als 0 m.
//! - Gesucht ist das kürzeste Fenster aus ganzen Sekunden, in dem die Strecke mindestens die
//!   Zielstrecke erreicht. Die Zeit ist damit auf ganze Sekunden nach oben gerundet.
//! - Erreicht kein Abschnitt die Strecke, fehlt der Wert (nicht in der Liste).

use serde::{Deserialize, Serialize};

use crate::load::{ActivityInput, LoadConfig, LoadError, PreparedSamples, check_plausibility};

/// Strecken der Bestzeiten in Metern: 1 km, 5 km, 10 km, Halbmarathon, Marathon (Konvention,
/// übliche Wettkampfstrecken; Halbmarathon und Marathon auf ganze Meter abgerundet).
pub const BEST_EFFORT_DISTANCES_M: [u32; 5] = [1000, 5000, 10_000, 21_097, 42_195];

/// Obergrenze der Laufgeschwindigkeit in m/s für Bestzeiten: 12 m/s (43,2 km/h). Faustregel:
/// liegt über der Spitzengeschwindigkeit im Sprint, darüber ist der Wert ein Messfehler.
pub const RUN_SPEED_MAX_MPS: f32 = 12.0;

/// Schnellste Zeit über eine Strecke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BestEffort {
    pub distance_m: u32,
    pub seconds: u32,
}

/// Bestzeiten der Aktivität für jede Strecke in `distances_m`, in deren Reihenfolge. Strecken,
/// die kein Abschnitt erreicht, fehlen. Ohne Geschwindigkeitswerte ist die Liste leer.
pub fn best_efforts(
    a: &ActivityInput,
    cfg: &LoadConfig,
    distances_m: &[u32],
) -> Result<Vec<BestEffort>, LoadError> {
    let prepared = check_plausibility(&a.samples, cfg)?;
    Ok(best_efforts_prepared(&prepared, distances_m))
}

/// Wie [`best_efforts`], aber mit den schon geprüften Samples (Ergebnis von
/// [`check_plausibility`]). Der Import prüft die Samples einmal statt je Berechnung.
pub fn best_efforts_prepared(prepared: &PreparedSamples, distances_m: &[u32]) -> Vec<BestEffort> {
    let mut any_speed = false;
    // Je Abschnitt die aufsummierte Strecke nach i Sekunden (Präfixsummen).
    let mut cumulative: Vec<Vec<f64>> = Vec::new();
    for seg in &prepared.segments {
        let (Some(first), Some(last)) = (seg.first(), seg.last()) else {
            continue;
        };
        let len = (last.t_s - first.t_s + 1) as usize;
        let mut dense = vec![0.0_f64; len];
        for s in seg {
            if let Some(v) = s.speed_mps {
                any_speed = true;
                if v.is_finite() && v > 0.0 && v <= RUN_SPEED_MAX_MPS {
                    dense[(s.t_s - first.t_s) as usize] = f64::from(v);
                }
            }
        }
        let mut sums = Vec::with_capacity(len + 1);
        let mut acc = 0.0_f64;
        sums.push(acc);
        for v in dense {
            acc += v;
            sums.push(acc);
        }
        cumulative.push(sums);
    }
    if !any_speed {
        return Vec::new();
    }

    let mut out = Vec::new();
    for &d in distances_m.iter().filter(|&&d| d > 0) {
        let target = f64::from(d);
        let mut best: Option<usize> = None;
        for sums in &cumulative {
            // Zwei Zeiger: Für jedes Ende j das späteste Anfang i mit Strecke >= Ziel.
            let mut i = 0;
            for j in 1..sums.len() {
                if sums[j] - sums[i] < target {
                    continue;
                }
                while i + 1 < j && sums[j] - sums[i + 1] >= target {
                    i += 1;
                }
                let len = j - i;
                if best.is_none_or(|b| len < b) {
                    best = Some(len);
                }
            }
        }
        if let Some(len) = best {
            out.push(BestEffort {
                distance_m: d,
                seconds: u32::try_from(len).unwrap_or(u32::MAX),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::{Sample, Sport};
    use chrono::{TimeZone, Utc};

    fn run(samples: Vec<Sample>) -> ActivityInput {
        ActivityInput {
            sport: Sport::Running,
            start_utc: Utc.with_ymd_and_hms(2026, 3, 1, 6, 30, 0).unwrap(),
            local_offset_s: 0,
            samples,
            rpe: None,
            pool_length_m: None,
        }
    }

    fn speed(t: u32, v: f32) -> Sample {
        Sample {
            speed_mps: Some(v),
            ..Sample::at(t)
        }
    }

    fn efforts(a: &ActivityInput, d: &[u32]) -> Vec<BestEffort> {
        best_efforts(a, &LoadConfig::default(), d).unwrap()
    }

    /// Referenz: 1000 s mit 4 m/s. 1000 m brauchen genau 1000 / 4 = 250 s, 4000 m genau 1000 s,
    /// 5000 m sind nicht erreichbar.
    const CONSTANT_1000M_S: u32 = 250;
    const CONSTANT_4000M_S: u32 = 1000;

    #[test]
    fn constant_speed_matches_hand_calculation() {
        let a = run((0..1000).map(|t| speed(t, 4.0)).collect());
        let e = efforts(&a, &[1000, 4000, 5000]);
        assert_eq!(
            e,
            [
                BestEffort {
                    distance_m: 1000,
                    seconds: CONSTANT_1000M_S
                },
                BestEffort {
                    distance_m: 4000,
                    seconds: CONSTANT_4000M_S
                },
            ]
        );
    }

    /// Referenz: 300 s mit 3 m/s, dann 300 s mit 5 m/s. Der schnellste Kilometer liegt ganz im
    /// schnellen Teil: 1000 / 5 = 200 s. Für 2000 m: 300 s mit 5 m/s geben 1500 m, es fehlen
    /// 500 m mit 3 m/s, also 500 / 3 = 166,7 s; zusammen 466,7 s, auf ganze Sekunden nach oben
    /// 467 s.
    const SPLIT_1000M_S: u32 = 200;
    const SPLIT_2000M_S: u32 = 467;

    #[test]
    fn fastest_window_is_found_and_rounded_up() {
        let mut s: Vec<Sample> = (0..300).map(|t| speed(t, 3.0)).collect();
        s.extend((300..600).map(|t| speed(t, 5.0)));
        let e = efforts(&run(s), &[1000, 2000]);
        assert_eq!(e[0].seconds, SPLIT_1000M_S);
        assert_eq!(e[1].seconds, SPLIT_2000M_S);
    }

    #[test]
    fn gps_jumps_and_missing_seconds_count_as_standstill() {
        // 300 s mit 4 m/s, in der Mitte ein Sprung auf 50 m/s und eine Sekunde ohne Wert.
        let mut s: Vec<Sample> = (0..300).map(|t| speed(t, 4.0)).collect();
        s[150].speed_mps = Some(50.0);
        s[151].speed_mps = None;
        // 1000 m ohne die zwei Sekunden: ganz in einem der beiden Teile (150 bzw. 148 s
        // zu je 4 m/s = 600 m) nicht erreichbar, also über die Lücke: 250 + 2 = 252 s.
        let e = efforts(&run(s), &[1000]);
        assert_eq!(e[0].seconds, 252);
    }

    #[test]
    fn never_across_a_pause_and_empty_without_speed() {
        // Zwei Abschnitte von je 200 s mit 4 m/s (800 m), dazwischen 60 s Pause: kein Kilometer.
        let mut s: Vec<Sample> = (0..200).map(|t| speed(t, 4.0)).collect();
        s.extend((260..460).map(|t| speed(t, 4.0)));
        assert!(efforts(&run(s), &[1000]).is_empty());
        let none = run((0..600).map(Sample::at).collect());
        assert!(efforts(&none, &BEST_EFFORT_DISTANCES_M).is_empty());
    }
}
