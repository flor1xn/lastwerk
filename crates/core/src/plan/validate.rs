//! Prüfung eines Plans gegen die Leitplanken R1 bis R11 (SPEC 8.4).
//!
//! Harte Verletzungen lehnen den Plan ab, weiche werden zu Warnungen mit Code `R**`. Die Härte
//! steht je Regel in `rules.default.json` (`meta.hardness`). Bewusst keine
//! Regel zu ACWR und keine Verletzungsprognose (SPEC 8.4, "Bewusst keine Leitplanke").
//!
//! Die Prüfung rechnet nur mit dem, was im Plan steht (`planned_load`, `duration_min`, `hard`).
//! Der Katalog dient nur R8, um die Zeit in Zone 1 und 2 aus der Vorlage zu bestimmen.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate, TimeDelta};
use serde::{Deserialize, Serialize};

use super::context::PlanContext;
use super::recovery::{first_hard_or_swapped, recovery_signal};
use super::rules::{Hardness, Rules};
use super::types::{Goal, GoalKind, Phase, Plan, PlanDay, PlanWarning, PlanWeek, Severity};
use crate::pmc::{DailyLoad, PmcPoint, pmc};

/// Toleranz für Vergleiche von Gleitkommawerten (Rundung auf eine Nachkommastelle im Plan).
const EPS: f64 = 1e-3;

/// Verletzung einer Regel. `message_key` verweist auf den Text in `catalog/locales`, `detail`
/// trägt die Parameter für den Text (Zahlen als Zeichenketten).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    /// Regelnummer, zum Beispiel `R03`.
    pub code: String,
    pub hardness: Hardness,
    pub date: Option<NaiveDate>,
    pub message_key: String,
    pub detail: BTreeMap<String, String>,
}

impl Violation {
    fn new(code: &str, hardness: Hardness, date: NaiveDate, detail: &[(&str, String)]) -> Self {
        Self {
            code: code.to_string(),
            hardness,
            date: Some(date),
            message_key: format!("rule.{code}.explanation"),
            detail: detail
                .iter()
                .map(|(k, v)| ((*k).to_string(), v.clone()))
                .collect(),
        }
    }
}

/// Weiche (und, falls der Plan trotzdem ausgegeben wird, harte) Verletzungen als Warnungen.
/// Sortiert nach Datum und Code, ohne Duplikate.
pub fn violations_to_warnings(violations: &[Violation]) -> Vec<PlanWarning> {
    let mut out: Vec<PlanWarning> = violations
        .iter()
        .map(|v| PlanWarning {
            code: v.code.clone(),
            severity: Severity::Warn,
            date: v.date,
        })
        .collect();
    out.sort_by(|a, b| (a.date, &a.code).cmp(&(b.date, &b.code)));
    out.dedup();
    out
}

/// Prüft `plan` gegen die Regeln. Ein Plan ist annehmbar, wenn keine Verletzung hart ist
/// (`Plan::is_acceptable`).
pub fn validate_plan(plan: &Plan, ctx: &PlanContext, rules: &Rules) -> Vec<Violation> {
    validate_weeks(&plan.weeks, &plan.goal, ctx, rules)
}

/// Parameter aus `f32` ohne Darstellungsrest (0,3 als `f32` ist 0,30000001), damit Grenzen wie
/// "ab 30 Prozent" genau greifen.
fn param(x: f32) -> f64 {
    (f64::from(x) * 1e6).round() / 1e6
}

fn weekday_index(date: NaiveDate) -> usize {
    date.weekday().num_days_from_monday() as usize
}

fn session_is_hard_day(day: &PlanDay) -> bool {
    day.sessions.iter().any(|s| s.hard)
}

fn session_is_strength_day(day: &PlanDay) -> bool {
    day.sessions
        .iter()
        .any(|s| s.sport == super::types::PlanSport::Strength)
}

fn week_end(w: &PlanWeek) -> NaiveDate {
    w.days
        .last()
        .map(|d| d.date)
        .unwrap_or(w.week_start + TimeDelta::days(6))
}

/// Verlauf ab einem Starttag, abfragbar nach Datum.
pub(crate) struct PmcSeries {
    from: NaiveDate,
    points: Vec<PmcPoint>,
}

impl PmcSeries {
    fn at(&self, date: NaiveDate) -> Option<&PmcPoint> {
        let i = usize::try_from((date - self.from).num_days()).ok()?;
        self.points.get(i)
    }
}

/// Verlauf von CTL, ATL und TSB über alle Wochen aus den geplanten Lasten.
pub(crate) fn plan_pmc(weeks: &[PlanWeek], ctx: &PlanContext) -> Vec<PmcPoint> {
    let Some(first) = weeks.first() else {
        return Vec::new();
    };
    let Some(last) = weeks.last() else {
        return Vec::new();
    };
    let daily: Vec<DailyLoad> = weeks
        .iter()
        .flat_map(|w| w.days.iter())
        .flat_map(|d| {
            d.sessions.iter().map(|s| DailyLoad {
                date: d.date,
                load: s.planned_load,
            })
        })
        .collect();
    pmc(&daily, &ctx.pmc_start(), first.week_start, week_end(last))
}

pub(crate) fn validate_weeks(
    weeks: &[PlanWeek],
    goal: &Goal,
    ctx: &PlanContext,
    rules: &Rules,
) -> Vec<Violation> {
    let mut out = Vec::new();
    if weeks.is_empty() {
        return out;
    }
    let points = PmcSeries {
        from: weeks[0].week_start,
        points: plan_pmc(weeks, ctx),
    };
    let point_at = &points;

    check_r01(weeks, ctx, rules, point_at, &mut out);
    check_r02(weeks, ctx, rules, &mut out);
    check_r03(weeks, rules, &mut out);
    check_r04(weeks, rules, &mut out);
    check_r05(weeks, rules, &mut out);
    check_r06(weeks, rules, &mut out);
    check_r07(weeks, ctx, rules, &mut out);
    check_r08(weeks, ctx, rules, &mut out);
    check_r09(weeks, goal, rules, &mut out);
    check_r10(weeks, rules, point_at, &mut out);
    check_r11(weeks, ctx, rules, &mut out);
    check_r12(weeks, ctx, rules, &mut out);

    out.sort_by(|a, b| (a.date, &a.code, &a.detail).cmp(&(b.date, &b.code, &b.detail)));
    out
}

/// R1 (hart, Faustregel): Anstieg der CTL je Woche. Gemessen als CTL am letzten Tag der Woche
/// minus CTL am Ende der Vorwoche (für die erste Woche `ctl0`). Grenze 5 Punkte, einstellbar
/// bis 8.
fn check_r01(
    weeks: &[PlanWeek],
    ctx: &PlanContext,
    rules: &Rules,
    point_at: &PmcSeries,
    out: &mut Vec<Violation>,
) {
    let limit = f64::from(ctx.ctl_rise_limit(rules));
    let mut prev = f64::from(ctx.ctl0);
    for w in weeks {
        let Some(p) = point_at.at(week_end(w)) else {
            continue;
        };
        let end = f64::from(p.ctl);
        let rise = end - prev;
        if rise > limit + EPS {
            out.push(Violation::new(
                "R01",
                rules.r01.meta.hardness,
                w.week_start,
                &[
                    ("rise", format!("{rise:.2}")),
                    ("limit", format!("{limit:.2}")),
                ],
            ));
        }
        prev = end;
    }
}

/// R2 (weich ab 10 Prozent, hart ab 30 Prozent; Faustregel, Q12, Q13): Anstieg des
/// Wochenvolumens, gemessen als Trainingszeit in Minuten. Bezug ist die Vorwoche. Nach einer
/// Erholungswoche gilt die letzte Woche davor als Bezug, sonst wäre jede Rückkehr zum
/// normalen Umfang eine Verletzung (Konvention). Ohne Bezug oder bei einem Bezug unter
/// `min_reference_minutes` gibt es keine Prüfung.
fn check_r02(weeks: &[PlanWeek], ctx: &PlanContext, rules: &Rules, out: &mut Vec<Violation>) {
    let r = &rules.r02;
    let min_ref = param(r.min_reference_minutes).max(f64::MIN_POSITIVE);
    let mut reference = ctx
        .previous_week()
        .map(|p| p.minutes)
        .filter(|m| *m >= min_ref);
    for w in weeks {
        let minutes = f64::from(w.total_minutes());
        if let Some(base) = reference {
            let ratio = (minutes - base) / base;
            let level = if ratio >= param(r.reject_at) - 1e-9 {
                Some(Hardness::Hard)
            } else if ratio >= param(r.warn_at) - 1e-9 {
                Some(r.meta.hardness)
            } else {
                None
            };
            if let Some(h) = level {
                out.push(Violation::new(
                    "R02",
                    h,
                    w.week_start,
                    &[
                        ("increase", format!("{:.3}", ratio)),
                        ("minutes", format!("{minutes:.0}")),
                        ("previous_minutes", format!("{base:.0}")),
                    ],
                ));
            }
        }
        if w.phase != Phase::Recovery {
            reference = (minutes >= min_ref).then_some(minutes);
        }
    }
}

/// R3 (hart, Faustregel, Q5 beschreibend): höchstens 2 harte Einheiten je Woche.
fn check_r03(weeks: &[PlanWeek], rules: &Rules, out: &mut Vec<Violation>) {
    for w in weeks {
        let hard = w
            .days
            .iter()
            .flat_map(|d| d.sessions.iter())
            .filter(|s| s.hard)
            .count();
        if hard > rules.r03.max_hard_per_week as usize {
            out.push(Violation::new(
                "R03",
                rules.r03.meta.hardness,
                w.week_start,
                &[
                    ("hard", hard.to_string()),
                    ("max", rules.r03.max_hard_per_week.to_string()),
                ],
            ));
        }
    }
}

/// R4 (hart, Faustregel): keine harten Einheiten an aufeinanderfolgenden Tagen, auch nicht
/// über die Wochengrenze.
fn check_r04(weeks: &[PlanWeek], rules: &Rules, out: &mut Vec<Violation>) {
    let days: Vec<&PlanDay> = weeks.iter().flat_map(|w| w.days.iter()).collect();
    for pair in days.windows(2) {
        if (pair[1].date - pair[0].date).num_days() == 1
            && session_is_hard_day(pair[0])
            && session_is_hard_day(pair[1])
        {
            out.push(Violation::new(
                "R04",
                rules.r04.meta.hardness,
                pair[1].date,
                &[("previous", pair[0].date.to_string())],
            ));
        }
    }
}

/// R5 (hart, Faustregel): mindestens ein Ruhetag je Woche. Ein Ruhetag ist ein Tag ohne
/// Einheit.
fn check_r05(weeks: &[PlanWeek], rules: &Rules, out: &mut Vec<Violation>) {
    for w in weeks {
        let rest = w.days.iter().filter(|d| d.sessions.is_empty()).count();
        if rest < rules.r05.min_rest_days_per_week as usize {
            out.push(Violation::new(
                "R05",
                rules.r05.meta.hardness,
                w.week_start,
                &[
                    ("rest_days", rest.to_string()),
                    ("min", rules.r05.min_rest_days_per_week.to_string()),
                ],
            ));
        }
    }
}

/// R6 (weich, Faustregel): nach 3 Belastungswochen eine Erholungswoche mit 60 bis 70 Prozent
/// der geplanten Last der letzten Belastungswoche. Taper und Rennwoche beenden den Block wie
/// eine Erholungswoche. Eine Belastungswoche über die Blocklänge hinaus ist erlaubt, wenn
/// Taper oder Rennwoche folgt, und am Planende ungeprüft.
fn check_r06(weeks: &[PlanWeek], rules: &Rules, out: &mut Vec<Violation>) {
    let r = &rules.r06;
    let mut streak = 0u32;
    for (i, w) in weeks.iter().enumerate() {
        if w.phase.is_load_week() {
            streak += 1;
            let next = weeks.get(i + 1).map(|n| n.phase);
            let excess = streak > r.load_weeks;
            if excess && next.is_some_and(|p| !matches!(p, Phase::Taper | Phase::Race)) {
                out.push(Violation::new(
                    "R06",
                    r.meta.hardness,
                    w.week_start,
                    &[
                        ("load_weeks", streak.to_string()),
                        ("max", r.load_weeks.to_string()),
                    ],
                ));
            }
        } else {
            streak = 0;
        }
        if w.phase == Phase::Recovery && i > 0 && weeks[i - 1].phase.is_load_week() {
            let prev = f64::from(weeks[i - 1].sessions_load());
            if prev > 0.0 {
                let ratio = f64::from(w.sessions_load()) / prev;
                let tol = f64::from(r.ratio_tolerance);
                if ratio < f64::from(r.recovery_min) - tol - EPS
                    || ratio > f64::from(r.recovery_max) + tol + EPS
                {
                    out.push(Violation::new(
                        "R06",
                        r.meta.hardness,
                        w.week_start,
                        &[
                            ("ratio", format!("{ratio:.3}")),
                            ("min", format!("{:.2}", r.recovery_min)),
                            ("max", format!("{:.2}", r.recovery_max)),
                        ],
                    ));
                }
            }
        }
    }
}

/// R7 (weich, Faustregel): 1 bis 2 Krafteinheiten je Woche (in Taper und Rennwoche höchstens
/// 1), nicht am Tag vor einer harten Einheit. Die Untergrenze gilt nur, wenn der Nutzer
/// Kraft wünscht (`strength_per_week > 0`) und die Woche keine Rennwoche ist.
fn check_r07(weeks: &[PlanWeek], ctx: &PlanContext, rules: &Rules, out: &mut Vec<Violation>) {
    let r = &rules.r07;
    for w in weeks {
        let count = w
            .days
            .iter()
            .flat_map(|d| d.sessions.iter())
            .filter(|s| s.sport == super::types::PlanSport::Strength)
            .count();
        let max = if matches!(w.phase, Phase::Taper | Phase::Race) {
            r.max_per_week_taper
        } else {
            r.max_per_week
        } as usize;
        let too_few = ctx.strength_per_week > 0
            && w.phase != Phase::Race
            && count < r.min_per_week as usize
            && w.days.iter().any(|d| !d.sessions.is_empty());
        if count > max || too_few {
            out.push(Violation::new(
                "R07",
                r.meta.hardness,
                w.week_start,
                &[
                    ("strength", count.to_string()),
                    ("min", r.min_per_week.to_string()),
                    ("max", max.to_string()),
                ],
            ));
        }
    }
    let days: Vec<&PlanDay> = weeks.iter().flat_map(|w| w.days.iter()).collect();
    for pair in days.windows(2) {
        if (pair[1].date - pair[0].date).num_days() == 1
            && session_is_strength_day(pair[0])
            && session_is_hard_day(pair[1])
        {
            out.push(Violation::new(
                "R07",
                r.meta.hardness,
                pair[0].date,
                &[("hard_next_day", pair[1].date.to_string())],
            ));
        }
    }
}

/// R8 (weich, Q5 beschreibend, Grenzwert 70 Prozent Faustregel): Anteil der Zeit in Zone 1 und
/// 2 an der Trainingszeit mit Zonen. Kraft zählt nicht. Die Zeit in Zone 1 und 2 folgt aus
/// den Schritten der Vorlage. Eine unbekannte Vorlage zählt als nicht leicht.
fn check_r08(weeks: &[PlanWeek], ctx: &PlanContext, rules: &Rules, out: &mut Vec<Violation>) {
    for w in weeks {
        let mut total = 0.0;
        let mut easy = 0.0;
        for s in w.days.iter().flat_map(|d| d.sessions.iter()) {
            if !s.sport.is_endurance() {
                continue;
            }
            let minutes = f64::from(s.duration_min);
            total += minutes;
            easy += ctx
                .catalog
                .get(&s.template_id)
                .and_then(|t| t.easy_zone_minutes(s.duration_min))
                .unwrap_or(0.0);
        }
        if total > 0.0 {
            let share = easy / total;
            if share < f64::from(rules.r08.min_easy_share) - EPS {
                out.push(Violation::new(
                    "R08",
                    rules.r08.meta.hardness,
                    w.week_start,
                    &[
                        ("easy_share", format!("{share:.3}")),
                        ("min", format!("{:.2}", rules.r08.min_easy_share)),
                    ],
                ));
            }
        }
    }
}

/// R9 (hart für die Länge, Q10 Prinzip, 7 bis 14 Tage Faustregel; weich für das Volumen):
/// Der Taper reicht vom Montag der ersten Taperwoche bis zum Termin. Liegt die Rennwoche am
/// Plananfang, begann der Taper vor dem Plan und wird nicht geprüft. Das Volumen der
/// Taperwochen und der Rennwoche muss unter der letzten Woche vor dem Taper liegen
/// ("Volumen sinkt"). Die Intensität ("bleibt") prüft die Regel nicht, sie folgt aus dem
/// Phasenprofil des Planers.
fn check_r09(weeks: &[PlanWeek], goal: &Goal, rules: &Rules, out: &mut Vec<Violation>) {
    let r = &rules.r09;
    let (GoalKind::Event, Some(event)) = (goal.kind, goal.event_date) else {
        return;
    };
    let Some(race) = weeks.iter().position(|w| {
        w.phase == Phase::Race
            && w.week_start <= event
            && event <= w.week_start + TimeDelta::days(6)
    }) else {
        return;
    };
    if race == 0 {
        return;
    }
    let mut first = race;
    while first > 0 && weeks[first - 1].phase == Phase::Taper {
        first -= 1;
    }
    let taper_start = weeks[first].week_start;
    let days = (event - taper_start).num_days();
    if days < i64::from(r.taper_days_min) || days > i64::from(r.taper_days_max) {
        out.push(Violation::new(
            "R09",
            r.meta.hardness,
            weeks[race].week_start,
            &[
                ("taper_days", days.to_string()),
                ("min", r.taper_days_min.to_string()),
                ("max", r.taper_days_max.to_string()),
            ],
        ));
    }
    if first > 0 && weeks[first - 1].phase != Phase::Recovery {
        let before = f64::from(weeks[first - 1].sessions_load());
        for w in &weeks[first..=race] {
            if before > 0.0 && f64::from(w.sessions_load()) >= before {
                out.push(Violation::new(
                    "R09",
                    Hardness::Soft,
                    w.week_start,
                    &[
                        ("kind", "volume".to_string()),
                        ("load", format!("{:.1}", w.sessions_load())),
                        ("before_taper", format!("{before:.1}")),
                    ],
                ));
            }
        }
    }
}

/// R10 (weich, Faustregel): vorausberechneter TSB unter -30. Eine Meldung je Woche, am Tag mit
/// dem tiefsten Wert.
fn check_r10(weeks: &[PlanWeek], rules: &Rules, point_at: &PmcSeries, out: &mut Vec<Violation>) {
    let limit = f64::from(rules.r10.tsb_warn_below);
    for w in weeks {
        let mut worst: Option<(NaiveDate, f64)> = None;
        for d in &w.days {
            if let Some(p) = point_at.at(d.date) {
                let tsb = f64::from(p.tsb);
                if tsb < limit && worst.is_none_or(|(_, t)| tsb < t) {
                    worst = Some((d.date, tsb));
                }
            }
        }
        if let Some((date, tsb)) = worst {
            out.push(Violation::new(
                "R10",
                rules.r10.meta.hardness,
                date,
                &[
                    ("tsb", format!("{tsb:.1}")),
                    ("limit", format!("{limit:.1}")),
                ],
            ));
        }
    }
}

/// R11 (hart, Nutzereingabe): Tageszeit je Wochentag nie überschreiten, feste Ruhetage und
/// Tage ohne harte Einheit einhalten, höchstens so viele Einheiten je Tag wie erlaubt.
fn check_r11(weeks: &[PlanWeek], ctx: &PlanContext, rules: &Rules, out: &mut Vec<Violation>) {
    let r = &rules.r11;
    for d in weeks.iter().flat_map(|w| w.days.iter()) {
        let a = ctx.availability[weekday_index(d.date)];
        let minutes: u32 = d.sessions.iter().map(|s| s.duration_min).sum();
        let mut reasons: Vec<&str> = Vec::new();
        if minutes > u32::from(a.max_minutes) {
            reasons.push("minutes");
        }
        if a.fixed_rest && !d.sessions.is_empty() {
            reasons.push("fixed_rest");
        }
        if !a.allow_hard && session_is_hard_day(d) {
            reasons.push("hard_not_allowed");
        }
        if d.sessions.len() > r.max_sessions_per_day as usize {
            reasons.push("sessions");
        }
        if !reasons.is_empty() {
            out.push(Violation::new(
                "R11",
                r.meta.hardness,
                d.date,
                &[
                    ("reason", reasons.join(",")),
                    ("minutes", minutes.to_string()),
                    ("available", a.max_minutes.to_string()),
                ],
            ));
        }
    }
}

/// R12 (weich, Q6 sekundär, Schwellen Faustregel): Liegen HRV oder Schlafdauer des jüngsten
/// Erholungstags deutlich unter der Basislinie, soll die erste harte Einheit des Plans locker
/// sein. Gemeldet wird die Verletzung am Tag dieser Einheit, wenn sie trotz Signal hart blieb.
/// Ohne Erholungsdaten gibt es keine Prüfung.
fn check_r12(weeks: &[PlanWeek], ctx: &PlanContext, rules: &Rules, out: &mut Vec<Violation>) {
    if ctx.recovery.is_empty() {
        return;
    }
    let Some(signal) = recovery_signal(&ctx.recovery, ctx.week_start, &rules.r12) else {
        return;
    };
    // Eine ersetzte Einheit erfüllt die Regel. Die erste Einheit, die noch hart ist, verletzt
    // sie nur, wenn vor ihr keine ersetzte liegt.
    let Some((wi, di, false)) = first_hard_or_swapped(weeks) else {
        return;
    };
    let mut detail: Vec<(&str, String)> = vec![("recovery_date", signal.date.to_string())];
    if let Some(x) = signal.hrv_drop {
        detail.push(("hrv_drop", format!("{x:.3}")));
    }
    if let Some(x) = signal.sleep_deficit {
        detail.push(("sleep_deficit", format!("{x:.3}")));
    }
    out.push(Violation::new(
        "R12",
        rules.r12.meta.hardness,
        weeks[wi].days[di].date,
        &detail,
    ));
}
