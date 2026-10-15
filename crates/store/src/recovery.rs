//! Erholungstage (Tabelle `recovery_day`, SPEC 6.2, 7.3).
//!
//! Die Werte kommen aus dem Zusatzprogramm (`exchange/out/recovery`) oder aus dem Abgleich mit
//! Garmin in der App (T22, `source = "garmin-app"`) und sind nicht aus FIT
//! ableitbar. Sie gehören deshalb zu den Nutzerdaten: Sie stehen im Backup, und
//! `rebuild_cache` lässt sie unberührt. Die Zeit des Imports kommt wie überall im Speicher von
//! außen. Die Prüfung der Dateien (Größe, Schema, Dateiname) macht die CLI, der Speicher
//! prüft nur noch die Wertebereiche des Schemas, damit nie ein unplausibler Wert in die
//! Tabelle gelangt.

use chrono::{DateTime, NaiveDate, Utc};
use lastwerk_core::plan::RecoveryDay;
use rusqlite::{OptionalExtension, params};

use crate::error::{Result, StoreError, conversion};
use crate::rows::{fmt_date, fmt_utc, parse_date, parse_utc};
use crate::store::Store;

/// Plausibilitätsgrenzen wie in `schema/recovery-day.schema.json` (Faustregel, SPEC 7.3).
pub const HRV_MAX_MS: f64 = 400.0;
pub const RESTING_HR_MIN_BPM: f64 = 20.0;
pub const RESTING_HR_MAX_BPM: f64 = 120.0;
pub const SLEEP_MAX_S: u32 = 86_400;
pub const SLEEP_SCORE_MAX: u8 = 100;
/// Body Battery liegt auf der Skala 0 bis 100 des Herstellers (Faustregel als Obergrenze,
/// T22).
pub const BODY_BATTERY_MAX: u8 = 100;
/// Längste `source` in Zeichen (Schema).
pub const SOURCE_MAX_CHARS: usize = 64;

/// Eine Zeile von `recovery_day`. Fehlende Werte sind `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryDayRecord {
    pub date: NaiveDate,
    pub hrv_overnight_ms: Option<f64>,
    pub resting_hr_bpm: Option<f64>,
    pub sleep_duration_s: Option<u32>,
    pub sleep_score: Option<u8>,
    /// Höchster Body-Battery-Wert des Tages (0 bis 100, seit T22).
    pub body_battery_high: Option<u8>,
    /// Tiefster Body-Battery-Wert des Tages (0 bis 100, seit T22).
    pub body_battery_low: Option<u8>,
    /// Absender der Datei, zum Beispiel `garmin-bridge 0.1.0`.
    pub source: String,
    /// Zeitpunkt des Imports (UTC), vom Aufrufer gesetzt.
    pub imported_at: DateTime<Utc>,
}

impl RecoveryDayRecord {
    /// Wertebereiche des Schemas 7.3. Gilt für jeden Schreibweg, auch für das Einspielen einer
    /// Sicherung (Integrationsprüfung, 14. Oktober 2026).
    pub(crate) fn check(&self) -> Result<()> {
        let bad = |what: &str| Err(StoreError::InvalidInput(format!("recovery day {what}")));
        if self
            .hrv_overnight_ms
            .is_some_and(|v| !(0.0..=HRV_MAX_MS).contains(&v))
        {
            return bad("hrv_overnight_ms out of range");
        }
        if self
            .resting_hr_bpm
            .is_some_and(|v| !(RESTING_HR_MIN_BPM..=RESTING_HR_MAX_BPM).contains(&v))
        {
            return bad("resting_hr_bpm out of range");
        }
        if self.sleep_duration_s.is_some_and(|v| v > SLEEP_MAX_S) {
            return bad("sleep_duration_s out of range");
        }
        if self.sleep_score.is_some_and(|v| v > SLEEP_SCORE_MAX) {
            return bad("sleep_score out of range");
        }
        if self.body_battery_high.is_some_and(|v| v > BODY_BATTERY_MAX) {
            return bad("body_battery_high out of range");
        }
        if self.body_battery_low.is_some_and(|v| v > BODY_BATTERY_MAX) {
            return bad("body_battery_low out of range");
        }
        if self.source.chars().count() > SOURCE_MAX_CHARS {
            return bad("source too long");
        }
        Ok(())
    }
}

/// Eingabe des Planers aus einer Zeile (Float-Genauigkeit des Planers ist `f32`).
impl From<&RecoveryDayRecord> for RecoveryDay {
    fn from(r: &RecoveryDayRecord) -> Self {
        Self {
            date: r.date,
            hrv_overnight_ms: r.hrv_overnight_ms.map(|v| v as f32),
            resting_hr_bpm: r.resting_hr_bpm.map(|v| v as f32),
            sleep_duration_s: r.sleep_duration_s,
            sleep_score: r.sleep_score,
        }
    }
}

/// Ergebnis von [`Store::upsert_recovery_day`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    Inserted,
    /// Das Datum gab es schon, mindestens ein Wert oder die Quelle wich ab.
    Updated,
    /// Gleiche Werte und Quelle wie die vorhandene Zeile, `imported_at` bleibt stehen.
    Unchanged,
}

fn read_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RecoveryDayRecord> {
    let date: String = r.get(0)?;
    let imported: String = r.get(8)?;
    Ok(RecoveryDayRecord {
        date: conversion(0, parse_date(&date))?,
        hrv_overnight_ms: r.get(1)?,
        resting_hr_bpm: r.get(2)?,
        // Kein `as`: ein Wert außerhalb des Typs ist ein Lesefehler, kein still umgebrochener Wert.
        sleep_duration_s: r
            .get::<_, Option<i64>>(3)?
            .map(|v| conversion(3, u32::try_from(v)))
            .transpose()?,
        sleep_score: r
            .get::<_, Option<i64>>(4)?
            .map(|v| conversion(4, u8::try_from(v)))
            .transpose()?,
        body_battery_high: r
            .get::<_, Option<i64>>(5)?
            .map(|v| conversion(5, u8::try_from(v)))
            .transpose()?,
        body_battery_low: r
            .get::<_, Option<i64>>(6)?
            .map(|v| conversion(6, u8::try_from(v)))
            .transpose()?,
        source: r.get(7)?,
        imported_at: conversion(8, parse_utc(&imported))?,
    })
}

const SELECT: &str = "SELECT local_date, hrv_overnight_ms, resting_hr_bpm, sleep_duration_s, \
                      sleep_score, body_battery_high, body_battery_low, source, imported_at \
                      FROM recovery_day";

/// Legt die Zeile an oder ersetzt sie, auf einer offenen Verbindung oder Transaktion.
fn upsert_in(conn: &rusqlite::Connection, d: &RecoveryDayRecord) -> Result<UpsertOutcome> {
    let existing = conn
        .query_row(
            &format!("{SELECT} WHERE local_date = ?1"),
            [fmt_date(d.date)],
            read_row,
        )
        .optional()?;
    let outcome = match &existing {
        None => UpsertOutcome::Inserted,
        Some(old) if same_values(old, d) => return Ok(UpsertOutcome::Unchanged),
        Some(_) => UpsertOutcome::Updated,
    };
    conn.execute(
        "INSERT INTO recovery_day (local_date, hrv_overnight_ms, resting_hr_bpm, \
                                   sleep_duration_s, sleep_score, body_battery_high, \
                                   body_battery_low, source, imported_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT(local_date) DO UPDATE SET \
           hrv_overnight_ms = excluded.hrv_overnight_ms, \
           resting_hr_bpm = excluded.resting_hr_bpm, \
           sleep_duration_s = excluded.sleep_duration_s, \
           sleep_score = excluded.sleep_score, \
           body_battery_high = excluded.body_battery_high, \
           body_battery_low = excluded.body_battery_low, \
           source = excluded.source, \
           imported_at = excluded.imported_at",
        params![
            fmt_date(d.date),
            d.hrv_overnight_ms,
            d.resting_hr_bpm,
            d.sleep_duration_s.map(i64::from),
            d.sleep_score.map(i64::from),
            d.body_battery_high.map(i64::from),
            d.body_battery_low.map(i64::from),
            d.source,
            fmt_utc(d.imported_at)
        ],
    )?;
    Ok(outcome)
}

impl Store {
    /// Legt einen Erholungstag an oder ersetzt den vorhandenen mit gleichem Datum (Garmin
    /// finalisiert Werte nachträglich, der jüngste Import gilt). Eine Zeile mit gleichen
    /// Werten und gleicher Quelle bleibt unverändert, auch `imported_at`, damit ein zweiter
    /// Import derselben Dateien nichts schreibt.
    pub fn upsert_recovery_day(&mut self, d: &RecoveryDayRecord) -> Result<UpsertOutcome> {
        d.check()?;
        upsert_in(&self.conn, d)
    }

    /// Wie [`Store::upsert_recovery_day`] für viele Tage in einer Transaktion. Alle Zeilen
    /// werden vorab geprüft: Ist eine ungültig, wird nichts geschrieben. Kommen mehrere Zeilen
    /// mit gleichem Datum vor, gilt die spätere. Die Ergebnisse stehen in der Reihenfolge der
    /// Eingabe.
    pub fn upsert_recovery_days(
        &mut self,
        days: &[RecoveryDayRecord],
    ) -> Result<Vec<UpsertOutcome>> {
        for d in days {
            d.check()?;
        }
        let tx = self.conn.transaction()?;
        let mut outcomes = Vec::with_capacity(days.len());
        for d in days {
            outcomes.push(upsert_in(&tx, d)?);
        }
        tx.commit()?;
        Ok(outcomes)
    }

    /// Erholungstage von `from` bis `to` einschließlich, nach Datum aufsteigend.
    pub fn recovery_days(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<RecoveryDayRecord>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SELECT} WHERE local_date >= ?1 AND local_date <= ?2 ORDER BY local_date"
        ))?;
        let rows = stmt
            .query_map([fmt_date(from), fmt_date(to)], read_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

fn same_values(a: &RecoveryDayRecord, b: &RecoveryDayRecord) -> bool {
    a.date == b.date
        && a.hrv_overnight_ms == b.hrv_overnight_ms
        && a.resting_hr_bpm == b.resting_hr_bpm
        && a.sleep_duration_s == b.sleep_duration_s
        && a.sleep_score == b.sleep_score
        && a.body_battery_high == b.body_battery_high
        && a.body_battery_low == b.body_battery_low
        && a.source == b.source
}
