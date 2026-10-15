//! Ziele und Verfügbarkeit (Tabellen `goal` und `availability`, SPEC 6.2).
//!
//! Beides sind Nutzereingaben für den Planer (SPEC 8.3, R11). Sie stehen im Backup.
//!
//! Konvention: Es gibt höchstens ein aktives Ziel. [`Store::set_goal`] legt ein neues Ziel
//! an und deaktiviert die übrigen, damit `lastwerk plan` ohne weitere Angabe das jüngste Ziel
//! nimmt. Frühere Ziele bleiben als Zeilen erhalten (Pläne verweisen über `goal_id` auf sie).

use chrono::NaiveDate;
use lastwerk_core::plan::{GoalKind, PlanSport, Priority};
use rusqlite::params;

use crate::error::{ParseError, Result, StoreError, conversion};
use crate::rows::{fmt_date, parse_date};
use crate::store::Store;

/// Ein Ziel zum Anlegen.
#[derive(Debug, Clone, PartialEq)]
pub struct NewGoal {
    pub kind: GoalKind,
    pub sport: Option<PlanSport>,
    /// Pflicht bei `Event`, bei `Continuous` nicht erlaubt.
    pub event_date: Option<NaiveDate>,
    /// Freitext, bleibt lokal und geht nie an ein Sprachmodell (SPEC 6.2).
    pub label: Option<String>,
    pub priority: Option<Priority>,
}

/// Ein gespeichertes Ziel.
#[derive(Debug, Clone, PartialEq)]
pub struct GoalRecord {
    pub id: i64,
    pub goal: NewGoal,
    pub active: bool,
}

/// Verfügbarkeit eines Wochentags (`weekday` 0 = Montag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvailabilityRecord {
    pub weekday: u8,
    pub max_minutes: u16,
    pub allow_hard: bool,
    pub fixed_rest: bool,
}

/// Längste Tagesverfügbarkeit in Minuten (ein Tag hat 1440, Plausibilitätsgrenze).
pub const MAX_AVAILABLE_MINUTES: u16 = 1440;

fn kind_str(k: GoalKind) -> &'static str {
    match k {
        GoalKind::Event => "event",
        GoalKind::Continuous => "continuous",
    }
}

fn parse_kind(s: &str) -> std::result::Result<GoalKind, ParseError> {
    match s {
        "event" => Ok(GoalKind::Event),
        "continuous" => Ok(GoalKind::Continuous),
        _ => Err(ParseError(format!("goal kind {s:?}"))),
    }
}

/// Sportart eines Ziels als Text, wie `plan.schema.json` sie schreibt.
pub fn plan_sport_str(s: PlanSport) -> &'static str {
    match s {
        PlanSport::Cycling => "cycling",
        PlanSport::Running => "running",
        PlanSport::Swimming => "swimming",
        PlanSport::Strength => "strength",
        PlanSport::Rest => "rest",
    }
}

fn parse_plan_sport(s: &str) -> std::result::Result<PlanSport, ParseError> {
    match s {
        "cycling" => Ok(PlanSport::Cycling),
        "running" => Ok(PlanSport::Running),
        "swimming" => Ok(PlanSport::Swimming),
        "strength" => Ok(PlanSport::Strength),
        "rest" => Ok(PlanSport::Rest),
        _ => Err(ParseError(format!("goal sport {s:?}"))),
    }
}

fn priority_str(p: Priority) -> &'static str {
    match p {
        Priority::A => "A",
        Priority::B => "B",
        Priority::C => "C",
    }
}

fn parse_priority(s: &str) -> std::result::Result<Priority, ParseError> {
    match s {
        "A" => Ok(Priority::A),
        "B" => Ok(Priority::B),
        "C" => Ok(Priority::C),
        _ => Err(ParseError(format!("goal priority {s:?}"))),
    }
}

impl Store {
    /// Legt ein Ziel an, macht es zum aktiven und deaktiviert alle anderen. Gibt die `id`
    /// zurück.
    pub fn set_goal(&mut self, g: &NewGoal) -> Result<i64> {
        match (g.kind, g.event_date) {
            (GoalKind::Event, None) => {
                return Err(StoreError::InvalidInput(
                    "an event goal needs an event date".into(),
                ));
            }
            (GoalKind::Continuous, Some(_)) => {
                return Err(StoreError::InvalidInput(
                    "a continuous goal has no event date".into(),
                ));
            }
            _ => {}
        }
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE goal SET active = 0 WHERE active <> 0", [])?;
        tx.execute(
            "INSERT INTO goal (kind, sport, event_date, label, priority, active) \
             VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            params![
                kind_str(g.kind),
                g.sport.map(plan_sport_str),
                g.event_date.map(fmt_date),
                g.label,
                g.priority.map(priority_str)
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }

    /// Alle Ziele nach `id`.
    pub fn list_goals(&self) -> Result<Vec<GoalRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, sport, event_date, label, priority, active FROM goal ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                let kind: String = r.get(1)?;
                let sport: Option<String> = r.get(2)?;
                let date: Option<String> = r.get(3)?;
                let priority: Option<String> = r.get(5)?;
                Ok(GoalRecord {
                    id: r.get(0)?,
                    goal: NewGoal {
                        kind: conversion(1, parse_kind(&kind))?,
                        sport: sport
                            .map(|s| conversion(2, parse_plan_sport(&s)))
                            .transpose()?,
                        event_date: date.map(|d| conversion(3, parse_date(&d))).transpose()?,
                        label: r.get(4)?,
                        priority: priority
                            .map(|p| conversion(5, parse_priority(&p)))
                            .transpose()?,
                    },
                    active: r.get::<_, i64>(6)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Das aktive Ziel (das mit der höchsten `id`, falls die Datenbank mehrere kennt).
    pub fn active_goal(&self) -> Result<Option<GoalRecord>> {
        Ok(self.list_goals()?.into_iter().rev().find(|g| g.active))
    }

    /// Setzt die Verfügbarkeit eines Wochentags (`weekday` 0 = Montag bis 6 = Sonntag).
    pub fn set_availability(
        &mut self,
        weekday: u8,
        max_minutes: u16,
        allow_hard: bool,
        fixed_rest: bool,
    ) -> Result<()> {
        if weekday > 6 {
            return Err(StoreError::InvalidInput(format!(
                "weekday must be 0..=6, got {weekday}"
            )));
        }
        if max_minutes > MAX_AVAILABLE_MINUTES {
            return Err(StoreError::InvalidInput(format!(
                "max_minutes must be at most {MAX_AVAILABLE_MINUTES}, got {max_minutes}"
            )));
        }
        self.conn.execute(
            "INSERT INTO availability (weekday, max_minutes, allow_hard, fixed_rest) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(weekday) DO UPDATE SET max_minutes = excluded.max_minutes, \
                 allow_hard = excluded.allow_hard, fixed_rest = excluded.fixed_rest",
            params![
                weekday,
                max_minutes,
                i64::from(allow_hard),
                i64::from(fixed_rest)
            ],
        )?;
        Ok(())
    }

    /// Die gesetzten Wochentage, sortiert nach Wochentag. Nicht gesetzte Tage fehlen.
    pub fn list_availability(&self) -> Result<Vec<AvailabilityRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT weekday, max_minutes, allow_hard, fixed_rest FROM availability \
             ORDER BY weekday",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(AvailabilityRecord {
                    weekday: r.get(0)?,
                    max_minutes: r.get(1)?,
                    allow_hard: r.get::<_, i64>(2)? != 0,
                    fixed_rest: r.get::<_, i64>(3)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}
