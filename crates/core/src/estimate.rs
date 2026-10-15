//! Schätzungen von Schwellenwerten (SPEC 8.6, 8.5).
//!
//! Die Anwendung schlägt Werte vor und übernimmt sie nie selbst. Dieses Modul rechnet nur,
//! es speichert nichts. Ein Vorschlag wird erst zum Schwellenwert, wenn der Nutzer ihn
//! ausdrücklich übernimmt (Herkunft `suggested_accepted`, SPEC 6.2).
//!
//! Alle Faktoren sind Faustregeln oder Sekundärquellen und stehen hier als benannte
//! Konstanten, damit eine Korrektur den Rechenweg nicht ändert (SPEC 11.1).

use serde::{Deserialize, Serialize};

use crate::load::estimate_hr_max;

/// Anteil der besten 20-Minuten-Leistung, der als FTP vorgeschlagen wird: 95 Prozent.
/// Faustregel, Q16 nur als Sekundärquelle (SPEC 8.6). Gilt nur, wenn die 20 Minuten ein
/// maximaler Test waren.
pub const FTP_FRACTION_OF_BEST_20_MIN: f32 = 0.95;

/// Dauer des Tests, aus dem die LTHR folgt, in Sekunden: 30 Minuten (Faustregel der
/// Trainerliteratur, SPEC 8.6).
pub const LTHR_TEST_S: usize = 1800;

/// Dauer am Ende des Tests, über die die Herzfrequenz gemittelt wird: die letzten 20 Minuten
/// (Faustregel der Trainerliteratur, SPEC 8.6).
pub const LTHR_AVERAGING_S: usize = 1200;

/// FTP-Vorschlag aus der besten mittleren Leistung über 20 Minuten: 95 Prozent davon
/// (Faustregel, Q16 sekundär, SPEC 8.6). `None` bei nicht positivem oder nicht endlichem Wert.
pub fn suggest_ftp(best_20_min_w: f32) -> Option<f32> {
    (best_20_min_w.is_finite() && best_20_min_w > 0.0)
        .then_some(best_20_min_w * FTP_FRACTION_OF_BEST_20_MIN)
}

/// LTHR-Vorschlag aus der Herzfrequenz eines 30-Minuten-Tests (Faustregel, SPEC 8.6):
/// Mittelwert der letzten 20 Minuten.
///
/// `hr_bpm` ist die 1-Hz-Reihe des Tests. Liegt mehr als der Test vor, zählt das Ende der
/// Reihe: die letzten 1800 Werte sind der Test, davon die letzten 1200 der Mittelungsteil.
/// `None`, wenn weniger als 1800 Werte vorliegen oder ein Wert nicht endlich ist.
pub fn suggest_lthr_from_test(hr_bpm: &[f32]) -> Option<f32> {
    if hr_bpm.len() < LTHR_TEST_S || hr_bpm.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let tail = &hr_bpm[hr_bpm.len() - LTHR_AVERAGING_S..];
    let sum: f64 = tail.iter().map(|&v| f64::from(v)).sum();
    Some((sum / LTHR_AVERAGING_S as f64) as f32)
}

/// Kritische Schwimmgeschwindigkeit (CSS) aus zwei Zeitläufen (Q15, Wakayoshi): Steigung der
/// Geraden Strecke gegen Zeit, `CSS = (D_lang - D_kurz) / (T_lang - T_kurz)` in m/s.
///
/// Beispiel 400 m und 200 m. `None`, wenn die lange Strecke nicht länger und nicht langsamer
/// ist als die kurze oder ein Wert nicht positiv und endlich ist.
pub fn suggest_css(
    long_distance_m: f32,
    long_time_s: f32,
    short_distance_m: f32,
    short_time_s: f32,
) -> Option<f32> {
    let all = [long_distance_m, long_time_s, short_distance_m, short_time_s];
    if all.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
        return None;
    }
    if long_distance_m <= short_distance_m || long_time_s <= short_time_s {
        return None;
    }
    Some((long_distance_m - short_distance_m) / (long_time_s - short_time_s))
}

/// Maximale Herzfrequenz mit Herkunft: gemessen oder aus dem Alter geschätzt.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HrMax {
    pub bpm: f32,
    /// `true`, wenn der Wert aus dem Alter geschätzt ist (`208 - 0,7 * Alter`, Q7). Die
    /// Oberfläche und die Ausgaben kennzeichnen ihn dann als Schätzung (SPEC 8.5).
    pub estimated: bool,
}

/// Maximale Herzfrequenz für die Zonen: der gemessene Wert, sonst die Schätzung aus dem Alter
/// (Q7), sonst `None` (Zonen dieses Schemas sind nicht berechenbar, SPEC 8.5).
pub fn resolve_hr_max(measured_bpm: Option<f32>, age_years: Option<f32>) -> Option<HrMax> {
    if let Some(bpm) = measured_bpm {
        return Some(HrMax {
            bpm,
            estimated: false,
        });
    }
    age_years.map(|age| HrMax {
        bpm: estimate_hr_max(age),
        estimated: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-3;

    /// Beste 20-Minuten-Leistung 300 W: `0,95 * 300 = 285`.
    #[test]
    fn ftp_is_95_percent_of_best_20_minutes() {
        assert!((suggest_ftp(300.0).unwrap() - 285.0).abs() < EPS);
        assert_eq!(suggest_ftp(0.0), None);
        assert_eq!(suggest_ftp(f32::NAN), None);
        assert_eq!(suggest_ftp(-5.0), None);
    }

    /// 1800 Werte: die ersten 600 mit 150 bpm (Einlaufen), die letzten 1200 mit 170 bpm.
    /// Mittel der letzten 20 Minuten: 170.
    #[test]
    fn lthr_is_the_mean_of_the_last_20_minutes_of_a_30_minute_test() {
        let mut hr = vec![150.0_f32; 600];
        hr.extend(vec![170.0_f32; 1200]);
        assert!((suggest_lthr_from_test(&hr).unwrap() - 170.0).abs() < EPS);
        // Mehr Werte als der Test: davor liegende Werte zählen nicht.
        let mut longer = vec![100.0_f32; 500];
        longer.extend(hr);
        assert!((suggest_lthr_from_test(&longer).unwrap() - 170.0).abs() < EPS);
        assert_eq!(suggest_lthr_from_test(&longer[..1799]), None);
    }

    /// 400 m in 300 s, 200 m in 130 s: `CSS = (400 - 200) / (300 - 130) = 200 / 170 = 1,17647`.
    #[test]
    fn css_is_the_slope_of_distance_over_time() {
        let css = suggest_css(400.0, 300.0, 200.0, 130.0).unwrap();
        assert!((css - 1.17647).abs() < EPS, "{css}");
        // Falsche Reihenfolge oder Nullen sind keine Vorschläge.
        assert_eq!(suggest_css(200.0, 130.0, 400.0, 300.0), None);
        assert_eq!(suggest_css(400.0, 120.0, 200.0, 130.0), None);
        assert_eq!(suggest_css(400.0, 300.0, 0.0, 130.0), None);
    }

    /// T-HRMAX-1: Alter 40 ergibt `208 - 0,7 * 40 = 180` und ist als Schätzung gekennzeichnet.
    /// Ein gemessener Wert hat Vorrang und ist keine Schätzung.
    #[test]
    fn hr_max_prefers_the_measured_value() {
        let est = resolve_hr_max(None, Some(40.0)).unwrap();
        assert!((est.bpm - 180.0).abs() < EPS);
        assert!(est.estimated);
        let measured = resolve_hr_max(Some(188.0), Some(40.0)).unwrap();
        assert!((measured.bpm - 188.0).abs() < EPS);
        assert!(!measured.estimated);
        assert_eq!(resolve_hr_max(None, None), None);
    }
}
