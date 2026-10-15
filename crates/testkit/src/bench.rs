//! Große synthetische Datensätze für Laufzeitmessungen (T38, Anhang Messung).
//!
//! Zwei Sätze, beide reine Funktionen ihrer Parameter und eines festen Startwerts:
//!
//! - [`long_rides`]: lange Radfahrten mit 1 Hz und Leistung, Herzfrequenz, Kadenz und
//!   Geschwindigkeit, Standard 5 Stunden (18 000 `record`-Nachrichten je Datei).
//! - [`history`]: mehrjährige Historie mit überwiegend einstündigen Einheiten über Rad,
//!   Lauf, Schwimmen und Kraft.
//!
//! Die Streuung der Parameter kommt aus einem linearen Kongruenzgenerator mit festem
//! Startwert, nicht aus einer Zufallsquelle: derselbe Startwert ergibt dieselben Dateien.
//! Die Werte (Leistung, Herzfrequenz, Dauer) sind Faustregeln für plausible Trainingsdaten,
//! keine Messwerte. Die Dateien werden nie eingecheckt, `gen-bench` schreibt sie in einen
//! Ordner, den der Aufrufer wählt.

use crate::generators::{ActivityParams, EDGE, HrProfile, PowerProfile, Sport, WATCH};

/// Standard-Startwert der Messdaten.
pub const DEFAULT_SEED: u64 = 0x5EED_0038;

/// Dauer einer langen Einheit: 5 Stunden mit 1 Hz.
pub const LONG_RIDE_S: u32 = 5 * 3600;

/// FIT-Zeitstempel von Montag, 4. Januar 2021, 00:00 UTC. Rechenweg: Unix-Zeit 1 609 718 400
/// minus 631 065 600 (Unix-Zeit des FIT-Nullpunkts 31. Dezember 1989) = 978 652 800.
pub const HISTORY_START_FIT: u32 = 978_652_800;

const DAY_S: u32 = 86_400;

/// Linearer Kongruenzgenerator mit den Konstanten von Knuth (MMIX). Für Messdaten genügt
/// eine gleichmäßige, wiederholbare Streuung; statistische Güte ist nicht gefragt.
#[derive(Debug, Clone)]
pub struct Lcg(u64);

impl Lcg {
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // Die oberen Bits haben die längere Periode.
        (self.0 >> 32) as u32
    }

    /// Gleichmäßig in `lo..=hi` (ohne Korrektur der Modulo-Schiefe, für Messdaten egal).
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.next_u32() % (hi - lo + 1)
    }
}

fn narrow_u16(v: u32) -> u16 {
    u16::try_from(v).expect("Wert im u16-Bereich")
}

fn narrow_u8(v: u32) -> u8 {
    u8::try_from(v).expect("Wert im u8-Bereich")
}

/// Radfahrt mit wechselnder Leistung, Herzfrequenzdrift und Kadenz.
fn ride(rng: &mut Lcg, start_fit: u32, duration_s: u32) -> ActivityParams {
    ActivityParams {
        power: PowerProfile::Intervals {
            low: narrow_u16(rng.range(140, 200)),
            high: narrow_u16(rng.range(220, 320)),
            period_s: rng.range(30, 600),
        },
        heart_rate: HrProfile::Drift {
            start: narrow_u8(rng.range(115, 135)),
            end: narrow_u8(rng.range(145, 170)),
        },
        cadence_rpm: Some(narrow_u8(rng.range(80, 95))),
        speed_mps: Some(rng.range(75, 95) as f32 / 10.0),
        device: EDGE,
        ..ActivityParams::cycling(start_fit, 3600, duration_s)
    }
}

/// `count` lange Radfahrten zu je [`LONG_RIDE_S`] Sekunden, eine je zweitem Tag ab
/// [`HISTORY_START_FIT`], jeweils um 8 Uhr UTC. Liefert Dateiname und Parameter.
pub fn long_rides(count: usize, seed: u64) -> Vec<(String, ActivityParams)> {
    let mut rng = Lcg::new(seed);
    (0..count)
        .map(|i| {
            let day = u32::try_from(i).expect("Anzahl im u32-Bereich") * 2;
            let start = HISTORY_START_FIT + day * DAY_S + 8 * 3600;
            (
                format!("long_ride_{i:03}.fit"),
                ride(&mut rng, start, LONG_RIDE_S),
            )
        })
        .collect()
}

/// `count` Einheiten gleichmäßig über `years` Jahre ab [`HISTORY_START_FIT`] verteilt.
///
/// Anteile (Faustregel für einen Mehrsportler): rund 50 Prozent Rad, 30 Prozent Lauf,
/// 15 Prozent Schwimmen, 5 Prozent Kraft. Dauer meist um eine Stunde, jede zehnte Radfahrt
/// drei Stunden. Startzeit zwischen 6 und 19 Uhr UTC.
pub fn history(years: u32, count: usize, seed: u64) -> Vec<(String, ActivityParams)> {
    let mut rng = Lcg::new(seed);
    let days = u64::from(years) * 365;
    let n = u64::try_from(count.max(1)).expect("Anzahl im u64-Bereich");
    (0..count)
        .map(|i| {
            let day = u32::try_from(u64::try_from(i).expect("Index") * days / n)
                .expect("Tag im u32-Bereich");
            let start = HISTORY_START_FIT + day * DAY_S + rng.range(6, 19) * 3600;
            let kind = rng.range(0, 99);
            let p = match kind {
                0..=49 => {
                    let long = rng.range(0, 9) == 0;
                    let duration = if long {
                        3 * 3600
                    } else {
                        rng.range(3000, 4200)
                    };
                    ride(&mut rng, start, duration)
                }
                50..=79 => ActivityParams {
                    sport: Sport::Running,
                    power: PowerProfile::None,
                    heart_rate: HrProfile::Drift {
                        start: narrow_u8(rng.range(125, 140)),
                        end: narrow_u8(rng.range(150, 175)),
                    },
                    cadence_rpm: Some(narrow_u8(rng.range(82, 92))),
                    speed_mps: Some(rng.range(28, 38) as f32 / 10.0),
                    device: WATCH,
                    ..ActivityParams::cycling(start, 3600, rng.range(2400, 4200))
                },
                80..=94 => ActivityParams {
                    sport: Sport::Swimming,
                    power: PowerProfile::None,
                    heart_rate: HrProfile::None,
                    speed_mps: Some(rng.range(9, 12) as f32 / 10.0),
                    pool_length_m: Some(25.0),
                    device: WATCH,
                    ..ActivityParams::cycling(start, 3600, rng.range(1800, 3600))
                },
                _ => ActivityParams {
                    sport: Sport::Strength,
                    power: PowerProfile::None,
                    heart_rate: HrProfile::Constant(narrow_u8(rng.range(95, 120))),
                    speed_mps: None,
                    device: WATCH,
                    ..ActivityParams::cycling(start, 3600, rng.range(2400, 3600))
                },
            };
            (format!("history_{i:04}.fit"), p)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::activity;

    #[test]
    fn same_seed_gives_same_parameters() {
        assert_eq!(history(1, 50, 7), history(1, 50, 7));
        assert_ne!(history(1, 50, 7), history(1, 50, 8));
        assert_eq!(long_rides(3, 7), long_rides(3, 7));
    }

    #[test]
    fn long_rides_have_18000_records_with_all_channels() {
        let rides = long_rides(2, DEFAULT_SEED);
        assert_eq!(rides.len(), 2);
        for (name, p) in &rides {
            assert!(name.starts_with("long_ride_"));
            assert_eq!(p.record_count(), 18_000);
            assert!(p.power_at(0).is_some());
            assert!(p.heart_rate_at(0).is_some());
            assert!(p.cadence_rpm.is_some());
        }
    }

    #[test]
    fn history_spans_the_years_and_all_sports() {
        let h = history(5, 1500, DEFAULT_SEED);
        assert_eq!(h.len(), 1500);
        let last = h.last().unwrap().1.start_fit;
        assert!(last - HISTORY_START_FIT > 4 * 365 * DAY_S);
        for sport in [
            Sport::Cycling,
            Sport::Running,
            Sport::Swimming,
            Sport::Strength,
        ] {
            assert!(h.iter().any(|(_, p)| p.sport == sport), "{sport:?}");
        }
    }

    #[test]
    fn files_are_valid_fit() {
        // Kopflänge 14 und Kennung ".FIT" genügen hier, den Rest prüft der Parser in seinen
        // eigenen Tests.
        let (_, p) = &long_rides(1, DEFAULT_SEED)[0];
        let bytes = activity(p);
        assert_eq!(bytes[0], 14);
        assert_eq!(&bytes[8..12], b".FIT");
    }
}
