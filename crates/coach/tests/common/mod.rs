//! Gemeinsame Hilfen der Integrationstests. Alle Daten sind synthetisch.

#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use lastwerk_core::plan::{
    DayAvailability, GoalSpec, Locale, Plan, PlanContext, PlanSport, PlanWeek, Priority, Rules,
    SportWeight, TemplateCatalog, WeekHistory, generate_plan,
};
use lastwerk_core::{PmcParams, Sport};

pub const TEMPLATES_JSON: &str = include_str!("../../../../catalog/templates.json");
pub const RULES_JSON: &str = include_str!("../../../../catalog/rules.default.json");

pub fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

pub fn rules() -> Rules {
    serde_json::from_str(RULES_JSON).unwrap()
}

pub fn catalog() -> TemplateCatalog {
    serde_json::from_str(TEMPLATES_JSON).unwrap()
}

pub fn created_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 13, 8, 30, 15).unwrap()
}

/// Kontext für zwei Wochen ab Montag, 4. Januar 2027: Rad und Lauf, eine Krafteinheit,
/// 90 Minuten je Tag, vier Wochen Historie mit Last.
pub fn context() -> PlanContext {
    let day = DayAvailability {
        max_minutes: 90,
        allow_hard: true,
        fixed_rest: false,
    };
    let week_start = date(2027, 1, 4);
    let mut history = Vec::new();
    for k in 1..=4 {
        let start = week_start - chrono::TimeDelta::days(7 * k);
        history.push(WeekHistory {
            week_start: start,
            sport: Sport::Cycling,
            load: 250.0 + 10.0 * k as f32,
            minutes: 330.0,
        });
        history.push(WeekHistory {
            week_start: start,
            sport: Sport::Running,
            load: 80.0,
            minutes: 120.0,
        });
    }
    PlanContext {
        plan_id: "00000000-0000-4000-8000-00000000000a".into(),
        created_at: created_at(),
        locale: Locale::De,
        week_start,
        goal: GoalSpec::Event {
            event_date: date(2027, 1, 16),
            sport: PlanSport::Cycling,
            priority: Priority::A,
        },
        horizon_weeks: 2,
        ctl0: 40.4,
        atl0: 45.6,
        history,
        availability: [day; 7],
        sports: vec![
            SportWeight {
                sport: PlanSport::Cycling,
                weight: 2.0,
            },
            SportWeight {
                sport: PlanSport::Running,
                weight: 1.0,
            },
        ],
        strength_per_week: 1,
        ctl_rise_per_week: None,
        pmc_params: PmcParams::default(),
        catalog: catalog(),
        recovery: Vec::new(),
    }
}

pub fn base_plan(ctx: &PlanContext) -> Plan {
    generate_plan(ctx, &rules()).unwrap()
}

/// Alle Slots des Plans als (Datum, Slot, Vorlage, Sportart, hart).
pub fn slots(plan: &Plan) -> Vec<(NaiveDate, &'static str, String, PlanSport, bool)> {
    let mut out = Vec::new();
    for d in plan.weeks.iter().flat_map(|w| w.days.iter()) {
        for s in &d.sessions {
            let slot = match s.slot {
                lastwerk_core::plan::Slot::Am => "am",
                lastwerk_core::plan::Slot::Pm => "pm",
            };
            out.push((d.date, slot, s.template_id.clone(), s.sport, s.hard));
        }
    }
    out
}

pub fn week_minutes(w: &PlanWeek) -> u32 {
    w.total_minutes()
}

/// Antwort des Modells als JSON-Text.
pub fn reply(edits: &str, explanation: &str) -> String {
    format!(
        r#"{{"schema_version":1,"edits":[{edits}],"explanation":{}}}"#,
        serde_json::to_string(explanation).unwrap()
    )
}
