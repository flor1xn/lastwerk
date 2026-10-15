//! Katalog der Modelle, die Lastwerk zum Laden anbietet (`catalog/models.json`, T29).
//!
//! Die Datei ist zur Übersetzungszeit eingebettet (`include_str!`), das fertige Programm braucht
//! sie nicht auf der Platte. Alle Zahlen darin sind Faustregel (siehe `recommend`), nicht
//! gemessen. Genau ein Eintrag ist das Startmodell.

use serde::Deserialize;
use thiserror::Error;

const EMBEDDED_JSON: &str = include_str!("../../../catalog/models.json");

/// Längster Modellname in Zeichen (wie in `provider` und `discover`, Konvention).
const MAX_NAME: usize = 200;

/// Höchstzahl Einträge (Konvention): Der Katalog soll eine kurze Auswahl bleiben.
const MAX_ENTRIES: usize = 20;

/// Ein Modell des Katalogs.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogModel {
    /// Name bei Ollama, zum Beispiel `qwen3:4b`.
    pub name: String,
    /// Das Startmodell, das der Installer und der Erststart vorschlagen. Genau eines.
    #[serde(default)]
    pub starter: bool,
    /// Größe der Datei in GB (Dezimal, wie die Ollama-Bibliothek sie nennt).
    pub size_gb: f64,
    /// Arbeitsspeicher des Rechners in GB, ab dem das Modell laufen kann (Faustregel).
    pub min_ram_gb: f64,
    /// Grafikspeicher in GB, bei dem das Modell samt Kontext komplett auf der Grafikkarte liegt
    /// (Faustregel).
    pub comfortable_vram_gb: f64,
    /// Schlüssel des Anzeigetextes in `catalog/locales`.
    pub label_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogFile {
    schema_version: u32,
    /// Herkunft und Prüfstand der Zahlen, nur zum Lesen.
    #[allow(dead_code)]
    source: String,
    models: Vec<CatalogModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogError {
    #[error("the catalog is not valid json")]
    Json,
    #[error("unsupported schema version {0}")]
    SchemaVersion(u32),
    #[error("the catalog is empty or too long")]
    Size,
    #[error("a model name is invalid or doubled")]
    Name,
    #[error("a number of a model is not positive and finite")]
    Number,
    #[error("a label key is empty")]
    Label,
    #[error("the catalog needs exactly one starter model")]
    Starter,
}

/// Ob `name` ein Modellname im einfachen Muster ist, das Lastwerk an Ollama schickt:
/// `^[a-z0-9][a-z0-9._-]*(:[a-z0-9._-]+)?$`, höchstens 200 Zeichen. Dasselbe Muster prüft das
/// Skript `ollama-setup.ps1` (T29 Teil A); es verhindert, dass ein Name als Befehl oder Pfad
/// gedeutet wird.
pub fn is_valid_model_name(name: &str) -> bool {
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return false;
    }
    let tail_ok = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
    };
    let (base, tag) = match name.split_once(':') {
        Some((b, t)) => (b, Some(t)),
        None => (name, None),
    };
    let first_ok = base
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    first_ok && tail_ok(base) && tag.is_none_or(|t| !t.is_empty() && tail_ok(t))
}

fn positive(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

/// Liest und prüft einen Katalog.
pub fn parse_catalog(json: &str) -> Result<Vec<CatalogModel>, CatalogError> {
    let file: CatalogFile = serde_json::from_str(json).map_err(|_| CatalogError::Json)?;
    if file.schema_version != 1 {
        return Err(CatalogError::SchemaVersion(file.schema_version));
    }
    if file.models.is_empty() || file.models.len() > MAX_ENTRIES {
        return Err(CatalogError::Size);
    }
    for (i, m) in file.models.iter().enumerate() {
        if !is_valid_model_name(&m.name) || file.models[..i].iter().any(|o| o.name == m.name) {
            return Err(CatalogError::Name);
        }
        if !(positive(m.size_gb) && positive(m.min_ram_gb) && positive(m.comfortable_vram_gb)) {
            return Err(CatalogError::Number);
        }
        if m.label_key.trim().is_empty() {
            return Err(CatalogError::Label);
        }
    }
    if file.models.iter().filter(|m| m.starter).count() != 1 {
        return Err(CatalogError::Starter);
    }
    Ok(file.models)
}

/// Der eingebettete Katalog. Ein Test stellt sicher, dass er gültig ist; fällt er doch aus,
/// bleibt die Liste leer, und die App zeigt nur installierte Modelle.
pub fn embedded_catalog() -> Vec<CatalogModel> {
    parse_catalog(EMBEDDED_JSON).unwrap_or_default()
}

/// Das Startmodell des Katalogs.
pub fn starter(models: &[CatalogModel]) -> Option<&CatalogModel> {
    models.iter().find(|m| m.starter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_catalog_is_valid_and_has_the_planned_models() {
        let models = parse_catalog(EMBEDDED_JSON).unwrap();
        let names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["qwen3:1.7b", "qwen3:4b", "qwen3:8b", "qwen3:14b"]);
        assert_eq!(starter(&models).unwrap().name, "qwen3:4b");
        assert_eq!(embedded_catalog(), models);
        // Größer heißt mehr Speicher: Die Einstufung in `recommend` setzt das voraus.
        for pair in models.windows(2) {
            assert!(pair[0].size_gb < pair[1].size_gb);
            assert!(pair[0].min_ram_gb < pair[1].min_ram_gb);
            assert!(pair[0].comfortable_vram_gb < pair[1].comfortable_vram_gb);
        }
    }

    #[test]
    fn model_names_follow_the_simple_pattern() {
        for ok in ["qwen3", "qwen3:4b", "llama3.2:3b", "a-b_c.d:q4_k-m", "0x:1"] {
            assert!(is_valid_model_name(ok), "{ok}");
        }
        for bad in [
            "",
            ":4b",
            "qwen3:",
            "Qwen3:4b",
            "-qwen3",
            ".x",
            "a b",
            "a;b",
            "a:b:c",
            "hf.co/user/m",
            "..\\x",
            "a\nb",
            &"x".repeat(201),
        ] {
            assert!(!is_valid_model_name(bad), "{bad:?}");
        }
        assert!(is_valid_model_name(&"x".repeat(200)));
    }

    fn one(extra: &str) -> String {
        format!(
            r#"{{"schema_version":1,"source":"t","models":[{{"name":"a:1b","starter":true,"size_gb":1,"min_ram_gb":4,"comfortable_vram_gb":2,"label_key":"k"{extra}}}]}}"#
        )
    }

    #[test]
    fn invalid_catalogs_are_rejected() {
        assert!(parse_catalog(&one("")).is_ok());
        assert_eq!(parse_catalog("{"), Err(CatalogError::Json));
        assert_eq!(parse_catalog(&one(r#","x":1"#)), Err(CatalogError::Json));
        assert_eq!(
            parse_catalog(&one("").replace("\"schema_version\":1", "\"schema_version\":2")),
            Err(CatalogError::SchemaVersion(2))
        );
        assert_eq!(
            parse_catalog(&one("").replace("a:1b", "A:1b")),
            Err(CatalogError::Name)
        );
        assert_eq!(
            parse_catalog(&one("").replace("\"size_gb\":1", "\"size_gb\":0")),
            Err(CatalogError::Number)
        );
        assert_eq!(
            parse_catalog(&one("").replace("\"label_key\":\"k\"", "\"label_key\":\" \"")),
            Err(CatalogError::Label)
        );
        // Kein Startmodell.
        assert_eq!(
            parse_catalog(&one("").replace("\"starter\":true,", "")),
            Err(CatalogError::Starter)
        );
        // Zwei Startmodelle und ein doppelter Name.
        let model = r#"{"name":"a:1b","starter":true,"size_gb":1,"min_ram_gb":4,"comfortable_vram_gb":2,"label_key":"k"}"#;
        let two = format!(r#"{{"schema_version":1,"source":"t","models":[{model},{model}]}}"#);
        assert_eq!(parse_catalog(&two), Err(CatalogError::Name));
        let other = model.replace("a:1b", "b:1b");
        let two = format!(r#"{{"schema_version":1,"source":"t","models":[{model},{other}]}}"#);
        assert_eq!(parse_catalog(&two), Err(CatalogError::Starter));
        let empty = r#"{"schema_version":1,"source":"t","models":[]}"#;
        assert_eq!(parse_catalog(empty), Err(CatalogError::Size));
    }
}
