//! Pläne in der Tabelle `plan` (SPEC 6.2, 5.4: `save_plan`).
//!
//! Der Plan liegt als JSON-Text in `plan.json`, die Spalten `id`, `created_at`, `source`,
//! `rules_version` und `locale` sind Kopien aus dem Plan, damit sich Pläne ohne Parsen
//! auflisten lassen. Die Schemaprüfung (`plan.schema.json`) liegt bei den Aufrufern, der
//! Speicher prüft nur, dass sich der Text als [`Plan`] lesen lässt.
//!
//! Konvention: Es gibt höchstens einen Plan mit Status `active`. Wer einen Plan als `active`
//! speichert oder auf `active` setzt, archiviert den bisherigen aktiven Plan in derselben
//! Transaktion.

use chrono::{DateTime, Utc};
use lastwerk_core::plan::{Locale, Plan, PlanSource};
use rusqlite::{OptionalExtension, params};

use crate::error::{ParseError, Result, StoreError, conversion};
use crate::rows::{fmt_utc, parse_utc};
use crate::store::Store;

/// Status eines Plans in der Datenbank (SPEC 6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStatus {
    Draft,
    Active,
    Archived,
}

impl PlanStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    pub fn parse(s: &str) -> std::result::Result<Self, ParseError> {
        Ok(match s {
            "draft" => Self::Draft,
            "active" => Self::Active,
            "archived" => Self::Archived,
            _ => return Err(ParseError(format!("plan status {s:?}"))),
        })
    }
}

/// Eine Zeile der Planliste, ohne das JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanSummary {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub goal_id: Option<i64>,
    pub source: PlanSource,
    pub rules_version: String,
    pub locale: Locale,
    pub status: PlanStatus,
}

fn source_str(s: PlanSource) -> &'static str {
    match s {
        PlanSource::Rules => "rules",
        PlanSource::Llm => "llm",
    }
}

fn locale_str(l: Locale) -> &'static str {
    match l {
        Locale::De => "de",
        Locale::En => "en",
    }
}

fn parse_source(s: &str) -> std::result::Result<PlanSource, ParseError> {
    match s {
        "rules" => Ok(PlanSource::Rules),
        "llm" => Ok(PlanSource::Llm),
        _ => Err(ParseError(format!("plan source {s:?}"))),
    }
}

fn parse_locale(s: &str) -> std::result::Result<Locale, ParseError> {
    match s {
        "de" => Ok(Locale::De),
        "en" => Ok(Locale::En),
        _ => Err(ParseError(format!("locale {s:?}"))),
    }
}

impl Store {
    /// Speichert einen Plan (SPEC 5.4). Ein Plan mit gleicher `plan_id` wird ersetzt. Mit
    /// Status `Active` wird der bisherige aktive Plan archiviert.
    pub fn save_plan(&mut self, plan: &Plan, status: PlanStatus) -> Result<()> {
        self.save_plan_for_goal(plan, status, None)
    }

    /// Wie [`Store::save_plan`], mit Verweis auf das Ziel in `goal` (`plan.goal_id`).
    pub fn save_plan_for_goal(
        &mut self,
        plan: &Plan,
        status: PlanStatus,
        goal_id: Option<i64>,
    ) -> Result<()> {
        let json = serde_json::to_string(plan)?;
        let tx = self.conn.transaction()?;
        if status == PlanStatus::Active {
            tx.execute(
                "UPDATE plan SET status = 'archived' WHERE status = 'active' AND id <> ?1",
                [&plan.plan_id],
            )?;
        }
        tx.execute(
            "INSERT INTO plan (id, created_at, goal_id, source, rules_version, locale, json, \
                               status) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(id) DO UPDATE SET \
                 created_at = excluded.created_at, goal_id = excluded.goal_id, \
                 source = excluded.source, rules_version = excluded.rules_version, \
                 locale = excluded.locale, json = excluded.json, status = excluded.status",
            params![
                plan.plan_id,
                fmt_utc(plan.created_at),
                goal_id,
                source_str(plan.source),
                plan.rules_version,
                locale_str(plan.locale),
                json,
                status.as_str()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Alle Pläne, die neuesten zuerst (Gleichstand nach `id`).
    pub fn list_plans(&self) -> Result<Vec<PlanSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, goal_id, source, rules_version, locale, status FROM plan \
             ORDER BY created_at DESC, id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                let created: String = r.get(1)?;
                let source: String = r.get(3)?;
                let locale: String = r.get(5)?;
                let status: String = r.get(6)?;
                Ok(PlanSummary {
                    id: r.get(0)?,
                    created_at: conversion(1, parse_utc(&created))?,
                    goal_id: r.get(2)?,
                    source: conversion(3, parse_source(&source))?,
                    rules_version: r.get(4)?,
                    locale: conversion(5, parse_locale(&locale))?,
                    status: conversion(6, PlanStatus::parse(&status))?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Liest einen Plan zurück, `None` bei unbekannter ID.
    pub fn load_plan(&self, id: &str) -> Result<Option<Plan>> {
        let json: Option<String> = self
            .conn
            .query_row("SELECT json FROM plan WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        json.map(|j| serde_json::from_str(&j).map_err(StoreError::from))
            .transpose()
    }

    /// Der aktive Plan, falls es einen gibt.
    pub fn active_plan(&self) -> Result<Option<Plan>> {
        let json: Option<String> = self
            .conn
            .query_row("SELECT json FROM plan WHERE status = 'active'", [], |r| {
                r.get(0)
            })
            .optional()?;
        json.map(|j| serde_json::from_str(&j).map_err(StoreError::from))
            .transpose()
    }

    /// Setzt den Status eines Plans. `Active` archiviert den bisherigen aktiven Plan.
    pub fn set_plan_status(&mut self, id: &str, status: PlanStatus) -> Result<()> {
        let tx = self.conn.transaction()?;
        if status == PlanStatus::Active {
            tx.execute(
                "UPDATE plan SET status = 'archived' WHERE status = 'active' AND id <> ?1",
                [id],
            )?;
        }
        let changed = tx.execute(
            "UPDATE plan SET status = ?2 WHERE id = ?1",
            params![id, status.as_str()],
        )?;
        if changed == 0 {
            return Err(StoreError::PlanNotFound(id.to_string()));
        }
        tx.commit()?;
        Ok(())
    }
}
