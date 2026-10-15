//! Eingang an das Modell (SPEC 9.3): nur aggregierte, nicht identifizierende Werte.
//!
//! `RefineInput` ist ein eigener Typ mit genau den Feldern aus 9.3. Er wird aus dem Plan, dem
//! Kontext und den Regeln gebaut und enthält nie: Freitext aus FIT-Dateien (Aktivitätsnamen,
//! Notizen, Workout-Namen), Zielbezeichnungen, Dateipfade, GPS, Zeitstempel einzelner
//! Aktivitäten, Gerätekennungen, Strava-Daten oder Erholungs-Rohwerte (nur das Zeichen
//! `recovery_signal`). Der Typ kennt keinen Text außer festen Codes (Sportart, Phase, Klasse,
//! Regelcode, Wochentag) und Vorlagen-IDs aus dem Katalog.
//!
//! Der JSON-Text von [`RefineInput::to_json`] ist genau der Text, der als Nutzernachricht zum
//! Modell geht. Die CLI zeigt ihn vor dem Aufruf an (F24).

use std::collections::BTreeMap;

use chrono::{NaiveDate, TimeDelta};
use lastwerk_core::Sport;
use lastwerk_core::plan::{
    GoalKind, IntensityClass, Phase, Plan, PlanContext, PlanSport, Rules, Slot, recovery_signal,
};
use serde::Serialize;

/// Version des Eingangsformats. Es gehört zur festen Systemanweisung.
pub const INPUT_VERSION: u32 = 1;

/// Zahl der Wochen vor dem Planbeginn, deren Last mitgeht (SPEC 9.3: letzte vier Wochen).
pub const HISTORY_WEEKS: i64 = 4;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GoalInput {
    /// `event` oder `continuous`.
    pub kind: &'static str,
    /// Sportart des Termins, nur bei `event`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sport: Option<PlanSport>,
    /// Ganze Wochen vom Planbeginn bis zum Termin, nur bei `event`. Nie das Datum selbst.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weeks_to_event: Option<i64>,
}

/// CTL, ATL und TSB am Tag vor dem Planbeginn, auf ganze Zahlen gerundet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FitnessInput {
    pub ctl: i64,
    pub atl: i64,
    pub tsb: i64,
}

/// Last einer vergangenen Woche je Sportart, gerundet. `weeks_ago` 1 ist die Woche vor dem
/// Planbeginn. Ohne Datum, damit kein Zeitbezug zu einzelnen Aktivitäten entsteht.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeeklyLoadInput {
    pub weeks_ago: i64,
    pub load_by_sport: BTreeMap<&'static str, i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DayInput {
    /// `mon` bis `sun`.
    pub weekday: &'static str,
    pub max_minutes: u16,
    pub allow_hard: bool,
    pub fixed_rest: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeekInput {
    pub week_start: NaiveDate,
    pub phase: Phase,
}

/// Ein belegter Slot des Basisplans.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SlotInput {
    pub date: NaiveDate,
    pub slot: Slot,
    pub template_id: String,
    pub duration_min: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TemplateInput {
    pub template_id: String,
    pub class: IntensityClass,
    pub min_duration_min: u32,
    pub max_duration_min: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GuardrailInput {
    /// Regelnummer, zum Beispiel `R03`.
    pub code: String,
    /// `hard` oder `soft`.
    pub hardness: &'static str,
    /// Parameter der Regel als Zahlen, gerundet auf höchstens drei Nachkommastellen.
    pub params: BTreeMap<&'static str, f64>,
}

/// Der gesamte Eingang des Modells (SPEC 9.3).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefineInput {
    pub input_version: u32,
    pub goal: GoalInput,
    pub fitness: FitnessInput,
    /// Erholungsdaten deuten auf einen müden Körper hin. Nur das Zeichen, nie ein Rohwert.
    pub recovery_signal: bool,
    pub weekly_load: Vec<WeeklyLoadInput>,
    pub availability: Vec<DayInput>,
    pub weeks: Vec<WeekInput>,
    pub base_plan: Vec<SlotInput>,
    /// Erlaubte Vorlagen je Sportart (nur Sportarten, die im Basisplan vorkommen).
    pub allowed_templates: BTreeMap<&'static str, Vec<TemplateInput>>,
    pub guardrails: Vec<GuardrailInput>,
}

fn sport_name(s: PlanSport) -> &'static str {
    match s {
        PlanSport::Cycling => "cycling",
        PlanSport::Running => "running",
        PlanSport::Swimming => "swimming",
        PlanSport::Strength => "strength",
        PlanSport::Rest => "rest",
    }
}

fn history_sport_name(s: Sport) -> &'static str {
    match s {
        Sport::Cycling => "cycling",
        Sport::Running => "running",
        Sport::Swimming => "swimming",
        Sport::Strength => "strength",
        Sport::Other => "other",
    }
}

fn weekday_name(i: usize) -> &'static str {
    ["mon", "tue", "wed", "thu", "fri", "sat", "sun"][i % 7]
}

/// `f32`-Parameter ohne Darstellungsrest (0,3 als `f32` ist 0,30000001) auf drei Stellen.
fn param(x: f32) -> f64 {
    (f64::from(x) * 1000.0).round() / 1000.0
}

fn hardness_name(h: lastwerk_core::plan::Hardness) -> &'static str {
    match h {
        lastwerk_core::plan::Hardness::Hard => "hard",
        lastwerk_core::plan::Hardness::Soft => "soft",
    }
}

/// Aktive Leitplanken als Codes mit Parametern. Die Reihenfolge ist R01 bis R12.
fn guardrails(ctx: &PlanContext, rules: &Rules) -> Vec<GuardrailInput> {
    let mut out = Vec::with_capacity(12);
    let mut add = |code: &str, hardness, params: &[(&'static str, f64)]| {
        out.push(GuardrailInput {
            code: code.to_string(),
            hardness: hardness_name(hardness),
            params: params.iter().copied().collect(),
        });
    };
    add(
        "R01",
        rules.r01.meta.hardness,
        &[("max_ctl_rise_per_week", param(ctx.ctl_rise_limit(rules)))],
    );
    add(
        "R02",
        rules.r02.meta.hardness,
        &[
            ("warn_at", param(rules.r02.warn_at)),
            ("reject_at", param(rules.r02.reject_at)),
        ],
    );
    add(
        "R03",
        rules.r03.meta.hardness,
        &[("max_hard_per_week", f64::from(rules.r03.max_hard_per_week))],
    );
    add("R04", rules.r04.meta.hardness, &[]);
    add(
        "R05",
        rules.r05.meta.hardness,
        &[(
            "min_rest_days_per_week",
            f64::from(rules.r05.min_rest_days_per_week),
        )],
    );
    add(
        "R06",
        rules.r06.meta.hardness,
        &[
            ("load_weeks", f64::from(rules.r06.load_weeks)),
            ("recovery_min", param(rules.r06.recovery_min)),
            ("recovery_max", param(rules.r06.recovery_max)),
        ],
    );
    add(
        "R07",
        rules.r07.meta.hardness,
        &[
            ("min_per_week", f64::from(rules.r07.min_per_week)),
            ("max_per_week", f64::from(rules.r07.max_per_week)),
            (
                "max_per_week_taper",
                f64::from(rules.r07.max_per_week_taper),
            ),
        ],
    );
    add(
        "R08",
        rules.r08.meta.hardness,
        &[("min_easy_share", param(rules.r08.min_easy_share))],
    );
    add(
        "R09",
        rules.r09.meta.hardness,
        &[
            ("taper_days_min", f64::from(rules.r09.taper_days_min)),
            ("taper_days_max", f64::from(rules.r09.taper_days_max)),
        ],
    );
    add(
        "R10",
        rules.r10.meta.hardness,
        &[("tsb_warn_below", param(rules.r10.tsb_warn_below))],
    );
    add(
        "R11",
        rules.r11.meta.hardness,
        &[(
            "max_sessions_per_day",
            f64::from(rules.r11.max_sessions_per_day),
        )],
    );
    add(
        "R12",
        rules.r12.meta.hardness,
        &[
            ("hrv_drop_fraction", param(rules.r12.hrv_drop_fraction)),
            (
                "sleep_deficit_fraction",
                param(rules.r12.sleep_deficit_fraction),
            ),
        ],
    );
    out
}

impl RefineInput {
    /// Baut den Eingang aus dem Basisplan, dem Kontext und den Regeln. Gelesen werden nur die
    /// Felder, die 9.3 nennt; `ctx.history` liefert die Wochenlast, `ctx.recovery` nur das
    /// Zeichen `recovery_signal`.
    pub fn build(base: &Plan, ctx: &PlanContext, rules: &Rules) -> Self {
        let goal = match (&base.goal.kind, base.goal.event_date) {
            (GoalKind::Event, Some(date)) => GoalInput {
                kind: "event",
                sport: base.goal.sport,
                weeks_to_event: Some((date - ctx.week_start).num_days().div_euclid(7).max(0)),
            },
            _ => GoalInput {
                kind: "continuous",
                sport: None,
                weeks_to_event: None,
            },
        };
        let fitness = FitnessInput {
            ctl: f64::from(ctx.ctl0).round() as i64,
            atl: f64::from(ctx.atl0).round() as i64,
            tsb: (f64::from(ctx.ctl0) - f64::from(ctx.atl0)).round() as i64,
        };
        let weekly_load = (1..=HISTORY_WEEKS)
            .map(|k| {
                let start = ctx.week_start - TimeDelta::days(7 * k);
                let mut by_sport: BTreeMap<&'static str, f64> = BTreeMap::new();
                for h in ctx.history.iter().filter(|h| h.week_start == start) {
                    *by_sport.entry(history_sport_name(h.sport)).or_insert(0.0) +=
                        f64::from(h.load.max(0.0));
                }
                WeeklyLoadInput {
                    weeks_ago: k,
                    load_by_sport: by_sport
                        .into_iter()
                        .map(|(s, l)| (s, l.round() as i64))
                        .collect(),
                }
            })
            .collect();
        let availability = ctx
            .availability
            .iter()
            .enumerate()
            .map(|(i, a)| DayInput {
                weekday: weekday_name(i),
                max_minutes: a.max_minutes,
                allow_hard: a.allow_hard,
                fixed_rest: a.fixed_rest,
            })
            .collect();
        let weeks = base
            .weeks
            .iter()
            .map(|w| WeekInput {
                week_start: w.week_start,
                phase: w.phase,
            })
            .collect();
        let mut sports: Vec<PlanSport> = Vec::new();
        let mut base_plan = Vec::new();
        for day in base.weeks.iter().flat_map(|w| w.days.iter()) {
            for s in &day.sessions {
                base_plan.push(SlotInput {
                    date: day.date,
                    slot: s.slot,
                    template_id: s.template_id.clone(),
                    duration_min: s.duration_min,
                });
                if !sports.contains(&s.sport) {
                    sports.push(s.sport);
                }
            }
        }
        let mut allowed_templates: BTreeMap<&'static str, Vec<TemplateInput>> = BTreeMap::new();
        for sport in sports {
            let list: Vec<TemplateInput> = ctx
                .catalog
                .templates
                .iter()
                .filter(|t| t.sport == sport)
                .map(|t| TemplateInput {
                    template_id: t.id.clone(),
                    class: t.intensity_class,
                    min_duration_min: t.duration_min.min,
                    max_duration_min: t.duration_min.max,
                })
                .collect();
            allowed_templates.insert(sport_name(sport), list);
        }
        Self {
            input_version: INPUT_VERSION,
            goal,
            fitness,
            recovery_signal: recovery_signal(&ctx.recovery, ctx.week_start, &rules.r12).is_some(),
            weekly_load,
            availability,
            weeks,
            base_plan,
            allowed_templates,
            guardrails: guardrails(ctx, rules),
        }
    }

    /// Der Eingang als JSON-Text, genau so, wie er zum Modell geht (kompakt, eine Zeile).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("RefineInput hat nur serialisierbare Felder")
    }
}
