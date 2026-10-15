//! Pläne, Ziele, Verfügbarkeit und Export der Oberfläche (T09, F8 bis F11).
//!
//! Die Logik stammt aus `lastwerk-cli` (Kontextbildung aus der Datenbank, Erzeugung, Prüfung
//! gegen Schema und Leitplanken, ICS), damit Oberfläche und Kommandozeile denselben Plan
//! erzeugen und prüfen. Hier steht nur die Übersetzung in die Typen der Oberfläche. Dateien
//! schreibt und liest der Befehlsteil in `lib.rs` nach einem Dateidialog, diese Funktionen
//! liefern und nehmen Text, damit sie ohne Fenster testbar sind.

use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc, Weekday};
use lastwerk_cli::error::CliError;
use lastwerk_cli::planning::{self, DEFAULT_DAY, PlanRequest};
use lastwerk_cli::texts::Texts;
use lastwerk_cli::{ics, schema};
use lastwerk_core::plan::{GoalKind, Hardness, Plan, PlanSport, validate_plan};
use lastwerk_core::{DailyLoad, pmc};
use lastwerk_store::{
    AvailabilityRecord, GoalRecord, MAX_AVAILABLE_MINUTES, NewGoal, PlanStatus, PlanSummary, Store,
    UserDataExport,
};

use crate::dto::{
    AvailabilityDay, BackupCounts, GeneratePlanInput, GoalInput, GoalItem, PlanDetail,
    PlanListItem, PlanValidation, PlanningInputs, PmcPointDto, ViolationDto,
};
use crate::error::{AppError, Result};
use crate::service::parse_date;

/// Höchstzahl Pläne in der Liste (Konvention, die neuesten zuerst).
pub const MAX_PLAN_LIST: usize = 100;
/// Längste Bezeichnung eines Ziels in Zeichen (Konvention).
pub const MAX_LABEL_CHARS: usize = 200;
/// Größte Sicherungsdatei, die die Wiederherstellung liest (Konvention, Schutz vor einer
/// falsch gewählten Datei).
pub const MAX_BACKUP_BYTES: u64 = 64 * 1024 * 1024;
/// Längster Planungshorizont in Wochen (SPEC 7.1).
pub const MAX_WEEKS: u32 = 26;
/// Höchste Zahl Krafteinheiten je Woche (R7).
pub const MAX_STRENGTH_PER_WEEK: u8 = 2;

/// Übersetzt Fehler der Planung. Der Text der Fehler (Deutsch, kann Werte nennen) geht nicht
/// an die Oberfläche, nur der Code.
fn cli_error(e: &CliError) -> AppError {
    match e {
        CliError::Validation(_) => AppError::new("plan_invalid"),
        CliError::Input(_) => AppError::new("plan_failed"),
        CliError::Internal(_) => AppError::new("internal"),
    }
}

fn endurance_only(sport: PlanSport) -> Result<PlanSport> {
    if sport.is_endurance() {
        Ok(sport)
    } else {
        Err(AppError::new("invalid_sport"))
    }
}

// --- Liste und Status ----------------------------------------------------------------

fn list_item(s: &PlanSummary, plan: &Plan) -> PlanListItem {
    PlanListItem {
        id: s.id.clone(),
        created_at: s
            .created_at
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        status: s.status.as_str().to_owned(),
        source: match s.source {
            lastwerk_core::plan::PlanSource::Rules => "rules",
            lastwerk_core::plan::PlanSource::Llm => "llm",
        }
        .to_owned(),
        locale: s.locale,
        rules_version: s.rules_version.clone(),
        goal_id: s.goal_id,
        goal: plan.goal.clone(),
        first_week: plan.weeks.first().map(|w| w.week_start.to_string()),
        last_week: plan.weeks.last().map(|w| w.week_start.to_string()),
        weeks: u32::try_from(plan.weeks.len()).unwrap_or(u32::MAX),
    }
}

/// Gespeicherte Pläne, die neuesten zuerst. Pläne, deren Text sich nicht lesen lässt, fehlen
/// in der Liste, statt sie zu sperren.
pub fn list_plans(store: &Store) -> Result<Vec<PlanListItem>> {
    let mut items = Vec::new();
    for s in store.list_plans()?.into_iter().take(MAX_PLAN_LIST) {
        if let Ok(Some(plan)) = store.load_plan(&s.id) {
            items.push(list_item(&s, &plan));
        }
    }
    Ok(items)
}

/// Setzt den Status eines Plans. `active` archiviert den bisherigen aktiven Plan (höchstens
/// einer, Konvention des Speichers).
pub fn set_plan_status(store: &mut Store, id: &str, status: &str) -> Result<Vec<PlanListItem>> {
    let status = PlanStatus::parse(status).map_err(|_| AppError::new("invalid_value"))?;
    store.set_plan_status(id, status)?;
    list_plans(store)
}

// --- Anzeige und Prüfung -------------------------------------------------------------

/// Last der Einheiten je Tag, als Eingabe des Verlaufs.
fn daily_loads(plan: &Plan) -> Vec<DailyLoad> {
    plan.weeks
        .iter()
        .flat_map(|w| w.days.iter())
        .flat_map(|d| {
            d.sessions.iter().map(|s| DailyLoad {
                date: d.date,
                load: s.planned_load,
            })
        })
        .collect()
}

/// Plan mit Vorausberechnung und Prüfung. Der Verlauf rechnet wie `validate`: aus den
/// geplanten Tageslasten, ab CTL und ATL am Vortag des Planbeginns, mit den Parametern des
/// Verlaufs aus den Einstellungen (wie der Planer).
fn detail(store: &Store, plan: Plan, status: PlanStatus) -> Result<PlanDetail> {
    let (rules, catalog) = planning::load_rules().map_err(|e| cli_error(&e))?;
    let mut notes = Vec::new();
    let ctx = planning::validation_context(store, &plan, &rules, catalog, &mut notes)
        .map_err(|e| cli_error(&e))?;

    let value = serde_json::to_value(&plan).map_err(|_| AppError::new("internal"))?;
    let schema_errors = schema::plan_errors(&value).map_err(|e| cli_error(&e))?;

    let mut violations = validate_plan(&plan, &ctx, &rules);
    violations.sort_by(|a, b| (a.date, &a.code).cmp(&(b.date, &b.code)));
    let violations = violations
        .into_iter()
        .map(|v| ViolationDto {
            hard: v.hardness == Hardness::Hard,
            code: v.code,
            date: v.date.map(|d| d.to_string()),
            detail: v.detail,
        })
        .collect();

    let projection = match (plan.weeks.first(), plan.weeks.last()) {
        (Some(first), Some(last)) => {
            let daily = daily_loads(&plan);
            let end = last.week_start + TimeDelta::days(6);
            pmc(&daily, &ctx.pmc_start(), first.week_start, end)
                .into_iter()
                .map(|p| PmcPointDto {
                    date: p.date.to_string(),
                    load: daily
                        .iter()
                        .filter(|d| d.date == p.date)
                        .map(|d| d.load)
                        .sum(),
                    ctl: p.ctl,
                    atl: p.atl,
                    tsb: p.tsb,
                })
                .collect()
        }
        _ => Vec::new(),
    };

    Ok(PlanDetail {
        status: status.as_str().to_owned(),
        projection,
        ctl0: ctx.ctl0,
        atl0: ctx.atl0,
        validation: PlanValidation {
            schema_errors,
            violations,
        },
        plan,
    })
}

/// Ein gespeicherter Plan mit Status, Verlauf und Prüfung.
pub fn get_plan(store: &Store, id: &str) -> Result<PlanDetail> {
    let status = store
        .list_plans()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::new("not_found"))?
        .status;
    let plan = store
        .load_plan(id)?
        .ok_or_else(|| AppError::new("not_found"))?;
    detail(store, plan, status)
}

// --- Erzeugen ------------------------------------------------------------------------

/// Erzeugt einen Plan wie `lastwerk plan`, prüft ihn gegen das Schema und speichert ihn als
/// Entwurf. `plan_id` (UUID v4) und `now` kommen vom Aufrufer.
pub fn generate(
    store: &mut Store,
    input: &GeneratePlanInput,
    plan_id: String,
    now: DateTime<Utc>,
) -> Result<PlanDetail> {
    let week_start = parse_date(&input.week_start)?;
    if week_start.weekday() != Weekday::Mon {
        return Err(AppError::new("not_monday"));
    }
    let goal_date = input.goal_date.as_deref().map(parse_date).transpose()?;
    if let Some(w) = input.weeks
        && !(1..=MAX_WEEKS).contains(&w)
    {
        return Err(AppError::new("invalid_value"));
    }
    if input.strength_per_week > MAX_STRENGTH_PER_WEEK {
        return Err(AppError::new("invalid_value"));
    }
    let goal_sport = input.goal_sport.map(endurance_only).transpose()?;
    let mut sports = Vec::new();
    for s in &input.sports {
        let s = endurance_only(*s)?;
        if !sports.contains(&s) {
            sports.push(s);
        }
    }

    let req = PlanRequest {
        week_start,
        goal_date,
        goal_sport,
        weeks: input.weeks,
        locale: input.locale,
        sports,
        strength_per_week: input.strength_per_week,
    };
    let generated = planning::generate(store, &req, plan_id, now).map_err(|e| cli_error(&e))?;
    // Wie `lastwerk plan`: Der Plan muss das Schema bestehen, bevor er gespeichert wird.
    planning::plan_json(&generated.plan).map_err(|e| cli_error(&e))?;
    store.save_plan_for_goal(&generated.plan, PlanStatus::Draft, generated.goal_id)?;
    detail(store, generated.plan, PlanStatus::Draft)
}

// --- Ziele und Verfügbarkeit ---------------------------------------------------------

fn goal_item(g: &GoalRecord) -> GoalItem {
    GoalItem {
        id: g.id,
        kind: match g.goal.kind {
            GoalKind::Event => "event",
            GoalKind::Continuous => "continuous",
        }
        .to_owned(),
        event_date: g.goal.event_date.map(|d| d.to_string()),
        sport: g.goal.sport,
        priority: g.goal.priority,
        label: g.goal.label.clone(),
        active: g.active,
    }
}

/// Sieben Tage, Montag zuerst. Nicht gesetzte Tage tragen den Standard des Planers.
fn week_availability(rows: &[AvailabilityRecord]) -> Vec<AvailabilityDay> {
    (0u8..7)
        .map(|weekday| {
            rows.iter().find(|r| r.weekday == weekday).map_or(
                AvailabilityDay {
                    weekday,
                    max_minutes: DEFAULT_DAY.max_minutes,
                    allow_hard: DEFAULT_DAY.allow_hard,
                    fixed_rest: DEFAULT_DAY.fixed_rest,
                },
                |r| AvailabilityDay {
                    weekday,
                    max_minutes: r.max_minutes,
                    allow_hard: r.allow_hard,
                    fixed_rest: r.fixed_rest,
                },
            )
        })
        .collect()
}

/// Ziele (neueste zuerst) und Verfügbarkeit.
pub fn planning_inputs(store: &Store) -> Result<PlanningInputs> {
    let mut goals: Vec<GoalItem> = store.list_goals()?.iter().map(goal_item).collect();
    goals.reverse();
    let rows = store.list_availability()?;
    Ok(PlanningInputs {
        goals,
        availability: week_availability(&rows),
        availability_set: u8::try_from(rows.len()).unwrap_or(7).min(7),
        default_minutes: DEFAULT_DAY.max_minutes,
    })
}

/// Setzt ein neues aktives Ziel: mit Datum ein Termin, ohne ein fortlaufendes Ziel.
pub fn set_goal(store: &mut Store, input: &GoalInput) -> Result<PlanningInputs> {
    let event_date: Option<NaiveDate> = input.event_date.as_deref().map(parse_date).transpose()?;
    let sport = input.sport.map(endurance_only).transpose()?;
    let label = input
        .label
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty());
    if label.is_some_and(|l| l.chars().count() > MAX_LABEL_CHARS) {
        return Err(AppError::new("invalid_value"));
    }
    let kind = if event_date.is_some() {
        GoalKind::Event
    } else {
        GoalKind::Continuous
    };
    store.set_goal(&NewGoal {
        kind,
        sport: if kind == GoalKind::Event { sport } else { None },
        event_date,
        label: label.map(str::to_owned),
        // Eine Priorität gibt es nur bei einem Termin (SPEC 6.2).
        priority: if kind == GoalKind::Event {
            input.priority
        } else {
            None
        },
    })?;
    planning_inputs(store)
}

/// Setzt die Verfügbarkeit der übergebenen Wochentage. Erst wird alles geprüft, dann
/// geschrieben: Ein ungültiger Tag ändert nichts.
pub fn set_availability(store: &mut Store, days: &[AvailabilityDay]) -> Result<PlanningInputs> {
    let mut seen = [false; 7];
    for d in days {
        let slot = seen
            .get_mut(usize::from(d.weekday))
            .ok_or_else(|| AppError::new("invalid_value"))?;
        if std::mem::replace(slot, true) || d.max_minutes > MAX_AVAILABLE_MINUTES {
            return Err(AppError::new("invalid_value"));
        }
    }
    for d in days {
        store.set_availability(d.weekday, d.max_minutes, d.allow_hard, d.fixed_rest)?;
    }
    planning_inputs(store)
}

// --- Export und Sicherung ------------------------------------------------------------

/// Plan aus der Datenbank, vor dem Export gegen das Schema geprüft.
fn checked_plan(store: &Store, id: &str) -> Result<Plan> {
    let plan = store
        .load_plan(id)?
        .ok_or_else(|| AppError::new("not_found"))?;
    let value = serde_json::to_value(&plan).map_err(|_| AppError::new("internal"))?;
    if !schema::plan_errors(&value)
        .map_err(|e| cli_error(&e))?
        .is_empty()
    {
        return Err(AppError::new("plan_invalid"));
    }
    Ok(plan)
}

/// Dateiname-Vorschlag mit dem Beginn der ersten Woche.
fn plan_file_stem(plan: &Plan) -> String {
    plan.weeks.first().map_or_else(
        || "lastwerk-plan".to_owned(),
        |w| format!("lastwerk-plan-{}", w.week_start),
    )
}

/// ICS-Text, Zahl der Veranstaltungen und Dateiname-Vorschlag (SPEC 10.2).
pub fn export_ics(store: &Store, id: &str) -> Result<(String, u32, String)> {
    let plan = checked_plan(store, id)?;
    let texts = Texts::load(plan.locale).map_err(|e| cli_error(&e))?;
    let events: usize = plan
        .weeks
        .iter()
        .flat_map(|w| w.days.iter())
        .map(|d| d.sessions.len())
        .sum();
    Ok((
        ics::plan_to_ics(&plan, &texts),
        u32::try_from(events).unwrap_or(u32::MAX),
        format!("{}.ics", plan_file_stem(&plan)),
    ))
}

/// Plan als JSON-Text (nach dem Schema geprüft, wie `lastwerk plan --out`) und
/// Dateiname-Vorschlag.
pub fn export_plan_json(store: &Store, id: &str) -> Result<(String, String)> {
    let plan = checked_plan(store, id)?;
    let text = planning::plan_json(&plan).map_err(|e| cli_error(&e))?;
    Ok((text, format!("{}.json", plan_file_stem(&plan))))
}

fn counts(e: &UserDataExport) -> BackupCounts {
    let n = |len: usize| u32::try_from(len).unwrap_or(u32::MAX);
    BackupCounts {
        thresholds: n(e.thresholds.len()),
        goals: n(e.goals.len()),
        availability: n(e.availability.len()),
        plans: n(e.plans.len()),
        manual_sessions: n(e.manual_sessions.len()),
        settings: n(e.settings.len()),
        recovery_days: n(e.recovery_days.len()),
    }
}

/// Sicherung der Nutzerdaten als JSON-Text, mit Zeilenzahlen und Dateiname-Vorschlag. Die
/// Datei kann persönliche Angaben enthalten (Zielbezeichnung, Pläne, Ordner der Einstellungen).
pub fn export_backup(store: &Store, now: DateTime<Utc>) -> Result<(String, BackupCounts, String)> {
    let export = store.export_user_data()?.with_exported_at(now);
    let mut text = export.to_json()?;
    text.push('\n');
    Ok((
        text,
        counts(&export),
        format!("lastwerk-backup-{}.json", now.date_naive()),
    ))
}

/// Spielt eine Sicherung ein und ersetzt dabei die Nutzertabellen vollständig (SPEC 6.3). Bei
/// einem Fehler bleibt der Bestand unverändert. Der Cache bleibt unberührt.
pub fn restore_backup(store: &mut Store, text: &str) -> Result<BackupCounts> {
    let export = UserDataExport::from_json(text).map_err(|_| AppError::new("restore_invalid"))?;
    store.import_user_data(&export)?;
    Ok(counts(&export))
}
