//! Zonenkatalog (SPEC 8.5, `catalog/zones.json`).
//!
//! `lastwerk-core` liest keine Dateien. Den Katalog laden Speicher und Programme, und zwar
//! hier: Die Datei ist zur Übersetzungszeit eingebettet (`include_str!`), damit das fertige
//! Programm ohne Nebendateien läuft. Der eingebettete Katalog ist der Standard. Wer einen
//! eigenen Katalog nutzen will, übergibt ihn über [`parse_zone_catalog`] in den
//! `ImportOptions`.

use lastwerk_core::ZoneCatalog;

use crate::error::{Result, StoreError};

/// Inhalt von `catalog/zones.json`, eingebettet zur Übersetzungszeit.
const EMBEDDED_ZONES_JSON: &str = include_str!("../../../catalog/zones.json");

/// Liest einen Zonenkatalog aus JSON und prüft jedes Schema (streng steigende Grenzen,
/// eindeutige Kennungen, Quelle und Prüfstatus angegeben).
pub fn parse_zone_catalog(json: &str) -> Result<ZoneCatalog> {
    let catalog: ZoneCatalog = serde_json::from_str(json)?;
    catalog
        .validate()
        .map_err(|e| StoreError::InvalidData(format!("zone catalog: {e}")))?;
    Ok(catalog)
}

/// Der eingebettete Katalog. Ein Test stellt sicher, dass er gültig ist; ein Fehler hier
/// wäre ein Fehler im Repository und kein Laufzeitfall.
pub fn embedded_zone_catalog() -> ZoneCatalog {
    parse_zone_catalog(EMBEDDED_ZONES_JSON).expect("catalog/zones.json is valid (checked by test)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lastwerk_core::{Sport, ZoneBasis};

    #[test]
    fn embedded_catalog_is_valid_and_complete() {
        let cat = embedded_zone_catalog();
        let ids: Vec<&str> = cat.schemes.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "power7",
                "hr_lthr5",
                "hr_karvonen",
                "run_speed5",
                "swim_css5"
            ]
        );
        assert_eq!(cat.scheme("power7").unwrap().zone_count(), 7);
        for id in ["hr_lthr5", "hr_karvonen", "run_speed5", "swim_css5"] {
            assert_eq!(cat.scheme(id).unwrap().zone_count(), 5, "{id}");
        }
    }

    /// Jede Grenze hat Herkunft und Prüfstatus (CLAUDE.md: keine Zahl ohne Quelle). Die
    /// Quelle ist eine Nummer aus SPEC Anhang A oder "Faustregel", der Status eine der beiden
    /// erlaubten Angaben.
    #[test]
    fn every_scheme_names_source_and_status() {
        let cat = embedded_zone_catalog();
        for s in &cat.schemes {
            let source_ok = s.source == "Faustregel"
                || (s.source.starts_with('Q') && s.source[1..].chars().all(|c| c.is_ascii_digit()));
            assert!(source_ok, "{}: Quelle {:?}", s.id, s.source);
            assert!(
                s.status == "Sekundärquelle" || s.status == "Faustregel",
                "{}: Status {:?}",
                s.id,
                s.status
            );
            assert!(!s.note.is_empty(), "{}: Hinweis fehlt", s.id);
        }
    }

    #[test]
    fn schemes_apply_to_the_expected_sports() {
        let cat = embedded_zone_catalog();
        let cycling: Vec<&str> = cat
            .for_sport(Sport::Cycling)
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(cycling, ["power7", "hr_lthr5", "hr_karvonen"]);
        let running: Vec<&str> = cat
            .for_sport(Sport::Running)
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(running, ["hr_lthr5", "hr_karvonen", "run_speed5"]);
        let swimming: Vec<&str> = cat
            .for_sport(Sport::Swimming)
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(swimming, ["hr_lthr5", "hr_karvonen", "swim_css5"]);
        assert_eq!(
            cat.scheme("hr_karvonen").unwrap().basis,
            ZoneBasis::HrReserve
        );
    }

    /// Alle Zonennamen des Katalogs stehen in beiden Sprachdateien (`zones.<id>.z<n>`).
    #[test]
    fn zone_names_exist_in_both_locales() {
        let cat = embedded_zone_catalog();
        for (lang, json) in [
            ("de", include_str!("../../../catalog/locales/de.json")),
            ("en", include_str!("../../../catalog/locales/en.json")),
        ] {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            for s in &cat.schemes {
                let node = &v["zones"][s.id.as_str()];
                assert!(node["name"].is_string(), "{lang}: zones.{}.name", s.id);
                for z in 1..=s.zone_count() {
                    assert!(
                        node[format!("z{z}")].is_string(),
                        "{lang}: zones.{}.z{z}",
                        s.id
                    );
                }
            }
        }
    }

    #[test]
    fn broken_catalogs_are_rejected() {
        assert!(parse_zone_catalog("{}").is_err());
        let unordered = r#"{"format_version":1,"schemes":[{"id":"x","basis":"ftp",
            "sports":["cycling"],"bounds":[0.9,0.5],"source":"Faustregel","status":"Faustregel"}]}"#;
        assert!(parse_zone_catalog(unordered).is_err());
    }
}
