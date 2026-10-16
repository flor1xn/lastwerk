//! Coach-Erkenntnisse auf "Heute": Auswahl und Reihenfolge der Zustandscodes (SPEC 8.7, T44).
//!
//! Die Kennzahlen deuten sich selbst (`code()` je Modul). Hier wird nur gewählt, welche
//! Deutungen oben stehen: zuerst Warnungen, dann positive Trends, höchstens
//! [`MAX_INSIGHTS`]. Reihenfolge und Anzahl sind eine Faustregel ohne Quelle: Was eine
//! Anpassung nahelegt, steht vor dem, was nur bestätigt. Neutrale Zustände (`*.no_data`,
//! `*.flat`, `ramp.steady` und ähnliche) erscheinen nie.

/// Höchstens so viele Erkenntnisse (Faustregel, damit "Heute" kurz bleibt).
pub const MAX_INSIGHTS: usize = 3;

/// Feste Reihenfolge der Codes, die als Erkenntnis taugen (Faustregel). Zuerst Warnungen:
/// Form überlastet (R10), Bereitschaft niedrig (R12), steile Rampe, viel Intensität, Plan
/// unter- oder übererfüllt. Danach positive Trends: Effizienz und Leistungskurve steigend,
/// Rampe im Aufbau.
pub const PRIORITY: [&str; 9] = [
    "form.overloaded",
    "readiness.low",
    "ramp.steep",
    "distribution.intensity_heavy",
    "adherence.under",
    "adherence.over",
    "efficiency.rising",
    "power_curve.rising",
    "ramp.building",
];

/// Wählt aus den aktuellen Zustandscodes die Erkenntnisse in fester Reihenfolge, ohne
/// Duplikate, höchstens [`MAX_INSIGHTS`]. Unbekannte und neutrale Codes fallen weg.
pub fn rank_insights(codes: &[&str]) -> Vec<&'static str> {
    PRIORITY
        .iter()
        .copied()
        .filter(|p| codes.contains(p))
        .take(MAX_INSIGHTS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::adherence::AdherenceState;
    use crate::metrics::distribution::DistributionState;
    use crate::metrics::form::FormState;
    use crate::metrics::ramp::RampState;
    use crate::metrics::readiness::ReadinessState;
    use crate::metrics::{TrendState, efficiency, power_curve};

    #[test]
    fn warnings_come_before_trends_and_at_most_three() {
        let codes = [
            "power_curve.rising",
            "ramp.steep",
            "efficiency.rising",
            "form.overloaded",
            "adherence.over",
        ];
        assert_eq!(
            rank_insights(&codes),
            vec!["form.overloaded", "ramp.steep", "adherence.over"]
        );
        // Nur Trends: in fester Reihenfolge.
        assert_eq!(
            rank_insights(&["ramp.building", "efficiency.rising"]),
            vec!["efficiency.rising", "ramp.building"]
        );
    }

    #[test]
    fn neutral_unknown_and_duplicate_codes_are_dropped() {
        assert!(rank_insights(&[]).is_empty());
        assert!(rank_insights(&["form.fresh", "ramp.steady", "readiness.no_data", "x"]).is_empty());
        assert_eq!(
            rank_insights(&["readiness.low", "readiness.low"]),
            vec!["readiness.low"]
        );
    }

    #[test]
    fn every_priority_code_exists_in_the_metrics() {
        // Die Liste nennt nur Codes, die die Kennzahlen wirklich liefern.
        let known = [
            FormState::Overloaded.code(),
            ReadinessState::Low.code(),
            RampState::Steep.code(),
            DistributionState::IntensityHeavy.code(),
            AdherenceState::Under.code(),
            AdherenceState::Over.code(),
            efficiency::ef_trend_code(TrendState::Rising),
            power_curve::power_curve_trend_code(TrendState::Rising),
            RampState::Building.code(),
        ];
        assert_eq!(known, PRIORITY);
    }
}
