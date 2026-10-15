//! `explain_rule`: Parameter, Quelle und Text einer Regel (SPEC 8.4).
//!
//! Alles stammt aus eingebetteten Katalogdateien des Programms (`catalog/rules.default.json`
//! und `catalog/locales/*.json`), nichts aus der Datenbank außer der Wahl der Sprache. Die
//! Dateien sind per `include_str!` eingebettet (Konvention wie beim Zonenkatalog in T05): das
//! fertige Programm läuft ohne Nebendateien.

use std::collections::BTreeMap;

use lastwerk_core::Rules;
use lastwerk_core::plan::Hardness;
use serde_json::Value;

use crate::error::{ToolError, ToolResult};
use crate::views::RuleExplanation;

const RULES_JSON: &str = include_str!("../../../catalog/rules.default.json");
const DE_JSON: &str = include_str!("../../../catalog/locales/de.json");
const EN_JSON: &str = include_str!("../../../catalog/locales/en.json");

/// Einstellung mit der Sprache der Oberfläche (`de` oder `en`), siehe SPEC 5.4 (T06).
pub const SETTING_LOCALE: &str = "app.locale";

/// Schlüssel einer Regel, die keine Parameter sind, sondern ihre Angaben (`RuleMeta`).
const META_KEYS: [&str; 4] = ["source", "references", "hardness", "note"];

/// Sprache der Texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    De,
    En,
}

impl Language {
    /// Wert der Einstellung `app.locale`. Alles außer `de` ergibt Englisch, damit kein Text
    /// aus der Einstellung selbst in die Antwort gelangt.
    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("de") => Self::De,
            _ => Self::En,
        }
    }
}

fn text<'a>(root: &'a Value, code: &str, field: &str) -> Option<&'a str> {
    root.get("rule")?.get(code)?.get(field)?.as_str()
}

/// Erklärung der Regel `code` (`R01` bis `R12`).
pub fn explain(code: &str, language: Language) -> ToolResult<RuleExplanation> {
    let raw: Value = serde_json::from_str(RULES_JSON).map_err(|_| ToolError::Internal)?;
    let rules: Rules = serde_json::from_value(raw.clone()).map_err(|_| ToolError::Internal)?;
    let Some((_, meta)) = rules.metas().into_iter().find(|(c, _)| *c == code) else {
        return Err(ToolError::NotFound(format!(
            "no rule {code} (rules R01 to R12 exist)"
        )));
    };
    let locale: Value = serde_json::from_str(match language {
        Language::De => DE_JSON,
        Language::En => EN_JSON,
    })
    .map_err(|_| ToolError::Internal)?;

    let parameters: BTreeMap<String, Value> = raw
        .get(code)
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter(|(k, v)| !META_KEYS.contains(&k.as_str()) && v.is_number())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();

    Ok(RuleExplanation {
        code: code.to_string(),
        title: text(&locale, code, "name").unwrap_or_default().to_string(),
        explanation: text(&locale, code, "explanation")
            .unwrap_or_default()
            .to_string(),
        hardness: match meta.hardness {
            Hardness::Hard => "hard",
            Hardness::Soft => "soft",
        }
        .to_string(),
        source: meta.source.clone(),
        references: meta.references.clone(),
        parameters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r03_has_its_parameter_source_and_text() {
        let e = explain("R03", Language::En).unwrap();
        assert_eq!(e.code, "R03");
        assert_eq!(e.hardness, "hard");
        assert_eq!(e.source, "Faustregel");
        assert_eq!(e.references, vec!["Q5".to_string()]);
        assert_eq!(e.parameters.len(), 1);
        assert_eq!(e.parameters["max_hard_per_week"], 2);
        assert_eq!(e.title, "Hard sessions per week");
        assert!(e.explanation.contains("Q5"));
    }

    #[test]
    fn german_text_follows_the_setting() {
        let de = explain("R03", Language::De).unwrap();
        let en = explain("R03", Language::En).unwrap();
        assert_ne!(de.title, en.title);
        assert!(!de.title.is_empty());
        assert_eq!(Language::from_setting(Some("de")), Language::De);
        assert_eq!(Language::from_setting(Some("fr")), Language::En);
        assert_eq!(Language::from_setting(None), Language::En);
    }

    #[test]
    fn every_rule_has_text_source_and_a_known_hardness_in_both_languages() {
        for n in 1..=12 {
            let code = format!("R{n:02}");
            for lang in [Language::De, Language::En] {
                let e = explain(&code, lang).unwrap();
                assert!(!e.title.is_empty(), "{code}");
                assert!(!e.explanation.is_empty(), "{code}");
                assert!(["hard", "soft"].contains(&e.hardness.as_str()));
                // Quelle: Nummer aus Anhang A oder eine der Kennzeichnungen (SPEC 8.4).
                assert!(lastwerk_core::plan::is_valid_source(&e.source), "{code}");
            }
        }
    }

    #[test]
    fn unknown_codes_are_not_found() {
        // R12 ist seit T14 eine echte Regel, reserviert ist nichts mehr.
        for code in ["R00", "R13", "R99"] {
            assert!(matches!(
                explain(code, Language::En),
                Err(ToolError::NotFound(_))
            ));
        }
    }

    #[test]
    fn only_numeric_parameters_are_returned() {
        // R06 trägt neben Zahlen den Text "note"; der gehört nicht zu den Parametern.
        let e = explain("R06", Language::En).unwrap();
        assert!(e.parameters.contains_key("load_weeks"));
        assert!(!e.parameters.contains_key("note"));
        assert!(e.parameters.values().all(Value::is_number));
    }
}
