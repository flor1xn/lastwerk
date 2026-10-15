//! Planer mit drei Fragen (T26 Teil B, SPEC 7.2, 8.3, 9.7, O31): Ziel, Stunden je Woche,
//! Trainingstage. Daraus entstehen Ziel, Verfügbarkeit und ein aktiver Plan in einem Schritt.
//!
//! Die Antworten werden auf Eingaben des Regelplaners abgebildet, der Planer entscheidet wie
//! sonst auch (harte Tage, Langeinheit, Dauern). Das Modell ist hier nicht beteiligt.

use chrono::{DateTime, Datelike, Utc, Weekday};
use lastwerk_cli::planning;
use lastwerk_core::plan::GoalKind;
use lastwerk_store::{MAX_AVAILABLE_MINUTES, Store};
use serde::Deserialize;

use crate::dto::{
    AvailabilityDay, GeneratePlanInput, GoalInput, PlanDetail, PlanLocale, PlanSport, Priority,
};
use crate::error::{AppError, Result};
use crate::service::parse_date;
use crate::service::plans;

/// Kleinste und größte Wochenzeit in Stunden (Konvention, Grenzen der Oberfläche 2 bis 20).
pub const MIN_HOURS: f64 = 1.0;
pub const MAX_HOURS: f64 = 30.0;

/// Die Minuten je Tag werden auf Vielfache von 5 gelegt, wie die Dauerstufen des Planers
/// (`duration_step_min` in `rules.default.json`).
const STEP_MIN: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickGoalKind {
    Event,
    Continuous,
}

/// Erste Frage: Wettkampf mit Datum und Sportart oder "Fit bleiben" (fortlaufend).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct QuickGoal {
    pub kind: QuickGoalKind,
    /// Nur bei `event`: Tag des Wettkampfs (YYYY-MM-DD).
    pub date: Option<String>,
    /// Nur bei `event`: Ausdauersportart. Ohne Angabe wählt der Planer die Hauptsportart.
    pub sport: Option<PlanSport>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct QuickPlanInput {
    pub goal: QuickGoal,
    /// Zweite Frage: Stunden je Woche, 1 bis 30.
    pub hours_per_week: f64,
    /// Dritte Frage: Trainingstage, 0 = Montag bis 6 = Sonntag.
    pub days: Vec<u8>,
    /// Montag der ersten Planwoche, wie bei `generate_plan`.
    pub week_start: String,
    pub locale: PlanLocale,
}

/// Verteilt die Wochenzeit auf die Tage (O31, Faustregel): gleichmäßig auf die gewählten Tage,
/// auf 5 Minuten gerundet, ein Rest von höchstens 5 Minuten je Tag geht an die späteren Tage.
/// Die Summe bleibt dabei gleich der auf 5 Minuten gerundeten Wochenzeit. Gewählte Tage
/// erlauben harte Einheiten, die übrigen sind feste Ruhetage mit 0 Minuten; welche Tage hart
/// werden und welcher die Langeinheit trägt, entscheidet der Planer (SPEC 8.3, Schritt 3).
///
/// Warum gleichmäßig: Die Minuten sind die Obergrenze je Tag, nicht das Ziel. Der Planer
/// wählt die Dauern nach dem Lastziel und der Rampe (R1) und legt die Langeinheit auf den
/// Tag mit der größten Zeit, bei Gleichstand auf den späteren. Eine ungleiche Verteilung
/// (zum Beispiel das Wochenende doppelt) wäre eine zusätzliche Annahme über den Alltag des
/// Nutzers, die der Planer nicht braucht. Feinheiten sagt der Nutzer im Gespräch oder stellt
/// sie in den Feineinstellungen ein.
///
/// `min_session_minutes` ist die kleinste Einheit des Planers; weniger Zeit je Tag ergäbe nur
/// unbrauchbare Tage (`too_little_time`).
pub fn distribute_minutes(
    hours: f64,
    days: &[u8],
    min_session_minutes: u32,
) -> Result<Vec<AvailabilityDay>> {
    if !hours.is_finite() || !(MIN_HOURS..=MAX_HOURS).contains(&hours) {
        return Err(AppError::new("invalid_value"));
    }
    let mut chosen: Vec<u8> = Vec::new();
    for &d in days {
        if d > 6 || chosen.contains(&d) {
            return Err(AppError::new("invalid_value"));
        }
        chosen.push(d);
    }
    if chosen.is_empty() {
        return Err(AppError::new("invalid_value"));
    }
    chosen.sort_unstable();
    let n = u32::try_from(chosen.len()).map_err(|_| AppError::new("invalid_value"))?;

    // Wochenzeit in Minuten, auf 5 gerundet. Die Prüfung der Grenzen oben schließt Überlauf aus.
    let total = ((hours * 60.0 / f64::from(STEP_MIN)).round() as u32) * STEP_MIN;
    let base = total / n / STEP_MIN * STEP_MIN;
    // Der Rest ist ein Vielfaches von 5 und kleiner als 5 je Tag, also höchstens ein Schritt
    // je Tag.
    let extra = ((total - base * n) / STEP_MIN) as usize;
    let first_extra = chosen.len() - extra;

    let mut out = Vec::with_capacity(7);
    for weekday in 0u8..7 {
        match chosen.iter().position(|d| *d == weekday) {
            Some(i) => {
                let minutes = base + if i >= first_extra { STEP_MIN } else { 0 };
                if minutes < min_session_minutes {
                    return Err(AppError::new("too_little_time"));
                }
                if minutes > u32::from(MAX_AVAILABLE_MINUTES) {
                    return Err(AppError::new("invalid_value"));
                }
                out.push(AvailabilityDay {
                    weekday,
                    max_minutes: u16::try_from(minutes)
                        .map_err(|_| AppError::new("invalid_value"))?,
                    allow_hard: true,
                    fixed_rest: false,
                });
            }
            None => out.push(AvailabilityDay {
                weekday,
                max_minutes: 0,
                allow_hard: false,
                fixed_rest: true,
            }),
        }
    }
    Ok(out)
}

/// Setzt das Ziel, ohne ein gleiches aktives Ziel zu verdoppeln: Wer die drei Fragen mehrfach
/// mit demselben Ziel beantwortet, behält Bezeichnung und Priorität des vorhandenen Ziels.
fn ensure_goal(
    store: &mut Store,
    kind: QuickGoalKind,
    date: Option<&str>,
    sport: Option<PlanSport>,
) -> Result<()> {
    let wanted_date = date.map(parse_date).transpose()?;
    if let Some(active) = store.active_goal()? {
        let same = match kind {
            QuickGoalKind::Continuous => active.goal.kind == GoalKind::Continuous,
            QuickGoalKind::Event => {
                active.goal.kind == GoalKind::Event
                    && active.goal.event_date == wanted_date
                    && active.goal.sport == sport
            }
        };
        if same {
            return Ok(());
        }
    }
    plans::set_goal(
        store,
        &GoalInput {
            event_date: date.map(str::to_owned),
            sport,
            priority: (kind == QuickGoalKind::Event).then_some(Priority::A),
            // Eine Bezeichnung fragt der Planer mit drei Fragen nicht ab.
            label: None,
        },
    )?;
    Ok(())
}

/// Die drei Fragen: schreibt Ziel und Verfügbarkeit, erzeugt den Plan mit dem Regelplaner und
/// macht ihn zum aktiven Plan (der bisherige aktive Plan wird archiviert).
///
/// Schlägt die Erzeugung fehl, bleiben Ziel und Verfügbarkeit gesetzt: Es sind die Antworten
/// des Nutzers, und der Planer in den Feineinstellungen arbeitet damit weiter.
pub fn quick_plan(
    store: &mut Store,
    input: &QuickPlanInput,
    plan_id: String,
    now: DateTime<Utc>,
) -> Result<PlanDetail> {
    let week_start = parse_date(&input.week_start)?;
    if week_start.weekday() != Weekday::Mon {
        return Err(AppError::new("not_monday"));
    }
    let (rules, _) = planning::load_rules().map_err(|_| AppError::new("internal"))?;
    let availability = distribute_minutes(
        input.hours_per_week,
        &input.days,
        rules.structure.min_session_minutes,
    )?;

    let (date, sport) = match input.goal.kind {
        QuickGoalKind::Continuous => (None, None),
        QuickGoalKind::Event => {
            let text = input
                .goal
                .date
                .as_deref()
                .ok_or_else(|| AppError::new("invalid_date"))?;
            // Ein Termin vor dem Planbeginn ließe den Planer stillschweigend fortlaufend planen.
            if parse_date(text)? < week_start {
                return Err(AppError::new("invalid_date"));
            }
            if input.goal.sport.is_some_and(|s| !s.is_endurance()) {
                return Err(AppError::new("invalid_sport"));
            }
            (Some(text), input.goal.sport)
        }
    };

    ensure_goal(store, input.goal.kind, date, sport)?;
    plans::set_availability(store, &availability)?;
    let detail = plans::generate(
        store,
        &GeneratePlanInput {
            week_start: input.week_start.clone(),
            goal_date: None,
            goal_sport: None,
            weeks: None,
            locale: input.locale,
            sports: Vec::new(),
            strength_per_week: planning::DEFAULT_STRENGTH_PER_WEEK,
        },
        plan_id,
        now,
    )?;
    plans::set_plan_status(store, &detail.plan.plan_id, "active")?;
    plans::get_plan(store, &detail.plan.plan_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minutes(days: &[AvailabilityDay]) -> Vec<u16> {
        days.iter().map(|d| d.max_minutes).collect()
    }

    /// Handrechnung: 6 Stunden = 360 Minuten auf Montag, Mittwoch, Freitag, Sonntag (4 Tage).
    /// 360 / 4 = 90, ein Vielfaches von 5, also 90 je gewähltem Tag, kein Rest.
    #[test]
    fn equal_split_without_remainder() {
        let d = distribute_minutes(6.0, &[0, 2, 4, 6], 20).unwrap();
        assert_eq!(minutes(&d), [90, 0, 90, 0, 90, 0, 90]);
        assert_eq!(
            d.iter().map(|x| x.fixed_rest).collect::<Vec<_>>(),
            [false, true, false, true, false, true, false]
        );
        assert!(d.iter().filter(|x| !x.fixed_rest).all(|x| x.allow_hard));
        assert!(d.iter().filter(|x| x.fixed_rest).all(|x| !x.allow_hard));
    }

    /// Handrechnung: 5,5 Stunden = 330 Minuten auf 4 Tage. 330 / 4 = 82,5, abgerundet auf
    /// Vielfache von 5: 80. Rest 330 - 4 * 80 = 10 Minuten = 2 Schritte, die die beiden
    /// späteren Tage bekommen: 80, 80, 85, 85. Summe 330.
    #[test]
    fn remainder_goes_to_the_later_days_and_the_sum_is_kept() {
        let d = distribute_minutes(5.5, &[3, 0, 5, 1], 20).unwrap();
        // Sortiert: Mo, Di, Do, Sa.
        assert_eq!(minutes(&d), [80, 80, 0, 85, 0, 85, 0]);
        assert_eq!(minutes(&d).iter().map(|m| u32::from(*m)).sum::<u32>(), 330);
    }

    /// Handrechnung: 7 Stunden = 420 Minuten auf alle 7 Tage ergibt 60 je Tag.
    #[test]
    fn seven_days_get_an_equal_share() {
        let d = distribute_minutes(7.0, &[0, 1, 2, 3, 4, 5, 6], 20).unwrap();
        assert_eq!(minutes(&d), [60; 7]);
        assert!(d.iter().all(|x| !x.fixed_rest));
    }

    /// Handrechnung: 3,33 Stunden = 199,8 Minuten, auf 5 gerundet 200, auf 2 Tage 100 je Tag.
    #[test]
    fn odd_hours_are_rounded_to_five_minutes_first() {
        let d = distribute_minutes(3.33, &[1, 4], 20).unwrap();
        assert_eq!(minutes(&d), [0, 100, 0, 0, 100, 0, 0]);
    }

    #[test]
    fn bounds_and_duplicates_are_rejected() {
        let code = |h: f64, days: &[u8]| distribute_minutes(h, days, 20).unwrap_err().code;
        assert_eq!(code(0.99, &[0]), "invalid_value");
        assert_eq!(code(30.1, &[0, 1, 2]), "invalid_value");
        assert_eq!(code(f64::NAN, &[0]), "invalid_value");
        assert_eq!(code(f64::INFINITY, &[0]), "invalid_value");
        assert_eq!(code(5.0, &[]), "invalid_value");
        assert_eq!(code(5.0, &[7]), "invalid_value");
        assert_eq!(code(5.0, &[1, 1]), "invalid_value");
        // 1 Stunde auf 7 Tage: 60 / 7 = 8,57, abgerundet 5 Minuten, unter der kleinsten Einheit.
        assert_eq!(code(1.0, &[0, 1, 2, 3, 4, 5, 6]), "too_little_time");
        // 30 Stunden auf einen Tag: 1800 Minuten, mehr als der Tag hat.
        assert_eq!(code(30.0, &[0]), "invalid_value");
        // Die Grenzen selbst gehen: 1 Stunde auf einen Tag, 30 Stunden auf 2 Tage.
        assert!(distribute_minutes(1.0, &[0], 20).is_ok());
        assert!(distribute_minutes(30.0, &[0, 1], 20).is_ok());
    }
}
