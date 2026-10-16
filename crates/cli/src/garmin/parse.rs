//! Werte aus den JSON-Antworten von Garmin lesen (SPEC 5.6, 7.3, T22).
//!
//! Die Antworten sind nicht vertrauenswürdig und können unvollständig sein. Jede Funktion liest
//! nur die benannten Felder, ignoriert alles Unerwartete (fehlendes Feld, `null`, falscher Typ)
//! und verwirft Werte außerhalb der Plausibilitätsgrenzen. Verworfene Werte werden gezählt.
//!
//! Plausibilitätsgrenzen: dieselben wie in SPEC 7.3 und `normalize.py` des Zusatzprogramms (Ordner `bridge/`)
//! (`LIMITS`), im Speicher als Konstanten (`lastwerk_store::HRV_MAX_MS` und weitere). Sie sind
//! Faustregeln, keine physiologischen Aussagen.
//!
//! Feldnamen laut `docs/garmin-feldabdeckung.md` (Sonde L1):
//!
//! | Wert | Endpunkt | Feld | Herkunft |
//! |---|---|---|---|
//! | HRV | `hrv_range` | `hrvSummaries[].calendarDate`, `.lastNightAvg` | Bericht nennt nur `hrvSummaries: array(7) / object(8 keys)`, die zwei Namen kommen aus dem Tagesendpunkt (`hrvSummary.calendarDate`, `.lastNightAvg`). ANNAHME (SPEC 14 O9) |
//! | Ruhepuls | `rhr_range` | `allMetrics.metricsMap.WELLNESS_RESTING_HEART_RATE[].calendarDate`, `.value` | Quelltext von garminconnect 0.3.17 (`get_rhr_daily`), das daraus die Liste `[].calendarDate/.value` des Berichts macht. Beide Formen werden gelesen. ANNAHME für die Rohform (O9) |
//! | Body Battery | `body_battery` | `[].date`, `[].bodyBatteryValuesArray[][1]` | Bericht (Wurzel: Liste mit `date` und `bodyBatteryValuesArray: array(2) of integer`) |
//! | Schlaf | `sleep_day` | `dailySleepDTO.sleepTimeSeconds`, `dailySleepDTO.sleepScores.overall.value` | Bericht (`sleep`), wie `FIELD_MAP` der Bridge |

use chrono::NaiveDate;
use lastwerk_store::{
    BODY_BATTERY_MAX, HRV_MAX_MS, RESTING_HR_MAX_BPM, RESTING_HR_MIN_BPM, SLEEP_MAX_S,
    SLEEP_SCORE_MAX,
};
use serde_json::Value;

/// Zähler für Werte, die verworfen wurden, weil sie außerhalb der Grenzen lagen.
#[derive(Debug, Default)]
pub struct Dropped(pub u32);

/// `YYYY-MM-DD`, auch wenn Garmin eine Uhrzeit anhängt. Alles andere ist kein Datum.
pub(super) fn date_of(v: &Value) -> Option<NaiveDate> {
    let text = v.as_str()?;
    NaiveDate::parse_from_str(text.get(..10)?, "%Y-%m-%d").ok()
}

pub(super) fn number(v: &Value) -> Option<f64> {
    v.as_f64().filter(|x| x.is_finite())
}

/// Hält `value` im Bereich oder zählt es als verworfen.
fn within(value: f64, low: f64, high: f64, dropped: &mut Dropped) -> Option<f64> {
    if (low..=high).contains(&value) {
        Some(value)
    } else {
        dropped.0 += 1;
        None
    }
}

/// HRV der Nacht je Tag aus `hrv_range`.
pub fn hrv_days(v: &Value, dropped: &mut Dropped) -> Vec<(NaiveDate, f64)> {
    let Some(items) = v.get("hrvSummaries").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let date = date_of(item.get("calendarDate")?)?;
            let raw = number(item.get("lastNightAvg")?)?;
            Some((date, within(raw, 0.0, HRV_MAX_MS, dropped)?))
        })
        .collect()
}

/// Ruhepuls je Tag aus `rhr_range`. Gelesen wird die Rohform der API und die Listenform, in die
/// die Python-Bibliothek sie umbaut (siehe Tabelle oben).
pub fn rhr_days(v: &Value, dropped: &mut Dropped) -> Vec<(NaiveDate, f64)> {
    let raw_rows = v
        .pointer("/allMetrics/metricsMap/WELLNESS_RESTING_HEART_RATE")
        .and_then(Value::as_array);
    let Some(rows) = raw_rows.or_else(|| v.as_array()) else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| {
            let date = date_of(row.get("calendarDate")?)?;
            let raw = number(row.get("value")?)?;
            Some((
                date,
                within(raw, RESTING_HR_MIN_BPM, RESTING_HR_MAX_BPM, dropped)?,
            ))
        })
        .collect()
}

/// Höchster und tiefster Body-Battery-Wert je Tag aus `body_battery`. Die Werte stehen als
/// `[Zeitstempel, Wert]` in `bodyBatteryValuesArray`; `null` und alles Nichtnumerische wird
/// übergangen, Werte außerhalb von 0 bis 100 werden verworfen (nicht begrenzt).
pub fn body_battery_days(v: &Value, dropped: &mut Dropped) -> Vec<(NaiveDate, u8, u8)> {
    let Some(days) = v.as_array() else {
        return Vec::new();
    };
    days.iter()
        .filter_map(|day| {
            let date = date_of(day.get("date")?)?;
            let values = day.get("bodyBatteryValuesArray")?.as_array()?;
            let mut levels = values
                .iter()
                .filter_map(|pair| number(pair.as_array()?.get(1)?))
                .filter_map(|x| within(x, 0.0, f64::from(BODY_BATTERY_MAX), dropped))
                .map(|x| x.round() as u8);
            let first = levels.next()?;
            let (low, high) = levels.fold((first, first), |(lo, hi), x| (lo.min(x), hi.max(x)));
            Some((date, high, low))
        })
        .collect()
}

/// Schlafdauer in Sekunden und Schlafwert eines Tages aus `sleep_day`.
///
/// Eine Dauer von 0 Sekunden ist keine Messung (Garmin liefert für Nächte ohne Uhr einen
/// leeren `dailySleepDTO`) und bleibt leer: Der Planer (R12) läse sonst "null Stunden
/// geschlafen" statt "unbekannt".
pub fn sleep(v: &Value, dropped: &mut Dropped) -> (Option<u32>, Option<u8>) {
    let Some(dto) = v.get("dailySleepDTO") else {
        return (None, None);
    };
    let duration = dto
        .get("sleepTimeSeconds")
        .and_then(number)
        .filter(|s| *s > 0.0)
        .and_then(|s| within(s, 0.0, f64::from(SLEEP_MAX_S), dropped))
        .map(|s| s.round() as u32);
    let score = dto
        .pointer("/sleepScores/overall/value")
        .and_then(number)
        .and_then(|s| within(s, 0.0, f64::from(SLEEP_SCORE_MAX), dropped))
        .map(|s| s.round() as u8);
    (duration, score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    #[test]
    fn hrv_reads_the_summaries_and_drops_out_of_range_values() {
        let v = json!({"userProfilePk": 1, "hrvSummaries": [
            {"calendarDate": "2026-10-01", "lastNightAvg": 61},
            {"calendarDate": "2026-10-02", "lastNightAvg": 400},
            {"calendarDate": "2026-10-03", "lastNightAvg": 400.5},
            {"calendarDate": "2026-10-04", "lastNightAvg": -1},
            {"calendarDate": "2026-10-05", "lastNightAvg": null},
            {"calendarDate": "kein Datum", "lastNightAvg": 50},
            {"lastNightAvg": 50},
            {"calendarDate": "2026-10-06T00:00:00", "lastNightAvg": 58.5}
        ]});
        let mut dropped = Dropped::default();
        assert_eq!(
            hrv_days(&v, &mut dropped),
            vec![(d(1), 61.0), (d(2), 400.0), (d(6), 58.5)]
        );
        assert_eq!(dropped.0, 2);
        assert!(hrv_days(&json!(null), &mut dropped).is_empty());
        assert!(hrv_days(&json!({"hrvSummaries": "x"}), &mut dropped).is_empty());
    }

    #[test]
    fn resting_heart_rate_reads_the_raw_and_the_list_form() {
        let raw = json!({"allMetrics": {"metricsMap": {"WELLNESS_RESTING_HEART_RATE": [
            {"value": 48.0, "calendarDate": "2026-10-01"},
            {"value": 19.0, "calendarDate": "2026-10-02"},
            {"value": 121.0, "calendarDate": "2026-10-03"},
            {"value": 120.0, "calendarDate": "2026-10-04"},
            {"value": null, "calendarDate": "2026-10-05"}
        ]}}});
        let list = json!([
            {"calendarDate": "2026-10-01", "value": 48.0},
            {"calendarDate": "2026-10-02", "value": 19.0},
            {"calendarDate": "2026-10-03", "value": 121.0},
            {"calendarDate": "2026-10-04", "value": 120.0},
            {"calendarDate": "2026-10-05", "value": null}
        ]);
        for v in [raw, list] {
            let mut dropped = Dropped::default();
            assert_eq!(
                rhr_days(&v, &mut dropped),
                vec![(d(1), 48.0), (d(4), 120.0)]
            );
            assert_eq!(dropped.0, 2);
        }
        let mut dropped = Dropped::default();
        assert!(rhr_days(&json!({"allMetrics": null}), &mut dropped).is_empty());
    }

    #[test]
    fn body_battery_takes_max_and_min_ignoring_nulls_and_drops_out_of_range() {
        let v = json!([
            {"date": "2026-10-01", "charged": 60, "bodyBatteryValuesArray": [
                [1, 35], [2, null], [3, 96], [4, 8], null, [5], "x", [6, 50.4]]},
            {"date": "2026-10-02", "bodyBatteryValuesArray": [[1, null], [2, null]]},
            {"date": "2026-10-03", "bodyBatteryValuesArray": [[1, 120], [2, -5], [3, 40]]},
            {"date": "2026-10-04", "bodyBatteryValuesArray": []},
            {"date": "2026-10-05"},
            {"date": "2026-10-06", "bodyBatteryValuesArray": [[1, 0], [2, 100]]}
        ]);
        let mut dropped = Dropped::default();
        assert_eq!(
            body_battery_days(&v, &mut dropped),
            vec![(d(1), 96, 8), (d(3), 40, 40), (d(6), 100, 0)]
        );
        assert_eq!(dropped.0, 2);
        assert!(body_battery_days(&json!({"x": 1}), &mut dropped).is_empty());
    }

    #[test]
    fn sleep_reads_duration_and_score_and_keeps_zero_seconds_empty() {
        let mut dropped = Dropped::default();
        let ok = json!({"dailySleepDTO": {"sleepTimeSeconds": 27000,
            "sleepScores": {"overall": {"value": 82, "qualifierKey": "GOOD"}}}});
        assert_eq!(sleep(&ok, &mut dropped), (Some(27_000), Some(82)));
        let zero = json!({"dailySleepDTO": {"sleepTimeSeconds": 0, "sleepScores": null}});
        assert_eq!(sleep(&zero, &mut dropped), (None, None));
        assert_eq!(dropped.0, 0);
        let bad = json!({"dailySleepDTO": {"sleepTimeSeconds": 90000,
            "sleepScores": {"overall": {"value": 101}}}});
        assert_eq!(sleep(&bad, &mut dropped), (None, None));
        assert_eq!(dropped.0, 2);
        assert_eq!(sleep(&json!(null), &mut dropped), (None, None));
        assert_eq!(
            sleep(&json!({"dailySleepDTO": null}), &mut dropped),
            (None, None)
        );
        let edge = json!({"dailySleepDTO": {"sleepTimeSeconds": 86400,
            "sleepScores": {"overall": {"value": 100}}}});
        assert_eq!(sleep(&edge, &mut dropped), (Some(86_400), Some(100)));
    }
}
