//! Anzeigetexte aus `catalog/locales/<locale>.json` (SPEC 7.5).
//!
//! Die Dateien sind per `include_str!` eingebettet (Konvention wie beim Zonenkatalog in T05).
//! Schlüssel sind Punktpfade durch die verschachtelten Objekte, zum Beispiel
//! `template.bike.endurance.steady.name`.

use lastwerk_core::plan::Locale;
use serde_json::Value;

use crate::error::CliError;

const DE_JSON: &str = include_str!("../../../catalog/locales/de.json");
const EN_JSON: &str = include_str!("../../../catalog/locales/en.json");

/// Texte einer Sprache.
pub struct Texts {
    root: Value,
}

impl Texts {
    pub fn load(locale: Locale) -> Result<Self, CliError> {
        let raw = match locale {
            Locale::De => DE_JSON,
            Locale::En => EN_JSON,
        };
        let root = serde_json::from_str(raw)
            .map_err(|e| CliError::Internal(format!("Die Sprachdatei ist kein JSON: {e}")))?;
        Ok(Self { root })
    }

    /// Text zu einem Punktpfad. `None`, wenn der Schlüssel fehlt oder kein Text ist.
    pub fn get(&self, key: &str) -> Option<&str> {
        let mut node = &self.root;
        for part in key.split('.') {
            node = node.get(part)?;
        }
        node.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_resolve_in_both_languages() {
        let de = Texts::load(Locale::De).unwrap();
        let en = Texts::load(Locale::En).unwrap();
        assert_eq!(
            de.get("template.bike.endurance.steady.name"),
            Some("Grundlagenfahrt")
        );
        assert!(en.get("template.bike.endurance.steady.name").is_some());
        assert_eq!(de.get("sport.cycling"), Some("Rad"));
        assert_eq!(en.get("sport.cycling"), Some("Cycling"));
        assert_eq!(de.get("template.bike"), None);
        assert_eq!(de.get("gibt.es.nicht"), None);
    }
}
