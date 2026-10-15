//! Dashboard "Heute" (T25 Teil B): ein Aufruf liefert alle Kennzahlen, Deutungscodes und Reihen.
//!
//! Die Rechnung steht in `lastwerk_core::metrics` (reine Funktionen). Diese Schicht liest aus
//! dem Speicher, bildet daraus die Eingabetypen von `core` (Zeit in Zonen je Aktivität,
//! Effizienz-Einheiten, Bestwerte je Zeitraum, Erholungstage), ruft die Funktionen und rundet für
//! die Oberfläche. Sie rechnet selbst nichts außer Summen, Gruppieren und Runden. Was die Daten
//! nicht hergeben, bleibt `None` oder eine leere Reihe mit Code, nie eine erfundene Zahl.
//!
//! Prognose: Anders als die Vorausberechnung der Planansicht (ab Planbeginn aus den geplanten
//! Lasten) rechnet das Dashboard mit den tatsächlichen Lasten bis heute und den geplanten
//! danach. Mitten in einem laufenden Plan geht die Kurve so ohne Sprung von den gemessenen
//! Werten in die Vorausberechnung über (Abweichung, in der Task-Datei T25 vermerkt).

use std::collections::{BTreeMap, HashMap};

use chrono::{NaiveDate, TimeDelta};
use lastwerk_cli::planning;
use lastwerk_core::metrics::adherence::{
    AdherenceState, WeekAdherence, adherence_state, planned_daily_loads, weekly_adherence,
};
use lastwerk_core::metrics::distribution::{
    DistributionBasis, ZoneTimeEntry, distribution_state, weekly_easy_share, window_easy_share,
};
use lastwerk_core::metrics::efficiency::{
    EF_WINDOW_DAYS, EfPoint, EfSession, ef_change_percent, ef_points, ef_rolling_mean,
    ef_rolling_series, ef_trend, ef_trend_code,
};
use lastwerk_core::metrics::form::form_state;
use lastwerk_core::metrics::power_curve::{compare_power_curves, power_curve_trend_code};
use lastwerk_core::metrics::ramp::{ctl_ramp, ramp_series, ramp_state};
use lastwerk_core::metrics::readiness::{ReadinessDeviation, readiness_state, recovery_deviation};
use lastwerk_core::plan::{Plan, RecoveryDay, Rules, reason_keys};
use lastwerk_core::{BestPower, DailyLoad, PmcPoint, Sport, pmc};
use lastwerk_store::{ActivityRecord, RecoveryDayRecord, Store, monday_of};

use crate::dto::{
    DashAdherence, DashDistribution, DashEfPoint, DashEfficiency, DashForm, DashLoadDay, DashPlan,
    DashPowerCurve, DashPowerDuration, DashRamp, DashRampPoint, DashReadiness, DashRecoveryPoint,
    DashRecoveryValue, DashWeekAdherence, DashWeekShare, Dashboard, PmcPointDto,
};
use crate::error::{AppError, Result};
use crate::service::parse_date;

/// Tage des Formverlaufs (SPEC 5.5, T25).
pub const PMC_DAYS: i64 = 120;
/// Tage der Rampenreihe, zwölf Wochen.
pub const RAMP_DAYS: i64 = 84;
/// Wochen der Verteilung.
pub const DISTRIBUTION_WEEKS: u32 = 8;
/// Tage der Effizienzreihe und der Leistungskurve (Zeitraum und Vorzeitraum je so lang).
pub const EF_DAYS: i64 = 90;
pub const POWER_DAYS: i64 = 90;
/// Tage der Erholungsreihe.
pub const READINESS_DAYS: i64 = 60;
/// Tage des Belastungskalenders.
pub const LOAD_DAYS: i64 = 365;
/// So weit zurück liest der Speicher Erholungstage: Reihe plus Basislinie plus Alterstoleranz
/// (Konvention, großzügig).
const RECOVERY_LOOKBACK_DAYS: i64 = 120;
/// Zurückliegende Planeinheit mit R12-Tausch zählt noch als "diese Woche" (Konvention).
const SWAP_RECENT_DAYS: i64 = 6;

/// Rundet auf `digits` Nachkommastellen. Nicht endliche Werte werden verworfen.
fn round_to(v: f64, digits: i32) -> Option<f64> {
    v.is_finite().then(|| {
        let f = 10f64.powi(digits);
        (v * f).round() / f
    })
}

fn r1(v: f64) -> f64 {
    round_to(v, 1).unwrap_or(0.0)
}

fn r1f(v: f32) -> f32 {
    ((f64::from(v) * 10.0).round() / 10.0) as f32
}

fn opt1(v: Option<f64>) -> Option<f64> {
    v.and_then(|x| round_to(x, 1))
}

fn day_before(d: NaiveDate, n: i64) -> NaiveDate {
    d - TimeDelta::days(n)
}

fn round_point(date: NaiveDate, load: f32, p: &PmcPoint) -> PmcPointDto {
    PmcPointDto {
        date: date.to_string(),
        load: r1f(load),
        ctl: r1f(p.ctl),
        atl: r1f(p.atl),
        tsb: r1f(p.tsb),
    }
}

/// Zusammenstellung für "Heute". `today` ist der lokale Tag des Nutzers.
pub fn dashboard(store: &Store, today: &str) -> Result<Dashboard> {
    let today = parse_date(today)?;
    let (rules, _) = planning::load_rules().map_err(|_| AppError::new("internal"))?;
    let plan = store.active_plan()?;

    let curve = form_curve(store, today, plan.as_ref(), &rules)?;
    let activities: Vec<ActivityRecord> = store
        .activities_between(day_before(today, 2 * POWER_DAYS - 1), today)?
        .into_iter()
        .filter(|a| a.is_primary)
        .collect();

    Ok(Dashboard {
        today: today.to_string(),
        form: curve.form,
        pmc: curve.history,
        pmc_first_load_date: curve.first_load_date,
        projection: curve.projection,
        plan: plan.as_ref().and_then(dash_plan),
        ramp: ramp_block(&curve.full, today, &rules),
        distribution: distribution_block(store, &activities, today, &rules)?,
        adherence: adherence_block(store, plan.as_ref(), today)?,
        efficiency: efficiency_block(&activities, today),
        power_curve: power_curve_block(store, &activities, today)?,
        readiness: readiness_block(store, plan.as_ref(), today, &rules)?,
        load_days: load_days_block(store, today)?,
    })
}

// --- Form und Prognose ---------------------------------------------------------------

struct FormCurve {
    form: Option<DashForm>,
    history: Vec<PmcPointDto>,
    projection: Vec<PmcPointDto>,
    first_load_date: Option<String>,
    /// Ungerundeter Verlauf ab dem ersten Tag mit Last bis heute, Grundlage der Rampe.
    full: Vec<PmcPoint>,
}

fn form_curve(
    store: &Store,
    today: NaiveDate,
    plan: Option<&Plan>,
    rules: &Rules,
) -> Result<FormCurve> {
    let first = store.first_load_date()?;
    let window_start = day_before(today, PMC_DAYS - 1);
    let Some(first) = first.filter(|f| *f <= today) else {
        // Ohne zählende Last bis heute gibt es keinen Verlauf und keine Prognose: Nullen wären
        // eine Aussage, die niemand gemessen hat (SPEC 5.4).
        return Ok(FormCurve {
            form: None,
            history: Vec::new(),
            projection: Vec::new(),
            first_load_date: None,
            full: Vec::new(),
        });
    };
    // Beginn am ersten Tag mit Last, falls der vor dem Fenster liegt (eingeschwungene Werte).
    let from = first.min(window_start);
    let params = store.pmc_params()?;
    let actual = store.daily_loads(from, today)?;

    // Prognose: tatsächliche Last bis heute, danach die geplante des aktiven Plans.
    let plan_end = plan
        .and_then(|p| p.weeks.last())
        .map(|w| w.week_start + TimeDelta::days(6))
        .filter(|end| *end > today);
    let mut daily = actual.clone();
    if let (Some(p), Some(_)) = (plan, plan_end) {
        daily.extend(
            planned_daily_loads(&p.weeks)
                .into_iter()
                .filter(|d| d.date > today),
        );
    }
    let to = plan_end.unwrap_or(today);
    let series = pmc(&daily, &params, from, to);
    let load_of: BTreeMap<NaiveDate, f32> = daily.iter().fold(BTreeMap::new(), |mut m, d| {
        *m.entry(d.date).or_insert(0.0) += d.load;
        m
    });
    let load_on = |d: NaiveDate| load_of.get(&d).copied().unwrap_or(0.0);

    let full: Vec<PmcPoint> = series.iter().filter(|p| p.date <= today).copied().collect();
    let history: Vec<PmcPointDto> = full
        .iter()
        .filter(|p| p.date >= window_start)
        .map(|p| round_point(p.date, load_on(p.date), p))
        .collect();
    let projection: Vec<PmcPointDto> = series
        .iter()
        .filter(|p| p.date > today)
        .map(|p| round_point(p.date, load_on(p.date), p))
        .collect();
    let form = full.last().map(|p| DashForm {
        tsb: r1(f64::from(p.tsb)),
        ctl: r1(f64::from(p.ctl)),
        atl: r1(f64::from(p.atl)),
        state: form_state(p.tsb, rules).code().to_owned(),
    });
    Ok(FormCurve {
        form,
        history,
        projection,
        first_load_date: Some(first.to_string()),
        full,
    })
}

fn dash_plan(plan: &Plan) -> Option<DashPlan> {
    let first = plan.weeks.first()?;
    let last = plan.weeks.last()?;
    Some(DashPlan {
        id: plan.plan_id.clone(),
        first_day: first.week_start.to_string(),
        last_day: (last.week_start + TimeDelta::days(6)).to_string(),
    })
}

// --- Rampe ---------------------------------------------------------------------------

fn ramp_block(full: &[PmcPoint], today: NaiveDate, rules: &Rules) -> DashRamp {
    let current = ctl_ramp(full, today);
    let series = ramp_series(full, day_before(today, RAMP_DAYS - 1), today)
        .into_iter()
        .map(|p| DashRampPoint {
            date: p.date.to_string(),
            ramp: r1(f64::from(p.ramp)),
        })
        .collect();
    DashRamp {
        current: current.map(|r| r1(f64::from(r))),
        state: ramp_state(current, rules).code().to_owned(),
        limit: r1(f64::from(rules.r01.max_ctl_rise_per_week)),
        series,
    }
}

// --- Verteilung ----------------------------------------------------------------------

/// Zeit in Zonen der Aktivitäten als Eingabe der Verteilung. Der Kanal folgt aus dem Schema des
/// eingebetteten Katalogs. Zeilen eines unbekannten Schemas (Katalog geändert) entfallen.
fn zone_entries(store: &Store, activities: &[ActivityRecord]) -> Result<Vec<ZoneTimeEntry>> {
    let catalog = lastwerk_store::embedded_zone_catalog();
    let by_id: HashMap<i64, &ActivityRecord> = activities.iter().map(|a| (a.id, a)).collect();
    let mut grouped: BTreeMap<(i64, String), Vec<u32>> = BTreeMap::new();
    for row in store.zone_times()? {
        if !by_id.contains_key(&row.activity_id) {
            continue;
        }
        let Some(scheme) = catalog.scheme(&row.scheme) else {
            continue;
        };
        let seconds = grouped
            .entry((row.activity_id, row.scheme.clone()))
            .or_insert_with(|| vec![0; scheme.zone_count()]);
        if let Some(slot) = (row.zone as usize)
            .checked_sub(1)
            .and_then(|i| seconds.get_mut(i))
        {
            *slot = row.seconds;
        }
    }
    Ok(grouped
        .into_iter()
        .filter_map(|((id, scheme_id), seconds)| {
            let a = by_id.get(&id)?;
            let scheme = catalog.scheme(&scheme_id)?;
            Some(ZoneTimeEntry {
                date: a.row.local_date,
                activity_id: id,
                scheme: scheme_id,
                channel: scheme.basis.channel(),
                seconds,
            })
        })
        .collect())
}

fn distribution_block(
    store: &Store,
    activities: &[ActivityRecord],
    today: NaiveDate,
    rules: &Rules,
) -> Result<DashDistribution> {
    let entries = zone_entries(store, activities)?;
    let basis = DistributionBasis::Priority;
    let first_week = monday_of(today) - TimeDelta::weeks(i64::from(DISTRIBUTION_WEEKS) - 1);
    let weeks = weekly_easy_share(&entries, first_week, DISTRIBUTION_WEEKS, &basis)
        .into_iter()
        .map(|w| DashWeekShare {
            week_start: w.week_start.to_string(),
            easy_percent: w.share.share().map(|s| r1(s * 100.0)),
            total_s: w.share.total_s,
        })
        .collect();
    let window = window_easy_share(&entries, today, 28, &basis);
    Ok(DashDistribution {
        window_easy_percent: window.share().map(|s| r1(s * 100.0)),
        window_total_s: window.total_s,
        min_easy_percent: r1(f64::from(rules.r08.min_easy_share) * 100.0),
        state: distribution_state(&window, rules).code().to_owned(),
        weeks,
    })
}

// --- Planerfüllung -------------------------------------------------------------------

fn adherence_block(store: &Store, plan: Option<&Plan>, today: NaiveDate) -> Result<DashAdherence> {
    let empty = DashAdherence {
        ratio_percent: None,
        state: AdherenceState::NoData.code().to_owned(),
        weeks: Vec::new(),
    };
    let Some(first_week) = plan.and_then(|p| p.weeks.first()) else {
        return Ok(empty);
    };
    let plan = plan.expect("Plan ist gesetzt, die erste Woche existiert");
    let planned = planned_daily_loads(&plan.weeks);
    let done: Vec<DailyLoad> = store.daily_loads(first_week.week_start, today)?;
    // Bis gestern: Die Einheit von heute ist am Morgen noch nicht gefahren und soll nicht als
    // verpasst zählen.
    let as_of = day_before(today, 1);
    let weeks: Vec<WeekAdherence> = weekly_adherence(
        &planned,
        &done,
        first_week.week_start,
        u32::try_from(plan.weeks.len()).unwrap_or(0),
        as_of,
    );
    // Deutung nach der jüngsten Woche, die schon begonnen hat und ein Verhältnis hat.
    let current = weeks
        .iter()
        .rev()
        .find(|w| w.week_start <= today && w.ratio.is_some());
    let ratio = current.and_then(|w| w.ratio);
    Ok(DashAdherence {
        ratio_percent: ratio.map(|r| r1(f64::from(r) * 100.0)),
        state: adherence_state(ratio).code().to_owned(),
        weeks: weeks
            .iter()
            .map(|w| DashWeekAdherence {
                week_start: w.week_start.to_string(),
                planned_total: r1(f64::from(w.planned_total)),
                planned_to_date: r1(f64::from(w.planned_to_date)),
                done_to_date: r1(f64::from(w.done_to_date)),
                complete: w.complete,
                ratio_percent: w.ratio.map(|r| r1(f64::from(r) * 100.0)),
            })
            .collect(),
    })
}

// --- Effizienz -----------------------------------------------------------------------

fn ef_sessions(activities: &[ActivityRecord]) -> Vec<EfSession> {
    activities
        .iter()
        .map(|a| EfSession {
            date: a.row.local_date,
            sport: a.row.sport,
            duration_s: u32::try_from(a.row.duration_s.max(0)).unwrap_or(u32::MAX),
            np_w: a.row.np_w.map(|v| v as f32),
            avg_hr_bpm: a.row.avg_hr.map(|v| v as f32),
        })
        .collect()
}

fn efficiency_block(activities: &[ActivityRecord], today: NaiveDate) -> DashEfficiency {
    let points: Vec<EfPoint> = ef_points(&ef_sessions(activities));
    let from = day_before(today, EF_DAYS - 1);
    let trend = ef_trend(&points, today, EF_WINDOW_DAYS);
    DashEfficiency {
        current: ef_rolling_mean(&points, today, EF_WINDOW_DAYS)
            .and_then(|v| round_to(f64::from(v), 2)),
        change_percent: opt1(ef_change_percent(&points, today, EF_WINDOW_DAYS)),
        trend: ef_trend_code(trend).to_owned(),
        sessions: u32::try_from(
            points
                .iter()
                .filter(|p| p.date >= from && p.date <= today)
                .count(),
        )
        .unwrap_or(u32::MAX),
        series: ef_rolling_series(&points, from, today, EF_WINDOW_DAYS)
            .into_iter()
            .filter_map(|p| {
                Some(DashEfPoint {
                    date: p.date.to_string(),
                    ef: round_to(f64::from(p.ef), 2)?,
                })
            })
            .collect(),
    }
}

// --- Leistungskurve ------------------------------------------------------------------

fn power_curve_block(
    store: &Store,
    activities: &[ActivityRecord],
    today: NaiveDate,
) -> Result<DashPowerCurve> {
    // Zeitraum: letzte 90 Tage, Vorzeitraum: die 90 davor. Nur Radeinheiten, weil Leistung beim
    // Laufen eine andere Größe ist und die Kurve sonst vermischt würde.
    let current_from = day_before(today, POWER_DAYS - 1);
    let previous_from = day_before(today, 2 * POWER_DAYS - 1);
    let cycling: HashMap<i64, NaiveDate> = activities
        .iter()
        .filter(|a| a.row.sport == Sport::Cycling)
        .map(|a| (a.id, a.row.local_date))
        .collect();
    let mut current = Vec::new();
    let mut previous = Vec::new();
    for b in store.best_powers()? {
        let Some(date) = cycling.get(&b.activity_id) else {
            continue;
        };
        let item = BestPower {
            duration_s: b.duration_s,
            watts: b.watts as f32,
        };
        if *date >= current_from && *date <= today {
            current.push(item);
        } else if *date >= previous_from && *date < current_from {
            previous.push(item);
        }
    }
    let durations = compare_power_curves(&current, &previous)
        .into_iter()
        .map(|c| DashPowerDuration {
            duration_s: c.duration_s,
            current_w: c.current_w.and_then(|w| round_to(f64::from(w), 1)),
            previous_w: c.previous_w.and_then(|w| round_to(f64::from(w), 1)),
            change_percent: opt1(c.change_percent),
            trend: power_curve_trend_code(c.trend).to_owned(),
        })
        .collect();
    Ok(DashPowerCurve { durations })
}

// --- Bereitschaft --------------------------------------------------------------------

/// Wert des jüngsten Tages mit Basis und Abweichung. Die Basis folgt aus der Abweichung
/// (`Wert / (1 + Abweichung)`), damit die Rechnung der Basislinie nur in `core` steht.
fn recovery_value(value: Option<f64>, deviation_percent: Option<f64>) -> Option<DashRecoveryValue> {
    let value = value.filter(|v| v.is_finite())?;
    let baseline = deviation_percent
        .filter(|d| *d > -100.0)
        .map(|d| value / (1.0 + d / 100.0));
    Some(DashRecoveryValue {
        value: r1(value),
        baseline: baseline.and_then(|b| round_to(b, 1)),
        deviation_percent: opt1(deviation_percent),
    })
}

fn sleep_hours(r: &RecoveryDayRecord) -> Option<f64> {
    r.sleep_duration_s
        .filter(|s| *s > 0)
        .map(|s| f64::from(s) / 3600.0)
}

/// Hat der aktive Plan die erste harte Einheit wegen der Erholung ersetzt (R12), und liegt
/// diese Einheit heute oder in den letzten Tagen?
fn plan_adjusted(plan: Option<&Plan>, today: NaiveDate) -> bool {
    let from = day_before(today, SWAP_RECENT_DAYS);
    plan.is_some_and(|p| {
        p.weeks
            .iter()
            .flat_map(|w| &w.days)
            .filter(|d| d.date >= from)
            .flat_map(|d| &d.sessions)
            .any(|s| {
                s.reason_keys
                    .iter()
                    .any(|k| k == reason_keys::SESSION_RECOVERY_SWAP)
            })
    })
}

fn readiness_block(
    store: &Store,
    plan: Option<&Plan>,
    today: NaiveDate,
    rules: &Rules,
) -> Result<DashReadiness> {
    let records = store.recovery_days(day_before(today, RECOVERY_LOOKBACK_DAYS), today)?;
    let days: Vec<RecoveryDay> = records.iter().map(RecoveryDay::from).collect();
    let dev: Option<ReadinessDeviation> = recovery_deviation(&days, today, rules);
    let state = readiness_state(dev.as_ref(), rules);

    // Mehrere Zeilen je Tag gibt es nicht (Schlüssel ist das Datum), der Tag der Abweichung
    // ist also eindeutig.
    let latest = dev
        .as_ref()
        .and_then(|d| records.iter().find(|r| r.date == d.date));
    let series_from = day_before(today, READINESS_DAYS - 1);
    let series = records
        .iter()
        .filter(|r| r.date >= series_from)
        .map(|r| DashRecoveryPoint {
            date: r.date.to_string(),
            hrv_ms: opt1(r.hrv_overnight_ms),
            resting_hr_bpm: opt1(r.resting_hr_bpm),
            sleep_h: opt1(sleep_hours(r)),
        })
        .collect();
    Ok(DashReadiness {
        state: state.code().to_owned(),
        last_date: records.last().map(|r| r.date.to_string()),
        date: dev.as_ref().map(|d| d.date.to_string()),
        hrv: latest
            .and_then(|r| recovery_value(r.hrv_overnight_ms, dev.and_then(|d| d.hrv_percent))),
        resting_hr: latest
            .and_then(|r| recovery_value(r.resting_hr_bpm, dev.and_then(|d| d.resting_hr_percent))),
        sleep_h: latest
            .and_then(|r| recovery_value(sleep_hours(r), dev.and_then(|d| d.sleep_percent))),
        // Body Battery ist ein Wert von Garmin ohne eigene Basis in `core`: angezeigt wird der
        // jüngste Tag mit Wert, ohne Deutung (T22 liefert die Spalten).
        body_battery_high: records
            .iter()
            .rev()
            .find_map(|r| r.body_battery_high)
            .map(f64::from),
        body_battery_low: records
            .iter()
            .rev()
            .find(|r| r.body_battery_high.is_some())
            .and_then(|r| r.body_battery_low)
            .map(f64::from),
        plan_adjusted: plan_adjusted(plan, today),
        series,
    })
}

// --- Belastungskalender --------------------------------------------------------------

fn load_days_block(store: &Store, today: NaiveDate) -> Result<Vec<DashLoadDay>> {
    Ok(store
        .daily_loads(day_before(today, LOAD_DAYS - 1), today)?
        .into_iter()
        .map(|d| DashLoadDay {
            date: d.date.to_string(),
            load: r1(f64::from(d.load)),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_keeps_one_decimal_and_drops_non_finite() {
        assert_eq!(r1(1.249), 1.2);
        assert_eq!(r1(1.25), 1.3);
        assert_eq!(r1(-0.04), -0.0);
        assert_eq!(round_to(1.2345, 2), Some(1.23));
        assert_eq!(round_to(f64::NAN, 1), None);
        assert_eq!(opt1(Some(f64::INFINITY)), None);
        assert_eq!(r1f(12.34), 12.3);
    }

    /// Basis folgt aus der Abweichung: Wert 45 bei -10 Prozent heißt Mittel 50
    /// (`45 / (1 - 0,10) = 50`). Ohne Abweichung gibt es keine Basis.
    #[test]
    fn baseline_follows_from_value_and_deviation() {
        let v = recovery_value(Some(45.0), Some(-10.0)).unwrap();
        assert_eq!(v.value, 45.0);
        assert_eq!(v.baseline, Some(50.0));
        assert_eq!(v.deviation_percent, Some(-10.0));
        let none = recovery_value(Some(45.0), None).unwrap();
        assert_eq!((none.baseline, none.deviation_percent), (None, None));
        assert!(recovery_value(None, Some(1.0)).is_none());
        assert!(recovery_value(Some(f64::NAN), None).is_none());
        // -100 Prozent hieße Wert 0, keine Basis ableitbar.
        assert_eq!(
            recovery_value(Some(1.0), Some(-100.0)).unwrap().baseline,
            None
        );
    }

    #[test]
    fn sleep_of_zero_seconds_is_missing() {
        let mut r = RecoveryDayRecord {
            date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            hrv_overnight_ms: None,
            resting_hr_bpm: None,
            sleep_duration_s: Some(0),
            sleep_score: None,
            body_battery_high: None,
            body_battery_low: None,
            source: "t".into(),
            imported_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
        };
        assert_eq!(sleep_hours(&r), None);
        r.sleep_duration_s = Some(27_000);
        assert_eq!(sleep_hours(&r), Some(7.5));
    }
}
