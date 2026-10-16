//! Vollabzug aus Garmin (T39, T40, SPEC 5.6, 8.6) gegen eine Attrappe der Quelle: Schwellen aus
//! Garmin ohne Eingabe samt Verlauf und Schwellentempo, Vorrang manueller Werte, Werte ohne
//! Datum, Tages- und Trainingswerte, S3 nur mit Freigabe je Bereich, nie Menstruations- oder
//! Schwangerschaftsdaten. Alle Daten synthetisch, kein Netz, alle Dateien unter `target/`.

use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use lastwerk_cli::garmin::{DETAIL_DAYS, GarminSource, SyncOptions, SyncSummary, sync};
use lastwerk_core::Sport;
use lastwerk_garmin::{
    ActivitySummary, GarminError, Profile, SensitiveConsent, SeriesSport, ThresholdSeries,
};
use lastwerk_store::{
    BodyDay, BpDay, LoadStatus, NewThreshold, NutritionDay, SETTING_GARMIN_SHARE_BODY,
    SETTING_GARMIN_SHARE_HYDRATION, SETTING_GARMIN_SHARE_NUTRITION, SensitiveDay, Store,
    ThresholdKind, ThresholdOrigin, TrainingDay, WellnessDay,
};
use lastwerk_testkit::fixtures::{LOCAL_OFFSET_S, START_FIT};
use lastwerk_testkit::generators::{ActivityParams, PowerProfile, activity};
use serde_json::{Value, json};
use zip::write::{SimpleFileOptions, ZipWriter};

fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli-garmin-full-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap()
}

/// Radfahrt am 10. Mai 2026 (`START_FIT`, lokal UTC+2), 1800 s mit konstant 200 W.
fn ride_zip() -> Vec<u8> {
    let fit = activity(&ActivityParams::cycling(START_FIT, LOCAL_OFFSET_S, 1800));
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    w.start_file("1001_ACTIVITY.fit", SimpleFileOptions::default())
        .unwrap();
    w.write_all(&fit).unwrap();
    w.finish().unwrap().into_inner()
}

/// Lauf am 10. Mai 2026, 3600 s mit konstant 3,0 m/s und ohne Leistung.
fn run_zip() -> Vec<u8> {
    let mut p = ActivityParams::cycling(START_FIT, LOCAL_OFFSET_S, 3600);
    p.sport = lastwerk_testkit::generators::Sport::Running;
    p.power = PowerProfile::None;
    p.speed_mps = Some(3.0);
    let fit = activity(&p);
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    w.start_file("1001_ACTIVITY.fit", SimpleFileOptions::default())
        .unwrap();
    w.write_all(&fit).unwrap();
    w.finish().unwrap().into_inner()
}

/// Handrechnung (SPEC 8.1): NP = 200 W (konstant), IF = 200 / 250 = 0,8,
/// TSS = 1800 * 200 * 0,8 / (250 * 3600) * 100 = 288 000 / 900 000 * 100 = 32.
const TSS_AT_FTP_250: f64 = 32.0;
/// Mit FTP 200: IF = 1, TSS = 1800 * 200 * 1 / (200 * 3600) * 100 = 50.
const TSS_AT_FTP_200: f64 = 50.0;

#[derive(Default)]
struct Fake {
    ftp: Option<Value>,
    zones: Option<Value>,
    profile: Option<Value>,
    /// Antwort der Laktatschwelle; ohne Wert die Vorgabe mit Herzfrequenz und Tempo.
    lactate: Option<Value>,
    /// Zeilen des FTP-Verlaufs (`functionalThresholdPower/range`).
    ftp_series: Option<Value>,
    /// Die Aktivität ist ein Lauf statt einer Radfahrt.
    running: bool,
    log: Mutex<Vec<String>>,
}

impl Fake {
    fn record(&self, entry: &str) {
        self.log.lock().unwrap().push(entry.to_owned());
    }

    fn count(&self, prefix: &str) -> usize {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.starts_with(prefix))
            .count()
    }

    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

impl GarminSource for Fake {
    fn social_profile(&self) -> Result<Profile, GarminError> {
        self.record("profile");
        Ok(Profile {
            display_name: "ANZEIGENAME".into(),
        })
    }
    fn activities(&self, start: u32, _limit: u32) -> Result<Vec<ActivitySummary>, GarminError> {
        self.record("activities");
        if start > 0 {
            return Ok(Vec::new());
        }
        Ok(vec![ActivitySummary {
            activity_id: 1001,
            start_time_local: NaiveDateTime::new(
                d("2026-05-10"),
                NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            ),
            activity_type: if self.running { "running" } else { "cycling" }.into(),
        }])
    }
    fn activity_fit(&self, _id: u64) -> Result<Vec<u8>, GarminError> {
        self.record("fit");
        Ok(if self.running { run_zip() } else { ride_zip() })
    }
    fn hrv_range(&self, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.record("hrv");
        Ok(Value::Null)
    }
    fn rhr_range(&self, _n: &str, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.record("rhr");
        Ok(Value::Null)
    }
    fn sleep_day(&self, _n: &str, _d: NaiveDate) -> Result<Value, GarminError> {
        self.record("sleep");
        Err(GarminError::NotAvailable)
    }
    fn body_battery(&self, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.record("bb");
        Ok(Value::Null)
    }
    fn user_settings(&self) -> Result<Value, GarminError> {
        self.record("user_settings");
        Ok(self.profile.clone().unwrap_or(Value::Null))
    }
    fn cycling_ftp(&self) -> Result<Value, GarminError> {
        self.record("ftp");
        Ok(self.ftp.clone().unwrap_or(Value::Null))
    }
    fn lactate_threshold(&self) -> Result<Value, GarminError> {
        self.record("lactate");
        Ok(self.lactate.clone().unwrap_or_else(
            || json!([{"calendarDate": "2026-04-01", "speed": 0.33, "heartRate": 168}]),
        ))
    }
    fn heart_rate_zones(&self) -> Result<Value, GarminError> {
        self.record("zones");
        Ok(self.zones.clone().unwrap_or(Value::Null))
    }
    fn daily_steps(&self, start: NaiveDate, end: NaiveDate) -> Result<Value, GarminError> {
        self.record("steps");
        let mut rows = Vec::new();
        let mut day = start;
        while day <= end {
            rows.push(json!({"calendarDate": day.to_string(), "totalSteps": 8000, "totalDistance": 6400, "stepGoal": 7000}));
            day = day.succ_opt().unwrap();
        }
        Ok(Value::Array(rows))
    }
    fn heart_rate_day(&self, name: &str, date: NaiveDate) -> Result<Value, GarminError> {
        assert_eq!(name, "ANZEIGENAME");
        self.record("heart_rate_day");
        Ok(
            json!({"calendarDate": date.to_string(), "minHeartRate": 45, "maxHeartRate": 171, "restingHeartRate": 48}),
        )
    }
    fn respiration_day(&self, date: NaiveDate) -> Result<Value, GarminError> {
        self.record("respiration");
        Ok(
            json!({"calendarDate": date.to_string(), "avgWakingRespirationValue": 14.0, "avgSleepRespirationValue": 12.5}),
        )
    }
    fn spo2_day(&self, date: NaiveDate) -> Result<Value, GarminError> {
        self.record("spo2");
        // Anderes Datum als angefragt: die Antwort wird verworfen.
        Ok(
            json!({"calendarDate": (date - chrono::TimeDelta::days(1)).to_string(), "averageSpO2": 96.0}),
        )
    }
    fn intensity_minutes_day(&self, date: NaiveDate) -> Result<Value, GarminError> {
        self.record("im");
        Ok(json!({"calendarDate": date.to_string(), "moderateMinutes": 20, "vigorousMinutes": 15}))
    }
    fn stress_day(&self, date: NaiveDate) -> Result<Value, GarminError> {
        self.record("stress");
        Ok(json!({"calendarDate": date.to_string(), "avgStressLevel": -1, "maxStressLevel": 80}))
    }
    fn training_readiness(&self, date: NaiveDate) -> Result<Value, GarminError> {
        self.record("readiness");
        Ok(
            json!([{"calendarDate": date.to_string(), "score": 70, "level": "HIGH", "acuteLoad": 400, "inputContext": "AFTER_WAKEUP_RESET"}]),
        )
    }
    fn fitness_age(&self, _date: NaiveDate) -> Result<Value, GarminError> {
        self.record("fitness_age");
        Ok(json!({"chronologicalAge": 36, "fitnessAge": 31.5}))
    }
    fn endurance_score(&self, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.record("endurance");
        Ok(json!({"enduranceScoreDTO": {"calendarDate": "2026-05-12", "overallScore": 6100}}))
    }
    fn hill_score(&self, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.record("hill");
        Ok(json!({"periodAvgScore": {"2026-05-11": 55}}))
    }
    fn training_load_activities(&self, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.record("loads");
        Ok(
            json!([{"activityId": 1001, "activityTrainingLoad": 95.0, "trainingEffectLabel": "TEMPO", "startLocal": "2026-05-10 08:00:00"}]),
        )
    }
    fn hydration_day(&self, _c: SensitiveConsent, date: NaiveDate) -> Result<Value, GarminError> {
        self.record("hydration");
        Ok(json!({"calendarDate": date.to_string(), "valueInML": 1800.0}))
    }
    fn threshold_range(
        &self,
        series: ThresholdSeries,
        sport: SeriesSport,
        _s: NaiveDate,
        _e: NaiveDate,
    ) -> Result<Value, GarminError> {
        self.record(&format!("series {series:?} {sport:?}"));
        Ok(match series {
            ThresholdSeries::FunctionalThresholdPower => {
                self.ftp_series.clone().unwrap_or(Value::Null)
            }
            _ => json!([]),
        })
    }
    fn max_metrics_range(&self, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.record("maxmet");
        Ok(
            json!([{"generic": {"calendarDate": "2026-05-09", "vo2MaxPreciseValue": 51.3}, "cycling": null}]),
        )
    }
    fn training_status_day(&self, date: NaiveDate) -> Result<Value, GarminError> {
        self.record("training_status");
        Ok(json!({"latestTrainingStatusData": {"42": {
            "calendarDate": date.to_string(), "trainingStatusFeedbackPhrase": "PRODUCTIVE_2",
            "primaryTrainingDevice": true,
            "acuteTrainingLoadDTO": {"dailyTrainingLoadAcute": 410, "dailyTrainingLoadChronic": 380, "dailyAcuteChronicWorkloadRatio": 1.1}}}}))
    }
    fn load_balance(&self, _date: NaiveDate) -> Result<Value, GarminError> {
        self.record("load_balance");
        Ok(json!({"metricsTrainingLoadBalanceDTOMap": {"42": {
            "monthlyLoadAerobicLow": 300.0, "monthlyLoadAerobicHigh": 500.0, "monthlyLoadAnaerobic": 90.0,
            "trainingBalanceFeedbackPhrase": "BALANCED", "primaryTrainingDevice": true}}}))
    }
    fn race_predictions_latest(&self, name: &str) -> Result<Value, GarminError> {
        assert_eq!(name, "ANZEIGENAME");
        self.record("race_latest");
        Ok(
            json!({"calendarDate": "2026-05-14", "time5K": 1320, "time10K": 2760, "timeHalfMarathon": 6120, "timeMarathon": 12900}),
        )
    }
    fn race_predictions_daily(
        &self,
        _name: &str,
        _s: NaiveDate,
        _e: NaiveDate,
    ) -> Result<Value, GarminError> {
        self.record("race_daily");
        Err(GarminError::NotAvailable)
    }
    fn running_tolerance(&self, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.record("tolerance");
        Ok(
            json!([{"calendarDate": "2026-05-13", "acuteDistance": 30000, "acuteImpactLoad": 31000, "acuteTolerance": 60000, "runningToleranceFeedBackPhrase": "LOW_LOAD"}]),
        )
    }
    fn floors_day(&self, _date: NaiveDate) -> Result<Value, GarminError> {
        self.record("floors");
        Ok(json!({
            "floorsValueDescriptorDTOList": [{"key": "floorsAscended", "index": 2}, {"key": "floorsDescended", "index": 3}],
            "floorValuesArray": [["a", "b", 4, 2], ["c", "d", 1, 1]]
        }))
    }
    fn weight_range(
        &self,
        _c: SensitiveConsent,
        _s: NaiveDate,
        _e: NaiveDate,
    ) -> Result<Value, GarminError> {
        self.record("weight");
        Ok(
            json!({"dailyWeightSummaries": [{"allWeightMetrics": [{"calendarDate": "2026-05-12", "date": 1, "weight": 71500.0, "bodyFat": 15.5}]}]}),
        )
    }
    fn blood_pressure_range(
        &self,
        _c: SensitiveConsent,
        _s: NaiveDate,
        _e: NaiveDate,
    ) -> Result<Value, GarminError> {
        self.record("blood_pressure");
        Ok(
            json!({"measurementSummaries": [{"measurements": [{"systolic": 120, "diastolic": 80, "measurementTimestampLocal": "2026-05-12T07:00:00.0"}]}]}),
        )
    }
    fn nutrition_day(&self, _c: SensitiveConsent, date: NaiveDate) -> Result<Value, GarminError> {
        self.record("nutrition");
        Ok(
            json!({"mealDate": date.to_string(), "dailyNutritionContent": {"calories": 2300, "carbs": 280, "fat": 75, "protein": 110}}),
        )
    }
}

fn store(days: u32) -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.set_garmin_backfill_days(days).unwrap();
    s
}

fn run(store: &mut Store, fake: &Fake, dir: &Path, today: &str) -> SyncSummary {
    sync(
        store,
        fake,
        dir,
        &SyncOptions::new(d(today), now()),
        &mut |_| {},
    )
    .unwrap()
}

fn ride_load(store: &Store) -> (LoadStatus, Option<f64>, Option<String>) {
    let loads = store.activity_loads().unwrap();
    assert_eq!(loads.len(), 1);
    (loads[0].status, loads[0].value, loads[0].missing.clone())
}

fn ftp_fake(watts: u32, date: &str) -> Fake {
    Fake {
        ftp: Some(
            json!({"calendarDate": date, "functionalThresholdPower": watts, "sport": "CYCLING"}),
        ),
        ..Fake::default()
    }
}

#[test]
fn an_activity_before_today_gets_a_load_from_an_earlier_garmin_threshold() {
    let dir = work_dir("earlier").join("fit");
    let mut store = store(10);
    // FTP gilt bei Garmin seit dem 1. März, die Fahrt war am 10. Mai, heute ist der 14. Mai.
    let fake = ftp_fake(250, "2026-03-01");
    let s = run(&mut store, &fake, &dir, "2026-05-14");
    assert_eq!(s.aborted, None);
    let (status, value, missing) = ride_load(&store);
    assert_eq!(status, LoadStatus::Ok);
    assert!((value.unwrap() - TSS_AT_FTP_250).abs() < 1e-3, "{value:?}");
    assert_eq!(missing, None);

    let th = store.list_thresholds().unwrap();
    let ftp = th.iter().find(|t| t.kind == ThresholdKind::FtpW).unwrap();
    assert_eq!(ftp.origin, ThresholdOrigin::Garmin);
    assert_eq!(ftp.valid_from, d("2026-03-01"));
    assert_eq!(ftp.sport, Some(Sport::Cycling));
    // Laktatschwelle mit ihrem Datum, nur für Lauf.
    let lthr = th
        .iter()
        .find(|t| t.kind == ThresholdKind::LthrBpm)
        .unwrap();
    assert_eq!(
        (lthr.sport, lthr.valid_from, lthr.value),
        (Some(Sport::Running), d("2026-04-01"), 168.0)
    );
    // Das Schwellentempo kommt seit T40 mit: 0,33 × 10 = 3,3 m/s (Q30).
    let speed = th
        .iter()
        .find(|t| t.kind == ThresholdKind::RunSpeedMps)
        .unwrap();
    assert_eq!(
        (speed.sport, speed.valid_from, speed.origin),
        (
            Some(Sport::Running),
            d("2026-04-01"),
            ThresholdOrigin::Garmin
        )
    );
    assert!((speed.value - 3.3).abs() < 1e-9);
}

/// Handrechnung (SPEC 8.1, rTSS): Schwelle 0,30 × 10 = 3,0 m/s, Lauf 3600 s mit 3,0 m/s,
/// IF = 3,0 / 3,0 = 1, rTSS = 1 h × 1² × 100 = 100.
const RTSS_AT_THRESHOLD: f64 = 100.0;

#[test]
fn a_run_gets_a_load_from_the_threshold_pace_of_garmin() {
    let dir = work_dir("pace").join("fit");
    let mut store = store(10);
    let fake = Fake {
        running: true,
        lactate: Some(json!([
            {"calendarDate": "2026-04-01", "speed": 0.30, "hearRate": null},
            {"calendarDate": "2026-04-01", "speed": null, "hearRate": 168}
        ])),
        ..Fake::default()
    };
    run(&mut store, &fake, &dir, "2026-05-14");
    let (status, value, missing) = ride_load(&store);
    assert_eq!((status, missing), (LoadStatus::Ok, None));
    assert!(
        (value.unwrap() - RTSS_AT_THRESHOLD).abs() < 1e-3,
        "{value:?}"
    );
}

#[test]
fn the_ftp_history_gives_an_earlier_activity_the_value_valid_on_its_day() {
    let dir = work_dir("history").join("fit");
    let mut store = store(120);
    // Garmin nennt heute 250 W seit dem 12. Mai, der Verlauf kennt 200 W seit dem 1. März.
    // Die Fahrt am 10. Mai gilt also mit 200 W, nicht mit dem jüngsten Wert.
    let fake = Fake {
        ftp: Some(json!({"calendarDate": "2026-05-12", "functionalThresholdPower": 250})),
        ftp_series: Some(json!([
            {"from": "2026-03-01", "until": "2026-03-01", "series": "cycling", "value": 200.0},
            {"from": "2026-05-12", "until": "2026-05-12", "series": "cycling", "value": 250.0},
            {"from": "2026-04-01", "until": "2026-04-01", "series": "running", "value": 330.0}
        ])),
        ..Fake::default()
    };
    let s = run(&mut store, &fake, &dir, "2026-05-14");
    let (status, value, _) = ride_load(&store);
    assert_eq!(status, LoadStatus::Ok);
    assert!((value.unwrap() - TSS_AT_FTP_200).abs() < 1e-3, "{value:?}");
    let ftp: Vec<(NaiveDate, f64)> = store
        .list_thresholds()
        .unwrap()
        .into_iter()
        .filter(|t| t.kind == ThresholdKind::FtpW)
        .map(|t| (t.valid_from, t.value))
        .collect();
    assert_eq!(ftp.len(), 2, "{ftp:?}");
    assert!(ftp.contains(&(d("2026-03-01"), 200.0)));
    assert!(ftp.contains(&(d("2026-05-12"), 250.0)));
    // Drei Verläufe (FTP Rad, LTHR und Tempo Lauf), je ein Stück: 121 Tage < 366.
    assert_eq!(fake.count("series"), 3);
    assert_eq!(fake.count("series FunctionalThresholdPower Cycling"), 1);
    // Konto zuerst, dann Aktivitäten, dann Erholung und Tageswerte (T40).
    let log = fake.log();
    let pos = |name: &str| log.iter().position(|e| e == name).unwrap();
    assert!(pos("user_settings") < pos("activities"));
    assert!(pos("fit") < pos("profile"));
    assert!(pos("profile") < pos("steps"));
    assert_eq!(s.aborted, None);
}

#[test]
fn a_later_garmin_threshold_recomputes_known_activities() {
    let dir = work_dir("recompute").join("fit");
    let mut store = store(10);
    // Erster Lauf ohne FTP bei Garmin: die Fahrt bekommt keine Last und nennt, was fehlt.
    let s = run(&mut store, &Fake::default(), &dir, "2026-05-14");
    assert_eq!(s.thresholds_changed, 2); // Laktatschwelle: Herzfrequenz und Tempo
    let (status, _, missing) = ride_load(&store);
    assert_eq!(status, LoadStatus::MissingThreshold);
    assert_eq!(missing.as_deref(), Some("ftp_w"));

    // Später liefert Garmin eine FTP mit früherem Datum: die bekannte Fahrt wird neu gerechnet.
    let fake = ftp_fake(200, "2026-01-01");
    let s = run(&mut store, &fake, &dir, "2026-05-15");
    assert_eq!(s.thresholds_changed, 1);
    assert_eq!(s.recomputed, 1);
    let (status, value, _) = ride_load(&store);
    assert_eq!(status, LoadStatus::Ok);
    assert!((value.unwrap() - TSS_AT_FTP_200).abs() < 1e-3, "{value:?}");

    // Ein dritter Lauf mit denselben Antworten ändert nichts und rechnet nichts neu.
    let s = run(&mut store, &fake, &dir, "2026-05-15");
    assert_eq!((s.thresholds_changed, s.recomputed), (0, 0));
    assert_eq!(
        (s.wellness_days, s.training_days, s.garmin_loads),
        (0, 0, 0)
    );
}

#[test]
fn a_manual_value_overrides_the_garmin_value_on_the_same_day() {
    let dir = work_dir("manual").join("fit");
    let mut store = store(10);
    store
        .insert_threshold(&NewThreshold {
            sport: Some(Sport::Cycling),
            kind: ThresholdKind::FtpW,
            value: 200.0,
            valid_from: d("2026-03-01"),
            origin: ThresholdOrigin::Manual,
            note: None,
        })
        .unwrap();
    let fake = ftp_fake(250, "2026-03-01");
    run(&mut store, &fake, &dir, "2026-05-14");
    // Beide Werte stehen in der Tabelle, gerechnet wird mit dem manuellen.
    assert_eq!(
        store
            .list_thresholds()
            .unwrap()
            .iter()
            .filter(|t| t.kind == ThresholdKind::FtpW)
            .count(),
        2
    );
    let (status, value, _) = ride_load(&store);
    assert_eq!(status, LoadStatus::Ok);
    assert!((value.unwrap() - TSS_AT_FTP_200).abs() < 1e-3, "{value:?}");
}

#[test]
fn values_without_a_date_start_at_the_window_then_change_from_today() {
    let dir = work_dir("undated").join("fit");
    let mut store = store(10);
    let mut fake = Fake {
        zones: Some(json!([
            {"sport": "DEFAULT", "maxHeartRateUsed": 190, "restingHeartRateUsed": 48},
            {"sport": "RUNNING", "maxHeartRateUsed": 190, "restingHeartRateUsed": 48}
        ])),
        ..Fake::default()
    };
    let s = run(&mut store, &fake, &dir, "2026-05-14");
    let hr_max = |store: &Store| -> Vec<(NaiveDate, f64)> {
        store
            .list_thresholds()
            .unwrap()
            .into_iter()
            .filter(|t| t.kind == ThresholdKind::HrMaxBpm)
            .map(|t| {
                assert_eq!((t.sport, t.origin), (None, ThresholdOrigin::Garmin));
                (t.valid_from, t.value)
            })
            .collect()
    };
    assert_eq!(hr_max(&store), [(s.window_start, 190.0)]);
    // Gleicher Wert: nichts Neues. Geänderter Wert: ab heute.
    run(&mut store, &fake, &dir, "2026-05-15");
    assert_eq!(hr_max(&store).len(), 1);
    fake.zones =
        Some(json!([{"sport": "DEFAULT", "maxHeartRateUsed": 188, "restingHeartRateUsed": 47}]));
    run(&mut store, &fake, &dir, "2026-05-16");
    assert_eq!(
        hr_max(&store),
        [(s.window_start, 190.0), (d("2026-05-16"), 188.0)]
    );
}

#[test]
fn sex_and_birth_year_come_from_garmin_unless_the_user_set_them() {
    let dir = work_dir("athlete").join("fit");
    let mut store = store(10);
    let fake = Fake {
        profile: Some(
            json!({"userData": {"gender": "MALE", "birthDate": "1988-07-01", "weight": 70000.0, "vo2MaxRunning": 54.0}}),
        ),
        ..Fake::default()
    };
    run(&mut store, &fake, &dir, "2026-05-14");
    assert_eq!(
        store.get_setting("athlete.sex").unwrap().as_deref(),
        Some("male")
    );
    assert_eq!(
        store.get_setting("athlete.birth_year").unwrap().as_deref(),
        Some("1988")
    );
    // Das Gewicht wird nie gespeichert, auch nicht als Einstellung.
    let export = store.export_user_data().unwrap().to_json().unwrap();
    assert!(!export.contains("70000"), "{export}");
    let training: Vec<TrainingDay> = store.garmin_rows(d("2026-05-14"), d("2026-05-14")).unwrap();
    assert_eq!(training[0].vo2max_running, Some(54.0));

    // Setzt der Nutzer das Geschlecht selbst, lässt Garmin es stehen.
    store.set_setting("athlete.sex", "female").unwrap();
    store.set_setting("athlete.sex_origin", "manual").unwrap();
    run(&mut store, &fake, &dir, "2026-05-15");
    assert_eq!(
        store.get_setting("athlete.sex").unwrap().as_deref(),
        Some("female")
    );
}

#[test]
fn daily_and_training_values_are_written_and_implausible_ones_dropped() {
    let dir = work_dir("daily").join("fit");
    let mut store = store(40);
    let s = run(&mut store, &Fake::default(), &dir, "2026-05-14");
    // Schritte im ganzen Fenster (41 Tage), die übrigen Tageswerte für die letzten 28 Tage.
    let rows: Vec<WellnessDay> = store.garmin_rows(d("2026-01-01"), d("2026-12-31")).unwrap();
    assert_eq!(rows.len(), 41);
    let detailed = rows.iter().filter(|r| r.hr_min_bpm.is_some()).count();
    assert_eq!(detailed as i64, DETAIL_DAYS);
    let last = rows.last().unwrap();
    assert_eq!(last.steps, Some(8000.0));
    assert_eq!(
        (last.hr_min_bpm, last.hr_max_bpm),
        (Some(45.0), Some(171.0))
    );
    assert_eq!(last.respiration_sleep_brpm, Some(12.5));
    assert_eq!(last.intensity_vigorous_min, Some(15.0));
    // Stress -1 heißt bei Garmin "keine Messung": verworfen, der Höchstwert bleibt.
    assert_eq!((last.stress_avg, last.stress_max), (None, Some(80.0)));
    assert!(s.dropped_values >= DETAIL_DAYS as u32);
    // SpO2 mit falschem Datum kommt nie in den Speicher.
    assert!(rows.iter().all(|r| r.spo2_avg_pct.is_none()));

    let training: Vec<TrainingDay> = store.garmin_rows(d("2026-01-01"), d("2026-12-31")).unwrap();
    let by_date: HashMap<NaiveDate, &TrainingDay> = training.iter().map(|t| (t.date, t)).collect();
    assert_eq!(by_date[&d("2026-05-12")].endurance_score, Some(6100.0));
    assert_eq!(by_date[&d("2026-05-11")].hill_score, Some(55.0));
    assert_eq!(by_date[&d("2026-05-14")].fitness_age, Some(31.5));
    assert_eq!(
        by_date[&d("2026-05-14")].readiness_level.as_deref(),
        Some("HIGH")
    );
    let load = store.garmin_activity_load(1001).unwrap().unwrap();
    assert_eq!(load.training_load, Some(95.0));
    assert_eq!(load.training_effect.as_deref(), Some("TEMPO"));

    // T40: Trainingsstatus und Etagen für die Detailtage, VO2max-Verlauf, Belastungsfokus,
    // Rennprognosen und Lauftoleranz.
    let today = by_date[&d("2026-05-14")];
    assert_eq!(today.training_status.as_deref(), Some("productive"));
    // Die akute Last der Trainingsbereitschaft (400) bleibt, die chronische kommt dazu.
    assert_eq!(
        (
            today.garmin_acute_load,
            today.garmin_chronic_load,
            today.garmin_acwr
        ),
        (Some(400.0), Some(380.0), Some(1.1))
    );
    assert_eq!(today.load_aerobic_high, Some(500.0));
    assert_eq!(today.load_balance.as_deref(), Some("balanced"));
    assert_eq!(
        (today.race_5k_s, today.race_marathon_s),
        (Some(1320.0), Some(12900.0))
    );
    assert_eq!(by_date[&d("2026-05-09")].vo2max_running, Some(51.3));
    let tolerance = by_date[&d("2026-05-13")];
    assert_eq!(
        (
            tolerance.run_tolerance_m,
            tolerance.run_tolerance_level.as_deref()
        ),
        (Some(60000.0), Some("low_load"))
    );
    // Handrechnung Etagen: auf 4 + 1 = 5, ab 2 + 1 = 3.
    assert_eq!((last.floors_up, last.floors_down), (Some(5.0), Some(3.0)));
    let floors_days = rows.iter().filter(|r| r.floors_up.is_some()).count();
    assert_eq!(floors_days as i64, DETAIL_DAYS);
    // Ohne Rennprognosen-Verlauf (404) läuft der Abgleich weiter.
    assert_eq!(s.aborted, None);
}

#[test]
fn sensitive_values_are_fetched_only_for_released_categories() {
    let dir = work_dir("s3-categories").join("fit");
    let mut store = store(10);
    store
        .set_setting(SETTING_GARMIN_SHARE_BODY, "true")
        .unwrap();
    let fake = Fake::default();
    let s = run(&mut store, &fake, &dir, "2026-05-14");
    assert!(s.sensitive_released);
    assert_eq!(fake.count("weight"), 1);
    assert_eq!(
        (
            fake.count("blood_pressure"),
            fake.count("nutrition"),
            fake.count("hydration")
        ),
        (0, 0, 0)
    );
    let body: Vec<BodyDay> = store.garmin_rows(d("2026-01-01"), d("2026-12-31")).unwrap();
    assert_eq!(body.len(), 1);
    // Handrechnung: 71 500 g = 71,5 kg.
    assert_eq!(
        (body[0].weight_kg, body[0].body_fat_pct),
        (Some(71.5), Some(15.5))
    );
    let bp: Vec<BpDay> = store.garmin_rows(d("2026-01-01"), d("2026-12-31")).unwrap();
    assert!(bp.is_empty());

    store
        .set_setting(SETTING_GARMIN_SHARE_NUTRITION, "true")
        .unwrap();
    let fake = Fake::default();
    run(&mut store, &fake, &dir, "2026-05-14");
    // Fenster 7. bis 14. Mai: ein Aufruf je Tag.
    assert_eq!(fake.count("nutrition"), 8);
    assert_eq!(fake.count("blood_pressure"), 0);
    let food: Vec<NutritionDay> = store.garmin_rows(d("2026-01-01"), d("2026-12-31")).unwrap();
    assert_eq!(food.len(), 8);
    assert_eq!(food[0].calories_kcal, Some(2300.0));
}

#[test]
fn sensitive_values_only_with_release_and_never_menstrual_or_pregnancy_data() {
    // Ohne Freigabe: kein einziger Aufruf der Hydration, keine Zeile.
    let dir = work_dir("s3-off").join("fit");
    let mut store = store(10);
    let fake = Fake::default();
    let s = run(&mut store, &fake, &dir, "2026-05-14");
    assert!(!s.sensitive_released);
    assert_eq!(fake.count("hydration"), 0);
    let rows: Vec<SensitiveDay> = store.garmin_rows(d("2026-01-01"), d("2026-12-31")).unwrap();
    assert!(rows.is_empty());
    // Ein anderer Wert als genau `true` gibt nicht frei.
    store
        .set_setting(SETTING_GARMIN_SHARE_HYDRATION, "yes")
        .unwrap();
    let fake = Fake::default();
    run(&mut store, &fake, &dir, "2026-05-14");
    assert_eq!(fake.count("hydration"), 0);

    // Mit Freigabe: ein Aufruf je Tag der letzten Tage des Fensters, eigene Tabelle. Nach dem
    // ersten Lauf beginnt das Fenster 7 Tage vor dem letzten Abgleich: 7. bis 14. Mai.
    store
        .set_setting(SETTING_GARMIN_SHARE_HYDRATION, "true")
        .unwrap();
    let fake = Fake::default();
    let s = run(&mut store, &fake, &dir, "2026-05-14");
    assert!(s.sensitive_released);
    assert_eq!(fake.count("hydration"), 8);
    let rows: Vec<SensitiveDay> = store.garmin_rows(d("2026-01-01"), d("2026-12-31")).unwrap();
    assert_eq!(rows.len(), 8);
    assert_eq!(rows[0].hydration_ml, Some(1800.0));

    // In keinem Lauf ein Aufruf zu Menstruation oder Schwangerschaft: Die Quelle hat dafür
    // keine Methode, und der Client lehnt die Pfade ab (Test in `lastwerk-garmin`).
    for entry in fake.log() {
        let e = entry.to_ascii_lowercase();
        assert!(
            !e.contains("menstrual") && !e.contains("pregnan") && !e.contains("periodic"),
            "{entry}"
        );
    }
}
