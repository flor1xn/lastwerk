//! Schreibgeschützter Zugang (T10, SPEC 5.4: `open_read_only`). Alle Daten sind synthetisch.

mod common;

use common::work_dir;
use lastwerk_store::{Store, StoreError};
use rusqlite::Connection;

#[test]
fn read_only_connection_reads_but_rejects_writes() {
    let dir = work_dir("readonly-rw");
    let path = dir.join("a.db");
    {
        let mut store = Store::open(&path).unwrap();
        store.set_setting("k", "v").unwrap();
    }
    let mut ro = Store::open_read_only(&path).unwrap();
    assert_eq!(ro.get_setting("k").unwrap().as_deref(), Some("v"));
    // Ein Schreibzugriff über die öffentliche API scheitert mit "readonly".
    let err = ro.set_setting("x", "y").unwrap_err();
    assert!(err.to_string().to_lowercase().contains("readonly"), "{err}");
    assert_eq!(ro.get_setting("x").unwrap(), None);
    // Auch mit offener Schreibverbindung daneben bleibt der Lesezugang lesend.
    let mut writer = Store::open(&path).unwrap();
    writer.set_setting("k", "w").unwrap();
    assert_eq!(ro.get_setting("k").unwrap().as_deref(), Some("w"));
}

#[test]
fn missing_file_is_an_error_and_is_not_created() {
    let dir = work_dir("readonly-missing");
    let path = dir.join("missing.db");
    assert!(Store::open_read_only(&path).is_err());
    assert!(!path.exists());
}

#[test]
fn unmigrated_database_is_rejected_without_change() {
    let dir = work_dir("readonly-old");
    let path = dir.join("old.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE setting (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
    }
    let err = Store::open_read_only(&path).err().unwrap();
    assert!(matches!(err, StoreError::InvalidData(_)), "{err}");
    let conn = Connection::open(&path).unwrap();
    let v: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, 0);
}

#[test]
fn newer_schema_is_rejected() {
    let dir = work_dir("readonly-new");
    let path = dir.join("new.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
    }
    let err = Store::open_read_only(&path).err().unwrap();
    assert!(matches!(err, StoreError::SchemaTooNew { found: 99, .. }));
}
