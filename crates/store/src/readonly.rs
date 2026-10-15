//! Schreibgeschützter Zugang für `lastwerk-mcp` (T10, SPEC 5.4, 9.6).
//!
//! Der MCP-Server darf nichts ändern. Deshalb öffnet [`Store::open_read_only`] die Datei mit
//! `SQLITE_OPEN_READ_ONLY` und setzt zusätzlich `PRAGMA query_only`. Anders als
//! [`Store::open`] legt es keine Datei an und führt keine Migration aus: Eine Datenbank mit
//! anderem Schema wird abgelehnt, statt sie zu verändern.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::error::{Result, StoreError};
use crate::migrate;
use crate::store::Store;

impl Store {
    /// Öffnet eine vorhandene Datenbank nur zum Lesen.
    ///
    /// - `SQLITE_OPEN_READ_ONLY` ohne `CREATE`: eine fehlende Datei ist ein Fehler.
    /// - `PRAGMA query_only = ON` als zweite Absicherung: jede schreibende Anweisung
    ///   scheitert, auch wenn jemand die Verbindung später anders öffnen würde.
    /// - Keine Migration. Ist das Schema älter als dieses Programm
    ///   ([`migrate::latest_version`]), meldet die Methode das, statt die Datei zu ändern.
    ///   Neuer ist ebenfalls ein Fehler ([`StoreError::SchemaTooNew`]).
    pub fn open_read_only(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.pragma_update(None, "query_only", true)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        let found = migrate::current_version(&conn)?;
        let supported = migrate::latest_version();
        if found > supported {
            return Err(StoreError::SchemaTooNew { found, supported });
        }
        if found < supported {
            return Err(StoreError::InvalidData(format!(
                "database schema version {found} is older than {supported}, \
                 open it once with lastwerk to migrate it"
            )));
        }
        Ok(Self { conn })
    }
}
