//! Verbindung mit einem kleinen Zwischenspeicher für vorbereitete Anweisungen.
//!
//! `Connection::execute` und `Connection::query_row` bereiten jede Anweisung neu vor. Beim
//! Import laufen je Aktivität rund ein Dutzend Anweisungen, das Vorbereiten kostete bei
//! einer Historie von 1500 Dateien etwa ein Viertel der Importzeit (T38, Anhang Messung).
//! rusqlite bringt einen eigenen Zwischenspeicher nur mit einer zusätzlichen Abhängigkeit
//! mit (Feature `cache`), deshalb dieser hier: eine kurze Liste je Lauf, Schlüssel ist der
//! feste SQL-Text.

use std::cell::RefCell;
use std::ops::Deref;

use rusqlite::{Connection, Statement};

use crate::error::Result;

/// Verbindung plus vorbereitete Anweisungen. Lebt so lange wie ein Lauf (Import,
/// Neuberechnung) oder ein einzelner Aufruf; Anweisungen werden beim Ende freigegeben.
pub(crate) struct Db<'c> {
    conn: &'c Connection,
    cache: RefCell<Vec<(&'static str, Statement<'c>)>>,
}

impl<'c> Db<'c> {
    pub(crate) fn new(conn: &'c Connection) -> Self {
        Self {
            conn,
            cache: RefCell::new(Vec::new()),
        }
    }

    /// Führt `f` mit der vorbereiteten Anweisung zu `sql` aus. Die Anweisung wird beim ersten
    /// Aufruf vorbereitet und danach wiederverwendet. Während `f` läuft, ist sie aus der
    /// Liste genommen; ein verschachtelter Aufruf mit demselben Text bereitet eine zweite vor.
    pub(crate) fn with<R>(
        &self,
        sql: &'static str,
        f: impl FnOnce(&mut Statement<'c>) -> rusqlite::Result<R>,
    ) -> Result<R> {
        let cached = {
            let mut cache = self.cache.borrow_mut();
            cache
                .iter()
                .position(|(s, _)| *s == sql)
                .map(|i| cache.swap_remove(i).1)
        };
        let mut stmt = match cached {
            Some(stmt) => stmt,
            None => self.conn.prepare(sql)?,
        };
        let result = f(&mut stmt);
        self.cache.borrow_mut().push((sql, stmt));
        Ok(result?)
    }
}

impl Deref for Db<'_> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_statements_and_keeps_results() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (v INTEGER)", []).unwrap();
        let db = Db::new(&conn);
        for v in 0..5 {
            db.with("INSERT INTO t (v) VALUES (?1)", |s| s.execute([v]))
                .unwrap();
        }
        assert_eq!(db.cache.borrow().len(), 1);
        let sum: i64 = db
            .with("SELECT SUM(v) FROM t", |s| s.query_row([], |r| r.get(0)))
            .unwrap();
        // 0 + 1 + 2 + 3 + 4 = 10
        assert_eq!(sum, 10);
        assert_eq!(db.cache.borrow().len(), 2);
    }

    #[test]
    fn nested_use_of_the_same_text_works() {
        let conn = Connection::open_in_memory().unwrap();
        let db = Db::new(&conn);
        let sql = "SELECT ?1 + 1";
        let v: i64 = db
            .with(sql, |outer| {
                let inner: i64 = db.with(sql, |s| s.query_row([1], |r| r.get(0))).unwrap();
                outer.query_row([inner], |r| r.get(0))
            })
            .unwrap();
        assert_eq!(v, 3);
    }

    #[test]
    fn errors_keep_the_statement_usable() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (v INTEGER NOT NULL)", [])
            .unwrap();
        let db = Db::new(&conn);
        let sql = "INSERT INTO t (v) VALUES (?1)";
        assert!(db.with(sql, |s| s.execute([None::<i64>])).is_err());
        assert_eq!(db.with(sql, |s| s.execute([7])).unwrap(), 1);
    }
}
