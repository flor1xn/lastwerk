//! Kontexte des Planers für die Tests der CLI (nur Tests, alles synthetisch).
//!
//! Die beiden festen Kontexte entsprechen den Golden-Kontexten aus `lastwerk-core`
//! (Termin in 12 Wochen, fortlaufend 8 Wochen). Die Strategie `arb_ctx` erzeugt zufällige
//! gültige Kontexte für Eigenschaftstests.

use chrono::{DateTime, NaiveDate, TimeDelta, TimeZone, Utc};
use lastwerk_core::plan::{
    DayAvailability, GoalSpec, Locale, PlanContext, PlanSport, Priority, Rules, SportWeight,
    TemplateCatalog, WeekHistory,
};
use lastwerk_core::{PmcParams, Sport};
use proptest::prelude::*;

use crate::planning::load_rules;

pub const PLAN_ID: &str = "00000000-0000-4000-8000-000000000001";

pub fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

pub fn created_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 13, 8, 0, 0).unwrap()
}

/// Montag, an dem die Kontexte beginnen (4. Januar 2027).
pub fn monday() -> NaiveDate {
    date(2027, 1, 4)
}

pub fn rules_and_catalog() -> (Rules, TemplateCatalog) {
    load_rules().expect("eingebettete Dateien lassen sich lesen")
}

fn avail(max_minutes: u16, allow_hard: bool, fixed_rest: bool) -> DayAvailability {
    DayAvailability {
        max_minutes,
        allow_hard,
        fixed_rest,
    }
}

/// Üblicher Kontext: Montag, Mittwoch und Sonntag ohne harte Einheiten, Freitag frei, Samstag
/// lang. Fortlaufend, 8 Wochen, Rad, Lauf und Schwimmen.
pub fn base_ctx() -> PlanContext {
    let (_, catalog) = rules_and_catalog();
    PlanContext {
        plan_id: PLAN_ID.to_string(),
        created_at: created_at(),
        locale: Locale::De,
        week_start: monday(),
        goal: GoalSpec::Continuous,
        horizon_weeks: 8,
        ctl0: 40.0,
        atl0: 45.0,
        history: Vec::new(),
        availability: [
            avail(60, false, false),
            avail(90, true, false),
            avail(75, false, false),
            avail(90, true, false),
            avail(0, false, true),
            avail(240, true, false),
            avail(180, false, false),
        ],
        sports: vec![
            SportWeight {
                sport: PlanSport::Cycling,
                weight: 3.0,
            },
            SportWeight {
                sport: PlanSport::Running,
                weight: 2.0,
            },
            SportWeight {
                sport: PlanSport::Swimming,
                weight: 1.0,
            },
        ],
        strength_per_week: 1,
        ctl_rise_per_week: None,
        pmc_params: PmcParams::default(),
        catalog,
        recovery: Vec::new(),
    }
}

/// Termin am Samstag, 27. März 2027 (Woche 12), Rad.
pub fn event_ctx() -> PlanContext {
    PlanContext {
        goal: GoalSpec::Event {
            event_date: date(2027, 3, 27),
            sport: PlanSport::Cycling,
            priority: Priority::A,
        },
        horizon_weeks: 26,
        history: vec![WeekHistory {
            week_start: monday() - TimeDelta::days(7),
            sport: Sport::Cycling,
            load: 300.0,
            minutes: 230.0,
        }],
        ..base_ctx()
    }
}

/// Fortlaufend, 8 Wochen.
pub fn continuous_ctx() -> PlanContext {
    base_ctx()
}

fn arb_availability() -> impl Strategy<Value = DayAvailability> {
    (
        prop_oneof![
            1 => Just(0u16),
            1 => 10u16..60,
            6 => 60u16..=300,
            1 => 301u16..=480,
        ],
        any::<bool>(),
        prop::bool::weighted(0.1),
    )
        .prop_map(|(max_minutes, allow_hard, fixed_rest)| DayAvailability {
            max_minutes,
            allow_hard,
            fixed_rest,
        })
}

fn arb_sports() -> impl Strategy<Value = Vec<SportWeight>> {
    (
        prop::sample::subsequence(
            vec![PlanSport::Cycling, PlanSport::Running, PlanSport::Swimming],
            1..=3,
        ),
        prop::collection::vec(1.0f32..6.0, 3),
    )
        .prop_map(|(sports, weights)| {
            sports
                .into_iter()
                .zip(weights)
                .map(|(sport, weight)| SportWeight { sport, weight })
                .collect()
        })
}

/// Zufällige, gültige Kontexte: Horizont bis 12 Wochen, Termin vor und hinter dem Horizont,
/// wechselnde Verfügbarkeit, Historie passend zur Fitness oder ohne Historie.
pub fn arb_ctx() -> impl Strategy<Value = PlanContext> {
    let time = (
        0i64..40,
        1u32..=12,
        prop::option::of((0i64..=98, 0usize..3, 0usize..3)),
        0.0f32..120.0,
        0.5f32..1.8,
    );
    let history = (prop::option::of(0.3f32..1.8), 0.7f32..1.6);
    let setup = (
        prop::array::uniform7(arb_availability()),
        arb_sports(),
        0u8..=2,
        prop::option::of(2.0f32..8.0),
        (30.0f32..60.0, 4.0f32..12.0),
    );
    (time, history, setup).prop_map(
        |(
            (week_offset, horizon, event, ctl0, atl_factor),
            (hist_factor, minutes_factor),
            (availability, sports, strength, rise, (tau_ctl, tau_atl)),
        )| {
            let week_start = monday() + TimeDelta::days(7 * week_offset);
            let goal = match event {
                Some((days, sport, prio)) => GoalSpec::Event {
                    event_date: week_start + TimeDelta::days(days),
                    sport: [PlanSport::Cycling, PlanSport::Running, PlanSport::Swimming][sport],
                    priority: [Priority::A, Priority::B, Priority::C][prio],
                },
                None => GoalSpec::Continuous,
            };
            let history = hist_factor
                .map(|f| {
                    let load = 7.0 * ctl0 * f;
                    vec![WeekHistory {
                        week_start: week_start - TimeDelta::days(7),
                        sport: Sport::Cycling,
                        load,
                        minutes: load * minutes_factor,
                    }]
                })
                .unwrap_or_default();
            PlanContext {
                week_start,
                goal,
                horizon_weeks: horizon,
                ctl0,
                atl0: ctl0 * atl_factor,
                history,
                availability,
                sports,
                strength_per_week: strength,
                ctl_rise_per_week: rise,
                pmc_params: PmcParams {
                    tau_ctl_d: tau_ctl,
                    tau_atl_d: tau_atl,
                    ..PmcParams::default()
                },
                ..base_ctx()
            }
        },
    )
}
