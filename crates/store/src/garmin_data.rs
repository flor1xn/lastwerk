//! Daten aus dem Vollabzug von Garmin (T39, SPEC 5.6, 6.2, 8.6).
//!
//! - Tageswerte (`wellness_day`, S2), Trainingswerte (`training_day`, S2), Garmins Last je
//!   Aktivität (`garmin_activity_load`, S1, nur Anzeige nach Entscheidung E2) und S3-Werte
//!   (`sensitive_day`, nur nach Freigabe). Alles Nutzerdaten: nicht aus FIT ableitbar, im
//!   Backup, `rebuild_cache` lässt sie stehen.
//! - Schwellen mit Herkunft `garmin` ([`Store::upsert_garmin_threshold`]).
//!
//! Alle vier Tabellen haben denselben Aufbau: ein Schlüssel, Wertespalten (jede leer erlaubt),
//! `source` und `imported_at`. Ein Abgleich mischt je Zeile wie bei `recovery_day`: ein neuer
//! Wert ersetzt den alten, ein fehlender lässt ihn stehen. Eine Zeile mit gleichen Werten bleibt
//! unverändert, auch `imported_at`, damit ein zweiter Abgleich nichts schreibt.
//!
//! Die Plausibilitätsgrenzen stehen je Spalte in [`Column`] und gelten für jeden Schreibweg,
//! auch für das Einspielen einer Sicherung. Der Abgleich verwirft Werte außerhalb vorher mit
//! [`DayRecord::sanitize`].

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::types::Value as Sql;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value as Json};

use crate::error::{Result, StoreError};
use crate::recovery::{SOURCE_MAX_CHARS, UpsertOutcome};
use crate::rows::{
    ThresholdKind, ThresholdOrigin, fmt_date, fmt_utc, parse_date, parse_utc, sport_to_str,
};
use crate::store::Store;

/// Einstellung, die den Abruf der Hydration (S3) freigibt. Nur genau `true` gibt frei
/// (SPEC 5.5, 6.2, Entscheidung E3).
pub const SETTING_GARMIN_SHARE_HYDRATION: &str = "garmin.share.hydration";
/// Herkunft von `athlete.sex` und `athlete.birth_year`: `garmin` oder `manual`. Garmin setzt die
/// Werte nur, solange sie nicht `manual` sind (SPEC 8.6).
pub const SETTING_ATHLETE_SEX_ORIGIN: &str = "athlete.sex_origin";
pub const SETTING_ATHLETE_BIRTH_YEAR_ORIGIN: &str = "athlete.birth_year_origin";
/// Quelle der Zeilen aus dem Abgleich in der App (wie `recovery_day`).
pub const GARMIN_APP_SOURCE: &str = "garmin-app";

/// Art und Grenzen einer Wertespalte.
#[derive(Debug, Clone, Copy)]
pub enum Column {
    /// Zahl im geschlossenen Bereich `min..=max`.
    Real {
        name: &'static str,
        min: f64,
        max: f64,
    },
    /// Text aus `A-Z`, `a-z`, `0-9` und `_`, höchstens `max_len` Zeichen. Garmin liefert hier
    /// Schlüssel wie `MODERATE` oder `AEROBIC_BASE`, nie freien Text.
    Key { name: &'static str, max_len: usize },
    /// Datum `YYYY-MM-DD`.
    Date { name: &'static str },
}

impl Column {
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Real { name, .. } | Self::Key { name, .. } | Self::Date { name } => name,
        }
    }

    fn accepts(&self, v: &Cell) -> bool {
        match (self, v) {
            (_, Cell::Null) => true,
            (Self::Real { min, max, .. }, Cell::Real(x)) => {
                x.is_finite() && (*min..=*max).contains(x)
            }
            (Self::Key { max_len, .. }, Cell::Text(t)) => {
                !t.is_empty()
                    && t.chars().count() <= *max_len
                    && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            }
            (Self::Date { .. }, Cell::Text(t)) => parse_date(t).is_ok(),
            _ => false,
        }
    }
}

/// Ein Wert einer Zeile.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Null,
    Real(f64),
    Text(String),
}

impl Cell {
    fn to_sql(&self) -> Sql {
        match self {
            Self::Null => Sql::Null,
            Self::Real(x) => Sql::Real(*x),
            Self::Text(t) => Sql::Text(t.clone()),
        }
    }

    fn from_sql(v: Sql) -> Self {
        match v {
            Sql::Null | Sql::Blob(_) => Self::Null,
            Sql::Integer(i) => Self::Real(i as f64),
            Sql::Real(x) => Self::Real(x),
            Sql::Text(t) => Self::Text(t),
        }
    }

    fn to_json(&self) -> Json {
        match self {
            Self::Null => Json::Null,
            Self::Real(x) => serde_json::Number::from_f64(*x).map_or(Json::Null, Json::Number),
            Self::Text(t) => Json::String(t.clone()),
        }
    }

    fn from_json(v: &Json) -> Option<Self> {
        match v {
            Json::Null => Some(Self::Null),
            Json::Number(n) => n.as_f64().map(Self::Real),
            Json::String(s) => Some(Self::Text(s.clone())),
            _ => None,
        }
    }

    fn real(v: Option<f64>) -> Self {
        v.map_or(Self::Null, Self::Real)
    }

    fn text(v: Option<&str>) -> Self {
        v.map_or(Self::Null, |t| Self::Text(t.to_owned()))
    }

    fn as_real(&self) -> Option<f64> {
        match self {
            Self::Real(x) => Some(*x),
            _ => None,
        }
    }

    fn as_text(&self) -> Option<String> {
        match self {
            Self::Text(t) => Some(t.clone()),
            _ => None,
        }
    }
}

/// Aufbau einer der vier Tabellen.
pub struct TableSpec {
    pub table: &'static str,
    /// Schlüsselspalte: `local_date` (Text) oder `garmin_activity_id` (Zahl).
    pub key: &'static str,
    pub columns: &'static [Column],
}

/// Eine Zeile einer der vier Tabellen als Rust-Typ.
pub trait DayRecord: Sized {
    const SPEC: TableSpec;
    fn key(&self) -> Sql;
    fn cells(&self) -> Vec<Cell>;
    fn source(&self) -> &str;
    fn imported_at(&self) -> DateTime<Utc>;
    fn from_parts(
        key: Sql,
        cells: &[Cell],
        source: String,
        imported_at: DateTime<Utc>,
    ) -> Option<Self>;
    fn set_cells(&mut self, cells: &[Cell]);

    /// Verwirft Werte außerhalb der Grenzen und gibt ihre Zahl zurück.
    fn sanitize(&mut self) -> u32 {
        let mut cells = self.cells();
        let mut dropped = 0;
        for (c, col) in cells.iter_mut().zip(Self::SPEC.columns) {
            if !col.accepts(c) {
                *c = Cell::Null;
                dropped += 1;
            }
        }
        self.set_cells(&cells);
        dropped
    }

    /// Ob die Zeile keinen einzigen Wert trägt (dann legt der Abgleich keine an).
    fn is_empty(&self) -> bool {
        Self::SPEC
            .columns
            .iter()
            .zip(self.cells())
            // Ein Datum allein (Garmins Last je Aktivität) ist kein Messwert.
            .all(|(col, c)| c == Cell::Null || matches!(col, Column::Date { .. }))
    }
}

fn check<T: DayRecord>(r: &T) -> Result<()> {
    for (c, col) in r.cells().iter().zip(T::SPEC.columns) {
        if !col.accepts(c) {
            return Err(StoreError::InvalidInput(format!(
                "{} {} out of range",
                T::SPEC.table,
                col.name()
            )));
        }
    }
    if r.source().chars().count() > SOURCE_MAX_CHARS {
        return Err(StoreError::InvalidInput(format!(
            "{} source too long",
            T::SPEC.table
        )));
    }
    Ok(())
}

fn column_list<T: DayRecord>() -> String {
    T::SPEC
        .columns
        .iter()
        .map(Column::name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Liest eine Zeile: Schlüssel, Werte, Quelle, Zeitpunkt.
fn read_parts<T: DayRecord>(r: &rusqlite::Row<'_>) -> rusqlite::Result<Option<T>> {
    let n = T::SPEC.columns.len();
    let key: Sql = r.get(0)?;
    let mut cells = Vec::with_capacity(n);
    for i in 0..n {
        cells.push(Cell::from_sql(r.get(i + 1)?));
    }
    let source: String = r.get(n + 1)?;
    let at: String = r.get(n + 2)?;
    let Ok(at) = parse_utc(&at) else {
        return Ok(None);
    };
    Ok(T::from_parts(key, &cells, source, at))
}

fn select_sql<T: DayRecord>() -> String {
    format!(
        "SELECT {}, {}, source, imported_at FROM {}",
        T::SPEC.key,
        column_list::<T>(),
        T::SPEC.table
    )
}

/// Mischt eine Zeile in die Tabelle (siehe Moduldokumentation).
fn merge_in<T: DayRecord>(conn: &Connection, new: &T) -> Result<UpsertOutcome> {
    let existing: Option<T> = conn
        .query_row(
            &format!("{} WHERE {} = ?1", select_sql::<T>(), T::SPEC.key),
            [new.key()],
            read_parts::<T>,
        )
        .optional()?
        .flatten();
    let new_cells = new.cells();
    let merged: Vec<Cell> = match &existing {
        None => new_cells,
        Some(old) => new_cells
            .into_iter()
            .zip(old.cells())
            .map(|(n, o)| if n == Cell::Null { o } else { n })
            .collect(),
    };
    let outcome = match &existing {
        None => UpsertOutcome::Inserted,
        Some(old) if old.cells() == merged && old.source() == new.source() => {
            return Ok(UpsertOutcome::Unchanged);
        }
        Some(_) => UpsertOutcome::Updated,
    };
    let cols = column_list::<T>();
    let n = T::SPEC.columns.len();
    let placeholders = (1..=n + 3)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let updates = T::SPEC
        .columns
        .iter()
        .map(|c| format!("{0} = excluded.{0}", c.name()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut values: Vec<Sql> = Vec::with_capacity(n + 3);
    values.push(new.key());
    values.extend(merged.iter().map(Cell::to_sql));
    values.push(Sql::Text(new.source().to_owned()));
    values.push(Sql::Text(fmt_utc(new.imported_at())));
    conn.execute(
        &format!(
            "INSERT INTO {table} ({key}, {cols}, source, imported_at) VALUES ({placeholders}) \
             ON CONFLICT({key}) DO UPDATE SET {updates}, source = excluded.source, \
             imported_at = excluded.imported_at",
            table = T::SPEC.table,
            key = T::SPEC.key,
        ),
        rusqlite::params_from_iter(values),
    )?;
    Ok(outcome)
}

fn date_key(key: &Sql) -> Option<NaiveDate> {
    match key {
        Sql::Text(t) => parse_date(t).ok(),
        _ => None,
    }
}

macro_rules! day_record {
    (
        $(#[$meta:meta])*
        $name:ident, $table:literal, [$($field:ident : $kind:ident $( ($($arg:expr),*) )?),* $(,)?]
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name {
            pub date: NaiveDate,
            $(pub $field: day_record!(@ty $kind),)*
            pub source: String,
            pub imported_at: DateTime<Utc>,
        }

        impl $name {
            /// Leere Zeile eines Tages.
            pub fn empty(date: NaiveDate, source: &str, imported_at: DateTime<Utc>) -> Self {
                Self { date, $($field: None,)* source: source.to_owned(), imported_at }
            }
        }

        impl DayRecord for $name {
            const SPEC: TableSpec = TableSpec {
                table: $table,
                key: "local_date",
                columns: &[$(day_record!(@col $field $kind $( ($($arg),*) )?)),*],
            };
            fn key(&self) -> Sql {
                Sql::Text(fmt_date(self.date))
            }
            fn cells(&self) -> Vec<Cell> {
                vec![$(day_record!(@cell self.$field, $kind)),*]
            }
            fn source(&self) -> &str {
                &self.source
            }
            fn imported_at(&self) -> DateTime<Utc> {
                self.imported_at
            }
            fn from_parts(
                key: Sql,
                cells: &[Cell],
                source: String,
                imported_at: DateTime<Utc>,
            ) -> Option<Self> {
                let mut r = Self::empty(date_key(&key)?, &source, imported_at);
                r.set_cells(cells);
                Some(r)
            }
            #[allow(unused_assignments)]
            fn set_cells(&mut self, cells: &[Cell]) {
                let mut i = 0;
                $(
                    self.$field = cells.get(i).and_then(day_record!(@get $kind));
                    i += 1;
                )*
            }
        }
    };
    (@ty Real) => { Option<f64> };
    (@ty Key) => { Option<String> };
    (@col $field:ident Real ($min:expr, $max:expr)) => {
        Column::Real { name: stringify!($field), min: $min, max: $max }
    };
    (@col $field:ident Key ($len:expr)) => {
        Column::Key { name: stringify!($field), max_len: $len }
    };
    (@cell $v:expr, Real) => { Cell::real($v) };
    (@cell $v:expr, Key) => { Cell::text($v.as_deref()) };
    (@get Real) => { Cell::as_real };
    (@get Key) => { Cell::as_text };
}

// Grenzen: Herzfrequenz nach SPEC 6.5 (30 bis 230 bpm). Alle übrigen sind Faustregeln, die nur
// unmögliche Werte abfangen: Schritte bis 200 000 und Strecke bis 400 km am Tag, Atmung 4 bis
// 60 Züge je Minute, SpO2 50 bis 100 Prozent, Intensitätsminuten höchstens die Minuten eines
// Tages, Stress auf Garmins Skala 0 bis 100 (negative Werte bedeuten bei Garmin "keine
// Messung" und fallen heraus).
day_record!(
    /// Zeile von `wellness_day` (S2): Tageswerte aus Garmin.
    WellnessDay, "wellness_day", [
        steps: Real(0.0, 200_000.0),
        step_distance_m: Real(0.0, 400_000.0),
        hr_min_bpm: Real(30.0, 230.0),
        hr_max_bpm: Real(30.0, 230.0),
        respiration_waking_brpm: Real(4.0, 60.0),
        respiration_sleep_brpm: Real(4.0, 60.0),
        spo2_avg_pct: Real(50.0, 100.0),
        spo2_low_pct: Real(50.0, 100.0),
        intensity_moderate_min: Real(0.0, 1440.0),
        intensity_vigorous_min: Real(0.0, 1440.0),
        stress_avg: Real(0.0, 100.0),
        stress_max: Real(0.0, 100.0),
    ]
);

// Grenzen (Faustregeln): Trainingsbereitschaft auf Garmins Skala 0 bis 100, akute Last bis
// 10 000, Fitnessalter 5 bis 120 Jahre, VO2max 10 bis 100 ml/kg/min, Ausdauerwert bis 20 000
// (Garmins Skala reicht laut Sonde über `gaugeUpperLimit`, Wert ungeprüft), Bergwert bis 200.
day_record!(
    /// Zeile von `training_day` (S2): Trainingswerte aus Garmin, nur zur Anzeige.
    TrainingDay, "training_day", [
        readiness_score: Real(0.0, 100.0),
        readiness_level: Key(32),
        garmin_acute_load: Real(0.0, 10_000.0),
        fitness_age: Real(5.0, 120.0),
        vo2max_running: Real(10.0, 100.0),
        vo2max_cycling: Real(10.0, 100.0),
        endurance_score: Real(0.0, 20_000.0),
        hill_score: Real(0.0, 200.0),
    ]
);

// Grenze (Faustregel): 20 Liter am Tag.
day_record!(
    /// Zeile von `sensitive_day` (S3): nur nach Freigabe in den Einstellungen geholt, vom
    /// MCP-Server nicht gelesen (SPEC 5.5, Entscheidung E3).
    SensitiveDay, "sensitive_day", [
        hydration_ml: Real(0.0, 20_000.0),
    ]
);

/// Zeile von `garmin_activity_load`: Garmins Trainingslast und Trainingseffekt einer
/// Aktivität. Nur zur Anzeige neben der eigenen Last (Entscheidung E2, SPEC 8.1).
#[derive(Debug, Clone, PartialEq)]
pub struct GarminActivityLoad {
    pub garmin_activity_id: u64,
    pub date: NaiveDate,
    /// Grenze 0 bis 10 000 (Faustregel).
    pub training_load: Option<f64>,
    /// Garmins Schlüssel, zum Beispiel `AEROBIC_BASE`.
    pub training_effect: Option<String>,
    pub source: String,
    pub imported_at: DateTime<Utc>,
}

impl DayRecord for GarminActivityLoad {
    const SPEC: TableSpec = TableSpec {
        table: "garmin_activity_load",
        key: "garmin_activity_id",
        columns: &[
            Column::Date { name: "local_date" },
            Column::Real {
                name: "training_load",
                min: 0.0,
                max: 10_000.0,
            },
            Column::Key {
                name: "training_effect",
                max_len: 40,
            },
        ],
    };
    fn key(&self) -> Sql {
        // SQLite kennt nur vorzeichenbehaftete Zahlen; Kennungen über i64::MAX gibt es nicht.
        Sql::Integer(i64::try_from(self.garmin_activity_id).unwrap_or(i64::MAX))
    }
    fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::Text(fmt_date(self.date)),
            Cell::real(self.training_load),
            Cell::text(self.training_effect.as_deref()),
        ]
    }
    fn source(&self) -> &str {
        &self.source
    }
    fn imported_at(&self) -> DateTime<Utc> {
        self.imported_at
    }
    fn from_parts(
        key: Sql,
        cells: &[Cell],
        source: String,
        imported_at: DateTime<Utc>,
    ) -> Option<Self> {
        let Sql::Integer(id) = key else { return None };
        let date = cells.first()?.as_text().and_then(|t| parse_date(&t).ok())?;
        Some(Self {
            garmin_activity_id: u64::try_from(id).ok()?,
            date,
            training_load: cells.get(1).and_then(Cell::as_real),
            training_effect: cells.get(2).and_then(Cell::as_text),
            source,
            imported_at,
        })
    }
    fn set_cells(&mut self, cells: &[Cell]) {
        if let Some(d) = cells
            .first()
            .and_then(Cell::as_text)
            .and_then(|t| parse_date(&t).ok())
        {
            self.date = d;
        }
        self.training_load = cells.get(1).and_then(Cell::as_real);
        self.training_effect = cells.get(2).and_then(Cell::as_text);
    }
}

/// Zeilen einer Tabelle als JSON-Objekte für die Sicherung: Schlüssel, alle Wertespalten,
/// `source`, `imported_at`.
pub(crate) fn export_table<T: DayRecord>(conn: &Connection) -> Result<Vec<Map<String, Json>>> {
    let mut stmt = conn.prepare(&format!("{} ORDER BY {}", select_sql::<T>(), T::SPEC.key))?;
    let rows = stmt
        .query_map([], read_parts::<T>)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = Vec::new();
    for r in rows.into_iter().flatten() {
        let mut m = Map::new();
        let key = match r.key() {
            Sql::Integer(i) => Json::from(i),
            Sql::Text(t) => Json::String(t),
            _ => Json::Null,
        };
        m.insert(T::SPEC.key.to_owned(), key);
        for (col, c) in T::SPEC.columns.iter().zip(r.cells()) {
            m.insert(col.name().to_owned(), c.to_json());
        }
        m.insert("source".to_owned(), Json::String(r.source().to_owned()));
        m.insert(
            "imported_at".to_owned(),
            Json::String(fmt_utc(r.imported_at())),
        );
        out.push(m);
    }
    Ok(out)
}

/// Spielt Zeilen aus der Sicherung ein (die Tabelle ist vorher geleert). Unbekannte Schlüssel,
/// falsche Typen und Werte außerhalb der Grenzen lehnen die ganze Sicherung ab.
pub(crate) fn import_table<T: DayRecord>(
    conn: &Connection,
    rows: &[Map<String, Json>],
) -> Result<()> {
    let bad = |what: &str| StoreError::InvalidInput(format!("{} {what}", T::SPEC.table));
    for m in rows {
        for k in m.keys() {
            let known = k == T::SPEC.key
                || k == "source"
                || k == "imported_at"
                || T::SPEC.columns.iter().any(|c| c.name() == k);
            if !known {
                return Err(bad("unknown field"));
            }
        }
        let key = match m.get(T::SPEC.key) {
            Some(Json::String(s)) if T::SPEC.key == "local_date" => Sql::Text(s.clone()),
            Some(Json::Number(n)) if T::SPEC.key != "local_date" => {
                Sql::Integer(n.as_i64().ok_or_else(|| bad("bad key"))?)
            }
            _ => return Err(bad("bad key")),
        };
        let mut cells = Vec::new();
        for col in T::SPEC.columns {
            let c = match m.get(col.name()) {
                None => Cell::Null,
                Some(v) => Cell::from_json(v).ok_or_else(|| bad("bad value"))?,
            };
            cells.push(c);
        }
        let source = match m.get("source") {
            Some(Json::String(s)) => s.clone(),
            _ => return Err(bad("bad source")),
        };
        let at = match m.get("imported_at") {
            Some(Json::String(s)) => parse_utc(s).map_err(|_| bad("bad imported_at"))?,
            _ => return Err(bad("bad imported_at")),
        };
        let r = T::from_parts(key, &cells, source, at).ok_or_else(|| bad("bad key"))?;
        // Ein Wert, den `from_parts` wegen des falschen Typs fallen ließ, ist ebenso ungültig.
        if r.cells() != cells {
            return Err(bad("bad value"));
        }
        check(&r)?;
        merge_in(conn, &r)?;
    }
    Ok(())
}

/// Ob eine Garmin-Schwelle neu ist oder sich geändert hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GarminThresholdOutcome {
    Inserted,
    Updated,
    Unchanged,
}

impl Store {
    /// Mischt Zeilen in ihre Tabelle (siehe Moduldokumentation), alle in einer Transaktion.
    /// Ist eine Zeile ungültig, wird nichts geschrieben. Zeilen ohne einen einzigen Wert werden
    /// übergangen. Gibt die Zahl der neuen oder geänderten Zeilen zurück.
    pub fn merge_garmin_rows<T: DayRecord>(&mut self, rows: &[T]) -> Result<u32> {
        for r in rows {
            check(r)?;
        }
        let tx = self.conn.transaction()?;
        let mut changed = 0;
        for r in rows.iter().filter(|r| !r.is_empty()) {
            if merge_in(&tx, r)? != UpsertOutcome::Unchanged {
                changed += 1;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Zeilen mit Datum von `from` bis `to` einschließlich, nach Datum (bei Garmins Last je
    /// Aktivität nach Datum und Kennung).
    pub fn garmin_rows<T: DayRecord>(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<T>> {
        let order = if T::SPEC.key == "local_date" {
            "local_date".to_owned()
        } else {
            format!("local_date, {}", T::SPEC.key)
        };
        let mut stmt = self.conn.prepare(&format!(
            "{} WHERE local_date >= ?1 AND local_date <= ?2 ORDER BY {order}",
            select_sql::<T>()
        ))?;
        let rows = stmt
            .query_map([fmt_date(from), fmt_date(to)], read_parts::<T>)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows.into_iter().flatten().collect())
    }

    /// Garmins Last zu einer Aktivität bei Garmin.
    pub fn garmin_activity_load(
        &self,
        garmin_activity_id: u64,
    ) -> Result<Option<GarminActivityLoad>> {
        let Ok(id) = i64::try_from(garmin_activity_id) else {
            return Ok(None);
        };
        Ok(self
            .conn
            .query_row(
                &format!(
                    "{} WHERE garmin_activity_id = ?1",
                    select_sql::<GarminActivityLoad>()
                ),
                [id],
                read_parts::<GarminActivityLoad>,
            )
            .optional()?
            .flatten())
    }

    /// Löscht alle S3-Werte (Einstellungen, "S3-Werte löschen"). Gibt die Zahl der Zeilen
    /// zurück.
    pub fn delete_sensitive_days(&mut self) -> Result<u32> {
        Ok(self.conn.execute("DELETE FROM sensitive_day", [])? as u32)
    }

    /// Schreibt eine Schwelle aus Garmin (Herkunft `garmin`, SPEC 8.6). Gibt es schon eine
    /// Garmin-Zeile mit gleicher Sportart, Art und gleichem Datum, wird ihr Wert ersetzt, nicht
    /// verdoppelt; ein unveränderter Wert schreibt nichts. Manuelle Werte bleiben unberührt.
    pub fn upsert_garmin_threshold(
        &mut self,
        sport: Option<lastwerk_core::Sport>,
        kind: ThresholdKind,
        value: f64,
        valid_from: NaiveDate,
    ) -> Result<GarminThresholdOutcome> {
        let sport = sport.map_or("all", sport_to_str);
        let existing: Option<(i64, f64)> = self
            .conn
            .query_row(
                "SELECT id, value FROM threshold WHERE origin = 'garmin' AND sport = ?1 \
                 AND kind = ?2 AND valid_from = ?3 ORDER BY id DESC LIMIT 1",
                params![sport, kind.as_str(), fmt_date(valid_from)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match existing {
            Some((_, old)) if (old - value).abs() < 1e-9 => Ok(GarminThresholdOutcome::Unchanged),
            Some((id, _)) => {
                self.conn.execute(
                    "UPDATE threshold SET value = ?1 WHERE id = ?2",
                    params![value, id],
                )?;
                Ok(GarminThresholdOutcome::Updated)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO threshold (sport, kind, value, valid_from, origin, note) \
                     VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
                    params![
                        sport,
                        kind.as_str(),
                        value,
                        fmt_date(valid_from),
                        ThresholdOrigin::Garmin.as_str()
                    ],
                )?;
                Ok(GarminThresholdOutcome::Inserted)
            }
        }
    }

    /// Jüngster Garmin-Wert einer Art und Sportart: Datum und Wert. Für Werte ohne Datum bei
    /// Garmin entscheidet der Abgleich damit, ob sich etwas geändert hat (SPEC 8.6).
    pub fn latest_garmin_threshold(
        &self,
        sport: Option<lastwerk_core::Sport>,
        kind: ThresholdKind,
    ) -> Result<Option<(NaiveDate, f64)>> {
        let row: Option<(String, f64)> = self
            .conn
            .query_row(
                "SELECT valid_from, value FROM threshold WHERE origin = 'garmin' AND sport = ?1 \
                 AND kind = ?2 ORDER BY valid_from DESC, id DESC LIMIT 1",
                params![sport.map_or("all", sport_to_str), kind.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        row.map(|(d, v)| {
            parse_date(&d)
                .map(|d| (d, v))
                .map_err(|e| StoreError::InvalidData(e.0))
        })
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lastwerk_core::Sport;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    fn at(h: u32) -> DateTime<Utc> {
        format!("2026-10-16T{h:02}:00:00Z").parse().unwrap()
    }

    #[test]
    fn merge_keeps_old_values_and_is_idempotent() {
        let mut s = Store::open_in_memory().unwrap();
        let mut a = WellnessDay::empty(d("2026-10-01"), "garmin-app", at(6));
        a.steps = Some(8000.0);
        a.hr_min_bpm = Some(48.0);
        assert_eq!(s.merge_garmin_rows(std::slice::from_ref(&a)).unwrap(), 1);
        assert_eq!(s.merge_garmin_rows(std::slice::from_ref(&a)).unwrap(), 0);

        // Später nur die Schritte: der Tiefstwert der Herzfrequenz bleibt.
        let mut b = WellnessDay::empty(d("2026-10-01"), "garmin-app", at(7));
        b.steps = Some(9000.0);
        assert_eq!(s.merge_garmin_rows(&[b]).unwrap(), 1);
        let rows: Vec<WellnessDay> = s.garmin_rows(d("2026-10-01"), d("2026-10-01")).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].steps, Some(9000.0));
        assert_eq!(rows[0].hr_min_bpm, Some(48.0));
        assert_eq!(rows[0].imported_at, at(7));

        // Ein Tag ohne Wert legt keine Zeile an.
        let empty = WellnessDay::empty(d("2026-10-02"), "garmin-app", at(7));
        assert_eq!(s.merge_garmin_rows(&[empty]).unwrap(), 0);
        let rows: Vec<WellnessDay> = s.garmin_rows(d("2026-10-02"), d("2026-10-02")).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn values_out_of_range_are_refused_or_dropped() {
        let mut s = Store::open_in_memory().unwrap();
        let mut a = WellnessDay::empty(d("2026-10-01"), "garmin-app", at(6));
        a.stress_avg = Some(-1.0);
        a.spo2_avg_pct = Some(95.0);
        assert!(s.merge_garmin_rows(std::slice::from_ref(&a)).is_err());
        assert_eq!(a.sanitize(), 1);
        assert_eq!(a.stress_avg, None);
        assert_eq!(s.merge_garmin_rows(&[a]).unwrap(), 1);

        let mut t = TrainingDay::empty(d("2026-10-01"), "garmin-app", at(6));
        t.readiness_level = Some("<script>".into());
        assert!(s.merge_garmin_rows(std::slice::from_ref(&t)).is_err());
        t.readiness_level = Some("MODERATE".into());
        assert_eq!(s.merge_garmin_rows(&[t]).unwrap(), 1);
    }

    #[test]
    fn activity_loads_are_keyed_by_garmin_id() {
        let mut s = Store::open_in_memory().unwrap();
        let l = GarminActivityLoad {
            garmin_activity_id: 98_765_432_101,
            date: d("2026-10-01"),
            training_load: Some(112.5),
            training_effect: Some("AEROBIC_BASE".into()),
            source: "garmin-app".into(),
            imported_at: at(6),
        };
        assert_eq!(s.merge_garmin_rows(std::slice::from_ref(&l)).unwrap(), 1);
        assert_eq!(s.garmin_activity_load(98_765_432_101).unwrap(), Some(l));
        assert_eq!(s.garmin_activity_load(1).unwrap(), None);
    }

    #[test]
    fn garmin_threshold_is_replaced_not_duplicated() {
        let mut s = Store::open_in_memory().unwrap();
        use GarminThresholdOutcome::*;
        let up = |s: &mut Store, v: f64, day: &str| {
            s.upsert_garmin_threshold(Some(Sport::Cycling), ThresholdKind::FtpW, v, d(day))
                .unwrap()
        };
        assert_eq!(up(&mut s, 250.0, "2026-03-01"), Inserted);
        assert_eq!(up(&mut s, 250.0, "2026-03-01"), Unchanged);
        assert_eq!(up(&mut s, 255.0, "2026-03-01"), Updated);
        assert_eq!(up(&mut s, 260.0, "2026-06-01"), Inserted);
        assert_eq!(s.list_thresholds().unwrap().len(), 2);
        assert_eq!(
            s.latest_garmin_threshold(Some(Sport::Cycling), ThresholdKind::FtpW)
                .unwrap(),
            Some((d("2026-06-01"), 260.0))
        );
    }

    #[test]
    fn manual_value_wins_over_garmin_on_the_same_day() {
        let mut s = Store::open_in_memory().unwrap();
        s.upsert_garmin_threshold(
            Some(Sport::Cycling),
            ThresholdKind::FtpW,
            250.0,
            d("2026-03-01"),
        )
        .unwrap();
        s.insert_threshold(&crate::NewThreshold {
            sport: Some(Sport::Cycling),
            kind: ThresholdKind::FtpW,
            value: 240.0,
            valid_from: d("2026-03-01"),
            origin: ThresholdOrigin::Manual,
            note: None,
        })
        .unwrap();
        // Ein zweiter Garmin-Abgleich mit demselben Wert ändert daran nichts.
        s.upsert_garmin_threshold(
            Some(Sport::Cycling),
            ThresholdKind::FtpW,
            250.0,
            d("2026-03-01"),
        )
        .unwrap();
        let th = s
            .thresholds_for(Sport::Cycling, d("2026-04-01"))
            .unwrap()
            .unwrap();
        assert_eq!(th.ftp_w, Some(240.0));

        // Ein späterer Garmin-Wert gilt ab seinem Datum wieder.
        s.upsert_garmin_threshold(
            Some(Sport::Cycling),
            ThresholdKind::FtpW,
            260.0,
            d("2026-05-01"),
        )
        .unwrap();
        let th = s
            .thresholds_for(Sport::Cycling, d("2026-05-02"))
            .unwrap()
            .unwrap();
        assert_eq!(th.ftp_w, Some(260.0));
        let th = s
            .thresholds_for(Sport::Cycling, d("2026-04-30"))
            .unwrap()
            .unwrap();
        assert_eq!(th.ftp_w, Some(240.0));
    }

    #[test]
    fn backup_round_trip_keeps_garmin_rows_and_rejects_bad_ones() {
        let mut s = Store::open_in_memory().unwrap();
        let mut w = WellnessDay::empty(d("2026-10-01"), "garmin-app", at(6));
        w.steps = Some(8000.0);
        let mut t = TrainingDay::empty(d("2026-10-01"), "garmin-app", at(6));
        t.readiness_level = Some("HIGH".into());
        let mut h = SensitiveDay::empty(d("2026-10-01"), "garmin-app", at(6));
        h.hydration_ml = Some(1500.0);
        s.merge_garmin_rows(&[w]).unwrap();
        s.merge_garmin_rows(&[t]).unwrap();
        s.merge_garmin_rows(&[h]).unwrap();
        let export = s.export_user_data().unwrap();
        assert_eq!(export.wellness_days.len(), 1);
        assert_eq!(export.sensitive_days.len(), 1);

        let mut fresh = Store::open_in_memory().unwrap();
        fresh.import_user_data(&export).unwrap();
        assert_eq!(fresh.export_user_data().unwrap(), export);

        let mut bad = export.clone();
        bad.wellness_days[0].insert("steps".into(), serde_json::json!(-5));
        assert!(fresh.import_user_data(&bad).is_err());
        let mut bad = export;
        bad.training_days[0].insert("unknown".into(), serde_json::json!(1));
        assert!(fresh.import_user_data(&bad).is_err());
    }
}
