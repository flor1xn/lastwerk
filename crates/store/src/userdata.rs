//! Export und Import der Nutzerdaten als JSON (SPEC 6.1, 6.3).
//!
//! Nutzerdaten sind nicht aus FIT ableitbar: `threshold`, `goal`, `availability`, `plan`,
//! `manual_session`, `setting`, `recovery_day` (seit T14), `wellness_day`, `training_day`,
//! `garmin_activity_load` und `sensitive_day` (seit T39). Der Cache gehört nicht dazu.
//!
//! `recovery_day` hält Gesundheitswerte (HRV, Ruhepuls, Schlaf). Die Sicherung enthält sie,
//! damit nach einer Wiederherstellung nichts fehlt, und gehört wie die ganze Sicherung nicht
//! ins Repository. Eine Sicherung ohne den Schlüssel `recovery_days` (vor T14) gilt als leere
//! Liste und leert die Tabelle beim Einspielen. Die Werte lassen sich mit `import-recovery`
//! aus `exchange/out/recovery` neu einlesen.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::error::{Result, StoreError};
use crate::garmin_data::{
    GarminActivityLoad, SensitiveDay, TrainingDay, WellnessDay, export_table, import_table,
};
use crate::store::Store;

/// Version des Backup-Formats (`backup_version`). Wird erhöht, wenn sich Felder ändern. Der
/// Import lehnt jede andere Version ab (SPEC 6.3).
pub const USER_DATA_FORMAT_VERSION: u32 = 1;

/// Zeile von `threshold`. Datum und Art als Text wie in der Datenbank, damit der Export
/// beim Einspielen nichts umdeutet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThresholdExport {
    pub id: i64,
    pub sport: String,
    pub kind: String,
    pub value: f64,
    pub valid_from: String,
    pub origin: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalExport {
    pub id: i64,
    pub kind: String,
    pub sport: Option<String>,
    pub event_date: Option<String>,
    pub label: Option<String>,
    pub priority: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AvailabilityExport {
    pub weekday: i64,
    pub max_minutes: i64,
    pub allow_hard: bool,
    pub fixed_rest: bool,
}

/// Zeile von `plan`. `json` bleibt ein Text, damit der Plan unverändert zurückkommt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanExport {
    pub id: String,
    pub created_at: String,
    pub goal_id: Option<i64>,
    pub source: String,
    pub rules_version: String,
    pub locale: String,
    pub json: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualSessionExport {
    pub id: i64,
    pub local_date: String,
    pub sport: String,
    pub duration_s: i64,
    pub rpe: Option<i64>,
}

/// Zeile von `recovery_day`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryDayExport {
    pub local_date: String,
    pub hrv_overnight_ms: Option<f64>,
    pub resting_hr_bpm: Option<f64>,
    pub sleep_duration_s: Option<i64>,
    pub sleep_score: Option<i64>,
    /// Seit T22. Fehlt der Schlüssel in einer älteren Sicherung, gilt der Wert als leer.
    #[serde(default)]
    pub body_battery_high: Option<i64>,
    #[serde(default)]
    pub body_battery_low: Option<i64>,
    pub source: String,
    pub imported_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingExport {
    pub key: String,
    pub value: String,
}

/// Alle Nutzerdaten, sortiert nach Schlüssel, damit zwei Exporte gleicher Daten gleich sind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserDataExport {
    /// Version des Formats. Der Name `format_version` aus T04 wird beim Lesen weiter erkannt.
    #[serde(alias = "format_version")]
    pub backup_version: u32,
    /// Zeitpunkt des Exports (UTC, RFC 3339), vom Aufrufer gesetzt, weil der Speicher keine
    /// Uhr liest (siehe [`UserDataExport::with_exported_at`]). Ohne Angabe `null`.
    #[serde(default)]
    pub exported_at: Option<String>,
    pub thresholds: Vec<ThresholdExport>,
    pub goals: Vec<GoalExport>,
    pub availability: Vec<AvailabilityExport>,
    pub plans: Vec<PlanExport>,
    pub manual_sessions: Vec<ManualSessionExport>,
    /// Erholungstage (seit T14), nach Datum sortiert. Fehlt der Schlüssel, gilt die Liste als
    /// leer.
    #[serde(default)]
    pub recovery_days: Vec<RecoveryDayExport>,
    /// Tageswerte aus Garmin (seit T39, S2). Fehlt der Schlüssel, gilt die Liste als leer. Je
    /// Zeile ein Objekt mit `local_date`, den Spalten aus 6.2, `source` und `imported_at`.
    #[serde(default)]
    pub wellness_days: Vec<serde_json::Map<String, serde_json::Value>>,
    /// Trainingswerte aus Garmin (seit T39, S2).
    #[serde(default)]
    pub training_days: Vec<serde_json::Map<String, serde_json::Value>>,
    /// Garmins Last je Aktivität (seit T39, nur Anzeige).
    #[serde(default)]
    pub garmin_activity_loads: Vec<serde_json::Map<String, serde_json::Value>>,
    /// S3-Werte (seit T39, nur nach Freigabe geholt). Die Sicherung enthält sie wie alle
    /// Gesundheitswerte, damit nach einer Wiederherstellung nichts fehlt; sie verlässt den
    /// Rechner nur durch den Nutzer.
    #[serde(default)]
    pub sensitive_days: Vec<serde_json::Map<String, serde_json::Value>>,
    pub settings: Vec<SettingExport>,
}

impl UserDataExport {
    /// Setzt den Zeitpunkt des Exports. Die CLI übergibt ihre Uhr, Tests einen festen Wert.
    #[must_use]
    pub fn with_exported_at(mut self, at: chrono::DateTime<chrono::Utc>) -> Self {
        self.exported_at = Some(at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        self
    }

    /// JSON-Text, eingerückt.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}

fn collect<T>(
    conn: &Connection,
    sql: &str,
    f: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map([], f)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

impl Store {
    /// Liest alle Nutzertabellen (SPEC 5.4).
    pub fn export_user_data(&self) -> Result<UserDataExport> {
        let c = &self.conn;
        Ok(UserDataExport {
            backup_version: USER_DATA_FORMAT_VERSION,
            exported_at: None,
            thresholds: collect(
                c,
                "SELECT id, sport, kind, value, valid_from, origin, note FROM threshold \
                 ORDER BY id",
                |r| {
                    Ok(ThresholdExport {
                        id: r.get(0)?,
                        sport: r.get(1)?,
                        kind: r.get(2)?,
                        value: r.get(3)?,
                        valid_from: r.get(4)?,
                        origin: r.get(5)?,
                        note: r.get(6)?,
                    })
                },
            )?,
            goals: collect(
                c,
                "SELECT id, kind, sport, event_date, label, priority, active FROM goal \
                 ORDER BY id",
                |r| {
                    Ok(GoalExport {
                        id: r.get(0)?,
                        kind: r.get(1)?,
                        sport: r.get(2)?,
                        event_date: r.get(3)?,
                        label: r.get(4)?,
                        priority: r.get(5)?,
                        active: r.get::<_, i64>(6)? != 0,
                    })
                },
            )?,
            availability: collect(
                c,
                "SELECT weekday, max_minutes, allow_hard, fixed_rest FROM availability \
                 ORDER BY weekday",
                |r| {
                    Ok(AvailabilityExport {
                        weekday: r.get(0)?,
                        max_minutes: r.get(1)?,
                        allow_hard: r.get::<_, i64>(2)? != 0,
                        fixed_rest: r.get::<_, i64>(3)? != 0,
                    })
                },
            )?,
            plans: collect(
                c,
                "SELECT id, created_at, goal_id, source, rules_version, locale, json, status \
                 FROM plan ORDER BY id",
                |r| {
                    Ok(PlanExport {
                        id: r.get(0)?,
                        created_at: r.get(1)?,
                        goal_id: r.get(2)?,
                        source: r.get(3)?,
                        rules_version: r.get(4)?,
                        locale: r.get(5)?,
                        json: r.get(6)?,
                        status: r.get(7)?,
                    })
                },
            )?,
            manual_sessions: collect(
                c,
                "SELECT id, local_date, sport, duration_s, rpe FROM manual_session ORDER BY id",
                |r| {
                    Ok(ManualSessionExport {
                        id: r.get(0)?,
                        local_date: r.get(1)?,
                        sport: r.get(2)?,
                        duration_s: r.get(3)?,
                        rpe: r.get(4)?,
                    })
                },
            )?,
            recovery_days: collect(
                c,
                "SELECT local_date, hrv_overnight_ms, resting_hr_bpm, sleep_duration_s, \
                        sleep_score, body_battery_high, body_battery_low, source, imported_at \
                 FROM recovery_day ORDER BY local_date",
                |r| {
                    Ok(RecoveryDayExport {
                        local_date: r.get(0)?,
                        hrv_overnight_ms: r.get(1)?,
                        resting_hr_bpm: r.get(2)?,
                        sleep_duration_s: r.get(3)?,
                        sleep_score: r.get(4)?,
                        body_battery_high: r.get(5)?,
                        body_battery_low: r.get(6)?,
                        source: r.get(7)?,
                        imported_at: r.get(8)?,
                    })
                },
            )?,
            wellness_days: export_table::<WellnessDay>(c)?,
            training_days: export_table::<TrainingDay>(c)?,
            garmin_activity_loads: export_table::<GarminActivityLoad>(c)?,
            sensitive_days: export_table::<SensitiveDay>(c)?,
            settings: collect(c, "SELECT key, value FROM setting ORDER BY key", |r| {
                Ok(SettingExport {
                    key: r.get(0)?,
                    value: r.get(1)?,
                })
            })?,
        })
    }

    /// Spielt Nutzerdaten ein und ersetzt dabei den Inhalt der Nutzertabellen vollständig
    /// (Rekonstruktion nach Datenverlust, SPEC 6.3). Alles in einer Transaktion: bei einem
    /// Fehler bleibt der alte Stand. Der Cache bleibt unberührt. Verweise des Caches auf
    /// Schwellen, die der Export nicht enthält, werden gelöst (`threshold_id` NULL).
    pub fn import_user_data(&mut self, e: &UserDataExport) -> Result<()> {
        if e.backup_version != USER_DATA_FORMAT_VERSION {
            return Err(StoreError::UnsupportedExportVersion(e.backup_version));
        }
        let tx = self.conn.transaction()?;
        // Löschen und Wiedereinfügen mit denselben IDs: die Fremdschlüssel (plan.goal_id,
        // activity_load.threshold_id) werden erst beim Abschluss geprüft.
        tx.pragma_update(None, "defer_foreign_keys", true)?;
        for table in [
            "plan",
            "goal",
            "threshold",
            "availability",
            "manual_session",
            "recovery_day",
            "wellness_day",
            "training_day",
            "garmin_activity_load",
            "sensitive_day",
            "setting",
        ] {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        for t in &e.thresholds {
            tx.execute(
                "INSERT INTO threshold (id, sport, kind, value, valid_from, origin, note) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    t.id,
                    t.sport,
                    t.kind,
                    t.value,
                    t.valid_from,
                    t.origin,
                    t.note
                ],
            )?;
        }
        for g in &e.goals {
            tx.execute(
                "INSERT INTO goal (id, kind, sport, event_date, label, priority, active) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    g.id,
                    g.kind,
                    g.sport,
                    g.event_date,
                    g.label,
                    g.priority,
                    i64::from(g.active)
                ],
            )?;
        }
        for a in &e.availability {
            tx.execute(
                "INSERT INTO availability (weekday, max_minutes, allow_hard, fixed_rest) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    a.weekday,
                    a.max_minutes,
                    i64::from(a.allow_hard),
                    i64::from(a.fixed_rest)
                ],
            )?;
        }
        for p in &e.plans {
            tx.execute(
                "INSERT INTO plan (id, created_at, goal_id, source, rules_version, locale, \
                                   json, status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    p.id,
                    p.created_at,
                    p.goal_id,
                    p.source,
                    p.rules_version,
                    p.locale,
                    p.json,
                    p.status
                ],
            )?;
        }
        for m in &e.manual_sessions {
            tx.execute(
                "INSERT INTO manual_session (id, local_date, sport, duration_s, rpe) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![m.id, m.local_date, m.sport, m.duration_s, m.rpe],
            )?;
        }
        for d in &e.recovery_days {
            // Später liest `recovery_days` Datum und Zeitpunkt als Text zurück. Ein Wert, der
            // sich nicht lesen lässt, gehört nicht in die Tabelle. Die Wertebereiche prüft
            // dieselbe Regel wie beim Import (`RecoveryDayRecord::check`), sonst gelangen
            // unplausible Werte über eine Sicherung zu MCP und R12.
            let date = crate::rows::parse_date(&d.local_date);
            let imported_at = crate::rows::parse_utc(&d.imported_at);
            let (Ok(date), Ok(imported_at)) = (date, imported_at) else {
                return Err(StoreError::InvalidInput(
                    "recovery day with an unreadable date".into(),
                ));
            };
            let out_of_range =
                || StoreError::InvalidInput("recovery day value out of range".into());
            let record = crate::recovery::RecoveryDayRecord {
                date,
                hrv_overnight_ms: d.hrv_overnight_ms,
                resting_hr_bpm: d.resting_hr_bpm,
                sleep_duration_s: d
                    .sleep_duration_s
                    .map(u32::try_from)
                    .transpose()
                    .map_err(|_| out_of_range())?,
                sleep_score: d
                    .sleep_score
                    .map(u8::try_from)
                    .transpose()
                    .map_err(|_| out_of_range())?,
                body_battery_high: d
                    .body_battery_high
                    .map(u8::try_from)
                    .transpose()
                    .map_err(|_| out_of_range())?,
                body_battery_low: d
                    .body_battery_low
                    .map(u8::try_from)
                    .transpose()
                    .map_err(|_| out_of_range())?,
                source: d.source.clone(),
                imported_at,
            };
            record.check()?;
            tx.execute(
                "INSERT INTO recovery_day (local_date, hrv_overnight_ms, resting_hr_bpm, \
                                           sleep_duration_s, sleep_score, body_battery_high, \
                                           body_battery_low, source, imported_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    d.local_date,
                    d.hrv_overnight_ms,
                    d.resting_hr_bpm,
                    d.sleep_duration_s,
                    d.sleep_score,
                    d.body_battery_high,
                    d.body_battery_low,
                    d.source,
                    d.imported_at
                ],
            )?;
        }
        import_table::<WellnessDay>(&tx, &e.wellness_days)?;
        import_table::<TrainingDay>(&tx, &e.training_days)?;
        import_table::<GarminActivityLoad>(&tx, &e.garmin_activity_loads)?;
        import_table::<SensitiveDay>(&tx, &e.sensitive_days)?;
        for s in &e.settings {
            tx.execute(
                "INSERT INTO setting (key, value) VALUES (?1, ?2)",
                params![s.key, s.value],
            )?;
        }
        tx.execute(
            "UPDATE activity_load SET threshold_id = NULL \
             WHERE threshold_id IS NOT NULL AND threshold_id NOT IN (SELECT id FROM threshold)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
}
