//! Empfehlung eines Modells nach der Hardware (T29, SPEC 9.2).
//!
//! Reine Funktion ohne Systemzugriff: Der Aufrufer misst Arbeitsspeicher und Grafikspeicher und
//! übergibt sie als [`Hardware`]. Alle Schwellen sind **Faustregel**, nicht gemessen (O13): Sie
//! sollen verhindern, dass ein Rechner ein Modell lädt, das nicht läuft oder Minuten braucht,
//! und sie sind absichtlich vorsichtig. Der Nutzer kann jedes Modell trotzdem wählen.
//!
//! Begründung der Regeln:
//!
//! - **Passt in den Speicher.** Ein Modell in 4-Bit-Quantisierung braucht im Betrieb etwa seine
//!   Dateigröße plus Kontext (Faustregel: 1 bis 2 GB) im Arbeitsspeicher, und Windows samt
//!   Programmen des Nutzers brauchen selbst 4 bis 6 GB. Der Katalog nennt dafür `min_ram_gb`,
//!   grob die doppelte Dateigröße, aufgerundet auf Stufen der Speichergrößen im Handel
//!   (4, 8, 16, 24). Darunter ist das Modell `too_big`.
//! - **Schnell genug.** Die Antwort eines Modells auf der CPU wird vom Speicherdurchsatz
//!   begrenzt: Für jedes erzeugte Wort liest das Modell alle Gewichte einmal. Ein Rechner mit
//!   zwei Speicherkanälen schafft grob 40 bis 60 GB/s, also bei 2,5 GB Modellgröße im günstigen
//!   Fall 15 bis 20 Wörter je Sekunde und in der Praxis die Hälfte oder weniger. Eine Antwort des
//!   Coachs ist einige hundert Wörter lang und hat ein Zeitlimit von 120 s (`DEFAULT_TIMEOUT_S`),
//!   dazu kommen bis zu zwei Korrekturaufrufe. Bis [`CPU_FIT_MAX_GB`] Dateigröße ist das auf
//!   einer CPU erträglich, darüber nicht.
//! - **Grafikkarte.** Liegt das Modell samt Kontext komplett im Grafikspeicher
//!   (`comfortable_vram_gb`), ist es um ein Mehrfaches schneller als auf der CPU. Liegt nur ein
//!   Teil dort, läuft es mit Auslagerung und nur wenig schneller als auf der CPU; dann zählt
//!   wieder die Regel zur CPU.
//! - **Arbeitsspeicher wird kleiner gemeldet, als verbaut ist** (Grafikspeicher der
//!   Prozessorgrafik, Firmware): Ein Rechner mit 8 GB meldet oft 7,5 bis 8 GB.
//!   [`RAM_MARGIN_GB`] gleicht das aus.

use serde::Serialize;

use crate::catalog::{CatalogModel, starter};

/// Toleranz beim Vergleich des Arbeitsspeichers in GB (Faustregel, siehe oben).
pub const RAM_MARGIN_GB: f64 = 0.5;

/// Größte Dateigröße in GB, die auf einer CPU noch als schnell genug gilt (Faustregel, siehe
/// oben). Das Startmodell `qwen3:4b` (2,5 GB) liegt darunter, `qwen3:8b` (5,2 GB) darüber.
pub const CPU_FIT_MAX_GB: f64 = 3.0;

/// Gemessene Hardware. Speicher in GB (Dezimal, 1 GB = 10^9 Byte).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Hardware {
    /// Gesamter Arbeitsspeicher.
    pub ram_gb: f64,
    /// Grafikspeicher der größten Grafikkarte, `None` ohne erkennbare Karte.
    pub vram_gb: Option<f64>,
}

/// Einstufung eines Modells für eine Hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// Läuft in angemessener Zeit.
    Fits,
    /// Der Speicher reicht, aber ohne passende Grafikkarte antwortet das Modell langsam.
    Slow,
    /// Zu wenig Arbeitsspeicher.
    TooBig,
}

impl Fit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fits => "fits",
            Self::Slow => "slow",
            Self::TooBig => "too_big",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelFit {
    pub name: String,
    pub fit: Fit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recommendation {
    /// Einstufung je Modell in der Reihenfolge des Katalogs.
    pub models: Vec<ModelFit>,
    /// Das größte Modell mit `fits`, sonst das Startmodell. `None` nur bei leerem Katalog.
    pub recommended: Option<String>,
}

/// Stuft ein Modell ein.
pub fn classify(hw: &Hardware, model: &CatalogModel) -> Fit {
    if hw.ram_gb + RAM_MARGIN_GB < model.min_ram_gb {
        return Fit::TooBig;
    }
    let on_gpu = hw.vram_gb.is_some_and(|v| v >= model.comfortable_vram_gb);
    if on_gpu || model.size_gb <= CPU_FIT_MAX_GB {
        Fit::Fits
    } else {
        Fit::Slow
    }
}

/// Stuft alle Modelle ein und wählt die Empfehlung: das größte Modell (nach Dateigröße), das
/// `fits` ist, sonst das Startmodell, sonst `None`.
pub fn recommend(hw: &Hardware, catalog: &[CatalogModel]) -> Recommendation {
    let models: Vec<ModelFit> = catalog
        .iter()
        .map(|m| ModelFit {
            name: m.name.clone(),
            fit: classify(hw, m),
        })
        .collect();
    let largest_fit = catalog
        .iter()
        .zip(&models)
        .filter(|(_, f)| f.fit == Fit::Fits)
        .max_by(|(a, _), (b, _)| a.size_gb.total_cmp(&b.size_gb))
        .map(|(m, _)| m.name.clone());
    let recommended = largest_fit.or_else(|| starter(catalog).map(|m| m.name.clone()));
    Recommendation {
        models,
        recommended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::embedded_catalog;

    fn hw(ram: f64, vram: Option<f64>) -> Hardware {
        Hardware {
            ram_gb: ram,
            vram_gb: vram,
        }
    }

    fn fits(r: &Recommendation) -> Vec<(&str, &'static str)> {
        r.models
            .iter()
            .map(|m| (m.name.as_str(), m.fit.as_str()))
            .collect()
    }

    // Katalog (catalog/models.json): Größe, min_ram, comfortable_vram
    //   qwen3:1.7b 1,4 GB, 4 GB, 2,5 GB
    //   qwen3:4b   2,5 GB, 8 GB, 4 GB   (Startmodell)
    //   qwen3:8b   5,2 GB, 16 GB, 7 GB
    //   qwen3:14b  9,3 GB, 24 GB, 12 GB
    // Regel: too_big, wenn RAM + 0,5 < min_ram; fits, wenn VRAM >= comfortable_vram oder
    // Größe <= 3,0 GB; sonst slow.

    /// 8 GB ohne Grafikkarte: 8,0 + 0,5 >= 4 und >= 8, also 1.7b und 4b im Speicher, beide
    /// <= 3 GB, also fits. 8b: 8,5 < 16, 14b: 8,5 < 24, beide too_big. Empfehlung: das größere
    /// der beiden, qwen3:4b.
    #[test]
    fn eight_gb_without_gpu() {
        let r = recommend(&hw(8.0, None), &embedded_catalog());
        assert_eq!(
            fits(&r),
            [
                ("qwen3:1.7b", "fits"),
                ("qwen3:4b", "fits"),
                ("qwen3:8b", "too_big"),
                ("qwen3:14b", "too_big"),
            ]
        );
        assert_eq!(r.recommended.as_deref(), Some("qwen3:4b"));
    }

    /// Ein Rechner mit 8 GB meldet 7,6 GB: 7,6 + 0,5 = 8,1 >= 8, das Startmodell bleibt fits.
    /// Bei 7,4 GB: 7,9 < 8, das Startmodell ist too_big, 1.7b (7,9 >= 4) fits und wird
    /// empfohlen.
    #[test]
    fn the_margin_covers_reserved_memory() {
        let r = recommend(&hw(7.6, None), &embedded_catalog());
        assert_eq!(r.models[1].fit, Fit::Fits);
        assert_eq!(r.recommended.as_deref(), Some("qwen3:4b"));
        let r = recommend(&hw(7.4, None), &embedded_catalog());
        assert_eq!(r.models[1].fit, Fit::TooBig);
        assert_eq!(r.recommended.as_deref(), Some("qwen3:1.7b"));
    }

    /// 8 GB mit Grafikkarte von 8 GB: der Arbeitsspeicher entscheidet zuerst, 8b bleibt
    /// too_big (8,5 < 16), obwohl der Grafikspeicher (8 >= 7) reichte.
    #[test]
    fn eight_gb_with_gpu_keeps_the_memory_limit() {
        let r = recommend(&hw(8.0, Some(8.0)), &embedded_catalog());
        assert_eq!(r.models[2].fit, Fit::TooBig);
        assert_eq!(r.recommended.as_deref(), Some("qwen3:4b"));
    }

    /// 16 GB ohne Grafikkarte: 16,5 >= 16, 8b ist im Speicher, aber 5,2 GB > 3 GB und kein
    /// Grafikspeicher, also slow. 14b: 16,5 < 24, too_big. Empfehlung: das größte fits,
    /// qwen3:4b.
    #[test]
    fn sixteen_gb_without_gpu() {
        let r = recommend(&hw(16.0, None), &embedded_catalog());
        assert_eq!(
            fits(&r),
            [
                ("qwen3:1.7b", "fits"),
                ("qwen3:4b", "fits"),
                ("qwen3:8b", "slow"),
                ("qwen3:14b", "too_big"),
            ]
        );
        assert_eq!(r.recommended.as_deref(), Some("qwen3:4b"));
    }

    /// 16 GB mit 8 GB Grafikspeicher: 8b liegt komplett auf der Karte (8 >= 7), fits. 14b
    /// bleibt too_big. Empfehlung: qwen3:8b (5,2 GB > 2,5 GB).
    #[test]
    fn sixteen_gb_with_8_gb_gpu() {
        let r = recommend(&hw(16.0, Some(8.0)), &embedded_catalog());
        assert_eq!(
            fits(&r),
            [
                ("qwen3:1.7b", "fits"),
                ("qwen3:4b", "fits"),
                ("qwen3:8b", "fits"),
                ("qwen3:14b", "too_big"),
            ]
        );
        assert_eq!(r.recommended.as_deref(), Some("qwen3:8b"));
    }

    /// 32 GB ohne Grafikkarte: 32,5 >= 24, alle im Speicher; 8b und 14b sind größer als 3 GB
    /// und ohne Karte slow. Empfehlung bleibt qwen3:4b.
    #[test]
    fn thirty_two_gb_without_gpu() {
        let r = recommend(&hw(32.0, None), &embedded_catalog());
        assert_eq!(
            fits(&r),
            [
                ("qwen3:1.7b", "fits"),
                ("qwen3:4b", "fits"),
                ("qwen3:8b", "slow"),
                ("qwen3:14b", "slow"),
            ]
        );
        assert_eq!(r.recommended.as_deref(), Some("qwen3:4b"));
    }

    /// 32 GB mit 6 GB Grafikspeicher: 6 < 7 für 8b (Auslagerung, slow) und 6 < 12 für 14b
    /// (slow), 4b liegt mit 6 >= 4 auf der Karte. Empfehlung qwen3:4b.
    #[test]
    fn thirty_two_gb_with_small_gpu() {
        let r = recommend(&hw(32.0, Some(6.0)), &embedded_catalog());
        assert_eq!(r.models[2].fit, Fit::Slow);
        assert_eq!(r.models[3].fit, Fit::Slow);
        assert_eq!(r.recommended.as_deref(), Some("qwen3:4b"));
    }

    /// 32 GB mit 12 GB Grafikspeicher: 12 >= 12 für 14b, alle fits. Empfehlung: das größte,
    /// qwen3:14b.
    #[test]
    fn thirty_two_gb_with_12_gb_gpu() {
        let r = recommend(&hw(32.0, Some(12.0)), &embedded_catalog());
        assert!(r.models.iter().all(|m| m.fit == Fit::Fits));
        assert_eq!(r.recommended.as_deref(), Some("qwen3:14b"));
    }

    /// 3 GB: nichts passt (3,5 < 4), alles too_big. Die Empfehlung fällt auf das Startmodell
    /// zurück (der Aufrufer zeigt sie dann nicht als "passt", sondern mit der Einstufung).
    #[test]
    fn nothing_fits_falls_back_to_the_starter() {
        let r = recommend(&hw(3.0, None), &embedded_catalog());
        assert!(r.models.iter().all(|m| m.fit == Fit::TooBig));
        assert_eq!(r.recommended.as_deref(), Some("qwen3:4b"));
    }

    #[test]
    fn an_empty_catalog_recommends_nothing() {
        let r = recommend(&hw(32.0, None), &[]);
        assert!(r.models.is_empty());
        assert_eq!(r.recommended, None);
    }
}
