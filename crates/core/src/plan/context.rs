//! Eingaben des Planers (SPEC 8.3): Ziel, Verfügbarkeit, Startwerte, Historie, Katalog.

use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use super::catalog::{IntensityClass, TemplateCatalog};
use super::error::PlanError;
use super::recovery::RecoveryDay;
use super::rules::Rules;
use super::types::{Goal, GoalKind, Locale, MAX_PLAN_WEEKS, PlanSport, Priority};
use crate::load::Sport;
use crate::pmc::PmcParams;

/// Ziel der Planung. `Event` plant rückwärts vom Termin, `Continuous` in Blöcken.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GoalSpec {
    Event {
        event_date: NaiveDate,
        sport: PlanSport,
        priority: Priority,
    },
    Continuous,
}

impl From<&GoalSpec> for Goal {
    fn from(g: &GoalSpec) -> Self {
        match g {
            GoalSpec::Event {
                event_date,
                sport,
                priority,
            } => Goal {
                kind: GoalKind::Event,
                event_date: Some(*event_date),
                sport: Some(*sport),
                priority: Some(*priority),
            },
            GoalSpec::Continuous => Goal {
                kind: GoalKind::Continuous,
                event_date: None,
                sport: None,
                priority: None,
            },
        }
    }
}

/// Verfügbarkeit eines Wochentags (Nutzereingabe, R11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayAvailability {
    /// Höchste Trainingszeit des Tages in Minuten, alle Einheiten zusammen.
    pub max_minutes: u16,
    /// Harte Einheiten erlaubt.
    pub allow_hard: bool,
    /// Fester Ruhetag, keine Einheit.
    pub fixed_rest: bool,
}

/// Gewicht einer Sportart im Plan. Das Gewicht verteilt die Einheiten, es ist kein Zeitanteil.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SportWeight {
    pub sport: PlanSport,
    pub weight: f32,
}

/// Last und Zeit einer Sportart in einer vergangenen Woche (aus der Speicherschicht).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeekHistory {
    /// Montag der Woche.
    pub week_start: NaiveDate,
    pub sport: Sport,
    pub load: f32,
    pub minutes: f32,
}

/// Alles, was der Planer außer den Regeln braucht. `plan_id` und `created_at` vergibt der
/// Aufrufer, `core` erzeugt keine UUID und liest keine Uhr.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanContext {
    pub plan_id: String,
    pub created_at: DateTime<Utc>,
    pub locale: Locale,
    /// Montag der ersten Planwoche.
    pub week_start: NaiveDate,
    pub goal: GoalSpec,
    /// Zahl der Wochen, höchstens 26. Mit Termin endet der Plan in der Woche des Termins,
    /// wenn der früher liegt.
    pub horizon_weeks: u32,
    /// CTL und ATL am Ende des Tages vor `week_start`.
    pub ctl0: f32,
    pub atl0: f32,
    /// Vergangene Wochen. Die Woche vor `week_start` dient als Bezug für den Anstieg (R2).
    pub history: Vec<WeekHistory>,
    /// Montag bis Sonntag.
    pub availability: [DayAvailability; 7],
    /// Ausdauersportarten mit Gewicht.
    pub sports: Vec<SportWeight>,
    /// Gewünschte Krafteinheiten je Woche (R7 begrenzt auf 1 bis 2).
    pub strength_per_week: u8,
    /// Eigene Grenze für R1, höchstens `Rules::r01.max_setting`.
    pub ctl_rise_per_week: Option<f32>,
    /// Zeitkonstanten für die Vorausberechnung. `ctl0` und `atl0` darin gelten nicht, es zählen
    /// die Felder oben.
    pub pmc_params: PmcParams,
    pub catalog: TemplateCatalog,
    /// Erholungstage für R12 (SPEC 8.4), in beliebiger Reihenfolge. Leer ist der Normalfall:
    /// Ohne Erholungsdaten ändert R12 nichts und der Plan bleibt Byte für Byte derselbe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovery: Vec<RecoveryDay>,
}

/// Gesamtwerte der Woche vor dem Planbeginn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviousWeek {
    pub load: f64,
    pub minutes: f64,
}

impl PlanContext {
    /// Grenze für R1 nach Einstellung und Regel.
    pub fn ctl_rise_limit(&self, rules: &Rules) -> f32 {
        self.ctl_rise_per_week
            .unwrap_or(rules.r01.max_ctl_rise_per_week)
            .min(rules.r01.max_setting)
    }

    /// Parameter für `pmc` mit den Startwerten dieses Kontexts.
    pub fn pmc_start(&self) -> PmcParams {
        PmcParams {
            ctl0: self.ctl0,
            atl0: self.atl0,
            ..self.pmc_params
        }
    }

    /// Summe aller Sportarten in der Woche unmittelbar vor `week_start`, falls vorhanden.
    pub fn previous_week(&self) -> Option<PreviousWeek> {
        let prev = self.week_start - TimeDelta::days(7);
        let mut found = false;
        let mut total = PreviousWeek {
            load: 0.0,
            minutes: 0.0,
        };
        for h in self.history.iter().filter(|h| h.week_start == prev) {
            found = true;
            total.load += f64::from(h.load.max(0.0));
            total.minutes += f64::from(h.minutes.max(0.0));
        }
        found.then_some(total)
    }

    /// Hauptsportart: die des Termins, sofern sie geplant wird, sonst die mit dem größten Gewicht.
    pub fn primary_sport(&self) -> PlanSport {
        if let GoalSpec::Event { sport, .. } = &self.goal
            && self.sports.iter().any(|s| s.sport == *sport)
        {
            return *sport;
        }
        let mut best = self.sports[0];
        for s in &self.sports[1..] {
            if s.weight > best.weight {
                best = *s;
            }
        }
        best.sport
    }

    /// Prüft den Kontext gegen die Regeln. Fehler, die kein Plan beheben kann, sind
    /// `PlanError`.
    pub fn validate(&self, rules: &Rules) -> Result<(), PlanError> {
        if self.week_start.weekday().num_days_from_monday() != 0 {
            return Err(PlanError::WeekStartNotMonday(self.week_start));
        }
        if self.horizon_weeks == 0 || self.horizon_weeks > MAX_PLAN_WEEKS {
            return Err(PlanError::InvalidHorizon {
                got: self.horizon_weeks,
                max: MAX_PLAN_WEEKS,
            });
        }
        if let GoalSpec::Event {
            event_date, sport, ..
        } = &self.goal
        {
            if *event_date < self.week_start {
                return Err(PlanError::EventBeforeStart {
                    event: *event_date,
                    week_start: self.week_start,
                });
            }
            if !sport.is_endurance() {
                return Err(PlanError::InvalidGoal(
                    "event sport must be cycling, running or swimming".to_string(),
                ));
            }
        }
        if self.sports.is_empty() {
            return Err(PlanError::NoSports);
        }
        for (i, s) in self.sports.iter().enumerate() {
            if !s.sport.is_endurance() || !s.weight.is_finite() || s.weight <= 0.0 {
                return Err(PlanError::InvalidSportWeight(format!("{:?}", s.sport)));
            }
            if self.sports[..i].iter().any(|o| o.sport == s.sport) {
                return Err(PlanError::InvalidSportWeight(format!(
                    "{:?} twice",
                    s.sport
                )));
            }
        }
        for (name, v) in [("ctl0", self.ctl0), ("atl0", self.atl0)] {
            if !v.is_finite() || v < 0.0 {
                return Err(PlanError::InvalidStartValues(name.to_string()));
            }
        }
        let p = &self.pmc_params;
        for (name, v) in [("tau_ctl_d", p.tau_ctl_d), ("tau_atl_d", p.tau_atl_d)] {
            if !v.is_finite() || v <= 0.0 {
                return Err(PlanError::InvalidStartValues(name.to_string()));
            }
        }
        if let Some(r) = self.ctl_rise_per_week
            && (!r.is_finite() || r <= 0.0 || r > rules.r01.max_setting)
        {
            return Err(PlanError::InvalidCtlRise {
                got: r,
                max: rules.r01.max_setting,
            });
        }
        self.catalog.validate()?;
        let need = |sport: PlanSport, class: IntensityClass| {
            if self.catalog.select(sport, class).is_empty() {
                Err(PlanError::MissingTemplate {
                    sport: format!("{sport:?}"),
                    class: format!("{class:?}"),
                })
            } else {
                Ok(())
            }
        };
        for s in &self.sports {
            need(s.sport, IntensityClass::Easy)?;
        }
        if self.strength_per_week > 0 {
            need(PlanSport::Strength, IntensityClass::Moderate)?;
        }
        Ok(())
    }
}
