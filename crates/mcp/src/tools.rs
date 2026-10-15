//! Die neun Werkzeuge als reine Funktionen über einer lesenden Datenbankverbindung.
//!
//! Hier liegt die Filtergrenze (SPEC 9.6, 11.4): Jede Funktion prüft ihre Argumente, liest
//! über die lesenden Methoden von `lastwerk-store` und baut aus den Zeilen die Ausgabetypen
//! aus `views.rs` von Hand. Die Funktionen kennen weder das Protokoll noch die Uhr: "heute"
//! kommt als Parameter, damit Tests deterministisch sind.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use lastwerk_core::plan::{Plan, PlanWeek};
use lastwerk_core::{BEST_POWER_DURATIONS_S, PmcParams, Sport as CoreSport, pmc};
use lastwerk_store::{
    ActivityRecord, LoadRow, LoadStatus, Store, ThresholdKind, embedded_zone_catalog, monday_of,
};
use serde_json::Value;

use crate::args::{
    ActivityArgs, FitnessArgs, ListActivitiesArgs, MAX_ACTIVITIES, MAX_FITNESS_DAYS,
    MAX_RECOVERY_DAYS, MAX_WEEKS, NoArgs, PlanWeekArgs, RecoveryArgs, RuleArgs, ThresholdsArgs,
    WeeklyLoadArgs,
};
use crate::error::{ToolError, ToolResult};
use crate::rules::{self, Language, SETTING_LOCALE};
use crate::views::{
    ActivityList, ActivityView, BestPower, FitnessStatus, FitnessView, Method, RecoveryDayView,
    RecoveryDays, RuleExplanation, Sport, SportThresholds, ThresholdValue, ThresholdsView,
    WeeklyLoad, WeeklyLoadView, ZoneMinutes,
};

/// Einstellung, die `get_recovery_days` freigibt: `true` oder `false`, Standard aus (O19).
pub const SETTING_RECOVERY_ENABLED: &str = lastwerk_store::SETTING_MCP_RECOVERY_ENABLED;

/// Zugriff auf die Datenbank und das heutige Datum.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub store: &'a Store,
    /// Lokales Datum von heute (nur für Zeiträume "bis heute").
    pub today: NaiveDate,
}

fn invalid<T>(msg: impl Into<String>) -> ToolResult<T> {
    Err(ToolError::InvalidParams(msg.into()))
}

/// Prüft `from <= to` und eine Obergrenze der Länge in Tagen (beide Enden eingeschlossen).
fn check_range(from: NaiveDate, to: NaiveDate, max_days: Option<i64>) -> ToolResult<()> {
    if from > to {
        return invalid("`from` is after `to`");
    }
    if let Some(max) = max_days
        && (to - from).num_days() + 1 > max
    {
        return invalid(format!("the range is longer than {max} days"));
    }
    Ok(())
}

fn pmc_params(store: &Store) -> ToolResult<PmcParams> {
    Ok(store.pmc_params()?)
}

// --- get_fitness_status ---------------------------------------------------------------

/// CTL, ATL und TSB je Tag (SPEC 8.2).
///
/// Der Verlauf beginnt am ersten Tag mit zählender Last (Startwerte aus den Einstellungen),
/// auch wenn der vor `from` liegt, damit die Werte bei `from` eingeschwungen sind. Ausgegeben
/// werden nur Tage von `from` an, die nach dem ersten Tag mit Last und nicht nach heute
/// liegen: vorher und danach gibt es keine gemessene Grundlage, und "Form" ohne Daten ist
/// nicht null.
pub fn get_fitness_status(ctx: Ctx<'_>, a: &FitnessArgs) -> ToolResult<FitnessStatus> {
    check_range(a.from, a.to, Some(MAX_FITNESS_DAYS))?;
    let to = a.to.min(ctx.today);
    let Some(first) = ctx.store.first_load_date()? else {
        return Ok(FitnessStatus { days: Vec::new() });
    };
    let from = a.from.max(first);
    if from > to {
        return Ok(FitnessStatus { days: Vec::new() });
    }
    let daily = ctx.store.daily_loads(first, to)?;
    let points = pmc(&daily, &pmc_params(ctx.store)?, first, to);
    let days = points
        .into_iter()
        .filter(|p| p.date >= from)
        .map(|p| FitnessView {
            date: p.date,
            ctl: r1(f64::from(p.ctl)),
            atl: r1(f64::from(p.atl)),
            tsb: r1(f64::from(p.tsb)),
        })
        .collect();
    Ok(FitnessStatus { days })
}

// --- get_weekly_load ------------------------------------------------------------------

/// Last und Trainingszeit je Woche und Sportart für die letzten `weeks` Wochen bis
/// einschließlich der laufenden. Es zählen primäre Aktivitäten: die Zeit aller, die Last nur
/// aus berechneten Belastungen (Status `ok`, wie im Verlauf). Kraft aus FIT hat keine
/// berechnete Last und erscheint mit Last 0 und ihrer Zeit.
pub fn get_weekly_load(ctx: Ctx<'_>, a: &WeeklyLoadArgs) -> ToolResult<WeeklyLoad> {
    if a.weeks == 0 || a.weeks > MAX_WEEKS {
        return invalid(format!("`weeks` must be between 1 and {MAX_WEEKS}"));
    }
    // `week_history` liefert die Wochen vor der Woche von `before`: der Montag der nächsten
    // Woche schließt die laufende ein.
    let before = monday_of(ctx.today) + TimeDelta::days(7);
    let weeks = ctx
        .store
        .week_history(before, a.weeks)?
        .into_iter()
        .map(|w| (Sport::from(w.sport), w))
        .filter(|(sport, _)| a.sport.is_none_or(|s| s == *sport))
        .map(|(sport, w)| WeeklyLoadView {
            week_start: w.week_start,
            sport,
            load: r1(f64::from(w.load)),
            duration_min: w.minutes.round() as u32,
        })
        .collect();
    Ok(WeeklyLoad { weeks })
}

// --- Aktivitäten ----------------------------------------------------------------------

/// Last, Methode und Intensitätsfaktor aus den Belastungszeilen einer Aktivität. Es zählen nur
/// Zeilen mit Status `ok`; ohne eine solche sind alle drei `None`.
fn load_of(rows: &[LoadRow]) -> (Option<f64>, Option<Method>, Option<f64>) {
    let ok: Vec<&LoadRow> = rows
        .iter()
        .filter(|r| r.status == LoadStatus::Ok && r.value.is_some())
        .collect();
    let Some(first) = ok.first() else {
        return (None, None, None);
    };
    let sum: f64 = ok.iter().filter_map(|r| r.value).sum();
    (
        Some(r1(sum)),
        Method::from_db(&first.method),
        first.intensity_factor.map(round2),
    )
}

/// Intensitätsfaktor auf zwei Nachkommastellen (z. B. 0,85).
fn round2(x: f64) -> f64 {
    ((x * 100.0).round() / 100.0) + 0.0
}

/// Rundung auf eine Nachkommastelle. Die Werte der Antworten sind `f64`, damit `1 / 10`
/// als `66.7` im JSON steht: Ein `f32` wird über `serde_json::Value` zu `f64` geweitet und
/// erschiene als `66.69999694824219`. `+ 0.0` macht aus `-0.0` eine `0.0`.
fn r1(x: f64) -> f64 {
    ((x * 10.0).round() / 10.0) + 0.0
}

/// Zonenzeiten einer Aktivität in Minuten. Nur Schemas des Zonenkatalogs, andere Zeilen
/// fallen weg: Der Text `scheme` aus der Datenbank wird nie ungeprüft weitergereicht.
fn zone_minutes(store: &Store, activity_id: i64) -> ToolResult<Vec<ZoneMinutes>> {
    let catalog = embedded_zone_catalog();
    Ok(store
        .zone_times_of(activity_id)?
        .into_iter()
        .filter(|z| catalog.scheme(&z.scheme).is_some())
        .map(|z| ZoneMinutes {
            scheme: z.scheme,
            zone: z.zone,
            minutes: r1(f64::from(z.seconds) / 60.0),
            estimated: z.estimated,
        })
        .collect())
}

fn best_powers(store: &Store, activity_id: i64) -> ToolResult<Vec<BestPower>> {
    Ok(store
        .best_powers_of(activity_id)?
        .into_iter()
        .filter(|b| BEST_POWER_DURATIONS_S.contains(&b.duration_s))
        .map(|b| BestPower {
            duration_s: b.duration_s,
            watts: r1(b.watts),
        })
        .collect())
}

fn activity_view(
    store: &Store,
    rec: &ActivityRecord,
    loads: &[LoadRow],
    with_best_power: bool,
) -> ToolResult<ActivityView> {
    let (load, load_method, intensity_factor) = load_of(loads);
    Ok(ActivityView {
        id: rec.id,
        local_date: rec.row.local_date,
        sport: rec.row.sport.into(),
        duration_min: (rec.row.duration_s.max(0) as f64 / 60.0).round() as u32,
        load,
        load_method,
        intensity_factor,
        zone_minutes: zone_minutes(store, rec.id)?,
        best_power_w: if with_best_power {
            Some(best_powers(store, rec.id)?)
        } else {
            None
        },
    })
}

/// Aktivitäten eines Zeitraums, älteste zuerst, höchstens 200, ohne Bestleistungen. Es zählen
/// primäre Aktivitäten (Duplikate nicht, SPEC 6.4).
pub fn list_activities(ctx: Ctx<'_>, a: &ListActivitiesArgs) -> ToolResult<ActivityList> {
    check_range(a.from, a.to, None)?;
    let core_sport: Option<CoreSport> = a.sport.map(Into::into);
    let mut loads: HashMap<i64, Vec<LoadRow>> = HashMap::new();
    for l in ctx.store.loads_between(a.from, a.to)? {
        loads.entry(l.activity_id).or_default().push(l);
    }
    let mut matching = ctx
        .store
        .activities_between(a.from, a.to)?
        .into_iter()
        .filter(|r| r.is_primary && core_sport.is_none_or(|s| s == r.row.sport));
    let selected: Vec<ActivityRecord> = matching.by_ref().take(MAX_ACTIVITIES).collect();
    let truncated = matching.next().is_some();
    let no_loads = Vec::new();
    let activities = selected
        .iter()
        .map(|r| activity_view(ctx.store, r, loads.get(&r.id).unwrap_or(&no_loads), false))
        .collect::<ToolResult<Vec<_>>>()?;
    Ok(ActivityList {
        activities,
        truncated,
    })
}

/// Eine Aktivität mit Bestleistungen.
pub fn get_activity(ctx: Ctx<'_>, a: &ActivityArgs) -> ToolResult<ActivityView> {
    if a.id < 1 {
        return invalid("`id` must be at least 1");
    }
    let Some(rec) = ctx.store.activity(a.id)? else {
        return Err(ToolError::NotFound(format!("no activity with id {}", a.id)));
    };
    let loads = ctx.store.activity_loads_of(a.id)?;
    activity_view(ctx.store, &rec, &loads, true)
}

// --- Plan -----------------------------------------------------------------------------

const PLAN_SCHEMA: &str = include_str!("../../../schema/plan.schema.json");

/// Der aktive Plan als JSON nach `plan.schema.json`, bereinigt für den Client:
///
/// - `explanation` entfällt (Freitext, bei einem LLM-Plan vom Modell geschrieben).
/// - `created_at` verliert die Uhrzeit (O20): es bleibt der Tag, mit 00:00:00 UTC.
/// - Das Ergebnis wird zuletzt gegen das Schema geprüft. Alle Zeichenketten im Schema sind
///   durch Aufzählung, Muster oder Format begrenzt, bis auf `explanation`. Ein gespeicherter
///   Plan, der das Schema verletzt (zum Beispiel Text in `reason_keys`), wird nicht gezeigt.
fn sanitized_active_plan(store: &Store) -> ToolResult<Value> {
    let Some(mut plan) = store.active_plan()? else {
        return Err(ToolError::NotFound("there is no active plan".into()));
    };
    plan.explanation = None;
    plan.created_at = midnight(plan.created_at);
    // Über den Text statt `to_value`: Die `f32` des Plans erscheinen so als `66.7` und nicht
    // als `66.69999694824219` (siehe `r1`).
    let value: Value = serde_json::to_string(&plan)
        .and_then(|t| serde_json::from_str(&t))
        .map_err(|_| ToolError::Internal)?;
    let schema: Value = serde_json::from_str(PLAN_SCHEMA).map_err(|_| ToolError::Internal)?;
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|_| ToolError::Internal)?;
    if !validator.is_valid(&value) {
        eprintln!("lastwerk-mcp: the stored active plan violates plan.schema.json, not shown");
        return Err(ToolError::Internal);
    }
    Ok(value)
}

fn midnight(t: DateTime<Utc>) -> DateTime<Utc> {
    t.date_naive().and_time(chrono::NaiveTime::MIN).and_utc()
}

/// Der aktive Plan (alle Wochen), ohne `explanation`.
pub fn get_current_plan(ctx: Ctx<'_>, _a: &NoArgs) -> ToolResult<Value> {
    sanitized_active_plan(ctx.store)
}

/// Eine Woche des aktiven Plans. Jeder Tag der Woche wählt sie.
pub fn get_plan_week(ctx: Ctx<'_>, a: &PlanWeekArgs) -> ToolResult<Value> {
    let monday = monday_of(a.week_start);
    let plan_value = sanitized_active_plan(ctx.store)?;
    let plan: Plan = serde_json::from_value(plan_value).map_err(|_| ToolError::Internal)?;
    let week: Option<&PlanWeek> = plan.weeks.iter().find(|w| w.week_start == monday);
    match week {
        Some(w) => serde_json::to_string(w)
            .and_then(|t| serde_json::from_str(&t))
            .map_err(|_| ToolError::Internal),
        None => Err(ToolError::NotFound(format!(
            "the active plan has no week starting {monday}"
        ))),
    }
}

// --- Schwellenwerte -------------------------------------------------------------------

fn unit_of(kind: ThresholdKind) -> &'static str {
    match kind {
        ThresholdKind::FtpW => "W",
        ThresholdKind::LthrBpm | ThresholdKind::HrMaxBpm | ThresholdKind::HrRestBpm => "bpm",
        ThresholdKind::RunSpeedMps | ThresholdKind::CssMps => "m/s",
    }
}

/// Feste Reihenfolge der Arten in der Ausgabe.
const KIND_ORDER: [ThresholdKind; 6] = [
    ThresholdKind::FtpW,
    ThresholdKind::LthrBpm,
    ThresholdKind::HrMaxBpm,
    ThresholdKind::HrRestBpm,
    ThresholdKind::RunSpeedMps,
    ThresholdKind::CssMps,
];

/// Schwellenwerte, die an `on_date` gelten, je Ausdauersportart, jeweils mit dem Tag, ab dem
/// der Wert gilt. Ohne Notiz und ohne Herkunft (die Notiz ist Freitext des Nutzers).
pub fn get_thresholds(ctx: Ctx<'_>, a: &ThresholdsArgs) -> ToolResult<ThresholdsView> {
    let records: HashMap<i64, _> = ctx
        .store
        .list_thresholds()?
        .into_iter()
        .map(|r| (r.id, r))
        .collect();
    let mut sports = Vec::new();
    for sport in [CoreSport::Cycling, CoreSport::Running, CoreSport::Swimming] {
        let Some(lookup) = ctx.store.threshold_lookup(sport, a.on_date)? else {
            continue;
        };
        let values: Vec<ThresholdValue> = KIND_ORDER
            .iter()
            .filter_map(|kind| {
                let rec = records.get(lookup.ids.get(kind)?)?;
                Some(ThresholdValue {
                    kind: kind.as_str().to_string(),
                    value: rec.value,
                    unit: unit_of(*kind).to_string(),
                    valid_from: rec.valid_from,
                })
            })
            .collect();
        if !values.is_empty() {
            sports.push(SportThresholds {
                sport: sport.into(),
                values,
            });
        }
    }
    Ok(ThresholdsView {
        on_date: a.on_date,
        sports,
    })
}

// --- Regeln ---------------------------------------------------------------------------

/// Parameter, Quelle und Text einer Regel.
pub fn explain_rule(ctx: Ctx<'_>, a: &RuleArgs) -> ToolResult<RuleExplanation> {
    let valid = a.code.len() == 3
        && a.code.starts_with('R')
        && a.code.bytes().skip(1).all(|b| b.is_ascii_digit());
    if !valid {
        return invalid("`code` must look like R03");
    }
    let language = Language::from_setting(ctx.store.get_setting(SETTING_LOCALE)?.as_deref());
    rules::explain(&a.code, language)
}

// --- Erholung -------------------------------------------------------------------------

/// Ob `get_recovery_days` freigegeben ist. Nur der genaue Wert `true` gibt frei.
pub fn recovery_enabled(store: &Store) -> ToolResult<bool> {
    Ok(store.get_setting(SETTING_RECOVERY_ENABLED)?.as_deref() == Some("true"))
}

/// HRV, Ruhepuls und Schlafdauer je Tag. Gesperrt, solange die Einstellung
/// `mcp.recovery_enabled` nicht `true` ist (Gesundheitsdaten, O19). `open_read_only` verlangt
/// die neueste Schemaversion, die Tabelle `recovery_day` existiert also immer.
pub fn get_recovery_days(ctx: Ctx<'_>, a: &RecoveryArgs) -> ToolResult<RecoveryDays> {
    if !recovery_enabled(ctx.store)? {
        return Err(ToolError::Disabled(
            "recovery data is blocked; the user can enable it with the setting \
             mcp.recovery_enabled"
                .into(),
        ));
    }
    check_range(a.from, a.to, Some(MAX_RECOVERY_DAYS))?;
    let days = ctx
        .store
        .recovery_days(a.from, a.to)?
        .into_iter()
        .map(|r| RecoveryDayView {
            date: r.date,
            hrv_overnight_ms: r.hrv_overnight_ms.map(r1),
            resting_hr_bpm: r.resting_hr_bpm.map(r1),
            sleep_duration_min: r
                .sleep_duration_s
                .map(|s| (f64::from(s) / 60.0).round() as u32),
        })
        .collect();
    Ok(RecoveryDays { days })
}

/// Namen aller Werkzeuge, in der Reihenfolge von SPEC 5.4.
pub const TOOL_NAMES: [&str; 9] = [
    "get_fitness_status",
    "get_weekly_load",
    "list_activities",
    "get_activity",
    "get_current_plan",
    "get_plan_week",
    "get_thresholds",
    "explain_rule",
    "get_recovery_days",
];

/// Hilfsfunktion für Tests und das Protokoll: Zahl der Zeilen eines JSON-Werts
/// (Wochen eines Plans, Tage einer Woche).
pub fn plan_rows(v: &Value) -> usize {
    match v.get("weeks").and_then(Value::as_array) {
        Some(weeks) => weeks.len(),
        None => v.get("days").and_then(Value::as_array).map_or(1, Vec::len),
    }
}
