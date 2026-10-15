//! Modelle eines lokalen Ollama-Servers finden und eines vorschlagen (T26).
//!
//! `GET /api/tags` an die geprüfte Basisadresse (`BaseUrl`, nur Loopback oder bestätigtes
//! lokales Netz), mit denselben Regeln wie die Aufrufe an das Modell: keine Weiterleitung, kein
//! Proxy aus der Umgebung, Zeitlimit, Antwort höchstens 2 MiB. Es wird nichts geladen und
//! nichts gesendet außer der Anfrage selbst.

use serde_json::Value;

use crate::client::{LlmError, get_json};
use crate::provider::BaseUrl;

/// Höchstzahl der Modellnamen, die übernommen werden (Konvention). Mehr hat kaum ein Rechner;
/// die Grenze hält eine fremde Antwort klein.
pub const MAX_MODELS: usize = 500;

/// Längster übernommener Modellname in Zeichen (wie in `provider`, Konvention).
const MAX_MODEL_NAME: usize = 200;

/// Vorzugsliste der Modellfamilien, die erste vorhandene gewinnt. Faustregel, nicht gemessen
/// (O13 offen): Familien mit Anweisungsfeinschliff, die strukturierte Ausgabe über `format`
/// gut einhalten, Deutsch ordentlich beherrschen und in kleinen Größen auf Rechnern ohne eigene
/// Grafikkarte laufen. Die Reihenfolge folgt dem Ruf bei JSON-Ausgabe, nicht einer Messung.
pub const PREFERRED_FAMILIES: &[&str] = &[
    "qwen3",
    "qwen2.5",
    "llama3.1",
    "gemma3",
    "mistral-small",
    "llama3.2",
];

/// Familie eines Modellnamens: ohne Namensraum (`hf.co/nutzer/`) und ohne Etikett (`:8b`),
/// klein geschrieben.
fn family(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    base.split(':').next().unwrap_or(base).to_ascii_lowercase()
}

fn usable_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed == name
        && name.chars().count() <= MAX_MODEL_NAME
        && !name.chars().any(char::is_control)
}

/// Die Namen der lokal vorhandenen Modelle in der Reihenfolge des Servers, ohne Doppel und ohne
/// unbrauchbare Namen (leer, zu lang, Steuerzeichen). `timeout_s` gilt für den ganzen Aufruf.
pub fn ollama_models(base: &BaseUrl, timeout_s: u32) -> Result<Vec<String>, LlmError> {
    let answer = get_json(&base.endpoint("/api/tags", false), timeout_s)?;
    let list = answer
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| LlmError::BadResponse("models is missing".into()))?;
    let mut names: Vec<String> = Vec::new();
    for entry in list {
        let Some(name) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        if usable_name(name) && !names.iter().any(|n| n == name) {
            names.push(name.to_string());
            if names.len() == MAX_MODELS {
                break;
            }
        }
    }
    Ok(names)
}

/// Wählt ein Modell: das erste aus der ersten vorhandenen Familie von [`PREFERRED_FAMILIES`],
/// sonst das erste Modell, das kein Einbettungsmodell ist (Name enthält `embed`; solche Modelle
/// können nicht antworten, Faustregel nach dem Namen). `None`, wenn nichts passt.
pub fn pick_model(models: &[String]) -> Option<String> {
    for preferred in PREFERRED_FAMILIES {
        if let Some(m) = models.iter().find(|m| family(m) == *preferred) {
            return Some(m.clone());
        }
    }
    models
        .iter()
        .find(|m| !m.to_ascii_lowercase().contains("embed"))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn family_ignores_namespace_and_tag() {
        assert_eq!(family("qwen3:8b"), "qwen3");
        assert_eq!(family("hf.co/someone/Qwen3:Q4_K_M"), "qwen3");
        assert_eq!(family("llama3.1"), "llama3.1");
    }

    #[test]
    fn the_first_present_family_of_the_preference_list_wins() {
        let models = names(&["phi4:14b", "llama3.2:3b", "qwen2.5:7b", "qwen2.5:14b"]);
        assert_eq!(pick_model(&models).as_deref(), Some("qwen2.5:7b"));
        let models = names(&["llama3.2:3b", "qwen3:4b"]);
        assert_eq!(pick_model(&models).as_deref(), Some("qwen3:4b"));
        // qwen3 ist nicht dasselbe wie qwen3-coder.
        let models = names(&["qwen3-coder:30b", "gemma3:4b"]);
        assert_eq!(pick_model(&models).as_deref(), Some("gemma3:4b"));
    }

    #[test]
    fn otherwise_the_first_model_that_is_not_an_embedding_model() {
        let models = names(&["nomic-embed-text:latest", "phi4:14b", "mistral:7b"]);
        assert_eq!(pick_model(&models).as_deref(), Some("phi4:14b"));
        assert_eq!(pick_model(&names(&["nomic-embed-text"])), None);
        assert_eq!(pick_model(&[]), None);
    }
}
