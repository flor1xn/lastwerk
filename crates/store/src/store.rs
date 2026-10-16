//! Das Store-Objekt und die Zugriffe auf Cache- und Schwellentabellen (SPEC 5.4, 6.2).

use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDate;
use lastwerk_core::{DailyLoad, HrMax, Sex, Sport, Thresholds};
use rusqlite::{Connection, OptionalExtension, params};

use crate::derived::zone_thresholds;
use crate::duplicates::{self, DuplicateRule};
use crate::error::{Result, StoreError, conversion};
use crate::migrate;
use crate::rows::{
    ActivityRecord, ActivityRow, BestEffortRow, BestPowerRow, LoadRow, LoadStatus, NewThreshold,
    PeakPower, SourceFileRecord, SourceFileRow, SourceStatus, ThresholdKind, ThresholdOrigin,
    ThresholdRecord, ZoneTimeRow, fmt_date, fmt_utc, parse_date, parse_utc, sport_from_str,
    sport_to_str,
};
use crate::statements::Db;

/// Schlüssel in `setting` für das Geschlecht, das nur die TRIMP-Konstanten wählt (SPEC 5.4,
/// `Thresholds::sex`). SPEC 6.2 kennt kein Feld dafür, daher die Ablage als Einstellung.
/// Werte: `male`, `female`.
pub const SETTING_ATHLETE_SEX: &str = "athlete.sex";

/// Schlüssel in `setting`, der `get_recovery_days` im MCP-Server freigibt (SPEC 5.4, 9.6, O19).
/// Nur der Wert `true` gibt frei, `false` oder jeder andere Wert sperrt. Die Oberfläche schreibt
/// genau `true` oder `false`, der MCP-Server liest genau `true`.
pub const SETTING_MCP_RECOVERY_ENABLED: &str = "mcp.recovery_enabled";

/// Schwellenwerte zu einem Datum samt der Zeilen, aus denen sie stammen.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdLookup {
    pub thresholds: Thresholds,
    /// `threshold.id` je Art, die in `thresholds` gesetzt ist.
    pub ids: HashMap<ThresholdKind, i64>,
}

/// SQLite-Speicher: Cache aus den FIT-Dateien plus Nutzerdaten (SPEC 6.1).
pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    /// Öffnet die Datenbank und führt alle ausstehenden Migrationen aus. Legt die Datei an,
    /// wenn sie fehlt. Der Ordner muss existieren.
    pub fn open(path: &Path) -> Result<Self> {
        Self::from_connection(Connection::open(path)?, true)
    }

    /// Flüchtige Datenbank im Speicher, zum Beispiel für `plausibility` und Tests.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?, false)
    }

    fn from_connection(mut conn: Connection, wal: bool) -> Result<Self> {
        // Fremdschlüssel sind je Verbindung auszuschalten, SQLite prüft sie sonst nicht.
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        if wal {
            // WAL: Lesen blockiert nicht beim Schreiben (SPEC 6.1). Das Ergebnis ist der
            // aktive Modus, bei Datenbanken im Speicher "memory".
            let _mode: String =
                conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
        }
        migrate::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Stand des Schemas (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<u32> {
        migrate::current_version(&self.conn)
    }

    // --- Einstellungen -------------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        get_setting(&Db::new(&self.conn), key)
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO setting (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    /// Entfernt eine Einstellung. Fehlt der Schlüssel, geschieht nichts. Gebraucht für die
    /// Übernahme des alten FIT-Ordners (T28), die `app.fit_dir` nach dem Kopieren löscht.
    pub fn delete_setting(&mut self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM setting WHERE key = ?1", [key])?;
        Ok(())
    }

    // --- Schwellenwerte ------------------------------------------------------------

    /// Legt einen Schwellenwert an und gibt seine `id` zurück. Ein früherer Wert bleibt
    /// erhalten, `thresholds_for` wählt je Art den jüngsten mit `valid_from <= Datum`.
    pub fn insert_threshold(&mut self, t: &NewThreshold) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO threshold (sport, kind, value, valid_from, origin, note) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                t.sport.map_or("all", sport_to_str),
                t.kind.as_str(),
                t.value,
                fmt_date(t.valid_from),
                t.origin.as_str(),
                t.note
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Alle Schwellenwerte, sortiert nach Art, Sportart und Gültigkeitsdatum (für
    /// `lastwerk thresholds list`).
    pub fn list_thresholds(&self) -> Result<Vec<ThresholdRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, sport, kind, value, valid_from, origin, note FROM threshold \
             ORDER BY kind, sport, valid_from, id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                let sport: String = r.get(1)?;
                let kind: String = r.get(2)?;
                let valid_from: String = r.get(4)?;
                let origin: String = r.get(5)?;
                Ok(ThresholdRecord {
                    id: r.get(0)?,
                    sport: if sport == "all" {
                        None
                    } else {
                        Some(conversion(1, sport_from_str(&sport))?)
                    },
                    kind: conversion(2, ThresholdKind::parse(&kind))?,
                    value: r.get(3)?,
                    valid_from: conversion(4, parse_date(&valid_from))?,
                    origin: conversion(5, ThresholdOrigin::parse(&origin))?,
                    note: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Letzter Tag (einschließlich), an dem der Schwellenwert `id` noch gilt, oder `None`,
    /// wenn kein späterer Wert derselben Art ihn ablöst.
    ///
    /// Ablösen kann ein späterer Wert derselben Art für dieselbe Sportart oder für alle
    /// Sportarten (`thresholds_for` nimmt je Art den jüngsten gültigen). Die CLI nutzt das,
    /// um nach `thresholds set` nur den betroffenen Zeitraum neu zu berechnen. Für einen Wert
    /// der Sportart `all` zählen nur spätere Werte für `all`; ein späterer Wert einer
    /// einzelnen Sportart führt zu einer größeren, aber nie zu einer zu kleinen Menge.
    pub fn threshold_valid_until(&self, id: i64) -> Result<Option<NaiveDate>> {
        let (sport, kind, valid_from): (String, String, String) = self.conn.query_row(
            "SELECT sport, kind, valid_from FROM threshold WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let next: Option<String> = self.conn.query_row(
            "SELECT MIN(valid_from) FROM threshold \
             WHERE kind = ?1 AND valid_from > ?2 AND (sport = ?3 OR sport = 'all')",
            params![kind, valid_from, sport],
            |r| r.get(0),
        )?;
        next.map(|d| {
            let date = parse_date(&d).map_err(|e| StoreError::InvalidData(e.0))?;
            Ok::<_, StoreError>(date.pred_opt().unwrap_or(date))
        })
        .transpose()
    }

    /// Maximale Herzfrequenz für die Zonen am Tag `on`: der gemessene Wert, sonst die
    /// Schätzung aus dem Alter (Q7) mit `estimated = true`, sonst `None`.
    pub fn effective_hr_max(&self, sport: Sport, on: NaiveDate) -> Result<Option<HrMax>> {
        let (th, estimated) = zone_thresholds(&Db::new(&self.conn), sport, on)?;
        Ok(th.hr_max_bpm.map(|bpm| HrMax { bpm, estimated }))
    }

    /// Alle Zeilen der Zeit in Zonen, sortiert nach Aktivität, Schema und Zone.
    pub fn zone_times(&self) -> Result<Vec<ZoneTimeRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT activity_id, scheme, zone, seconds, estimated FROM activity_zone_time \
             ORDER BY activity_id, scheme, zone",
        )?;
        let rows = stmt
            .query_map([], |r| {
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

    /// Alle Bestleistungen, sortiert nach Aktivität und Dauer.
    pub fn best_powers(&self) -> Result<Vec<BestPowerRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT activity_id, duration_s, watts FROM activity_best_power \
             ORDER BY activity_id, duration_s",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(BestPowerRow {
                    activity_id: r.get(0)?,
                    duration_s: r.get(1)?,
                    watts: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Alle Bestzeiten im Laufen (T37), sortiert nach Aktivität und Strecke.
    pub fn best_efforts(&self) -> Result<Vec<BestEffortRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT activity_id, distance_m, seconds FROM activity_best_effort \
             ORDER BY activity_id, distance_m",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(BestEffortRow {
                    activity_id: r.get(0)?,
                    distance_m: r.get(1)?,
                    seconds: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Beste Leistung über `duration_s` unter allen zählenden Aktivitäten der Sportart. Bei
    /// gleicher Leistung gewinnt die frühere Aktivität. Grundlage der FTP-Schätzung (SPEC 8.6).
    pub fn best_power_peak(&self, sport: Sport, duration_s: u32) -> Result<Option<PeakPower>> {
        let row = self
            .conn
            .query_row(
                "SELECT a.id, a.local_date, b.watts FROM activity_best_power b \
                 JOIN activity a ON a.id = b.activity_id \
                 WHERE a.is_primary = 1 AND a.sport = ?1 AND b.duration_s = ?2 \
                 ORDER BY b.watts DESC, a.local_date ASC, a.id ASC LIMIT 1",
                params![sport_to_str(sport), duration_s],
                |r| {
                    let date: String = r.get(1)?;
                    Ok(PeakPower {
                        activity_id: r.get(0)?,
                        local_date: conversion(1, parse_date(&date))?,
                        duration_s,
                        watts: r.get(2)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Erster Tag mit einer zählenden Belastung (primäre Aktivität, Status `ok`). Der Verlauf
    /// von CTL und ATL beginnt dort mit den Startwerten (SPEC 8.2).
    pub fn first_load_date(&self) -> Result<Option<NaiveDate>> {
        let date: Option<String> = self.conn.query_row(
            "SELECT MIN(a.local_date) FROM activity a \
             JOIN activity_load l ON l.activity_id = a.id \
             WHERE a.is_primary = 1 AND l.status = 'ok'",
            [],
            |r| r.get(0),
        )?;
        date.map(|d| parse_date(&d).map_err(|e| StoreError::InvalidData(e.0)))
            .transpose()
    }

    /// Schwellenwerte für eine Sportart, gültig am Datum `on` (SPEC 5.4).
    ///
    /// Je Art gilt der jüngste Eintrag mit `valid_from <= on`. Es zählen Einträge der
    /// Sportart und Einträge für alle Sportarten (`all`). Bei gleichem `valid_from` gewinnt
    /// ein Eintrag, der nicht aus Garmin stammt (Entscheidung E1, SPEC 8.6), danach der der
    /// Sportart, danach der später angelegte. `None`, wenn kein einziger Wert gilt.
    /// `sex` kommt aus der Einstellung [`SETTING_ATHLETE_SEX`].
    pub fn thresholds_for(&self, sport: Sport, on: NaiveDate) -> Result<Option<Thresholds>> {
        Ok(self.threshold_lookup(sport, on)?.map(|l| l.thresholds))
    }

    /// Wie [`Store::thresholds_for`], zusätzlich mit den Zeilen-IDs der gewählten Werte.
    pub fn threshold_lookup(&self, sport: Sport, on: NaiveDate) -> Result<Option<ThresholdLookup>> {
        threshold_lookup(&Db::new(&self.conn), sport, on)
    }

    // --- Cache: Dateien, Aktivitäten, Belastungen ---------------------------------

    /// Legt eine Quelldatei an oder aktualisiert die mit demselben `rel_path`. Gibt die `id`
    /// zurück.
    pub fn upsert_source_file(&mut self, f: &SourceFileRow) -> Result<i64> {
        upsert_source_file(&Db::new(&self.conn), f)
    }

    /// Legt eine Aktivität an oder aktualisiert die der Quelldatei `a.source_file_id`.
    /// `is_primary` und `duplicate_of` werden zurückgesetzt, bis die Duplikatregel läuft
    /// ([`Store::apply_duplicate_rule`]).
    pub fn upsert_activity(&mut self, a: &ActivityRow) -> Result<i64> {
        upsert_activity(&Db::new(&self.conn), a)
    }

    /// Ersetzt die Belastung einer Aktivität für diese Methode.
    pub fn upsert_activity_load(&mut self, l: &LoadRow) -> Result<()> {
        upsert_activity_load(&Db::new(&self.conn), l)
    }

    /// Führt die Duplikatregel (SPEC 6.4) über alle Aktivitäten aus. Gibt die Zahl der
    /// Duplikate zurück.
    pub fn apply_duplicate_rule(&mut self, rule: &DuplicateRule) -> Result<usize> {
        duplicates::apply(&self.conn, rule)
    }

    /// Tageslast von `from` bis `to` einschließlich (SPEC 5.4, 6.4).
    ///
    /// Es zählen nur primäre Aktivitäten (`is_primary = 1`) und Belastungen mit Status `ok`.
    /// Ein Tag ohne zählende Belastung fehlt in der Antwort, er wird nicht als Null geführt
    /// (fehlende Daten sind nicht Null, SPEC 5.4). Das Tagesdatum ist das lokale Datum (6.1).
    pub fn daily_loads(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<DailyLoad>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.local_date, SUM(l.value) FROM activity a \
             JOIN activity_load l ON l.activity_id = a.id \
             WHERE a.is_primary = 1 AND l.status = 'ok' \
               AND a.local_date >= ?1 AND a.local_date <= ?2 \
             GROUP BY a.local_date ORDER BY a.local_date",
        )?;
        let rows = stmt
            .query_map(params![fmt_date(from), fmt_date(to)], |r| {
                let d: String = r.get(0)?;
                Ok(DailyLoad {
                    date: conversion(0, parse_date(&d))?,
                    load: r.get::<_, f64>(1)? as f32,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Alle Quelldateien, sortiert nach `id`.
    pub fn source_files(&self) -> Result<Vec<SourceFileRecord>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SOURCE_FILE_COLUMNS} FROM source_file ORDER BY id"
        ))?;
        let rows = stmt
            .query_map([], source_file_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Alle Aktivitäten, sortiert nach `id`.
    pub fn activities(&self) -> Result<Vec<ActivityRecord>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ACTIVITY_COLUMNS} FROM activity ORDER BY id"
        ))?;
        let rows = stmt
            .query_map([], activity_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Alle Belastungszeilen, sortiert nach Aktivität und Methode.
    pub fn activity_loads(&self) -> Result<Vec<LoadRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {LOAD_COLUMNS} FROM activity_load ORDER BY activity_id, method"
        ))?;
        let rows = stmt
            .query_map([], load_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        // Nur die Art des Fehlers, nie den Pfad: Meldungen landen in Protokollen.
        StoreError::Io(e.kind().to_string())
    }
}

// --- Spalten und Zeilenumwandlung (auch von `read.rs` genutzt) ---------------------------

/// Spalten von `source_file` in der Reihenfolge, die [`source_file_from_row`] erwartet.
pub(crate) const SOURCE_FILE_COLUMNS: &str =
    "id, rel_path, sha256, size_bytes, mtime_utc, imported_at, status, error_code";

pub(crate) fn source_file_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SourceFileRecord> {
    let mtime: String = r.get(4)?;
    let imported: String = r.get(5)?;
    let status: String = r.get(6)?;
    Ok(SourceFileRecord {
        id: r.get(0)?,
        row: SourceFileRow {
            rel_path: r.get(1)?,
            sha256: r.get(2)?,
            size_bytes: r.get(3)?,
            mtime_utc: conversion(4, parse_utc(&mtime))?,
            imported_at: conversion(5, parse_utc(&imported))?,
            status: conversion(6, SourceStatus::parse(&status))?,
            error_code: r.get(7)?,
        },
    })
}

/// Spalten von `activity` in der Reihenfolge, die [`activity_from_row`] erwartet.
pub(crate) const ACTIVITY_COLUMNS: &str = "id, source_file_id, sport, sub_sport, start_utc, \
     local_offset_s, local_date, duration_s, moving_s, distance_m, ascent_m, avg_hr, max_hr, \
     avg_power_w, np_w, device_serial, device_product, has_power, has_hr, is_primary, \
     duplicate_of";

pub(crate) fn activity_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityRecord> {
    let sport: String = r.get(2)?;
    let start: String = r.get(4)?;
    let date: String = r.get(6)?;
    Ok(ActivityRecord {
        id: r.get(0)?,
        row: ActivityRow {
            source_file_id: r.get(1)?,
            sport: conversion(2, sport_from_str(&sport))?,
            sub_sport: r.get(3)?,
            start_utc: conversion(4, parse_utc(&start))?,
            local_offset_s: r.get(5)?,
            local_date: conversion(6, parse_date(&date))?,
            duration_s: r.get(7)?,
            moving_s: r.get(8)?,
            distance_m: r.get(9)?,
            ascent_m: r.get(10)?,
            avg_hr: r.get(11)?,
            max_hr: r.get(12)?,
            avg_power_w: r.get(13)?,
            np_w: r.get(14)?,
            device_serial: r.get(15)?,
            device_product: r.get(16)?,
            has_power: r.get::<_, i64>(17)? != 0,
            has_hr: r.get::<_, i64>(18)? != 0,
        },
        is_primary: r.get::<_, i64>(19)? != 0,
        duplicate_of: r.get(20)?,
    })
}

/// Spalten von `activity_load` in der Reihenfolge, die [`load_from_row`] erwartet.
pub(crate) const LOAD_COLUMNS: &str =
    "activity_id, method, value, intensity_factor, threshold_id, status, computed_at, missing";

pub(crate) fn load_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LoadRow> {
    let status: String = r.get(5)?;
    let at: String = r.get(6)?;
    Ok(LoadRow {
        activity_id: r.get(0)?,
        method: r.get(1)?,
        value: r.get(2)?,
        intensity_factor: r.get(3)?,
        threshold_id: r.get(4)?,
        status: conversion(5, LoadStatus::parse(&status))?,
        computed_at: conversion(6, parse_utc(&at))?,
        missing: r.get(7)?,
    })
}

// --- Zugriffe auf eine Verbindung (auch innerhalb einer Transaktion nutzbar) ------------

pub(crate) fn get_setting(db: &Db<'_>, key: &str) -> Result<Option<String>> {
    db.with("SELECT value FROM setting WHERE key = ?1", |s| {
        s.query_row([key], |r| r.get(0)).optional()
    })
}

/// Siehe [`Store::threshold_lookup`].
pub(crate) fn threshold_lookup(
    db: &Db<'_>,
    sport: Sport,
    on: NaiveDate,
) -> Result<Option<ThresholdLookup>> {
    let sport_text = sport_to_str(sport);
    let rows = db.with(
        "SELECT id, kind, value, valid_from FROM threshold \
         WHERE valid_from <= ?1 AND sport IN (?2, 'all') \
         ORDER BY kind, valid_from DESC, (origin = 'garmin') ASC, (sport = ?2) DESC, id DESC",
        |stmt| {
            stmt.query_map(params![fmt_date(on), sport_text], |r| {
                let kind: String = r.get(1)?;
                let valid_from: String = r.get(3)?;
                Ok((
                    r.get::<_, i64>(0)?,
                    conversion(1, ThresholdKind::parse(&kind))?,
                    r.get::<_, f64>(2)?,
                    conversion(3, parse_date(&valid_from))?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
        },
    )?;

    let mut ids: HashMap<ThresholdKind, i64> = HashMap::new();
    let mut th: Option<Thresholds> = None;
    for (id, kind, value, valid_from) in rows {
        // Sortiert nach Art, innerhalb der Art der gewählte zuerst.
        if ids.contains_key(&kind) {
            continue;
        }
        ids.insert(kind, id);
        let t = th.get_or_insert_with(|| Thresholds::empty(valid_from));
        t.valid_from = t.valid_from.max(valid_from);
        let v = Some(value as f32);
        match kind {
            ThresholdKind::FtpW => t.ftp_w = v,
            ThresholdKind::LthrBpm => t.lthr_bpm = v,
            ThresholdKind::HrMaxBpm => t.hr_max_bpm = v,
            ThresholdKind::HrRestBpm => t.hr_rest_bpm = v,
            ThresholdKind::RunSpeedMps => t.run_threshold_speed_mps = v,
            ThresholdKind::CssMps => t.css_mps = v,
        }
    }
    let Some(mut thresholds) = th else {
        return Ok(None);
    };
    thresholds.sex = match get_setting(db, SETTING_ATHLETE_SEX)?.as_deref() {
        Some("male") => Some(Sex::Male),
        Some("female") => Some(Sex::Female),
        _ => None,
    };
    Ok(Some(ThresholdLookup { thresholds, ids }))
}

pub(crate) fn upsert_source_file(db: &Db<'_>, f: &SourceFileRow) -> Result<i64> {
    db.with(
        "INSERT INTO source_file \
             (rel_path, sha256, size_bytes, mtime_utc, imported_at, status, error_code) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
         ON CONFLICT(rel_path) DO UPDATE SET \
             sha256 = excluded.sha256, size_bytes = excluded.size_bytes, \
             mtime_utc = excluded.mtime_utc, imported_at = excluded.imported_at, \
             status = excluded.status, error_code = excluded.error_code \
         RETURNING id",
        |s| {
            s.query_row(
                params![
                    f.rel_path,
                    f.sha256,
                    f.size_bytes,
                    fmt_utc(f.mtime_utc),
                    fmt_utc(f.imported_at),
                    f.status.as_str(),
                    f.error_code
                ],
                |r| r.get(0),
            )
        },
    )
}

pub(crate) fn upsert_activity(db: &Db<'_>, a: &ActivityRow) -> Result<i64> {
    db.with(
        "INSERT INTO activity \
             (source_file_id, sport, sub_sport, start_utc, local_offset_s, local_date, \
              duration_s, moving_s, distance_m, ascent_m, avg_hr, max_hr, avg_power_w, np_w, \
              device_serial, device_product, has_power, has_hr, is_primary, duplicate_of) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
                 ?18, 1, NULL) \
         ON CONFLICT(source_file_id) DO UPDATE SET \
             sport = excluded.sport, sub_sport = excluded.sub_sport, \
             start_utc = excluded.start_utc, local_offset_s = excluded.local_offset_s, \
             local_date = excluded.local_date, duration_s = excluded.duration_s, \
             moving_s = excluded.moving_s, distance_m = excluded.distance_m, \
             ascent_m = excluded.ascent_m, avg_hr = excluded.avg_hr, max_hr = excluded.max_hr, \
             avg_power_w = excluded.avg_power_w, np_w = excluded.np_w, \
             device_serial = excluded.device_serial, device_product = excluded.device_product, \
             has_power = excluded.has_power, has_hr = excluded.has_hr, \
             is_primary = 1, duplicate_of = NULL \
         RETURNING id",
        |s| {
            s.query_row(
                params![
                    a.source_file_id,
                    sport_to_str(a.sport),
                    a.sub_sport,
                    fmt_utc(a.start_utc),
                    a.local_offset_s,
                    fmt_date(a.local_date),
                    a.duration_s,
                    a.moving_s,
                    a.distance_m,
                    a.ascent_m,
                    a.avg_hr,
                    a.max_hr,
                    a.avg_power_w,
                    a.np_w,
                    a.device_serial,
                    a.device_product,
                    i64::from(a.has_power),
                    i64::from(a.has_hr),
                ],
                |r| r.get(0),
            )
        },
    )
}

pub(crate) fn upsert_activity_load(db: &Db<'_>, l: &LoadRow) -> Result<()> {
    db.with(
        "INSERT INTO activity_load \
             (activity_id, method, value, intensity_factor, threshold_id, status, computed_at, \
              missing) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(activity_id, method) DO UPDATE SET \
             value = excluded.value, intensity_factor = excluded.intensity_factor, \
             threshold_id = excluded.threshold_id, status = excluded.status, \
             computed_at = excluded.computed_at, missing = excluded.missing",
        |s| {
            s.execute(params![
                l.activity_id,
                l.method,
                l.value,
                l.intensity_factor,
                l.threshold_id,
                l.status.as_str(),
                fmt_utc(l.computed_at),
                l.missing
            ])
        },
    )?;
    Ok(())
}

/// Entfernt die Aktivität einer Quelldatei samt allem, was an ihr hängt.
pub(crate) fn delete_activity_of_source(db: &Db<'_>, source_file_id: i64) -> Result<()> {
    let id: Option<i64> = db.with("SELECT id FROM activity WHERE source_file_id = ?1", |s| {
        s.query_row([source_file_id], |r| r.get(0)).optional()
    })?;
    let Some(id) = id else { return Ok(()) };
    delete_activity_children(db, id)?;
    // Verweise anderer Aktivitäten lösen, die Duplikatregel setzt sie danach neu.
    db.with(
        "UPDATE activity SET duplicate_of = NULL, is_primary = 1 WHERE duplicate_of = ?1",
        |s| s.execute([id]),
    )?;
    db.with("DELETE FROM activity WHERE id = ?1", |s| s.execute([id]))?;
    Ok(())
}

/// Entfernt alles Abgeleitete einer Aktivität außer ihr selbst.
pub(crate) fn delete_activity_children(db: &Db<'_>, activity_id: i64) -> Result<()> {
    // Feste Texte statt `format!`, damit `Db` die Anweisungen wiederverwenden kann.
    for sql in [
        "DELETE FROM activity_load WHERE activity_id = ?1",
        "DELETE FROM activity_zone_time WHERE activity_id = ?1",
        "DELETE FROM activity_best_power WHERE activity_id = ?1",
        "DELETE FROM activity_best_effort WHERE activity_id = ?1",
    ] {
        db.with(sql, |s| s.execute([activity_id]))?;
    }
    Ok(())
}

/// Leert alle Cache-Tabellen (SPEC 6.3), Kinder vor Eltern wegen der Fremdschlüssel.
pub(crate) fn clear_cache(conn: &Connection) -> Result<()> {
    for table in [
        "activity_best_effort",
        "activity_best_power",
        "activity_zone_time",
        "activity_load",
        "activity",
        "source_file",
    ] {
        conn.execute(&format!("DELETE FROM {table}"), [])?;
    }
    Ok(())
}
