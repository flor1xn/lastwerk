//! Formzustand aus dem TSB (SPEC 5.5, "Formzustand").
//!
//! Vier Zustände nach dem TSB des letzten Tages. Die Grenze zwischen "belastet" und "überlastet"
//! ist die von R10 (`R10.tsb_warn_below`, Faustregel), die anderen beiden sind Faustregeln ohne
//! begutachtete Quelle. Die Deutung beschreibt Trainingsdaten und sagt nichts über Gesundheit
//! (SPEC 2.3).

use serde::{Deserialize, Serialize};

use crate::plan::Rules;

/// Ab diesem TSB gilt die Form als "frisch" (Faustregel).
pub const FORM_FRESH_FROM: f32 = 5.0;

/// Ab diesem TSB bis unter [`FORM_FRESH_FROM`] gilt die Form als "ausgewogen" (Faustregel).
pub const FORM_BALANCED_FROM: f32 = -10.0;

/// Toleranz der Grenzprüfung (Rundungsreste der f32-Rechnung).
const LIMIT_EPS: f32 = 1e-4;

/// Deutung der Form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormState {
    /// TSB ab [`FORM_FRESH_FROM`].
    Fresh,
    /// TSB von [`FORM_BALANCED_FROM`] bis unter [`FORM_FRESH_FROM`].
    Balanced,
    /// TSB von `R10.tsb_warn_below` bis unter [`FORM_BALANCED_FROM`].
    Loaded,
    /// TSB unter `R10.tsb_warn_below`.
    Overloaded,
}

impl FormState {
    /// Stabiler Code. Die Texte stehen unter `ui.form.<name>` in `catalog/locales`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Fresh => "form.fresh",
            Self::Balanced => "form.balanced",
            Self::Loaded => "form.loaded",
            Self::Overloaded => "form.overloaded",
        }
    }
}

/// Deutet den TSB. Die Grenzen gelten "ab": genau +5 ist schon frisch, genau -10 noch
/// ausgewogen, genau die Grenze von R10 noch belastet.
pub fn form_state(tsb: f32, rules: &Rules) -> FormState {
    if tsb >= FORM_FRESH_FROM - LIMIT_EPS {
        FormState::Fresh
    } else if tsb >= FORM_BALANCED_FROM - LIMIT_EPS {
        FormState::Balanced
    } else if tsb >= rules.r10.tsb_warn_below - LIMIT_EPS {
        FormState::Loaded
    } else {
        FormState::Overloaded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::default_rules;

    /// Grenzfälle der Tabelle in SPEC 5.5 mit der Grenze -30 aus `rules.default.json`:
    /// +5 und darüber frisch, -10 bis unter +5 ausgewogen, -30 bis unter -10 belastet,
    /// darunter überlastet.
    #[test]
    fn states_follow_the_table_in_the_spec() {
        let rules = default_rules();
        assert_eq!(form_state(12.0, &rules), FormState::Fresh);
        assert_eq!(form_state(5.0, &rules), FormState::Fresh);
        assert_eq!(form_state(4.9, &rules), FormState::Balanced);
        assert_eq!(form_state(-10.0, &rules), FormState::Balanced);
        assert_eq!(form_state(-10.1, &rules), FormState::Loaded);
        assert_eq!(form_state(-30.0, &rules), FormState::Loaded);
        assert_eq!(form_state(-30.1, &rules), FormState::Overloaded);
        assert_eq!(form_state(-80.0, &rules), FormState::Overloaded);
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(FormState::Fresh.code(), "form.fresh");
        assert_eq!(FormState::Overloaded.code(), "form.overloaded");
    }
}
