//! Kennzahlen für das Dashboard (SPEC 4.2 F20, F35, F36; T25 Teil A).
//!
//! Reine Funktionen über übergebene Daten: kein Dateisystem, keine Uhr. Jede Kennzahl hat
//! eine Berechnung und eine Deutung. Die Deutung liefert einen stabilen Code (Aufzählung mit
//! `code()`), nie Text. Texte entstehen erst in der Oberfläche aus `catalog/locales`.
//!
//! Schwellen stammen aus `rules.default.json` (R1, R8, R12), wo die Regeln sie schon führen,
//! sonst sind sie hier als Faustregel benannt, mit Wert und Begründung als Konstante. Eine
//! Faustregel ist ein Parameter und kein verstecktes Literal (SPEC 11.1, Punkt 4).
//!
//! Bewusst nicht enthalten: Monotonie und Strain nach Foster 1998. Die Quelle steht nicht in
//! SPEC Anhang A (Q4 ist Foster 2001, Session-RPE), also entfällt die Kennzahl.

pub mod adherence;
pub mod distribution;
pub mod efficiency;
pub mod form;
pub mod laps;
pub mod power_curve;
pub mod ramp;
pub mod readiness;

use serde::{Deserialize, Serialize};

/// Bandbreite in Prozent, innerhalb derer der Vergleich zweier Zeiträume als "gleich" gilt
/// (Faustregel). Sie soll über der Streuung liegen, die Wiederholmessungen von Leistung und
/// Herzfrequenz ohnehin haben, und verhindert, dass Rauschen als Trend gedeutet wird. Der Wert
/// ist ein Parameter dieser Funktionen und lässt sich hier korrigieren, ohne Logik zu ändern.
pub const TREND_BAND_PERCENT: f64 = 2.0;

/// Richtung eines Vergleichs zweier Zeiträume (Effizienz, Leistungskurve).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendState {
    /// Einer der beiden Zeiträume hat nicht genug Daten.
    NoData,
    Falling,
    Flat,
    Rising,
}

impl TrendState {
    /// Deutung der prozentualen Änderung gegenüber dem Vorzeitraum. Die Grenzen gelten "ab":
    /// genau `band_percent` Änderung zählt schon als Anstieg oder Rückgang (wie bei R2).
    pub fn from_change(change_percent: Option<f64>, band_percent: f64) -> Self {
        match change_percent {
            Some(c) if c.is_finite() => {
                // Kleine Toleranz gegen Rundungsreste der Prozentrechnung.
                if c >= band_percent - 1e-9 {
                    Self::Rising
                } else if c <= -band_percent + 1e-9 {
                    Self::Falling
                } else {
                    Self::Flat
                }
            }
            _ => Self::NoData,
        }
    }
}

/// Änderung von `current` gegenüber `previous` in Prozent von `previous`. `None`, wenn ein
/// Wert fehlt, nicht endlich ist oder `previous` nicht positiv ist.
pub fn change_percent(current: Option<f64>, previous: Option<f64>) -> Option<f64> {
    let (c, p) = (current?, previous?);
    (c.is_finite() && p.is_finite() && p > 0.0).then(|| (c - p) / p * 100.0)
}

/// Standardregeln für Tests der Deutung (`catalog/rules.default.json`). Nur der Test lädt die
/// Datei, der Produktivcode bekommt die Regeln übergeben.
#[cfg(test)]
pub(crate) fn default_rules() -> crate::plan::Rules {
    serde_json::from_str(include_str!("../../../../catalog/rules.default.json"))
        .expect("rules.default.json lässt sich lesen")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_percent_is_relative_to_the_previous_value() {
        // (110 - 100) / 100 * 100 = 10
        assert_eq!(change_percent(Some(110.0), Some(100.0)), Some(10.0));
        // (90 - 120) / 120 * 100 = -25
        assert_eq!(change_percent(Some(90.0), Some(120.0)), Some(-25.0));
        assert_eq!(change_percent(None, Some(100.0)), None);
        assert_eq!(change_percent(Some(1.0), Some(0.0)), None);
        assert_eq!(change_percent(Some(f64::NAN), Some(1.0)), None);
    }

    #[test]
    fn trend_bands_are_inclusive_at_the_limit() {
        assert_eq!(TrendState::from_change(None, 2.0), TrendState::NoData);
        assert_eq!(TrendState::from_change(Some(2.0), 2.0), TrendState::Rising);
        assert_eq!(
            TrendState::from_change(Some(-2.0), 2.0),
            TrendState::Falling
        );
        assert_eq!(TrendState::from_change(Some(1.99), 2.0), TrendState::Flat);
        assert_eq!(TrendState::from_change(Some(-1.99), 2.0), TrendState::Flat);
    }
}
