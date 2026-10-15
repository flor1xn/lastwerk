//! Vorlagenkatalog (`catalog/templates.json`, SPEC 7.4).
//!
//! Jede Vorlage ist eine feste, von Hand geschriebene Struktur. `core` liest die Datei nicht,
//! der Aufrufer übergibt den deserialisierten Katalog. Dauern und Strukturen sind Konvention,
//! ohne Behauptung über die Wirksamkeit einzelner Einheiten.
//!
//! Die Schritte beschreiben die Struktur bei der Nenndauer (Summe aller Schritte). Bei anderer
//! Dauer bleiben die Intervallblöcke (Arbeit und Pause) unverändert, die übrigen Schritte
//! (Aufwärmen, gleichmäßig, Abwärmen) werden im Verhältnis ihrer Nennzeit gedehnt oder
//! gestaucht (Konvention). Daraus folgt: Die Mindestdauer einer Vorlage ist mindestens die
//! Länge der Intervallblöcke.

use serde::{Deserialize, Serialize};

use super::error::PlanError;
use super::types::{MAX_SESSION_MINUTES, MIN_SESSION_MINUTES, PlanSport};

/// Intensitätsklasse. `hard` ist die Voraussetzung für die Leitplanken R3 und R4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntensityClass {
    Easy,
    Moderate,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Warmup,
    Steady,
    Interval,
    Cooldown,
}

/// Ein Schritt einer Vorlage. Bei `interval` ist `minutes` die Dauer einer Wiederholung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    pub minutes: u32,
    /// Zonenschema (`power7`, `run_speed5`, `swim_css5`), bei Kraft nicht gesetzt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_scheme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_minutes: Option<u32>,
    /// Anstrengung des Schritts (nur Kraft).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpe: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationWindow {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub sport: PlanSport,
    pub intensity_class: IntensityClass,
    pub duration_min: DurationWindow,
    /// Anstrengung der ganzen Einheit, nur Kraft (Session-RPE, Q4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpe: Option<u8>,
    pub steps: Vec<Step>,
    pub text_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateCatalog {
    pub format_version: u32,
    pub templates: Vec<Template>,
}

/// Zonenschema und Zonenzahl je Sportart.
fn expected_scheme(sport: PlanSport) -> Option<(&'static str, u8)> {
    match sport {
        PlanSport::Cycling => Some(("power7", 7)),
        PlanSport::Running => Some(("run_speed5", 5)),
        PlanSport::Swimming => Some(("swim_css5", 5)),
        PlanSport::Strength | PlanSport::Rest => None,
    }
}

/// Muster `^[a-z]+(\.[a-z0-9_]+)+$` für `template_id` (SPEC 7.1), ohne Regex-Bibliothek.
pub fn is_valid_template_id(id: &str) -> bool {
    let mut parts = id.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    if first.is_empty() || !first.bytes().all(|b| b.is_ascii_lowercase()) {
        return false;
    }
    let mut count = 0;
    for p in parts {
        count += 1;
        if p.is_empty()
            || !p
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return false;
        }
    }
    count >= 1
}

/// Muster `^[a-z0-9_.]+$` für Begründungsschlüssel (SPEC 7.1).
pub fn is_valid_reason_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
}

impl Template {
    /// Minuten der Intervallblöcke: Arbeit mal Wiederholungen plus Pausen dazwischen.
    pub fn fixed_minutes(&self) -> u32 {
        self.steps
            .iter()
            .filter(|s| s.kind == StepKind::Interval)
            .map(|s| {
                let reps = s.reps.unwrap_or(1);
                reps * s.minutes + reps.saturating_sub(1) * s.rest_minutes.unwrap_or(0)
            })
            .sum()
    }

    /// Nennzeit der dehnbaren Schritte (Aufwärmen, gleichmäßig, Abwärmen).
    pub fn flexible_minutes(&self) -> u32 {
        self.steps
            .iter()
            .filter(|s| s.kind != StepKind::Interval)
            .map(|s| s.minutes)
            .sum()
    }

    /// Dauer der Struktur wie im Katalog beschrieben.
    pub fn nominal_minutes(&self) -> u32 {
        self.fixed_minutes() + self.flexible_minutes()
    }

    /// Die Vorlage hat Zonen (Ausdauer), im Gegensatz zu Kraft und Ruhe.
    pub fn has_zones(&self) -> bool {
        self.sport.is_endurance()
    }

    /// Minuten in Zone 1 und 2 bei der Dauer `duration_min` (R8). Bei Vorlagen ohne Zonen
    /// gibt es keinen Wert. Pausen zwischen Intervallen zählen als Zone 1.
    pub fn easy_zone_minutes(&self, duration_min: u32) -> Option<f64> {
        if !self.has_zones() {
            return None;
        }
        let fixed = f64::from(self.fixed_minutes());
        let flex_nominal = f64::from(self.flexible_minutes());
        let flex_now = (f64::from(duration_min) - fixed).max(0.0);
        let scale = if flex_nominal > 0.0 {
            flex_now / flex_nominal
        } else {
            0.0
        };
        let mut easy = 0.0;
        for s in &self.steps {
            let is_easy_zone = s.zone.is_some_and(|z| z <= 2);
            if s.kind == StepKind::Interval {
                let reps = f64::from(s.reps.unwrap_or(1));
                let rests = f64::from(s.reps.unwrap_or(1).saturating_sub(1))
                    * f64::from(s.rest_minutes.unwrap_or(0));
                easy += rests;
                if is_easy_zone {
                    easy += reps * f64::from(s.minutes);
                }
            } else if is_easy_zone {
                easy += f64::from(s.minutes) * scale;
            }
        }
        Some(easy.min(f64::from(duration_min)))
    }
}

impl TemplateCatalog {
    pub fn get(&self, id: &str) -> Option<&Template> {
        self.templates.iter().find(|t| t.id == id)
    }

    /// Vorlagen einer Sportart und Klasse in Katalogreihenfolge.
    pub fn select(&self, sport: PlanSport, class: IntensityClass) -> Vec<&Template> {
        self.templates
            .iter()
            .filter(|t| t.sport == sport && t.intensity_class == class)
            .collect()
    }

    /// Prüft den Katalog auf Muster, Eindeutigkeit und Stimmigkeit der Dauern und Zonen.
    pub fn validate(&self) -> Result<(), PlanError> {
        let bad = |id: &str, reason: &str| PlanError::Catalog {
            template_id: id.to_string(),
            reason: reason.to_string(),
        };
        if self.format_version != 1 {
            return Err(bad("", "format_version"));
        }
        for (i, t) in self.templates.iter().enumerate() {
            if !is_valid_template_id(&t.id) {
                return Err(bad(&t.id, "id_pattern"));
            }
            if self.templates[..i].iter().any(|o| o.id == t.id) {
                return Err(bad(&t.id, "duplicate_id"));
            }
            if t.text_key != format!("template.{}", t.id) {
                return Err(bad(&t.id, "text_key"));
            }
            let w = t.duration_min;
            if t.sport == PlanSport::Rest {
                if w.min != 0 || w.max != 0 || !t.steps.is_empty() {
                    return Err(bad(&t.id, "rest_must_be_empty"));
                }
                continue;
            }
            if w.min < MIN_SESSION_MINUTES || w.max > MAX_SESSION_MINUTES || w.min > w.max {
                return Err(bad(&t.id, "duration_window"));
            }
            if t.steps.is_empty() {
                return Err(bad(&t.id, "no_steps"));
            }
            match expected_scheme(t.sport) {
                Some((scheme, zones)) => {
                    if t.rpe.is_some() {
                        return Err(bad(&t.id, "rpe_only_for_strength"));
                    }
                    for s in &t.steps {
                        if s.minutes == 0 {
                            return Err(bad(&t.id, "step_minutes"));
                        }
                        if s.zone_scheme.as_deref() != Some(scheme) {
                            return Err(bad(&t.id, "zone_scheme"));
                        }
                        if !s.zone.is_some_and(|z| (1..=zones).contains(&z)) {
                            return Err(bad(&t.id, "zone"));
                        }
                    }
                    if t.intensity_class == IntensityClass::Hard
                        && !t.steps.iter().any(|s| s.zone.is_some_and(|z| z >= 4))
                    {
                        return Err(bad(&t.id, "hard_without_hard_zone"));
                    }
                }
                None => {
                    if !t.rpe.is_some_and(|r| (1..=10).contains(&r)) {
                        return Err(bad(&t.id, "strength_needs_rpe"));
                    }
                    if t.steps
                        .iter()
                        .any(|s| s.zone.is_some() || s.zone_scheme.is_some())
                    {
                        return Err(bad(&t.id, "strength_without_zones"));
                    }
                }
            }
            for s in &t.steps {
                if s.kind == StepKind::Interval {
                    if s.reps.is_none_or(|r| r == 0) || s.rest_minutes.is_none() {
                        return Err(bad(&t.id, "interval_needs_reps_and_rest"));
                    }
                } else if s.reps.is_some() || s.rest_minutes.is_some() {
                    return Err(bad(&t.id, "reps_only_for_interval"));
                }
            }
            if t.fixed_minutes() > w.min {
                return Err(bad(&t.id, "min_below_interval_blocks"));
            }
            if w.max > t.fixed_minutes() && t.flexible_minutes() == 0 {
                return Err(bad(&t.id, "window_without_flexible_steps"));
            }
            if !(w.min..=w.max).contains(&t.nominal_minutes()) {
                return Err(bad(&t.id, "nominal_outside_window"));
            }
        }
        Ok(())
    }
}
