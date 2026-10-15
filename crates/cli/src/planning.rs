//! Befehle `plan` und `validate`: Kontext des Planers aus der Datenbank, Erzeugung, Prüfung
//! gegen Schema und Leitplanken (SPEC 5.4, 7.1, 8.3, 8.4).
//!
//! `core` liest keine Dateien und keine Uhr. Katalog (`templates.json`) und Regeln
//! (`rules.default.json`) sind hier per `include_str!` eingebettet (Konvention wie beim
//! Zonenkatalog in T05), `plan_id` und `created_at` kommen vom Aufrufer.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc};
use lastwerk_core::plan::{
    DayAvailability, GoalKind, GoalSpec, Hardness, Locale, Plan, PlanContext, PlanError, PlanSport,
    Priority, RecoveryDay, Rules, SportWeight, TemplateCatalog, Violation, WeekHistory,
    generate_plan, validate_plan,
};
use lastwerk_core::{PmcParams, Sport};
use lastwerk_store::{PlanStatus, Store, monday_of};

use crate::error::CliError;
use crate::schema;
use crate::texts::Texts;

const TEMPLATES_JSON: &str = include_str!("../../../catalog/templates.json");
const RULES_JSON: &str = include_str!("../../../catalog/rules.default.json");

/// Standard, wenn für einen Wochentag keine Verfügbarkeit gesetzt ist (Konvention): 90
/// Minuten, harte Einheiten erlaubt, kein fester Ruhetag.
pub const DEFAULT_DAY: DayAvailability = DayAvailability {
    max_minutes: 90,
    allow_hard: true,
    fixed_rest: false,
};

/// Wochen der Historie, die der Planer bekommt (Konvention). Die Woche vor dem Planbeginn
/// dient als Bezug für den Anstieg (R2), die übrigen als Grundlage für die Wahl der
/// Sportarten.
pub const HISTORY_WEEKS: u32 = 12;

/// Planungshorizont ohne Termin und ohne `--weeks` (Konvention): zwei Blöcke aus drei
/// Belastungswochen und einer Erholungswoche.
pub const DEFAULT_CONTINUOUS_WEEKS: u32 = 8;

/// Gewünschte Krafteinheiten je Woche, wenn nichts angegeben ist (Konvention, R7 begrenzt auf
/// 1 bis 2).
pub const DEFAULT_STRENGTH_PER_WEEK: u8 = 1;

/// Erholungstage für R12: die Tage, die das Mittel der Basislinie und das Höchstalter des
/// jüngsten Tages brauchen (Parameter in `rules.default.json`), bis zum Tag vor dem Planbeginn.
/// Ohne importierte Erholungstage ist die Liste leer und R12 ändert nichts.
fn recovery_input(
    store: &Store,
    week_start: NaiveDate,
    rules: &Rules,
) -> Result<Vec<RecoveryDay>, CliError> {
    let lookback = i64::from(rules.r12.baseline_days) + i64::from(rules.r12.max_age_days);
    let from = week_start - TimeDelta::days(lookback);
    let to = week_start - TimeDelta::days(1);
    Ok(store
        .recovery_days(from, to)?
        .iter()
        .map(RecoveryDay::from)
        .collect())
}

/// Eingaben von `lastwerk plan`.
#[derive(Debug, Clone)]
pub struct PlanRequest {
    pub week_start: NaiveDate,
    pub goal_date: Option<NaiveDate>,
    pub goal_sport: Option<PlanSport>,
    pub weeks: Option<u32>,
    pub locale: Locale,
    /// Leer heißt: Sportarten aus der Historie.
    pub sports: Vec<PlanSport>,
    pub strength_per_week: u8,
}

/// Ergebnis der Erzeugung.
#[derive(Debug)]
pub struct Generated {
    pub plan: Plan,
    /// `goal.id` des Ziels aus der Datenbank, falls der Plan darauf beruht.
    pub goal_id: Option<i64>,
    /// Hinweise für die Fehlerausgabe.
    pub notes: Vec<String>,
}

/// Regeln und Katalog aus den eingebetteten Dateien.
pub fn load_rules() -> Result<(Rules, TemplateCatalog), CliError> {
    let internal = |what: &str, e: &dyn std::fmt::Display| {
        CliError::Internal(format!("{what} lässt sich nicht lesen: {e}"))
    };
    let rules: Rules =
        serde_json::from_str(RULES_JSON).map_err(|e| internal("rules.default.json", &e))?;
    let catalog: TemplateCatalog =
        serde_json::from_str(TEMPLATES_JSON).map_err(|e| internal("templates.json", &e))?;
    Ok((rules, catalog))
}

/// Fehler des Planers: Eingabefehler des Nutzers sind Exit 2, kaputte Kataloge Exit 3.
fn plan_error(e: &PlanError) -> CliError {
    match e {
        PlanError::Catalog { .. }
        | PlanError::MissingTemplate { .. }
        | PlanError::InvalidRules(_) => CliError::Internal(format!("Planer ({}): {e}", e.code())),
        _ => CliError::Input(format!("Planer ({}): {e}", e.code())),
    }
}

/// Verfügbarkeit Montag bis Sonntag. Fehlende Tage bekommen [`DEFAULT_DAY`].
fn availability(store: &Store, notes: &mut Vec<String>) -> Result<[DayAvailability; 7], CliError> {
    let mut days = [DEFAULT_DAY; 7];
    let mut set = 0;
    for a in store.list_availability()? {
        days[usize::from(a.weekday)] = DayAvailability {
            max_minutes: a.max_minutes,
            allow_hard: a.allow_hard,
            fixed_rest: a.fixed_rest,
        };
        set += 1;
    }
    if set == 0 {
        notes.push(format!(
            "Hinweis: Keine Verfügbarkeit gesetzt. Es gelten alle sieben Tage mit je {} Minuten \
             und erlaubten harten Einheiten. Mit `lastwerk availability set` anpassen.",
            DEFAULT_DAY.max_minutes
        ));
    } else if set < 7 {
        notes.push(format!(
            "Hinweis: Für {} Wochentage ist keine Verfügbarkeit gesetzt. Dort gelten {} Minuten \
             und erlaubte harte Einheiten.",
            7 - set,
            DEFAULT_DAY.max_minutes
        ));
    }
    Ok(days)
}

/// CTL und ATL am Ende des Tages vor `week_start`, gerechnet mit den Parametern des Verlaufs
/// aus den Einstellungen `pmc.*` (SPEC 8.2; `Store::pmc_params`, dieselbe Auslese wie in der
/// Oberfläche und im MCP-Server).
fn start_state(
    store: &Store,
    params: &PmcParams,
    week_start: NaiveDate,
) -> Result<(f32, f32), CliError> {
    let day_before = week_start - TimeDelta::days(1);
    let p = store.pmc_state_at(day_before, params)?;
    Ok((p.ctl, p.atl))
}

fn endurance_rank(s: PlanSport) -> u8 {
    match s {
        PlanSport::Cycling => 0,
        PlanSport::Running => 1,
        PlanSport::Swimming => 2,
        PlanSport::Strength => 3,
        PlanSport::Rest => 4,
    }
}

/// Sportarten mit Gewicht. Angegebene Sportarten zählen gleich. Sonst ist das Gewicht die
/// Trainingszeit der Ausdauersportart in der Historie. Ohne Historie bleibt nur das Rad
/// (Konvention) mit einem Hinweis.
fn sport_weights(
    requested: &[PlanSport],
    history: &[WeekHistory],
    notes: &mut Vec<String>,
) -> Vec<SportWeight> {
    let mut weights: BTreeMap<u8, SportWeight> = BTreeMap::new();
    if requested.is_empty() {
        for h in history {
            let sport = match h.sport {
                Sport::Cycling => PlanSport::Cycling,
                Sport::Running => PlanSport::Running,
                Sport::Swimming => PlanSport::Swimming,
                Sport::Strength | Sport::Other => continue,
            };
            weights
                .entry(endurance_rank(sport))
                .or_insert(SportWeight { sport, weight: 0.0 })
                .weight += h.minutes.max(0.0);
        }
        weights.retain(|_, w| w.weight > 0.0);
        if weights.is_empty() {
            notes.push(
                "Hinweis: In den letzten zwölf Wochen gibt es keine Ausdauereinheiten. Geplant \
                 wird Rad. Mit --sports andere Sportarten wählen."
                    .to_string(),
            );
            weights.insert(
                0,
                SportWeight {
                    sport: PlanSport::Cycling,
                    weight: 1.0,
                },
            );
        }
    } else {
        for &sport in requested {
            weights.insert(endurance_rank(sport), SportWeight { sport, weight: 1.0 });
        }
    }
    weights.into_values().collect()
}

/// Das Ziel des Plans: Angaben der Kommandozeile gehen vor, sonst das aktive Ziel der
/// Datenbank, sonst fortlaufend.
fn resolve_goal(
    store: &Store,
    req: &PlanRequest,
    sports: &mut Vec<SportWeight>,
    notes: &mut Vec<String>,
) -> Result<(GoalSpec, Option<i64>), CliError> {
    let main_sport = |sports: &[SportWeight]| {
        sports
            .iter()
            .fold(
                sports[0],
                |best, s| if s.weight > best.weight { *s } else { best },
            )
            .sport
    };
    let (date, sport, priority, goal_id) = if let Some(date) = req.goal_date {
        (Some(date), req.goal_sport, Priority::A, None)
    } else {
        match store.active_goal()? {
            Some(g) if g.goal.kind == GoalKind::Event => {
                let date = g
                    .goal
                    .event_date
                    .expect("Termin ist beim Speichern geprüft");
                if date < req.week_start {
                    notes.push(format!(
                        "Hinweis: Der Termin des aktiven Ziels ({date}) liegt vor dem Planbeginn. \
                         Geplant wird fortlaufend. Mit `lastwerk goal set` ein neues Ziel setzen."
                    ));
                    (None, None, Priority::A, None)
                } else {
                    (
                        Some(date),
                        req.goal_sport.or(g.goal.sport),
                        g.goal.priority.unwrap_or(Priority::A),
                        Some(g.id),
                    )
                }
            }
            Some(g) => (None, None, Priority::A, Some(g.id)),
            None => (None, None, Priority::A, None),
        }
    };
    let Some(event_date) = date else {
        return Ok((GoalSpec::Continuous, goal_id));
    };
    let sport = sport.unwrap_or_else(|| main_sport(sports));
    if !sport.is_endurance() {
        return Err(CliError::Input(
            "Die Sportart des Ziels muss Rad, Lauf oder Schwimmen sein.".into(),
        ));
    }
    if !sports.iter().any(|s| s.sport == sport) {
        let weight = sports.iter().map(|s| s.weight).fold(1.0, f32::max);
        notes.push(
            "Hinweis: Die Sportart des Ziels kommt nicht in den Sportarten des Plans vor und \
             wird mit dem höchsten Gewicht ergänzt."
                .to_string(),
        );
        sports.push(SportWeight { sport, weight });
        sports.sort_by_key(|s| endurance_rank(s.sport));
    }
    Ok((
        GoalSpec::Event {
            event_date,
            sport,
            priority,
        },
        goal_id,
    ))
}

fn default_horizon(goal: &GoalSpec, week_start: NaiveDate) -> u32 {
    match goal {
        GoalSpec::Event { event_date, .. } => {
            let weeks = (monday_of(*event_date) - week_start)
                .num_days()
                .div_euclid(7)
                + 1;
            u32::try_from(weeks.clamp(1, 26)).unwrap_or(26)
        }
        GoalSpec::Continuous => DEFAULT_CONTINUOUS_WEEKS,
    }
}

/// Erzeugt den Plan für `req`. Prüft ihn gegen das Schema und gegen die Regeln: Eine harte
/// Verletzung ist ein Fehler (Exit 1), weiche stehen als Warnungen im Plan.
pub fn generate(
    store: &Store,
    req: &PlanRequest,
    plan_id: String,
    created_at: DateTime<Utc>,
) -> Result<Generated, CliError> {
    generate_with(store, req, None, plan_id, created_at)
}

/// Wie [`generate`], aber mit einer anderen Verfügbarkeit als der gespeicherten. Die App
/// zeigt damit vor dem Übernehmen eines Vorschlags des Coachs (T26, SPEC 9.7), was der
/// Regelplaner aus neuen Zeiten machen würde, ohne etwas in die Datenbank zu schreiben.
pub fn generate_with(
    store: &Store,
    req: &PlanRequest,
    availability_override: Option<[DayAvailability; 7]>,
    plan_id: String,
    created_at: DateTime<Utc>,
) -> Result<Generated, CliError> {
    if req.week_start.weekday().num_days_from_monday() != 0 {
        return Err(CliError::Input(format!(
            "Der Planbeginn {} ist kein Montag.",
            req.week_start
        )));
    }
    let (rules, catalog) = load_rules()?;
    let mut notes = Vec::new();
    let availability = match availability_override {
        Some(days) => days,
        None => availability(store, &mut notes)?,
    };
    let history = store.week_history(req.week_start, HISTORY_WEEKS)?;
    let pmc_params = store.pmc_params()?;
    let (ctl0, atl0) = start_state(store, &pmc_params, req.week_start)?;
    let mut sports = sport_weights(&req.sports, &history, &mut notes);
    let (goal, goal_id) = resolve_goal(store, req, &mut sports, &mut notes)?;
    let ctx = PlanContext {
        plan_id,
        created_at,
        locale: req.locale,
        week_start: req.week_start,
        horizon_weeks: req
            .weeks
            .unwrap_or_else(|| default_horizon(&goal, req.week_start)),
        goal,
        ctl0,
        atl0,
        history,
        availability,
        sports,
        strength_per_week: req.strength_per_week,
        ctl_rise_per_week: None,
        pmc_params,
        catalog,
        recovery: recovery_input(store, req.week_start, &rules)?,
    };
    let plan = generate_plan(&ctx, &rules).map_err(|e| plan_error(&e))?;
    // Der Planer erzeugt nie eine harte Verletzung. Die Prüfung bleibt als Absicherung, weil
    // `plan` der Weg ist, auf dem Pläne in die Datenbank kommen.
    let violations = validate_plan(&plan, &ctx, &rules);
    let hard: Vec<&Violation> = violations
        .iter()
        .filter(|v| v.hardness == Hardness::Hard)
        .collect();
    if !hard.is_empty() {
        let texts = Texts::load(req.locale)?;
        return Err(CliError::Validation(format!(
            "Der erzeugte Plan verletzt harte Regeln:\n{}",
            violation_lines(&hard, &texts)
        )));
    }
    Ok(Generated {
        plan,
        goal_id,
        notes,
    })
}

/// Plan als JSON-Text: Felder in der Reihenfolge des Schemas, eingerückt, mit Zeilenumbruch
/// am Ende. Der Text wird vor der Rückgabe gegen das Schema geprüft.
pub fn plan_json(plan: &Plan) -> Result<String, CliError> {
    let value = serde_json::to_value(plan)
        .map_err(|e| CliError::Internal(format!("Plan lässt sich nicht schreiben: {e}")))?;
    let errors = schema::plan_errors(&value)?;
    if !errors.is_empty() {
        return Err(CliError::Internal(format!(
            "Der erzeugte Plan verletzt das Schema:\n{}",
            errors.join("\n")
        )));
    }
    let mut text = serde_json::to_string_pretty(plan)
        .map_err(|e| CliError::Internal(format!("Plan lässt sich nicht schreiben: {e}")))?;
    text.push('\n');
    Ok(text)
}

fn write_error(e: &std::io::Error) -> CliError {
    CliError::Input(format!(
        "Die Ausgabedatei lässt sich nicht schreiben: {}",
        e.kind()
    ))
}

fn output_error(e: &std::io::Error) -> CliError {
    CliError::Internal(format!("Ausgabefehler: {}", e.kind()))
}

/// `lastwerk plan`: erzeugt den Plan, schreibt `out_path` und speichert ihn als Entwurf.
pub fn run_plan(
    store: &mut Store,
    req: &PlanRequest,
    out_path: &Path,
    plan_id: String,
    created_at: DateTime<Utc>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let generated = generate(store, req, plan_id, created_at)?;
    let json = plan_json(&generated.plan)?;
    std::fs::write(out_path, json).map_err(|e| write_error(&e))?;
    store.save_plan_for_goal(&generated.plan, PlanStatus::Draft, generated.goal_id)?;
    for note in &generated.notes {
        writeln!(err, "{note}").map_err(|e| output_error(&e))?;
    }
    let plan = &generated.plan;
    let sessions: usize = plan
        .weeks
        .iter()
        .flat_map(|w| w.days.iter())
        .map(|d| d.sessions.len())
        .sum();
    let first = plan.weeks.first().map(|w| w.week_start);
    let last = plan.weeks.last().map(|w| w.week_start);
    writeln!(
        out,
        "Plan {} erzeugt: {} Wochen ({} bis {}), {} Einheiten, {} Warnungen. Als Entwurf \
         gespeichert.",
        plan.plan_id,
        plan.weeks.len(),
        first.map_or_else(String::new, |d| d.to_string()),
        last.map_or_else(String::new, |d| d.to_string()),
        sessions,
        plan.warnings.len()
    )
    .map_err(|e| output_error(&e))
}

/// Liest einen Plan aus einer Datei und prüft ihn gegen das Schema. Eine Schemaverletzung ist
/// ein Validierungsfehler (Exit 1), eine unlesbare Datei ein Eingabefehler (Exit 2).
pub fn read_plan(path: &Path) -> Result<Plan, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        CliError::Input(format!(
            "Die Plandatei lässt sich nicht lesen: {}",
            e.kind()
        ))
    })?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| CliError::Input(format!("Die Plandatei ist kein gültiges JSON: {e}")))?;
    let errors = schema::plan_errors(&value)?;
    if !errors.is_empty() {
        return Err(CliError::Validation(format!(
            "Der Plan verletzt das Schema ({} Verletzungen):\n{}",
            errors.len(),
            errors.join("\n")
        )));
    }
    serde_json::from_value(value)
        .map_err(|e| CliError::Validation(format!("Der Plan lässt sich nicht einlesen: {e}")))
}

fn violation_lines(violations: &[&Violation], texts: &Texts) -> String {
    let mut lines = Vec::new();
    for v in violations {
        let name = texts
            .get(&format!("rule.{}.name", v.code))
            .unwrap_or("unbekannte Regel");
        let when = v.date.map_or_else(String::new, |d| format!(" am {d}"));
        let kind = match v.hardness {
            Hardness::Hard => "hart",
            Hardness::Soft => "weich",
        };
        let mut line = format!("  {} ({kind}){when}: {name}", v.code);
        if !v.detail.is_empty() {
            let detail: Vec<String> = v
                .detail
                .iter()
                .map(|(k, val)| format!("{k}={val}"))
                .collect();
            line.push_str(&format!(" [{}]", detail.join(", ")));
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Kontext für die Prüfung eines vorhandenen Plans: Verfügbarkeit, Historie und Startwerte
/// aus der Datenbank zum Beginn des Plans. Öffentlich, weil die App Pläne mit demselben
/// Kontext prüft wie `lastwerk validate` (T09).
pub fn validation_context(
    store: &Store,
    plan: &Plan,
    rules: &Rules,
    catalog: TemplateCatalog,
    notes: &mut Vec<String>,
) -> Result<PlanContext, CliError> {
    let week_start = plan
        .weeks
        .first()
        .map(|w| w.week_start)
        .ok_or_else(|| CliError::Validation("Der Plan enthält keine Woche.".into()))?;
    if week_start.weekday().num_days_from_monday() != 0 {
        return Err(CliError::Validation(format!(
            "Die erste Woche beginnt am {week_start}, das ist kein Montag."
        )));
    }
    let mut sports: BTreeMap<u8, SportWeight> = BTreeMap::new();
    for s in plan
        .weeks
        .iter()
        .flat_map(|w| w.days.iter())
        .flat_map(|d| d.sessions.iter())
    {
        if s.sport.is_endurance() {
            sports.insert(
                endurance_rank(s.sport),
                SportWeight {
                    sport: s.sport,
                    weight: 1.0,
                },
            );
        }
    }
    let has_strength = plan
        .weeks
        .iter()
        .flat_map(|w| w.days.iter())
        .flat_map(|d| d.sessions.iter())
        .any(|s| s.sport == PlanSport::Strength);
    let goal = match (plan.goal.kind, plan.goal.event_date) {
        (GoalKind::Event, Some(event_date)) => GoalSpec::Event {
            event_date,
            sport: plan.goal.sport.unwrap_or(PlanSport::Cycling),
            priority: plan.goal.priority.unwrap_or(Priority::A),
        },
        _ => GoalSpec::Continuous,
    };
    let pmc_params = store.pmc_params()?;
    let (ctl0, atl0) = start_state(store, &pmc_params, week_start)?;
    Ok(PlanContext {
        plan_id: plan.plan_id.clone(),
        created_at: plan.created_at,
        locale: plan.locale,
        week_start,
        goal,
        horizon_weeks: u32::try_from(plan.weeks.len()).unwrap_or(u32::MAX),
        ctl0,
        atl0,
        history: store.week_history(week_start, HISTORY_WEEKS)?,
        availability: availability(store, notes)?,
        sports: sports.into_values().collect(),
        // Ein Plan ohne Krafteinheit gilt als ohne Kraftwunsch erzeugt, R7 fordert dann keine.
        strength_per_week: u8::from(has_strength),
        ctl_rise_per_week: None,
        pmc_params,
        catalog,
        recovery: recovery_input(store, week_start, rules)?,
    })
}

/// `lastwerk validate`: Schema, dann Leitplanken gegen den Kontext aus der Datenbank.
/// Harte Verletzung: Fehler mit Exit 1. Weiche Verletzungen erscheinen als Warnungen.
pub fn run_validate(
    store: &Store,
    plan_path: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let plan = read_plan(plan_path)?;
    let (rules, catalog) = load_rules()?;
    let mut notes = Vec::new();
    let ctx = validation_context(store, &plan, &rules, catalog, &mut notes)?;
    for note in &notes {
        writeln!(err, "{note}").map_err(|e| output_error(&e))?;
    }
    let mut violations = validate_plan(&plan, &ctx, &rules);
    violations.sort_by(|a, b| (a.date, &a.code).cmp(&(b.date, &b.code)));
    let texts = Texts::load(plan.locale)?;
    let hard: Vec<&Violation> = violations
        .iter()
        .filter(|v| v.hardness == Hardness::Hard)
        .collect();
    let soft: Vec<&Violation> = violations
        .iter()
        .filter(|v| v.hardness == Hardness::Soft)
        .collect();
    writeln!(out, "Schema: in Ordnung").map_err(|e| output_error(&e))?;
    if !soft.is_empty() {
        writeln!(
            out,
            "Warnungen ({}):\n{}",
            soft.len(),
            violation_lines(&soft, &texts)
        )
        .map_err(|e| output_error(&e))?;
    }
    if hard.is_empty() {
        writeln!(out, "Regeln: keine harte Verletzung").map_err(|e| output_error(&e))
    } else {
        Err(CliError::Validation(format!(
            "Der Plan verletzt {} harte Regeln:\n{}",
            hard.len(),
            violation_lines(&hard, &texts)
        )))
    }
}
