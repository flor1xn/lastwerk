//! Leistungskurve: beste Leistung je Dauer im Zeitraum gegen den Vorzeitraum (SPEC 4.2 F35,
//! 6.2 `activity_best_power`).
//!
//! Eingang sind schon aggregierte Bestwerte, in der Regel das Maximum der Bestwerte aller
//! Aktivitäten eines Zeitraums (zum Beispiel 90 Tage gegen die 90 davor). Kommt eine Dauer
//! mehrfach vor, zählt das Maximum, die Funktion aggregiert also auch ungeordnete Listen.
//! Die Dauern sind [`BEST_POWER_DURATIONS_S`] (5 s, 1 min, 5 min, 20 min, 1 h), für die ganze
//! Kurve (T33) beliebige über [`compare_power_curves_over`].
//!
//! Der Vergleich ist rein beschreibend. Ein Rückgang kann an fehlenden Maximalbelastungen im
//! Zeitraum liegen und nicht an der Form, deshalb heißt die Deutung nur "gestiegen",
//! "gleich" oder "gesunken". Das Band für "gleich" ist die Faustregel [`TREND_BAND_PERCENT`].

use serde::{Deserialize, Serialize};

use super::{TREND_BAND_PERCENT, TrendState, change_percent};
use crate::best_power::{BEST_POWER_DURATIONS_S, BestPower};

/// Bestleistung einer Dauer im Zeitraum und im Vorzeitraum.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PowerCurveCompare {
    pub duration_s: u32,
    pub current_w: Option<f32>,
    pub previous_w: Option<f32>,
    /// Änderung in Prozent des Vorzeitraums, `None` wenn einer der Werte fehlt.
    pub change_percent: Option<f64>,
    pub trend: TrendState,
}

/// Höchste Leistung einer Dauer in der Liste (nur endliche, positive Werte).
fn best_of(list: &[BestPower], duration_s: u32) -> Option<f32> {
    list.iter()
        .filter(|b| b.duration_s == duration_s && b.watts.is_finite() && b.watts > 0.0)
        .map(|b| b.watts)
        .reduce(f32::max)
}

/// Vergleicht die Bestwerte zweier Zeiträume für alle Dauern aus [`BEST_POWER_DURATIONS_S`],
/// in deren Reihenfolge. Fehlt eine Dauer in einem Zeitraum, ist `change_percent` `None` und
/// die Deutung `NoData`. Es wird nichts mit Null ersetzt.
pub fn compare_power_curves(
    current: &[BestPower],
    previous: &[BestPower],
) -> Vec<PowerCurveCompare> {
    compare_power_curves_over(&BEST_POWER_DURATIONS_S, current, previous)
}

/// Wie [`compare_power_curves`], aber für beliebige Dauern in deren Reihenfolge (T33: die ganze
/// Kurve aus `POWER_CURVE_DURATIONS_S`).
pub fn compare_power_curves_over(
    durations_s: &[u32],
    current: &[BestPower],
    previous: &[BestPower],
) -> Vec<PowerCurveCompare> {
    durations_s
        .iter()
        .map(|&duration_s| {
            let c = best_of(current, duration_s);
            let p = best_of(previous, duration_s);
            let change = change_percent(c.map(f64::from), p.map(f64::from));
            PowerCurveCompare {
                duration_s,
                current_w: c,
                previous_w: p,
                change_percent: change,
                trend: TrendState::from_change(change, TREND_BAND_PERCENT),
            }
        })
        .collect()
}

/// Stabiler Code der Deutung einer Dauer, Schlüssel für Texte in `catalog/locales`.
pub const fn power_curve_trend_code(state: TrendState) -> &'static str {
    match state {
        TrendState::NoData => "power_curve.no_data",
        TrendState::Falling => "power_curve.falling",
        TrendState::Flat => "power_curve.flat",
        TrendState::Rising => "power_curve.rising",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bp(duration_s: u32, watts: f32) -> BestPower {
        BestPower { duration_s, watts }
    }

    /// Zeitraum: 5 s 900 W, 20 min 270 W. Vorzeitraum: 5 s 880 W, 20 min 250 W, 1 h 220 W.
    /// 5 s: `(900 - 880) / 880 * 100 = 2,2727` Prozent, über dem Band von 2: gestiegen.
    /// 20 min: `(270 - 250) / 250 * 100 = 8` Prozent: gestiegen.
    /// 1 h: im Zeitraum kein Wert: keine Aussage.
    const CHANGE_5_S: f64 = 2.2727;
    const CHANGE_20_MIN: f64 = 8.0;

    #[test]
    fn compares_every_standard_duration_in_order() {
        let cur = [bp(1200, 270.0), bp(5, 900.0)];
        let prev = [bp(5, 880.0), bp(1200, 250.0), bp(3600, 220.0)];
        let r = compare_power_curves(&cur, &prev);
        let durations: Vec<u32> = r.iter().map(|x| x.duration_s).collect();
        assert_eq!(durations, BEST_POWER_DURATIONS_S);

        assert_eq!(r[0].current_w, Some(900.0));
        assert!((r[0].change_percent.unwrap() - CHANGE_5_S).abs() < 1e-3);
        assert_eq!(r[0].trend, TrendState::Rising);

        // 60 s und 300 s fehlen in beiden Zeiträumen.
        assert_eq!(r[1].trend, TrendState::NoData);
        assert_eq!(r[2].change_percent, None);

        assert!((r[3].change_percent.unwrap() - CHANGE_20_MIN).abs() < 1e-3);

        // 1 h nur im Vorzeitraum.
        assert_eq!(r[4].current_w, None);
        assert_eq!(r[4].previous_w, Some(220.0));
        assert_eq!(r[4].trend, TrendState::NoData);
    }

    #[test]
    fn falling_flat_and_duplicates() {
        // Doppelte Dauer: das Maximum zählt (300 W, nicht 280 W).
        let cur = [bp(300, 280.0), bp(300, 300.0), bp(60, 400.0), bp(5, 1000.0)];
        let prev = [bp(300, 320.0), bp(60, 399.0), bp(5, f32::NAN)];
        let r = compare_power_curves(&cur, &prev);
        // 300 s: (300 - 320) / 320 * 100 = -6,25 Prozent: gesunken.
        assert_eq!(r[2].current_w, Some(300.0));
        assert_eq!(r[2].trend, TrendState::Falling);
        // 60 s: (400 - 399) / 399 * 100 = 0,25 Prozent: gleich.
        assert_eq!(r[1].trend, TrendState::Flat);
        // Unbrauchbarer Vorwert zählt als fehlend, nicht als Null.
        assert_eq!(r[0].previous_w, None);
        assert_eq!(r[0].trend, TrendState::NoData);
    }

    #[test]
    fn empty_inputs_give_five_rows_without_data() {
        let r = compare_power_curves(&[], &[]);
        assert_eq!(r.len(), 5);
        assert!(r.iter().all(|x| x.trend == TrendState::NoData));
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(
            power_curve_trend_code(TrendState::Rising),
            "power_curve.rising"
        );
        assert_eq!(power_curve_trend_code(TrendState::Flat), "power_curve.flat");
        assert_eq!(
            power_curve_trend_code(TrendState::Falling),
            "power_curve.falling"
        );
        assert_eq!(
            power_curve_trend_code(TrendState::NoData),
            "power_curve.no_data"
        );
    }
}
