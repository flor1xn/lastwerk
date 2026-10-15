//! Abgeleitete Zeilen einer Aktivität: Belastung, Zeit in Zonen, Bestleistungen.
//!
//! Die Funktionen schreiben auf eine Verbindung und laufen deshalb sowohl beim Import als auch
//! bei der Neuberechnung nach einer geänderten Schwelle (`recompute`) innerhalb einer
//! Transaktion. Sie lesen die Schwellenwerte, die am Tag der Aktivität gelten (SPEC 3, 5.4).

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use lastwerk_core::{
    ActivityInput, BEST_EFFORT_DISTANCES_M, POWER_CURVE_DURATIONS_S, PreparedSamples, Thresholds,
    ZoneBasis, ZoneError, best_efforts, best_efforts_prepared, best_power, best_power_prepared,
    check_plausibility, compute_load, compute_load_prepared, resolve_hr_max, zone_seconds,
    zone_seconds_prepared,
};
use rusqlite::params;

use crate::error::{Result, StoreError};
use crate::import::ImportOptions;
use crate::rows::{LoadRow, LoadStatus, METHOD_NONE, ThresholdKind, method_to_str};
use crate::statements::Db;
use crate::store::{get_setting, threshold_lookup, upsert_activity_load};

/// Schlüssel in `setting` für das Geburtsjahr. Es liefert das Alter für die Schätzung der
/// maximalen Herzfrequenz (`208 - 0,7 * Alter`, Q7), wenn kein Messwert vorliegt (SPEC 8.5).
/// Konvention: nur das Jahr, das Alter ist Kalenderjahr der Aktivität minus Geburtsjahr.
pub const SETTING_ATHLETE_BIRTH_YEAR: &str = "athlete.birth_year";

/// Prüft die Samples einmal für alle abgeleiteten Zeilen einer Aktivität (SPEC 6.5).
///
/// Belastung, jedes Zonenschema und die Bestleistung prüften die Samples bisher je für
/// sich; bei großen Dateien war das der größte Rechenposten des Imports (T38, Anhang
/// Messung). `None` heißt, die Prüfung scheitert (zum Beispiel nicht steigende
/// Zeitstempel). Dann rechnen die Funktionen unten wie bisher selbst und melden denselben
/// Fehler wie vorher.
pub(crate) fn prepare(input: &ActivityInput, opts: &ImportOptions) -> Option<PreparedSamples> {
    check_plausibility(&input.samples, &opts.load).ok()
}

/// Berechnet und speichert die Belastung der Aktivität mit den Schwellen vom `local_date`.
/// Ohne Schwellenwert rechnet `compute_load` mit leeren Schwellen und meldet den fehlenden
/// Wert selbst. Ein Fehler wird als Zeile ohne Wert gespeichert, nie als Null (SPEC 5.4).
pub(crate) fn write_load(
    db: &Db<'_>,
    activity_id: i64,
    input: &ActivityInput,
    prepared: Option<&PreparedSamples>,
    local_date: NaiveDate,
    now: DateTime<Utc>,
    opts: &ImportOptions,
) -> Result<LoadStatus> {
    let (thresholds, ids) = match threshold_lookup(db, input.sport, local_date)? {
        Some(l) => (l.thresholds, l.ids),
        None => (Thresholds::empty(local_date), Default::default()),
    };
    let load = match prepared {
        Some(p) => compute_load_prepared(input, p, &thresholds, &opts.load),
        None => compute_load(input, &thresholds, &opts.load),
    };
    let row = match load {
        Ok(load) => LoadRow {
            activity_id,
            method: method_to_str(load.method).to_owned(),
            value: Some(f64::from(load.value)),
            intensity_factor: load.intensity_factor.map(f64::from),
            threshold_id: ThresholdKind::for_method(load.method).and_then(|k| ids.get(&k).copied()),
            status: LoadStatus::Ok,
            computed_at: now,
        },
        Err(e) => LoadRow {
            activity_id,
            method: METHOD_NONE.to_owned(),
            value: None,
            intensity_factor: None,
            threshold_id: None,
            status: LoadStatus::from_error(&e),
            computed_at: now,
        },
    };
    upsert_activity_load(db, &row)?;
    Ok(row.status)
}

/// Schwellenwerte für die Zonen am Tag `on`. Fehlt die maximale Herzfrequenz, nimmt der
/// Speicher die Schätzung aus dem Alter (Q7), wenn das Geburtsjahr als Einstellung vorliegt
/// (Konvention, SPEC 6.2). Der zweite Wert ist `true`, wenn die Herzfrequenz geschätzt ist.
/// Ohne Messwert und ohne Geburtsjahr bleibt sie leer, Zonen nach Herzfrequenzreserve sind
/// dann nicht berechenbar.
pub(crate) fn zone_thresholds(
    db: &Db<'_>,
    sport: lastwerk_core::Sport,
    on: NaiveDate,
) -> Result<(Thresholds, bool)> {
    let mut th =
        threshold_lookup(db, sport, on)?.map_or_else(|| Thresholds::empty(on), |l| l.thresholds);
    let mut estimated = false;
    if th.hr_max_bpm.is_none()
        && let Some(age) = age_on(db, on)?
        && let Some(hr_max) = resolve_hr_max(None, Some(age))
    {
        th.hr_max_bpm = Some(hr_max.bpm);
        estimated = hr_max.estimated;
    }
    Ok((th, estimated))
}

/// Alter in Jahren am Tag `on` aus dem Geburtsjahr der Einstellungen. Ein Geburtsjahr, das
/// keine Zahl ist, nach `on` liegt oder vor 1900, gilt als nicht vorhanden.
fn age_on(db: &Db<'_>, on: NaiveDate) -> Result<Option<f32>> {
    let Some(text) = get_setting(db, SETTING_ATHLETE_BIRTH_YEAR)? else {
        return Ok(None);
    };
    let Ok(year) = text.trim().parse::<i32>() else {
        return Ok(None);
    };
    if year < 1900 || year > on.year() {
        return Ok(None);
    }
    Ok(Some((on.year() - year) as f32))
}

/// Berechnet die Zeit in Zonen für alle Schemata des Katalogs, die für die Sportart gelten,
/// und speichert sie (alle Zonen eines Schemas, auch mit 0 Sekunden). Ein Schema, dessen
/// Bezugswert oder Kanal fehlt, erzeugt keine Zeilen: "nicht berechenbar" ist keine Null.
pub(crate) fn write_zone_times(
    db: &Db<'_>,
    activity_id: i64,
    input: &ActivityInput,
    prepared: Option<&PreparedSamples>,
    local_date: NaiveDate,
    opts: &ImportOptions,
) -> Result<()> {
    let (th, hr_max_estimated) = zone_thresholds(db, input.sport, local_date)?;
    for scheme in opts.zones.for_sport(input.sport) {
        let seconds = match prepared {
            Some(p) => zone_seconds_prepared(p, scheme, &th),
            None => zone_seconds(input, scheme, &th, &opts.load),
        };
        let seconds = match seconds {
            Ok(s) => s,
            Err(ZoneError::InvalidScheme(why)) => {
                return Err(StoreError::InvalidData(format!("zone scheme: {why}")));
            }
            // Fehlender Bezugswert, fehlender Kanal oder unplausible Eingabe: keine Zeilen.
            Err(_) => continue,
        };
        let estimated = hr_max_estimated && scheme.basis == ZoneBasis::HrReserve;
        for (i, s) in seconds.iter().enumerate() {
            db.with(
                "INSERT INTO activity_zone_time (activity_id, scheme, zone, seconds, estimated) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                |stmt| {
                    stmt.execute(params![
                        activity_id,
                        scheme.id,
                        i as i64 + 1,
                        s,
                        i64::from(estimated)
                    ])
                },
            )?;
        }
    }
    Ok(())
}

/// Berechnet und speichert die beste mittlere Leistung über das Raster
/// `POWER_CURVE_DURATIONS_S` (SPEC 6.2, seit T33). Ohne Leistungswerte entstehen keine Zeilen.
pub(crate) fn write_best_power(
    db: &Db<'_>,
    activity_id: i64,
    input: &ActivityInput,
    prepared: Option<&PreparedSamples>,
    opts: &ImportOptions,
) -> Result<()> {
    // Ein unplausibler Zeitstempel verhindert nur die Bestwerte, die Aktivität bleibt
    // gespeichert (die Belastung meldet denselben Fehler als `implausible`).
    let list = match prepared {
        Some(p) => best_power_prepared(p, &POWER_CURVE_DURATIONS_S),
        None => match best_power(input, &opts.load, &POWER_CURVE_DURATIONS_S) {
            Ok(list) => list,
            Err(_) => return Ok(()),
        },
    };
    for b in list {
        db.with(
            "INSERT INTO activity_best_power (activity_id, duration_s, watts) \
             VALUES (?1, ?2, ?3)",
            |stmt| stmt.execute(params![activity_id, b.duration_s, f64::from(b.watts)]),
        )?;
    }
    Ok(())
}

/// Berechnet und speichert die Bestzeiten über `BEST_EFFORT_DISTANCES_M`, nur für Läufe (T37).
/// Ohne Geschwindigkeit oder bei zu kurzer Strecke entstehen keine Zeilen.
pub(crate) fn write_best_effort(
    db: &Db<'_>,
    activity_id: i64,
    input: &ActivityInput,
    prepared: Option<&PreparedSamples>,
    opts: &ImportOptions,
) -> Result<()> {
    if input.sport != lastwerk_core::Sport::Running {
        return Ok(());
    }
    let list = match prepared {
        Some(p) => best_efforts_prepared(p, &BEST_EFFORT_DISTANCES_M),
        None => match best_efforts(input, &opts.load, &BEST_EFFORT_DISTANCES_M) {
            Ok(list) => list,
            Err(_) => return Ok(()),
        },
    };
    for e in list {
        db.with(
            "INSERT INTO activity_best_effort (activity_id, distance_m, seconds) \
             VALUES (?1, ?2, ?3)",
            |stmt| stmt.execute(params![activity_id, e.distance_m, e.seconds]),
        )?;
    }
    Ok(())
}

/// Entfernt Belastung und Zeit in Zonen einer Aktivität. Die Bestleistungen hängen nicht von
/// Schwellenwerten ab und bleiben stehen.
pub(crate) fn delete_threshold_dependent(db: &Db<'_>, activity_id: i64) -> Result<()> {
    for sql in [
        "DELETE FROM activity_load WHERE activity_id = ?1",
        "DELETE FROM activity_zone_time WHERE activity_id = ?1",
    ] {
        db.with(sql, |s| s.execute([activity_id]))?;
    }
    Ok(())
}
