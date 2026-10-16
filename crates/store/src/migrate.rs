//! Schema-Migrationen (SPEC 6.2, 11.4).
//!
//! Jede Migration ist ein nummeriertes SQL-Skript unter `crates/store/migrations/`. Der
//! Stand steht in `PRAGMA user_version`. Eine neue Migration braucht genau eine Datei und
//! einen Eintrag in [`MIGRATIONS`]. Eine Migration löscht niemals Nutzerdaten.

use rusqlite::Connection;

use crate::error::{Result, StoreError};

/// Eine Migration: Zielversion und SQL-Skript.
pub struct Migration {
    pub version: u32,
    pub sql: &'static str,
    /// Die Migration baut eine Tabelle neu, auf die Fremdschlüssel zeigen. Sie läuft dann mit
    /// ausgeschalteten Fremdschlüsseln, danach muss `PRAGMA foreign_key_check` leer sein
    /// (Verfahren der SQLite-Dokumentation zu ALTER TABLE; innerhalb einer Transaktion lässt
    /// sich `foreign_keys` nicht umschalten).
    pub foreign_keys_off: bool,
}

/// Alle Migrationen in aufsteigender Reihenfolge ohne Lücken, beginnend bei 1.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../migrations/0001_initial.sql"),
        foreign_keys_off: false,
    },
    Migration {
        version: 2,
        sql: include_str!("../migrations/0002_zone_time_estimated.sql"),
        foreign_keys_off: false,
    },
    Migration {
        version: 3,
        sql: include_str!("../migrations/0003_recovery_day.sql"),
        foreign_keys_off: false,
    },
    Migration {
        version: 4,
        sql: include_str!("../migrations/0004_recovery_garmin.sql"),
        foreign_keys_off: false,
    },
    Migration {
        version: 5,
        sql: include_str!("../migrations/0005_cache_meta.sql"),
        foreign_keys_off: false,
    },
    Migration {
        version: 6,
        sql: include_str!("../migrations/0006_best_effort.sql"),
        foreign_keys_off: false,
    },
    Migration {
        version: 7,
        sql: include_str!("../migrations/0007_garmin_full.sql"),
        foreign_keys_off: true,
    },
    Migration {
        version: 8,
        sql: include_str!("../migrations/0008_garmin_more.sql"),
        foreign_keys_off: false,
    },
];

/// Neueste Schemaversion dieses Programms.
pub fn latest_version() -> u32 {
    MIGRATIONS.last().map_or(0, |m| m.version)
}

/// Eine Migration in einer Transaktion. Mit `check_keys` muss `foreign_key_check` vor dem
/// Abschluss leer sein, sonst bleibt der alte Stand.
fn run_one(conn: &mut Connection, m: &Migration, check_keys: bool) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch(m.sql)?;
    if check_keys {
        let broken: i64 =
            tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })?;
        if broken > 0 {
            return Err(StoreError::InvalidData(format!(
                "migration {} leaves {broken} broken references",
                m.version
            )));
        }
    }
    tx.pragma_update(None, "user_version", m.version)?;
    tx.commit()?;
    Ok(())
}

/// Liest `PRAGMA user_version`.
pub fn current_version(conn: &Connection) -> Result<u32> {
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    u32::try_from(v).map_err(|_| StoreError::InvalidData(format!("user_version {v}")))
}

/// Hebt die Datenbank auf die neueste Version. Idempotent: ein zweiter Aufruf tut nichts.
pub fn migrate(conn: &mut Connection) -> Result<()> {
    migrate_with(conn, MIGRATIONS)
}

/// Wie [`migrate`] mit einer beliebigen Liste, damit sich der Mechanismus mit Attrappen
/// prüfen lässt.
pub(crate) fn migrate_with(conn: &mut Connection, migrations: &[Migration]) -> Result<()> {
    let latest = migrations.last().map_or(0, |m| m.version);
    let found = current_version(conn)?;
    if found > latest {
        return Err(StoreError::SchemaTooNew {
            found,
            supported: latest,
        });
    }
    for m in migrations.iter().filter(|m| m.version > found) {
        // Eine Transaktion je Migration: bricht ein Skript ab, bleibt der alte Stand
        // erhalten. `user_version` ändert sich innerhalb der Transaktion mit.
        if m.foreign_keys_off {
            let enabled: bool = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
            conn.pragma_update(None, "foreign_keys", false)?;
            let result = run_one(conn, m, true);
            if enabled {
                conn.pragma_update(None, "foreign_keys", true)?;
            }
            result?;
        } else {
            run_one(conn, m, false)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn migrations_are_numbered_without_gaps() {
        for (i, m) in MIGRATIONS.iter().enumerate() {
            assert_eq!(m.version as usize, i + 1);
        }
    }

    #[test]
    fn empty_database_is_raised_from_0_to_latest() {
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(current_version(&conn).unwrap(), 0);
        assert!(tables(&conn).is_empty());
        migrate(&mut conn).unwrap();
        assert_eq!(current_version(&conn).unwrap(), latest_version());
        let t = tables(&conn);
        for expected in [
            "source_file",
            "activity",
            "activity_load",
            "activity_zone_time",
            "activity_best_power",
            "threshold",
            "goal",
            "availability",
            "plan",
            "manual_session",
            "recovery_day",
            "setting",
            "cache_meta",
            "activity_best_effort",
            "wellness_day",
            "training_day",
            "garmin_activity_load",
            "sensitive_day",
            "body_day",
            "bp_day",
            "nutrition_day",
        ] {
            assert!(t.iter().any(|n| n == expected), "{expected} fehlt: {t:?}");
        }
    }

    #[test]
    fn migrating_twice_changes_nothing() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO setting (key, value) VALUES ('k', 'v')", [])
            .unwrap();
        migrate(&mut conn).unwrap();
        let v: String = conn
            .query_row("SELECT value FROM setting WHERE key = 'k'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, "v");
        assert_eq!(current_version(&conn).unwrap(), latest_version());
    }

    #[test]
    fn newer_database_is_refused() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", latest_version() + 1)
            .unwrap();
        let err = migrate(&mut conn).unwrap_err();
        assert!(matches!(err, StoreError::SchemaTooNew { .. }));
    }

    /// N-1 nach N: Eine Datenbank auf dem Stand der Migration 1 mit Nutzerdaten wird mit einer
    /// zweiten (Attrappen-)Migration gehoben, ohne dass Daten verloren gehen. So prüft der
    /// Test den Mechanismus, den jede künftige Migration N nutzt.
    #[test]
    fn step_from_n_minus_1_to_n_keeps_user_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO threshold (sport, kind, value, valid_from, origin) \
             VALUES ('cycling', 'ftp_w', 250, '2026-01-01', 'manual')",
            [],
        )
        .unwrap();
        let n = latest_version() + 1;
        let mut list: Vec<Migration> = MIGRATIONS
            .iter()
            .map(|m| Migration {
                version: m.version,
                sql: m.sql,
                foreign_keys_off: m.foreign_keys_off,
            })
            .collect();
        list.push(Migration {
            version: n,
            sql: "ALTER TABLE goal ADD COLUMN note TEXT;",
            foreign_keys_off: false,
        });
        migrate_with(&mut conn, &list).unwrap();
        assert_eq!(current_version(&conn).unwrap(), n);
        let ftp: f64 = conn
            .query_row("SELECT value FROM threshold", [], |r| r.get(0))
            .unwrap();
        assert!((ftp - 250.0).abs() < f64::EPSILON);
        conn.execute(
            "INSERT INTO goal (kind, note) VALUES ('continuous', 'x')",
            [],
        )
        .unwrap();
    }

    /// Migration 2 (T05): Stand 1 mit Nutzerdaten und Cache-Zeilen wird gehoben. Die neue
    /// Spalte `estimated` ist für vorhandene Zeilen 0, Schwellenwerte bleiben erhalten.
    #[test]
    fn migration_2_adds_the_estimated_column_and_keeps_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..1]).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 1);
        for sql in [
            "INSERT INTO threshold (sport, kind, value, valid_from, origin) \
             VALUES ('cycling', 'ftp_w', 250, '2026-01-01', 'manual')",
            "INSERT INTO source_file (rel_path, sha256, size_bytes, mtime_utc, imported_at, status) \
             VALUES ('a.fit', 'x', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'ok')",
            "INSERT INTO activity (source_file_id, sport, start_utc, local_offset_s, local_date, \
                                   duration_s) \
             VALUES (1, 'cycling', '2026-01-01T00:00:00Z', 0, '2026-01-01', 60)",
            "INSERT INTO activity_zone_time (activity_id, scheme, zone, seconds) \
             VALUES (1, 'power7', 3, 60)",
        ] {
            conn.execute(sql, []).unwrap();
        }
        migrate(&mut conn).unwrap();
        assert_eq!(current_version(&conn).unwrap(), latest_version());
        let (seconds, estimated): (i64, i64) = conn
            .query_row(
                "SELECT seconds, estimated FROM activity_zone_time",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((seconds, estimated), (60, 0));
        let ftp: f64 = conn
            .query_row("SELECT value FROM threshold", [], |r| r.get(0))
            .unwrap();
        assert!((ftp - 250.0).abs() < f64::EPSILON);
    }

    /// Migration 3 (T14): Stand 2 mit Nutzerdaten und Cache-Zeilen wird gehoben. Die neue
    /// Tabelle `recovery_day` ist leer, nichts Vorhandenes geht verloren.
    #[test]
    fn migration_3_adds_recovery_day_and_keeps_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..2]).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 2);
        assert!(!tables(&conn).iter().any(|n| n == "recovery_day"));
        for sql in [
            "INSERT INTO threshold (sport, kind, value, valid_from, origin) \
             VALUES ('cycling', 'ftp_w', 250, '2026-01-01', 'manual')",
            "INSERT INTO setting (key, value) VALUES ('athlete.sex', 'male')",
            "INSERT INTO source_file (rel_path, sha256, size_bytes, mtime_utc, imported_at, status) \
             VALUES ('a.fit', 'x', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'ok')",
        ] {
            conn.execute(sql, []).unwrap();
        }
        migrate_with(&mut conn, &MIGRATIONS[..3]).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 3);
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM recovery_day", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0);
        for (table, expected) in [("threshold", 1), ("setting", 1), ("source_file", 1)] {
            let n: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, expected, "{table}");
        }
        // Spalten wie in SPEC 6.2.
        let mut stmt = conn.prepare("PRAGMA table_info(recovery_day)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get(1))
            .unwrap()
            .map(|c| c.unwrap())
            .collect();
        assert_eq!(
            cols,
            [
                "local_date",
                "hrv_overnight_ms",
                "resting_hr_bpm",
                "sleep_duration_s",
                "sleep_score",
                "source",
                "imported_at"
            ]
        );
    }

    /// Migration 4 (T22): Stand 3 mit einer Erholungszeile wird gehoben. Die Zeile behält alle
    /// Werte, die zwei neuen Spalten sind leer.
    #[test]
    fn migration_4_adds_body_battery_and_keeps_recovery_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..3]).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 3);
        conn.execute(
            "INSERT INTO recovery_day (local_date, hrv_overnight_ms, resting_hr_bpm, \
                                       sleep_duration_s, sleep_score, source, imported_at) \
             VALUES ('2026-10-10', 61.5, 48, 27000, 82, 'garmin-bridge 0.1.0', \
                     '2026-10-11T00:00:00Z')",
            [],
        )
        .unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..4]).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 4);
        let row: (f64, f64, i64, i64, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT hrv_overnight_ms, resting_hr_bpm, sleep_duration_s, sleep_score, \
                        body_battery_high, body_battery_low FROM recovery_day",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(row, (61.5, 48.0, 27_000, 82, None, None));
    }

    #[test]
    fn failing_migration_leaves_the_old_state() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let n = latest_version() + 1;
        let mut list: Vec<Migration> = MIGRATIONS
            .iter()
            .map(|m| Migration {
                version: m.version,
                sql: m.sql,
                foreign_keys_off: m.foreign_keys_off,
            })
            .collect();
        list.push(Migration {
            version: n,
            sql: "CREATE TABLE half (a); SELECT * FROM does_not_exist;",
            foreign_keys_off: false,
        });
        assert!(migrate_with(&mut conn, &list).is_err());
        assert_eq!(current_version(&conn).unwrap(), latest_version());
        assert!(!tables(&conn).iter().any(|t| t == "half"));
    }

    /// Migration 7 (T39): Stand 6 mit Schwellen, auf die Belastungen verweisen, wird gehoben.
    /// `threshold` ist neu angelegt, kennt `garmin`, behält IDs und Verweise; die neuen Tabellen
    /// sind leer, `activity_load.missing` ist leer, Fremdschlüssel sind danach wieder an.
    #[test]
    fn migration_7_rebuilds_threshold_and_keeps_references() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..6]).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 6);
        for sql in [
            "INSERT INTO threshold (id, sport, kind, value, valid_from, origin, note) \
             VALUES (7, 'cycling', 'ftp_w', 250, '2026-01-01', 'manual', 'Test')",
            "INSERT INTO source_file (rel_path, sha256, size_bytes, mtime_utc, imported_at, status) \
             VALUES ('a.fit', 'x', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 'ok')",
            "INSERT INTO activity (source_file_id, sport, start_utc, local_offset_s, local_date, \
                                   duration_s) \
             VALUES (1, 'cycling', '2026-01-02T00:00:00Z', 0, '2026-01-02', 60)",
            "INSERT INTO activity_load (activity_id, method, value, threshold_id, status, \
                                        computed_at) \
             VALUES (1, 'tss', 50, 7, 'ok', '2026-01-02T00:00:00Z')",
            "INSERT INTO recovery_day (local_date, resting_hr_bpm, source, imported_at) \
             VALUES ('2026-01-02', 50, 'garmin-app', '2026-01-02T00:00:00Z')",
        ] {
            conn.execute(sql, []).unwrap();
        }
        // Vor der Migration ist `garmin` keine erlaubte Herkunft.
        assert!(
            conn.execute(
                "INSERT INTO threshold (sport, kind, value, valid_from, origin) \
                 VALUES ('all', 'hr_max_bpm', 190, '2026-01-01', 'garmin')",
                [],
            )
            .is_err()
        );

        migrate_with(&mut conn, &MIGRATIONS[..7]).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 7);
        let fk: bool = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert!(fk);
        let (id, value, note): (i64, f64, String) = conn
            .query_row("SELECT id, value, note FROM threshold", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((id, note.as_str()), (7, "Test"));
        assert!((value - 250.0).abs() < f64::EPSILON);
        let (tid, missing): (i64, Option<String>) = conn
            .query_row("SELECT threshold_id, missing FROM activity_load", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((tid, missing), (7, None));
        conn.execute(
            "INSERT INTO threshold (sport, kind, value, valid_from, origin) \
             VALUES ('all', 'hr_max_bpm', 190, '2026-01-01', 'garmin')",
            [],
        )
        .unwrap();
        // Der Fremdschlüssel greift wieder: ein Verweis ins Leere scheitert.
        assert!(
            conn.execute("UPDATE activity_load SET threshold_id = 999", [])
                .is_err()
        );
        for table in [
            "wellness_day",
            "training_day",
            "garmin_activity_load",
            "sensitive_day",
        ] {
            let n: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 0, "{table}");
        }
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM recovery_day", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let idx: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'threshold_lookup'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx, 1);
    }

    /// Migration 8 (T40): Stand 7 mit Nutzerdaten wird gehoben. Bestehende Zeilen bleiben mit
    /// allen Werten, die neuen Spalten sind leer, die neuen Tabellen leer.
    #[test]
    fn migration_8_adds_columns_and_keeps_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..7]).unwrap();
        for sql in [
            "INSERT INTO wellness_day (local_date, steps, source, imported_at) \
             VALUES ('2026-10-01', 9000, 'garmin-app', '2026-10-02T00:00:00Z')",
            "INSERT INTO training_day (local_date, vo2max_running, source, imported_at) \
             VALUES ('2026-10-01', 52.5, 'garmin-app', '2026-10-02T00:00:00Z')",
            "INSERT INTO sensitive_day (local_date, hydration_ml, source, imported_at) \
             VALUES ('2026-10-01', 2000, 'garmin-app', '2026-10-02T00:00:00Z')",
            "INSERT INTO threshold (sport, kind, value, valid_from, origin) \
             VALUES ('cycling', 'ftp_w', 250, '2026-01-01', 'garmin')",
        ] {
            conn.execute(sql, []).unwrap();
        }
        migrate_with(&mut conn, &MIGRATIONS[..8]).unwrap();
        assert_eq!(current_version(&conn).unwrap(), 8);
        let (steps, floors): (i64, Option<f64>) = conn
            .query_row("SELECT steps, floors_up FROM wellness_day", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((steps, floors), (9000, None));
        let (vo2, status): (f64, Option<String>) = conn
            .query_row(
                "SELECT vo2max_running, training_status FROM training_day",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!((vo2 - 52.5).abs() < f64::EPSILON);
        assert_eq!(status, None);
        for (table, rows) in [
            ("sensitive_day", 1),
            ("threshold", 1),
            ("body_day", 0),
            ("bp_day", 0),
            ("nutrition_day", 0),
        ] {
            let n: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, rows, "{table}");
        }
    }
}
