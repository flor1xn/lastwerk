//! Planerfüllung: erledigte Last geteilt durch geplante Last je Planwoche (SPEC 4.2 F36, T25).
//!
//! Reine Zählung ohne Modell, deshalb Faustregel als Quelle. Die geplante Last einer Einheit
//! ist die Last der Vorlage (`planned_load`, SPEC 8.1 und 8.3), die erledigte Last die
//! Tageslast aus den Aktivitäten (`DailyLoad`, wie bei der PMC). Beide sind Schätzungen auf
//! derselben Skala (100 je Stunde an der Schwelle), ihr Verhältnis ist ein Maß für die
//! Umsetzung, kein Urteil über die Qualität des Trainings.
//!
//! Die laufende Woche wird nur gegen die bis `as_of` geplante Last gerechnet. Sonst läge jede
//! Woche am Montag bei null Prozent.

use chrono::{NaiveDate, TimeDelta};
use serde::{Deserialize, Serialize};

use crate::plan::PlanWeek;
use crate::pmc::DailyLoad;

/// Unter diesem Verhältnis gilt die Woche als "unter Plan" (Faustregel, 80 Prozent).
pub const ADHERENCE_LOW: f32 = 0.8;

/// Über diesem Verhältnis gilt die Woche als "über Plan" (Faustregel, 120 Prozent).
/// Die Grenzen sind bewusst weit: Die Last einer Einheit weicht schon durch die Schätzung der
/// Vorlage um mehr als wenige Prozent von der gemessenen ab.
pub const ADHERENCE_HIGH: f32 = 1.2;

/// Toleranz der Grenzprüfung (Rundungsreste der Verhältnisse).
const LIMIT_EPS: f32 = 1e-6;

/// Geplante Last je Tag aus den Wochen eines Plans (`Plan::weeks`): Summe der `planned_load`
/// aller Einheiten des Tages. Tage ohne Einheit fehlen.
pub fn planned_daily_loads(weeks: &[PlanWeek]) -> Vec<DailyLoad> {
    weeks
        .iter()
        .flat_map(|w| &w.days)
        .filter(|d| !d.sessions.is_empty())
        .map(|d| DailyLoad {
            date: d.date,
            load: d.sessions.iter().map(|s| s.planned_load).sum(),
        })
        .collect()
}

/// Erfüllung einer Planwoche.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeekAdherence {
    pub week_start: NaiveDate,
    /// Geplante Last der ganzen Woche.
    pub planned_total: f32,
    /// Geplante Last der Tage bis einschließlich `as_of`.
    pub planned_to_date: f32,
    /// Erledigte Last der Tage bis einschließlich `as_of`.
    pub done_to_date: f32,
    /// Die Woche ist vorbei (`as_of` am letzten Tag oder später).
    pub complete: bool,
    /// `done_to_date / planned_to_date`, `None` ohne geplante Last bis `as_of`.
    pub ratio: Option<f32>,
}

fn sum_between(loads: &[DailyLoad], from: NaiveDate, to: NaiveDate) -> f32 {
    // Summe in f64, damit viele kleine Einträge nicht an Genauigkeit verlieren.
    loads
        .iter()
        .filter(|d| d.date >= from && d.date <= to)
        .map(|d| f64::from(d.load))
        .sum::<f64>() as f32
}

/// Erfüllung für `weeks` aufeinanderfolgende Wochen ab `first_week_start` (der Aufrufer nimmt den
/// Montag der ersten Planwoche, wie `PlanWeek::week_start`). Mehrere Einträge für denselben Tag
/// werden addiert.
pub fn weekly_adherence(
    planned: &[DailyLoad],
    done: &[DailyLoad],
    first_week_start: NaiveDate,
    weeks: u32,
    as_of: NaiveDate,
) -> Vec<WeekAdherence> {
    (0..weeks)
        .filter_map(|w| {
            let start = first_week_start.checked_add_signed(TimeDelta::days(7 * i64::from(w)))?;
            let end = start.checked_add_signed(TimeDelta::days(6))?;
            let upto = end.min(as_of);
            let planned_to_date = sum_between(planned, start, upto);
            let done_to_date = sum_between(done, start, upto);
            Some(WeekAdherence {
                week_start: start,
                planned_total: sum_between(planned, start, end),
                planned_to_date,
                done_to_date,
                complete: as_of >= end,
                ratio: (planned_to_date > 0.0).then(|| done_to_date / planned_to_date),
            })
        })
        .collect()
}

/// Deutung der Planerfüllung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdherenceState {
    /// Keine geplante Last bis heute, kein Verhältnis.
    NoData,
    /// Weniger als [`ADHERENCE_LOW`] der geplanten Last erledigt.
    Under,
    /// Zwischen [`ADHERENCE_LOW`] und [`ADHERENCE_HIGH`] einschließlich.
    OnTrack,
    /// Mehr als [`ADHERENCE_HIGH`] der geplanten Last erledigt.
    Over,
}

impl AdherenceState {
    /// Stabiler Code, Schlüssel für Texte in `catalog/locales`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoData => "adherence.no_data",
            Self::Under => "adherence.under",
            Self::OnTrack => "adherence.on_track",
            Self::Over => "adherence.over",
        }
    }
}

/// Deutet ein Verhältnis (Faustregel: Grenzen [`ADHERENCE_LOW`] und [`ADHERENCE_HIGH`]).
pub fn adherence_state(ratio: Option<f32>) -> AdherenceState {
    match ratio {
        Some(r) if r.is_finite() && r >= 0.0 => {
            if r < ADHERENCE_LOW - LIMIT_EPS {
                AdherenceState::Under
            } else if r > ADHERENCE_HIGH + LIMIT_EPS {
                AdherenceState::Over
            } else {
                AdherenceState::OnTrack
            }
        }
        _ => AdherenceState::NoData,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Montag.
    fn monday() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()
    }

    fn day(n: i64) -> NaiveDate {
        monday() + TimeDelta::days(n)
    }

    fn dl(n: i64, load: f32) -> DailyLoad {
        DailyLoad { date: day(n), load }
    }

    /// Plan: Montag 40, Dienstag 60, Donnerstag 100, in Woche 2 Montag 80.
    fn planned() -> Vec<DailyLoad> {
        vec![dl(0, 40.0), dl(1, 60.0), dl(3, 100.0), dl(7, 80.0)]
    }

    #[test]
    fn full_week_is_done_over_planned() {
        // Erledigt 38 + 62 + 50 = 150, geplant 40 + 60 + 100 = 200: 150 / 200 = 0,75.
        let done = vec![dl(0, 38.0), dl(1, 62.0), dl(3, 50.0)];
        let w = weekly_adherence(&planned(), &done, day(0), 1, day(6));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].planned_total, 200.0);
        assert_eq!(w[0].planned_to_date, 200.0);
        assert_eq!(w[0].done_to_date, 150.0);
        assert!(w[0].complete);
        assert_eq!(w[0].ratio, Some(0.75));
        assert_eq!(adherence_state(w[0].ratio), AdherenceState::Under);
    }

    #[test]
    fn the_running_week_is_compared_with_the_planned_load_so_far() {
        // Mittwoch: geplant bis dahin 40 + 60 = 100, erledigt 38 + 62 = 100: Verhältnis 1,0.
        // Die Donnerstagseinheit (100) steht noch aus und zählt nicht gegen die Woche.
        let done = vec![dl(0, 38.0), dl(1, 62.0)];
        let w = weekly_adherence(&planned(), &done, day(0), 2, day(2));
        assert_eq!(w[0].planned_total, 200.0);
        assert_eq!(w[0].planned_to_date, 100.0);
        assert_eq!(w[0].ratio, Some(1.0));
        assert!(!w[0].complete);
        assert_eq!(adherence_state(w[0].ratio), AdherenceState::OnTrack);
        // Die Folgewoche hat bis Mittwoch noch nichts zu erfüllen.
        assert_eq!(w[1].week_start, day(7));
        assert_eq!(w[1].planned_total, 80.0);
        assert_eq!(w[1].planned_to_date, 0.0);
        assert_eq!(w[1].ratio, None);
        assert_eq!(adherence_state(w[1].ratio), AdherenceState::NoData);
    }

    #[test]
    fn extra_training_outside_the_plan_counts_as_done() {
        // Erledigt 100 + 150 = 250 gegen geplant 200: 1,25, über Plan.
        let done = vec![dl(0, 100.0), dl(2, 150.0)];
        let w = weekly_adherence(&planned(), &done, day(0), 1, day(6));
        assert_eq!(w[0].ratio, Some(1.25));
        assert_eq!(adherence_state(w[0].ratio), AdherenceState::Over);
    }

    #[test]
    fn state_limits_are_inclusive() {
        assert_eq!(adherence_state(Some(0.8)), AdherenceState::OnTrack);
        assert_eq!(adherence_state(Some(0.79)), AdherenceState::Under);
        assert_eq!(adherence_state(Some(1.2)), AdherenceState::OnTrack);
        assert_eq!(adherence_state(Some(1.21)), AdherenceState::Over);
        assert_eq!(adherence_state(Some(0.0)), AdherenceState::Under);
        assert_eq!(adherence_state(Some(f32::NAN)), AdherenceState::NoData);
        assert_eq!(adherence_state(Some(-1.0)), AdherenceState::NoData);
    }

    #[test]
    fn duplicate_days_add_up_and_empty_input_is_safe() {
        let doubled = vec![dl(0, 20.0), dl(0, 20.0)];
        let w = weekly_adherence(&doubled, &doubled, day(0), 1, day(6));
        assert_eq!(w[0].planned_total, 40.0);
        assert_eq!(w[0].ratio, Some(1.0));
        assert!(weekly_adherence(&[], &[], day(0), 0, day(6)).is_empty());
        let none = weekly_adherence(&[], &[], day(0), 1, day(6));
        assert_eq!(none[0].ratio, None);
    }

    #[test]
    fn planned_loads_sum_the_sessions_of_a_day() {
        use crate::plan::{Phase, PlanDay, PlanSession, PlanSport, Slot};
        let session = |slot, load| PlanSession {
            slot,
            sport: PlanSport::Cycling,
            template_id: "t".into(),
            duration_min: 60,
            planned_load: load,
            hard: false,
            rpe: None,
            reason_keys: vec![],
        };
        let weeks = vec![PlanWeek {
            week_start: day(0),
            phase: Phase::Base,
            target_load: 100.0,
            planned_load: Some(100.0),
            reason_keys: vec![],
            days: vec![
                PlanDay {
                    date: day(0),
                    sessions: vec![session(Slot::Am, 30.5), session(Slot::Pm, 20.0)],
                },
                PlanDay {
                    date: day(1),
                    sessions: vec![],
                },
                PlanDay {
                    date: day(2),
                    sessions: vec![session(Slot::Am, 49.5)],
                },
            ],
        }];
        let loads = planned_daily_loads(&weeks);
        // 30,5 + 20 = 50,5 am Montag, der leere Dienstag fehlt, 49,5 am Mittwoch.
        assert_eq!(loads, vec![dl(0, 50.5), dl(2, 49.5)]);
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(AdherenceState::Under.code(), "adherence.under");
        assert_eq!(AdherenceState::OnTrack.code(), "adherence.on_track");
        assert_eq!(AdherenceState::Over.code(), "adherence.over");
        assert_eq!(AdherenceState::NoData.code(), "adherence.no_data");
    }
}
