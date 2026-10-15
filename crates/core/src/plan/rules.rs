//! Parameter der Leitplanken R1 bis R11 (`catalog/rules.default.json`, SPEC 8.4).
//!
//! Jede Regel trägt Quelle (Nummer aus Anhang A oder "Faustregel"), Härte und Parameter.
//! Eine Quelle, die nur als Faustregel gilt, ist ein Parameter und kein Code (SPEC 11.1):
//! Wer einen Wert korrigiert, ändert die Datei, nicht die Logik. R12 (Erholungsdaten) kam mit
//! T14 hinzu. Der Schlüssel `reserved` hält Plätze für künftige Regeln frei, unbekannte
//! Schlüssel werden beim Lesen ignoriert.

use serde::{Deserialize, Serialize};

use super::catalog::{IntensityClass, Template};
use super::error::PlanError;
use super::types::Phase;

/// Härte einer Regel: `hard` lehnt einen Plan ab, `soft` erzeugt eine Warnung (SPEC 8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hardness {
    Hard,
    Soft,
}

/// Angaben, die jede Regel trägt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleMeta {
    /// `Qnn` aus SPEC Anhang A, `Faustregel`, `Konvention` oder `Nutzereingabe`.
    pub source: String,
    #[serde(default)]
    pub references: Vec<String>,
    pub hardness: Hardness,
    #[serde(default)]
    pub note: String,
}

/// Quelle ist eine Nummer aus Anhang A oder eine der erlaubten Kennzeichnungen.
pub fn is_valid_source(source: &str) -> bool {
    if matches!(source, "Faustregel" | "Konvention" | "Nutzereingabe") {
        return true;
    }
    source
        .strip_prefix('Q')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Last je Minute aus der Intensitätsklasse (Faustregel): `Stunden * IF^2 * 100`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadModel {
    pub source: String,
    #[serde(default)]
    pub note: String,
    pub if_easy: f32,
    pub if_moderate: f32,
    pub if_hard: f32,
}

impl LoadModel {
    /// Last je Minute einer Intensitätsklasse: `100 / 60 * IF^2` (Faustregel).
    pub fn rate_per_min(&self, class: IntensityClass) -> f64 {
        let f = f64::from(match class {
            IntensityClass::Easy => self.if_easy,
            IntensityClass::Moderate => self.if_moderate,
            IntensityClass::Hard => self.if_hard,
        });
        100.0 / 60.0 * f * f
    }

    /// Last je Minute einer Krafteinheit: `100 / 60 * (RPE / 10)^2` (SPEC 8.1, Faustregel).
    pub fn strength_rate_per_min(rpe: u8) -> f64 {
        let r = f64::from(rpe) / 10.0;
        100.0 / 60.0 * r * r
    }

    /// Geplante Last einer Einheit, gerundet auf eine Nachkommastelle. Kraft rechnet mit der
    /// RPE der Vorlage, alles andere mit der Intensitätsklasse.
    pub fn session_load(&self, template: &Template, minutes: u32) -> f32 {
        let rate = match template.rpe {
            Some(rpe) => Self::strength_rate_per_min(rpe),
            None => self.rate_per_min(template.intensity_class),
        };
        round1(rate * f64::from(minutes))
    }
}

/// Rundung auf eine Nachkommastelle, damit Plan-JSON stabil und lesbar bleibt.
pub fn round1(x: f64) -> f32 {
    let r = (x * 10.0).round() / 10.0;
    // Kein negatives Null im JSON.
    if r == 0.0 { 0.0 } else { r as f32 }
}

/// Gestaltung der Wochen (Konvention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureParams {
    pub source: String,
    /// Kürzeste Einheit, die der Planer ansetzt (Schema: mindestens 10).
    pub min_session_minutes: u32,
    /// Dauern werden auf Vielfache dieses Schritts gewählt, soweit Fenster und Verfügbarkeit es zulassen.
    pub duration_step_min: u32,
    /// Die Langeinheit ist so viel länger wie eine gewöhnliche Einheit (Gewicht in der Wochenlast).
    pub long_weight: f32,
    /// Wochenlast, unter die der Planer nicht fällt, wenn kein Bezug vorliegt.
    pub min_week_load: f32,
}

/// Phasenlängen bei Planung auf einen Termin zu. Die Rennwoche ist immer genau eine Woche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventPhases {
    pub taper_weeks: u32,
    pub peak_weeks: u32,
    pub build_weeks: u32,
    /// Harte Einheiten der Rennwoche liegen mindestens so viele Tage vor dem Termin.
    pub race_opener_min_days_before: u32,
}

/// Fortlaufende Planung: Belastungswochen tragen diese Phase, Blöcke 3 zu 1 (R6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContinuousPhases {
    pub load_phase: Phase,
}

/// Zusammensetzung einer Woche je Phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseProfile {
    pub max_hard: u32,
    pub moderate_sessions: u32,
    pub long_session: bool,
    pub strength_max: u32,
    /// Wochenzuwachs gegenüber der letzten Belastungswoche (Belastungsphasen).
    #[serde(default)]
    pub weekly_growth: Option<f32>,
    /// Anteil der letzten Belastungswoche (Erholung, Taper, Rennwoche).
    #[serde(default)]
    pub volume_factor: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseParams {
    pub source: String,
    pub event: EventPhases,
    pub continuous: ContinuousPhases,
    pub base: PhaseProfile,
    pub build: PhaseProfile,
    pub peak: PhaseProfile,
    pub taper: PhaseProfile,
    pub recovery: PhaseProfile,
    pub race: PhaseProfile,
}

impl PhaseParams {
    pub fn profile(&self, phase: Phase) -> &PhaseProfile {
        match phase {
            Phase::Base => &self.base,
            Phase::Build => &self.build,
            Phase::Peak => &self.peak,
            Phase::Taper => &self.taper,
            Phase::Recovery => &self.recovery,
            Phase::Race => &self.race,
        }
    }
}

/// R1: Anstieg der CTL je Woche (hart, Faustregel).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R01 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub max_ctl_rise_per_week: f32,
    /// Obergrenze für die Einstellung des Nutzers.
    pub max_setting: f32,
    /// Der Planer zielt auf diesen Anteil der Grenze, weil der Wochenverlauf ungleich verteilt ist.
    pub planning_margin: f32,
}

/// R2: Anstieg des Wochenvolumens (Trainingszeit) gegenüber der Vorwoche.
/// Weich ab `warn_at`, hart ab `reject_at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R02 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub warn_at: f32,
    pub reject_at: f32,
    /// Unter dieser Vorwochenzeit (Minuten) ist ein Prozentwert nicht aussagekräftig, weil schon
    /// eine zusätzliche kurze Einheit die Grenze überschreitet. Dann gilt R2 nicht (Konvention).
    pub min_reference_minutes: f32,
}

/// R3: harte Einheiten je Woche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R03 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub max_hard_per_week: u32,
}

/// R4: harte Einheiten nicht an aufeinanderfolgenden Tagen (kein Parameter).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R04 {
    #[serde(flatten)]
    pub meta: RuleMeta,
}

/// R5: Ruhetage je Woche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R05 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub min_rest_days_per_week: u32,
}

/// R6: Erholungswoche nach Belastungswochen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R06 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub load_weeks: u32,
    pub recovery_min: f32,
    pub recovery_max: f32,
    pub ratio_tolerance: f32,
}

/// R7: Krafteinheiten je Woche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R07 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub min_per_week: u32,
    pub max_per_week: u32,
    pub max_per_week_taper: u32,
}

/// R8: Anteil leichter Einheiten an der Trainingszeit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R08 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub min_easy_share: f32,
}

/// R9: Taper vor dem Wettkampf (Länge hart, sinkendes Volumen weich).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R09 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub taper_days_min: u32,
    pub taper_days_max: u32,
}

/// R10: vorausberechnete Form (TSB).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R10 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub tsb_warn_below: f32,
}

/// R11: Verfügbarkeit (Nutzereingabe).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R11 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    pub max_sessions_per_day: u32,
}

/// R12: Erholungsdaten (weich, SPEC 8.4). Die Basislinie ist ein gleitendes Mittel über
/// `baseline_days` Tage (Q6, die Empfehlung von sieben Tagen steht nur in einem Sekundärbericht).
/// Alle Schwellen sind Faustregeln.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R12 {
    #[serde(flatten)]
    pub meta: RuleMeta,
    /// HRV gilt als deutlich unter der Basislinie, wenn sie um mindestens diesen Anteil darunter
    /// liegt (Faustregel).
    pub hrv_drop_fraction: f32,
    /// Schlafdauer gilt als zu kurz, wenn sie um mindestens diesen Anteil unter der Basislinie
    /// liegt (Faustregel).
    pub sleep_deficit_fraction: f32,
    /// Länge des gleitenden Mittels in Tagen vor dem jüngsten Erholungstag (Q6, sekundär).
    pub baseline_days: u32,
    /// So viele Tage mit Wert braucht die Basislinie mindestens (Faustregel), sonst gibt es kein
    /// Signal.
    pub min_baseline_days: u32,
    /// Der jüngste Erholungstag darf höchstens so viele Tage vor dem Planbeginn liegen
    /// (Faustregel), sonst ist er veraltet und zählt nicht.
    pub max_age_days: u32,
}

/// Wiederholung der Auswahl beim Planen (Schritt 6 in SPEC 8.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratorParams {
    pub source: String,
    /// Höchstzahl der Versuche je Woche.
    pub max_retries: u32,
    /// Verkleinerung des Wochenziels je Versuch.
    pub scale_step: f32,
    /// Zahl der Verkleinerungen je Aufbau der Woche.
    pub scale_steps: u32,
}

/// Alle Parameter des Planers (`catalog/rules.default.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rules {
    pub format_version: u32,
    /// `x.y.z`, landet als `rules_version` im Plan.
    pub version: String,
    #[serde(default)]
    pub reserved: Vec<String>,
    pub load_model: LoadModel,
    pub structure: StructureParams,
    pub phases: PhaseParams,
    #[serde(rename = "R01")]
    pub r01: R01,
    #[serde(rename = "R02")]
    pub r02: R02,
    #[serde(rename = "R03")]
    pub r03: R03,
    #[serde(rename = "R04")]
    pub r04: R04,
    #[serde(rename = "R05")]
    pub r05: R05,
    #[serde(rename = "R06")]
    pub r06: R06,
    #[serde(rename = "R07")]
    pub r07: R07,
    #[serde(rename = "R08")]
    pub r08: R08,
    #[serde(rename = "R09")]
    pub r09: R09,
    #[serde(rename = "R10")]
    pub r10: R10,
    #[serde(rename = "R11")]
    pub r11: R11,
    #[serde(rename = "R12")]
    pub r12: R12,
    pub generator: GeneratorParams,
}

fn version_is_valid(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

impl Rules {
    /// Meta-Angaben aller Regeln mit Code, in der Reihenfolge R01 bis R12.
    pub fn metas(&self) -> [(&'static str, &RuleMeta); 12] {
        [
            ("R01", &self.r01.meta),
            ("R02", &self.r02.meta),
            ("R03", &self.r03.meta),
            ("R04", &self.r04.meta),
            ("R05", &self.r05.meta),
            ("R06", &self.r06.meta),
            ("R07", &self.r07.meta),
            ("R08", &self.r08.meta),
            ("R09", &self.r09.meta),
            ("R10", &self.r10.meta),
            ("R11", &self.r11.meta),
            ("R12", &self.r12.meta),
        ]
    }

    /// Prüft Wertebereiche und Widersprüche. Der Planer ruft das vor jedem Lauf.
    pub fn validate(&self) -> Result<(), PlanError> {
        let bad = |what: &str| Err(PlanError::InvalidRules(what.to_string()));
        let frac = |x: f32| x.is_finite() && x > 0.0 && x <= 1.0;
        if self.format_version != 1 {
            return bad("format_version");
        }
        if !version_is_valid(&self.version) {
            return bad("version");
        }
        for (code, meta) in self.metas() {
            if !is_valid_source(&meta.source) {
                return Err(PlanError::InvalidRules(format!("{code}.source")));
            }
        }
        let lm = &self.load_model;
        if !(lm.if_easy.is_finite()
            && lm.if_easy > 0.0
            && lm.if_easy <= lm.if_moderate
            && lm.if_moderate <= lm.if_hard
            && lm.if_hard <= 2.0)
        {
            return bad("load_model.if");
        }
        let st = &self.structure;
        if st.min_session_minutes < 10
            || st.duration_step_min == 0
            || !(st.long_weight.is_finite() && st.long_weight > 0.0)
            || !(st.min_week_load.is_finite() && st.min_week_load >= 0.0)
        {
            return bad("structure");
        }
        let ev = &self.phases.event;
        if ev.taper_weeks == 0
            || 7 * ev.taper_weeks < self.r09.taper_days_min
            || 7 * ev.taper_weeks + 6 > self.r09.taper_days_max
        {
            // Der Taper beginnt am Montag der ersten Taperwoche und reicht bis zum Termin
            // in der Rennwoche: zwischen 7 * Wochen und 7 * Wochen + 6 Tage (R9).
            return bad("phases.event.taper_weeks");
        }
        for (name, p) in [
            ("base", &self.phases.base),
            ("build", &self.phases.build),
            ("peak", &self.phases.peak),
        ] {
            if !p
                .weekly_growth
                .is_some_and(|g| g.is_finite() && (0.0..1.0).contains(&g))
            {
                return Err(PlanError::InvalidRules(format!(
                    "phases.{name}.weekly_growth"
                )));
            }
        }
        for (name, p) in [
            ("taper", &self.phases.taper),
            ("recovery", &self.phases.recovery),
            ("race", &self.phases.race),
        ] {
            if !p.volume_factor.is_some_and(frac) {
                return Err(PlanError::InvalidRules(format!(
                    "phases.{name}.volume_factor"
                )));
            }
        }
        if !self.phases.continuous.load_phase.is_load_week() {
            return bad("phases.continuous.load_phase");
        }
        let r1 = &self.r01;
        if !(r1.max_ctl_rise_per_week.is_finite()
            && r1.max_ctl_rise_per_week > 0.0
            && r1.max_ctl_rise_per_week <= r1.max_setting
            && frac(r1.planning_margin))
        {
            return bad("R01");
        }
        let r2 = &self.r02;
        if !(r2.warn_at.is_finite()
            && r2.warn_at > 0.0
            && r2.warn_at < r2.reject_at
            && r2.min_reference_minutes.is_finite()
            && r2.min_reference_minutes >= 0.0)
        {
            return bad("R02");
        }
        if self.r05.min_rest_days_per_week > 6 {
            return bad("R05");
        }
        let r6 = &self.r06;
        if r6.load_weeks == 0
            || !(r6.recovery_min.is_finite()
                && r6.recovery_min > 0.0
                && r6.recovery_min <= r6.recovery_max
                && r6.recovery_max <= 1.0
                && r6.ratio_tolerance.is_finite()
                && r6.ratio_tolerance >= 0.0)
        {
            return bad("R06");
        }
        let r7 = &self.r07;
        if r7.min_per_week > r7.max_per_week || r7.max_per_week_taper > r7.max_per_week {
            return bad("R07");
        }
        if !(self.r08.min_easy_share.is_finite() && (0.0..=1.0).contains(&self.r08.min_easy_share))
        {
            return bad("R08");
        }
        if self.r09.taper_days_min == 0 || self.r09.taper_days_min > self.r09.taper_days_max {
            return bad("R09");
        }
        if !self.r10.tsb_warn_below.is_finite() {
            return bad("R10");
        }
        if !(1..=2).contains(&self.r11.max_sessions_per_day) {
            return bad("R11");
        }
        let r12 = &self.r12;
        if !(frac(r12.hrv_drop_fraction)
            && frac(r12.sleep_deficit_fraction)
            && r12.baseline_days > 0
            && r12.min_baseline_days > 0
            && r12.min_baseline_days <= r12.baseline_days
            && r12.max_age_days > 0)
        {
            return bad("R12");
        }
        let g = &self.generator;
        if g.max_retries == 0
            || !(g.scale_step.is_finite() && g.scale_step > 0.0)
            || g.scale_step * g.scale_steps as f32 >= 1.0
        {
            return bad("generator");
        }
        Ok(())
    }
}
