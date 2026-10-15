//! Gezielte Lesezugriffe für die Oberfläche (T06).
//!
//! Die Oberfläche braucht Listen eines Zeitraums und die Daten einer einzelnen Aktivität. Die
//! Gesamtlesungen (`activities`, `activity_loads`, `zone_times`, `best_powers`) würden bei
//! jedem Klick die ganze Datenbank lesen. Hier filtert SQL. Nichts davon schreibt, und nichts
//! davon rechnet: Es sind Abfragen auf die Cache-Tabellen aus SPEC 6.2.

use chrono::NaiveDate;
use rusqlite::{OptionalExtension, params};

use crate::error::Result;
use crate::rows::{ActivityRecord, BestPowerRow, LoadRow, SourceFileRecord, ZoneTimeRow, fmt_date};
use crate::store::{
    ACTIVITY_COLUMNS, LOAD_COLUMNS, SOURCE_FILE_COLUMNS, Store, activity_from_row, load_from_row,
    source_file_from_row,
};

impl Store {
    /// Eine Aktivität, oder `None`, wenn es die `id` nicht gibt.
    pub fn activity(&self, id: i64) -> Result<Option<ActivityRecord>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {ACTIVITY_COLUMNS} FROM activity WHERE id = ?1"),
                [id],
                activity_from_row,
            )
            .optional()?)
    }

    /// Aktivitäten, deren lokales Datum in `from..=to` liegt, älteste zuerst (Start, dann
    /// `id`). Enthält auch Duplikate (`is_primary = 0`), die Oberfläche zeigt sie an.
    pub fn activities_between(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<ActivityRecord>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ACTIVITY_COLUMNS} FROM activity \
             WHERE local_date >= ?1 AND local_date <= ?2 ORDER BY start_utc, id"
        ))?;
        let rows = stmt
            .query_map(params![fmt_date(from), fmt_date(to)], activity_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Belastungszeilen der Aktivitäten, deren lokales Datum in `from..=to` liegt, sortiert
    /// nach Aktivität und Methode.
    pub fn loads_between(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<LoadRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.activity_id, l.method, l.value, l.intensity_factor, l.threshold_id, \
                    l.status, l.computed_at \
             FROM activity_load l JOIN activity a ON a.id = l.activity_id \
             WHERE a.local_date >= ?1 AND a.local_date <= ?2 ORDER BY l.activity_id, l.method",
        )?;
        let rows = stmt
            .query_map(params![fmt_date(from), fmt_date(to)], load_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Belastungszeilen einer Aktivität, sortiert nach Methode.
    pub fn activity_loads_of(&self, activity_id: i64) -> Result<Vec<LoadRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {LOAD_COLUMNS} FROM activity_load WHERE activity_id = ?1 ORDER BY method"
        ))?;
        let rows = stmt
            .query_map([activity_id], load_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Zeit in Zonen einer Aktivität, sortiert nach Schema und Zone. Ein Schema, das für die
    /// Aktivität nicht berechenbar war, fehlt (keine Nullen, SPEC 6.2).
    pub fn zone_times_of(&self, activity_id: i64) -> Result<Vec<ZoneTimeRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT activity_id, scheme, zone, seconds, estimated FROM activity_zone_time \
             WHERE activity_id = ?1 ORDER BY scheme, zone",
        )?;
        let rows = stmt
            .query_map([activity_id], |r| {
                Ok(ZoneTimeRow {
                    activity_id: r.get(0)?,
                    scheme: r.get(1)?,
                    zone: r.get(2)?,
                    seconds: r.get(3)?,
                    estimated: r.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Bestleistungen einer Aktivität, sortiert nach Dauer.
    pub fn best_powers_of(&self, activity_id: i64) -> Result<Vec<BestPowerRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT activity_id, duration_s, watts FROM activity_best_power \
             WHERE activity_id = ?1 ORDER BY duration_s",
        )?;
        let rows = stmt
            .query_map([activity_id], |r| {
                Ok(BestPowerRow {
                    activity_id: r.get(0)?,
                    duration_s: r.get(1)?,
                    watts: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Eine Quelldatei, oder `None`. Die Oberfläche braucht Pfad und Prüfsumme, um die
    /// Messpunkte einer Aktivität erneut aus der Datei zu lesen (der Cache hat keine, SPEC 6.1).
    pub fn source_file(&self, id: i64) -> Result<Option<SourceFileRecord>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {SOURCE_FILE_COLUMNS} FROM source_file WHERE id = ?1"),
                [id],
                source_file_from_row,
            )
            .optional()?)
    }
}
