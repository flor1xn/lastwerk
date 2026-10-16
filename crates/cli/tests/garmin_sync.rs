//! Abgleich mit Garmin (SPEC 5.6, Task T22) gegen eine Attrappe der Quelle: erster Lauf,
//! Folgelauf mit Überlappung, bekannte Dateien, kaputte und böse ZIPs, unplausible Werte,
//! Mischen ohne Überschreiben mit Null, Sperre mitten im Lauf und Sicherung. Alle Daten sind
//! synthetisch, es gibt weder Netz noch Schlüsselbund, alle Dateien liegen unter `target/`.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeDelta, Utc};
use lastwerk_cli::garmin::{
    GARMIN_SUBDIR, GarminSource, RECOVERY_SOURCE, SyncOptions, SyncPhase, SyncProgress,
    SyncSummary, abort_message, fit_location, run_logout, run_status, run_sync, sync, sync_into,
    window_start,
};
use lastwerk_garmin::{
    ActivitySummary, Endpoints, GarminClient, GarminError, MemoryStore, Profile,
};
use lastwerk_store::{ImportOptions, RecoveryDayRecord, Store, UserDataExport};
use lastwerk_testkit::fixtures::{LOCAL_OFFSET_S, START_FIT};
use lastwerk_testkit::generators::{ActivityParams, activity};
use serde_json::{Value, json};
use zip::write::{SimpleFileOptions, ZipWriter};

// --- Hilfen -------------------------------------------------------------------------

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli-garmin-sync-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap()
}

/// 10. Mai 2026 ist der Starttag der Fixtures (`START_FIT`, 06:00 UTC, lokal UTC+2).
const FIRST_DAY: (i32, u32, u32) = (2026, 5, 10);

/// FIT-Datei einer Radfahrt am `day`-ten Tag nach dem 10. Mai 2026.
fn fit_bytes(day: u32) -> Vec<u8> {
    activity(&ActivityParams::cycling(
        START_FIT + day * 86_400,
        LOCAL_OFFSET_S,
        1800 + day * 60,
    ))
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        w.start_file(*name, SimpleFileOptions::default()).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn summary(id: u64, day: u32) -> ActivitySummary {
    let date = d(FIRST_DAY.0, FIRST_DAY.1, FIRST_DAY.2 + day);
    ActivitySummary {
        activity_id: id,
        start_time_local: NaiveDateTime::new(
            date,
            chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
        ),
        activity_type: "cycling".into(),
    }
}

// --- Attrappe -----------------------------------------------------------------------

/// Antworten nach Datum, im Aufbau der echten Antworten (Feldnamen laut
/// `docs/garmin-feldabdeckung.md`). Jeder Aufruf steht im Protokoll.
#[derive(Default)]
struct Fake {
    /// Neueste zuerst.
    activities: Vec<ActivitySummary>,
    zips: HashMap<u64, Vec<u8>>,
    hrv: BTreeMap<NaiveDate, Value>,
    rhr: BTreeMap<NaiveDate, Value>,
    battery: BTreeMap<NaiveDate, Value>,
    sleep: BTreeMap<NaiveDate, Value>,
    /// Ab dem n-ten Aufruf (0-basiert) schlagen alle Aufrufe mit diesem Fehler fehl.
    fail_from_call: Option<(usize, GarminError)>,
    /// Aufrufe, deren Protokollname mit dem Schlüssel beginnt, scheitern mit dem Fehler.
    fail_prefix: Vec<(String, GarminError)>,
    /// Antworten der Bereichsabfragen enthalten alle Tage, auch die außerhalb der Anfrage
    /// (ein fehlerhafter oder böswilliger Server).
    leaky: bool,
    log: Mutex<Vec<String>>,
}

impl Fake {
    fn record(&self, entry: String) -> Result<(), GarminError> {
        let mut log = self.log.lock().unwrap();
        let n = log.len();
        log.push(entry.clone());
        if let Some((from, err)) = &self.fail_from_call
            && n >= *from
        {
            return Err(err.clone());
        }
        for (prefix, err) in &self.fail_prefix {
            if entry.starts_with(prefix.as_str()) {
                return Err(err.clone());
            }
        }
        Ok(())
    }

    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    fn count(&self, prefix: &str) -> usize {
        self.log().iter().filter(|e| e.starts_with(prefix)).count()
    }
}

fn in_range(
    map: &BTreeMap<NaiveDate, Value>,
    start: NaiveDate,
    end: NaiveDate,
    leaky: bool,
) -> Vec<(NaiveDate, Value)> {
    map.iter()
        .filter(|(date, _)| leaky || (start..=end).contains(date))
        .map(|(date, v)| (*date, v.clone()))
        .collect()
}

impl GarminSource for Fake {
    fn social_profile(&self) -> Result<Profile, GarminError> {
        self.record("profile".into())?;
        Ok(Profile {
            display_name: "ANZEIGENAME-MARKER".into(),
        })
    }

    fn activities(&self, start: u32, limit: u32) -> Result<Vec<ActivitySummary>, GarminError> {
        self.record(format!("activities:{start}"))?;
        Ok(self
            .activities
            .iter()
            .skip(start as usize)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    fn activity_fit(&self, activity_id: u64) -> Result<Vec<u8>, GarminError> {
        self.record(format!("fit:{activity_id}"))?;
        self.zips
            .get(&activity_id)
            .cloned()
            .ok_or(GarminError::NotAvailable)
    }

    fn hrv_range(&self, start: NaiveDate, end: NaiveDate) -> Result<Value, GarminError> {
        self.record(format!("hrv:{start}:{end}"))?;
        let items: Vec<Value> = in_range(&self.hrv, start, end, self.leaky)
            .into_iter()
            .map(|(date, avg)| json!({"calendarDate": date.to_string(), "lastNightAvg": avg}))
            .collect();
        Ok(json!({"userProfilePk": 1, "hrvSummaries": items}))
    }

    fn rhr_range(
        &self,
        display_name: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Value, GarminError> {
        assert_eq!(display_name, "ANZEIGENAME-MARKER");
        self.record(format!("rhr:{start}:{end}"))?;
        let rows: Vec<Value> = in_range(&self.rhr, start, end, self.leaky)
            .into_iter()
            .map(|(date, v)| json!({"value": v, "calendarDate": date.to_string()}))
            .collect();
        Ok(json!({"allMetrics": {"metricsMap": {"WELLNESS_RESTING_HEART_RATE": rows}}}))
    }

    fn sleep_day(&self, display_name: &str, date: NaiveDate) -> Result<Value, GarminError> {
        assert_eq!(display_name, "ANZEIGENAME-MARKER");
        self.record(format!("sleep:{date}"))?;
        self.sleep
            .get(&date)
            .cloned()
            .ok_or(GarminError::NotAvailable)
    }

    fn body_battery(&self, start: NaiveDate, end: NaiveDate) -> Result<Value, GarminError> {
        self.record(format!("bb:{start}:{end}"))?;
        let days: Vec<Value> = in_range(&self.battery, start, end, self.leaky)
            .into_iter()
            .map(|(date, levels)| {
                let values: Vec<Value> = levels
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(i, level)| json!([1_700_000_000_000_i64 + i as i64 * 900_000, level]))
                    .collect();
                json!({"date": date.to_string(), "charged": 40, "bodyBatteryValuesArray": values})
            })
            .collect();
        Ok(Value::Array(days))
    }
}

fn sleep_json(seconds: Value, score: Value) -> Value {
    json!({"dailySleepDTO": {"sleepTimeSeconds": seconds, "sleepScores": {"overall": {"value": score}}}})
}

/// Aktivitäten an den Tagen 0 bis 3 nach dem 10. Mai (Kennungen 1001 bis 1004, neueste zuerst)
/// und eine vor dem Fenster, dazu Erholungswerte für die Tage 10. bis 14. Mai.
fn standard_fake() -> Fake {
    let mut fake = Fake {
        activities: vec![
            summary(1004, 3),
            summary(1003, 2),
            summary(1002, 1),
            summary(1001, 0),
            // 1. Mai 2026: vor dem Fenster, wenn das Fenster am 4. Mai beginnt.
            ActivitySummary {
                activity_id: 900,
                start_time_local: NaiveDateTime::new(
                    d(2026, 5, 1),
                    chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
                ),
                activity_type: "cycling".into(),
            },
        ],
        ..Fake::default()
    };
    for (i, id) in [1001u64, 1002, 1003, 1004].iter().enumerate() {
        fake.zips.insert(
            *id,
            zip_of(&[(&format!("{id}_ACTIVITY.fit"), &fit_bytes(i as u32))]),
        );
    }
    for day in 10..=14 {
        let date = d(2026, 5, day);
        fake.hrv.insert(date, json!(60 + day));
        fake.rhr.insert(date, json!(40 + day));
        fake.battery
            .insert(date, json!([30, null, 90 + (day % 5), 12 + day, 55]));
        fake.sleep
            .insert(date, sleep_json(json!(25_000 + day), json!(70 + day)));
    }
    fake
}

fn no_progress() -> impl FnMut(SyncProgress) {
    |_| {}
}

/// Speicher mit Rückschau von 10 Tagen (das Fenster beginnt am 4. Mai, wenn heute der 14. ist).
fn store_with_backfill(days: u32) -> Store {
    let mut store = Store::open_in_memory().unwrap();
    store.set_garmin_backfill_days(days).unwrap();
    store
}

fn run(store: &mut Store, fake: &Fake, dir: &Path, today: NaiveDate) -> SyncSummary {
    sync(
        store,
        fake,
        dir,
        &SyncOptions::new(today, now()),
        &mut no_progress(),
    )
    .unwrap()
}

fn recovery(store: &Store, from: NaiveDate, to: NaiveDate) -> Vec<RecoveryDayRecord> {
    store.recovery_days(from, to).unwrap()
}

// --- Fenster ------------------------------------------------------------------------

#[test]
fn the_window_starts_at_the_backfill_then_seven_days_before_the_last_sync() {
    let today = d(2026, 5, 14);
    let opts = SyncOptions::new(today, now());
    let mut store = Store::open_in_memory().unwrap();
    // Standard: 365 Tage.
    assert_eq!(
        window_start(&store, &opts).unwrap(),
        today - TimeDelta::days(365)
    );
    store.set_garmin_backfill_days(30).unwrap();
    assert_eq!(window_start(&store, &opts).unwrap(), d(2026, 4, 14));
    // Mit letztem Abgleich: 7 Tage davor, die Rückschau zählt nicht mehr.
    store.set_garmin_last_sync(d(2026, 5, 12)).unwrap();
    assert_eq!(window_start(&store, &opts).unwrap(), d(2026, 5, 5));
    // `--days` ersetzt beides.
    let opts_days = SyncOptions {
        days: Some(3),
        ..opts
    };
    assert_eq!(window_start(&store, &opts_days).unwrap(), d(2026, 5, 11));
    // Ein letzter Abgleich in der Zukunft (Uhr zurückgestellt) macht das Fenster nicht leer.
    store.set_garmin_last_sync(d(2026, 6, 30)).unwrap();
    assert_eq!(window_start(&store, &opts).unwrap(), today);
}

// --- Erster Lauf --------------------------------------------------------------------

#[test]
fn the_first_run_downloads_imports_and_writes_recovery_days() {
    let dir = work_dir("first").join("fit");
    let fake = standard_fake();
    let mut store = store_with_backfill(10);
    let today = d(2026, 5, 14);
    let mut phases = Vec::new();
    let s = sync(
        &mut store,
        &fake,
        &dir,
        &SyncOptions::new(today, now()),
        &mut |p| phases.push(p),
    )
    .unwrap();

    assert_eq!(s.window_start, d(2026, 5, 4));
    assert_eq!(s.activities_listed, 4);
    assert_eq!(s.new_activities, 4);
    assert_eq!(s.known_activities, 0);
    assert_eq!(s.imported, 4);
    assert_eq!(s.import_errors, 0);
    // Für den 4. bis 9. Mai hat die Attrappe keinen Schlaf (404, "nicht verfügbar"): das ist
    // der Normalfall für Tage ohne Uhr und kein Grund zum Abbruch.
    assert_eq!(
        s.errors,
        BTreeMap::from([("not_available".to_owned(), 6)]),
        "{:?}",
        s.errors
    );
    assert_eq!(s.aborted, None);
    assert!(s.last_sync_updated);
    // Aufrufe: 1 Seite, 4 Dateien, Profil, HRV, Body Battery, Ruhepuls, 11 Schlaftage. Dazu
    // seit T39: 4 Aufrufe der Phase Konto, je ein Aufruf Schritte, Bergwert, Garmins Last,
    // Ausdauerwert und Fitnessalter, je Tag 6 Tageswerte für 11 Tage (ohne Freigabe keine
    // Hydration). Seit T40: 3 Schwellenverläufe in der Phase Konto, je ein Aufruf
    // VO2max-Verlauf, Belastungsfokus, Lauftoleranz, 2 Aufrufe Rennprognosen und je Tag
    // Trainingsstatus und Etagen. Die Attrappe beantwortet sie ohne Daten.
    assert_eq!(
        s.calls,
        1 + 4 + 1 + 1 + 1 + 1 + 11 + 4 + 5 + 6 * 11 + 3 + 1 + 1 + 1 + 2 + 2 * 11
    );
    assert_eq!(fake.count("fit:"), 4);
    // Der Name besteht nur aus der Kennung.
    for id in [1001, 1002, 1003, 1004] {
        assert!(dir.join(format!("{id}.fit")).is_file());
    }
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 4);
    assert_eq!(store.activities().unwrap().len(), 4);
    assert_eq!(
        store.garmin_settings().unwrap().last_sync,
        Some(today),
        "last_sync"
    );

    // Erholung: fünf Tage mit allen sechs Werten.
    assert_eq!(s.recovery_days, 5);
    let rows = recovery(&store, d(2026, 5, 1), today);
    assert_eq!(rows.len(), 5);
    let r = &rows[2]; // 12. Mai
    assert_eq!(r.date, d(2026, 5, 12));
    assert_eq!(r.hrv_overnight_ms, Some(72.0));
    assert_eq!(r.resting_hr_bpm, Some(52.0));
    assert_eq!(r.sleep_duration_s, Some(25_012));
    assert_eq!(r.sleep_score, Some(82));
    // Body Battery: Werte 30, null, 92, 24, 55, Höchstwert 92, Tiefstwert 24 (null ignoriert).
    assert_eq!(r.body_battery_high, Some(92));
    assert_eq!(r.body_battery_low, Some(24));
    assert_eq!(r.source, RECOVERY_SOURCE);
    assert_eq!(r.imported_at, now());

    // Fortschritt: alle fünf Phasen (seit T39 mit Konto und Tageswerten) kommen vor, seit T40 in
    // dieser Reihenfolge (Last früh da), und enden mit done == total.
    let order: Vec<SyncPhase> = phases.iter().map(|p| p.phase).fold(Vec::new(), |mut v, p| {
        if v.last() != Some(&p) {
            v.push(p);
        }
        v
    });
    assert_eq!(
        order,
        [
            SyncPhase::Account,
            SyncPhase::Activities,
            SyncPhase::Import,
            SyncPhase::Recovery,
            SyncPhase::Wellness
        ]
    );
    for phase in [
        SyncPhase::Activities,
        SyncPhase::Recovery,
        SyncPhase::Import,
    ] {
        let last = phases.iter().rev().find(|p| p.phase == phase).unwrap();
        assert_eq!(Some(last.done), last.total, "{phase:?}");
    }
}

#[test]
fn the_activity_list_is_read_page_by_page_until_the_first_activity_before_the_window() {
    let dir = work_dir("paging").join("fit");
    let mut fake = Fake::default();
    // 230 Aktivitäten im Fenster (alle am 14. Mai), danach 20 vor dem Fenster. Die ZIPs haben
    // keine FIT-Datei, so bleibt der Test schnell.
    for i in 0..250u64 {
        let day = if i < 230 {
            d(2026, 5, 14)
        } else {
            d(2026, 4, 1)
        };
        fake.activities.push(ActivitySummary {
            activity_id: 5000 + i,
            start_time_local: NaiveDateTime::new(
                day,
                chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            ),
            activity_type: "running".into(),
        });
        fake.zips.insert(5000 + i, zip_of(&[("readme.txt", b"x")]));
    }
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(s.activities_listed, 230);
    // Drei Seiten (0, 100, 200); die dritte enthält Aktivitäten vor dem Fenster, danach ist Schluss.
    assert_eq!(fake.count("activities:"), 3);
    assert_eq!(fake.count("fit:"), 230);
    assert_eq!(s.errors.get("zip_no_fit"), Some(&230));
    assert_eq!(s.new_activities, 0);
    assert_eq!(s.aborted, None);
}

// --- Folgelauf ----------------------------------------------------------------------

#[test]
fn a_follow_up_run_covers_seven_days_before_the_last_sync_and_loads_only_new_files() {
    let dir = work_dir("follow").join("fit");
    let mut fake = standard_fake();
    let mut store = store_with_backfill(10);
    run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(
        store.garmin_settings().unwrap().last_sync,
        Some(d(2026, 5, 14))
    );

    // Zwei Tage später: eine neue Aktivität am 15. Mai, und Garmin hat die HRV des 13. Mai
    // nachträglich geändert.
    fake.activities.insert(0, summary(1005, 5));
    fake.zips
        .insert(1005, zip_of(&[("1005.fit", &fit_bytes(5))]));
    fake.hrv.insert(d(2026, 5, 13), json!(88));
    fake.hrv.insert(d(2026, 5, 15), json!(66));
    fake.rhr.insert(d(2026, 5, 15), json!(47));
    let fake = Fake {
        log: Mutex::new(Vec::new()),
        ..fake
    };
    let s = run(&mut store, &fake, &dir, d(2026, 5, 16));

    // Fenster: 14. Mai minus 7 Tage.
    assert_eq!(s.window_start, d(2026, 5, 7));
    assert_eq!(s.activities_listed, 5);
    assert_eq!(s.known_activities, 4);
    assert_eq!(s.new_activities, 1);
    assert_eq!(
        fake.log()
            .iter()
            .filter(|e| e.starts_with("fit:"))
            .collect::<Vec<_>>(),
        ["fit:1005"]
    );
    assert_eq!(s.imported, 1);
    assert_eq!(store.activities().unwrap().len(), 5);
    assert_eq!(
        store.garmin_settings().unwrap().last_sync,
        Some(d(2026, 5, 16))
    );

    // Die Überlappung holt Tage neu: der 13. Mai hat jetzt 88, der 15. Mai ist neu.
    let rows = recovery(&store, d(2026, 5, 1), d(2026, 5, 31));
    let by_date: BTreeMap<_, _> = rows.iter().map(|r| (r.date, r)).collect();
    assert_eq!(by_date[&d(2026, 5, 13)].hrv_overnight_ms, Some(88.0));
    assert_eq!(by_date[&d(2026, 5, 15)].hrv_overnight_ms, Some(66.0));
    assert_eq!(by_date[&d(2026, 5, 15)].resting_hr_bpm, Some(47.0));
    // 10. und 11. Mai liegen vor dem Fenster (7. Mai bis 16. Mai ... sie liegen darin) und
    // bleiben erhalten.
    assert!(by_date.contains_key(&d(2026, 5, 10)));
    // Schlaf wird für jeden Tag des Fensters abgefragt (10 Tage: 7. bis 16. Mai).
    assert_eq!(fake.count("sleep:"), 10);
}

#[test]
fn a_file_that_is_already_there_is_not_downloaded_again() {
    let dir = work_dir("known").join("fit");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("1002.fit"), fit_bytes(1)).unwrap();
    let fake = standard_fake();
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(s.known_activities, 1);
    assert_eq!(s.new_activities, 3);
    assert_eq!(fake.count("fit:"), 3);
    assert_eq!(fake.count("fit:1002"), 0);
    // Die vorhandene Datei wird trotzdem importiert.
    assert_eq!(s.imported, 4);
    // Eine zweite Ausführung lädt gar nichts mehr.
    let fake2 = standard_fake();
    let s2 = run(&mut store, &fake2, &dir, d(2026, 5, 14));
    assert_eq!(fake2.count("fit:"), 0);
    assert_eq!(s2.known_activities, 4);
    assert_eq!(s2.imported, 0);
}

// --- Kaputte und böse ZIPs ----------------------------------------------------------

#[test]
fn zips_without_fit_or_broken_ones_are_counted_and_the_run_goes_on() {
    let dir = work_dir("badzip").join("fit");
    let mut fake = standard_fake();
    fake.zips
        .insert(1002, zip_of(&[("readme.txt", b"kein FIT")]));
    fake.zips.insert(1003, b"das ist kein ZIP".to_vec());
    fake.zips.insert(1004, Vec::new()); // 204
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(s.new_activities, 1);
    assert_eq!(s.errors.get("zip_no_fit"), Some(&1));
    assert_eq!(s.errors.get("zip_invalid"), Some(&1));
    assert_eq!(s.errors.get("empty_download"), Some(&1));
    assert_eq!(s.aborted, None);
    assert!(s.last_sync_updated);
    let names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["1001.fit"]);
}

#[test]
fn a_zip_slip_name_in_the_archive_never_becomes_a_path() {
    let root = work_dir("slip");
    let dir = root.join("data").join("fit");
    let mut fake = standard_fake();
    fake.zips
        .insert(1001, zip_of(&[("../../../escape.fit", &fit_bytes(0))]));
    fake.zips
        .insert(1002, zip_of(&[("/tmp/absolut.fit", &fit_bytes(1))]));
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(s.new_activities, 4);
    // Nichts außerhalb des FIT-Ordners.
    let mut beside: Vec<_> = fs::read_dir(root.join("data"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    beside.sort();
    assert_eq!(beside, ["fit"]);
    assert!(!root.join("escape.fit").exists());
    assert!(!Path::new("/tmp/absolut.fit").exists());
    // Im Ordner liegen genau die vier Dateien mit den Namen aus den Kennungen.
    let mut names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["1001.fit", "1002.fit", "1003.fit", "1004.fit"]);
}

// --- Unplausible Werte und Mischen ---------------------------------------------------

#[test]
fn implausible_values_are_dropped_and_counted() {
    let dir = work_dir("implausible").join("fit");
    let mut fake = Fake::default();
    let day = d(2026, 5, 13);
    fake.hrv.insert(day, json!(400.5));
    fake.rhr.insert(day, json!(19));
    fake.battery.insert(day, json!([120, -3, 77]));
    fake.sleep
        .insert(day, sleep_json(json!(90_000), json!(101)));
    // Ein zweiter Tag mit gültigen und ungültigen Werten gemischt: nur die gültigen bleiben.
    let day2 = d(2026, 5, 14);
    fake.hrv.insert(day2, json!(400));
    fake.rhr.insert(day2, json!(121));
    fake.battery.insert(day2, json!([0, 100]));
    fake.sleep
        .insert(day2, sleep_json(json!(86_400), json!(100)));
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));

    // Verworfen: HRV 400,5, Ruhepuls 19, Body Battery 120 und -3, Schlaf 90000 und Wert 101
    // (Tag 1), Ruhepuls 121 (Tag 2).
    assert_eq!(s.dropped_values, 7);
    let rows = recovery(&store, d(2026, 5, 1), d(2026, 5, 31));
    assert_eq!(rows.len(), 2);
    let r1 = &rows[0];
    assert_eq!(
        (
            r1.hrv_overnight_ms,
            r1.resting_hr_bpm,
            r1.sleep_duration_s,
            r1.sleep_score
        ),
        (None, None, None, None)
    );
    // Vom Body Battery bleibt nur der gültige Wert 77.
    assert_eq!(
        (r1.body_battery_high, r1.body_battery_low),
        (Some(77), Some(77))
    );
    let r2 = &rows[1];
    assert_eq!(r2.hrv_overnight_ms, Some(400.0));
    assert_eq!(r2.resting_hr_bpm, None);
    assert_eq!(r2.sleep_duration_s, Some(86_400));
    assert_eq!(r2.sleep_score, Some(100));
    assert_eq!(
        (r2.body_battery_high, r2.body_battery_low),
        (Some(100), Some(0))
    );
}

#[test]
fn a_day_without_any_plausible_value_creates_no_row() {
    let dir = work_dir("emptyday").join("fit");
    let mut fake = Fake::default();
    fake.hrv.insert(d(2026, 5, 13), json!(9999));
    fake.sleep
        .insert(d(2026, 5, 13), sleep_json(json!(0), json!(null)));
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(s.recovery_days, 0);
    assert!(recovery(&store, d(2026, 5, 1), d(2026, 5, 31)).is_empty());
}

#[test]
fn an_existing_value_is_never_overwritten_with_null() {
    let dir = work_dir("merge").join("fit");
    let mut store = store_with_backfill(10);
    let day = d(2026, 5, 13);
    // Vorhanden: alle Werte, aus dem Zusatzprogramm.
    store
        .upsert_recovery_day(&RecoveryDayRecord {
            date: day,
            hrv_overnight_ms: Some(60.0),
            resting_hr_bpm: Some(48.0),
            sleep_duration_s: Some(27_000),
            sleep_score: Some(82),
            body_battery_high: Some(95),
            body_battery_low: Some(10),
            source: "garmin-bridge 0.1.0".into(),
            imported_at: now(),
        })
        .unwrap();
    // Zweiter Tag, nur im Speicher: Garmin liefert diesmal nichts dazu.
    let untouched = d(2026, 5, 12);
    store
        .upsert_recovery_day(&RecoveryDayRecord {
            date: untouched,
            hrv_overnight_ms: Some(55.0),
            resting_hr_bpm: None,
            sleep_duration_s: None,
            sleep_score: None,
            body_battery_high: None,
            body_battery_low: None,
            source: "garmin-bridge 0.1.0".into(),
            imported_at: now(),
        })
        .unwrap();

    let mut fake = Fake::default();
    // Garmin liefert für den 13. Mai nur den Ruhepuls (neu: 50) und null bei HRV und Schlaf.
    fake.rhr.insert(day, json!(50));
    fake.hrv.insert(day, Value::Null);
    fake.sleep.insert(day, sleep_json(json!(null), json!(null)));
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(s.recovery_days, 1);

    let rows = recovery(&store, d(2026, 5, 12), d(2026, 5, 13));
    let merged = &rows[1];
    assert_eq!(merged.resting_hr_bpm, Some(50.0), "neuer Wert gilt");
    assert_eq!(merged.hrv_overnight_ms, Some(60.0), "alter Wert bleibt");
    assert_eq!(merged.sleep_duration_s, Some(27_000));
    assert_eq!(merged.sleep_score, Some(82));
    assert_eq!(merged.body_battery_high, Some(95));
    assert_eq!(merged.body_battery_low, Some(10));
    assert_eq!(merged.source, RECOVERY_SOURCE);
    // Der Tag ohne Antwort ist unverändert, samt Quelle.
    let old = &rows[0];
    assert_eq!(old.hrv_overnight_ms, Some(55.0));
    assert_eq!(old.source, "garmin-bridge 0.1.0");
}

#[test]
fn days_outside_the_requested_range_in_a_response_are_ignored() {
    let dir = work_dir("outside").join("fit");
    // Die Attrappe antwortet mit allen Tagen, auch denen außerhalb der Anfrage.
    let mut fake = Fake {
        leaky: true,
        ..Fake::default()
    };
    fake.hrv.insert(d(2026, 5, 14), json!(61));
    fake.hrv.insert(d(2026, 6, 30), json!(99));
    fake.hrv.insert(d(2020, 1, 1), json!(98));
    fake.rhr.insert(d(2026, 6, 30), json!(50));
    let mut store = store_with_backfill(10);
    run(&mut store, &fake, &dir, d(2026, 5, 14));
    let rows = recovery(&store, d(2026, 1, 1), d(2026, 12, 31));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].date, d(2026, 5, 14));
}

// --- Abbruch ------------------------------------------------------------------------

#[test]
fn a_lockout_in_the_middle_of_the_run_sets_no_last_sync_but_keeps_what_is_there() {
    let dir = work_dir("locked").join("fit");
    let mut fake = standard_fake();
    let until = DateTime::from_timestamp(1_790_021_600, 0).unwrap();
    // Aufruf 0: Liste, 1 und 2: die ersten zwei Dateien, dann die Sperre.
    fake.fail_from_call = Some((3, GarminError::Locked { until }));
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));

    let abort = s.aborted.clone().expect("Abbruch");
    assert_eq!(abort.code, "locked");
    assert_eq!(abort.until, Some(until));
    assert!(!s.last_sync_updated);
    assert_eq!(store.garmin_settings().unwrap().last_sync, None);
    assert_eq!(s.new_activities, 2);
    // Was geladen war, ist importiert; Erholung wurde nicht mehr abgefragt.
    assert_eq!(s.imported, 2);
    assert_eq!(store.activities().unwrap().len(), 2);
    assert_eq!(fake.count("hrv:"), 0);
    assert_eq!(s.recovery_days, 0);
    // Nach dem Abbruch kein weiterer Aufruf (keine Wiederholung gegen die Sperre). Davor liefen
    // seit T40 die 7 Aufrufe der Phase Konto (4 und 3 Schwellenverläufe), die die Attrappe ohne
    // Daten und ohne Zählung beantwortet.
    assert_eq!(s.calls, 7 + 4);
    // Die Meldung nennt das Ende der Sperre, aber keinen Pfad.
    let message = abort_message(&abort);
    assert!(message.contains("gesperrt"), "{message}");
    assert!(!message.contains(&dir.to_string_lossy().to_string()));
}

#[test]
fn a_lockout_during_recovery_keeps_the_recovery_days_fetched_so_far() {
    let dir = work_dir("locked-recovery").join("fit");
    let mut fake = standard_fake();
    // Liste, 4 Dateien, Profil, HRV, Body Battery: Aufruf 0 bis 7. Der Ruhepuls (Aufruf 8) scheitert.
    fake.fail_prefix = vec![(
        "rhr:".into(),
        GarminError::Locked {
            until: DateTime::from_timestamp(1_790_021_600, 0).unwrap(),
        },
    )];
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(s.aborted.as_ref().map(|a| a.code.as_str()), Some("locked"));
    assert_eq!(store.garmin_settings().unwrap().last_sync, None);
    // HRV und Body Battery sind schon geschrieben, Ruhepuls und Schlaf fehlen.
    let rows = recovery(&store, d(2026, 5, 1), d(2026, 5, 31));
    assert_eq!(rows.len(), 5);
    assert!(
        rows.iter()
            .all(|r| r.hrv_overnight_ms.is_some() && r.body_battery_high.is_some())
    );
    assert!(
        rows.iter()
            .all(|r| r.resting_hr_bpm.is_none() && r.sleep_score.is_none())
    );
    assert_eq!(s.imported, 4);
}

#[test]
fn other_runs_ending_errors_end_the_run_without_last_sync() {
    for (error, code) in [
        (GarminError::NotConnected, "not_connected"),
        (
            GarminError::AuthFailed {
                attempts: Vec::new(),
            },
            "auth_failed",
        ),
        (GarminError::Throttled, "throttled"),
        (
            GarminError::Network(lastwerk_garmin::NetworkKind::Timeout),
            "network",
        ),
    ] {
        let dir = work_dir(&format!("ends-{code}")).join("fit");
        let mut fake = standard_fake();
        fake.fail_from_call = Some((0, error));
        let mut store = store_with_backfill(10);
        let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
        assert_eq!(s.aborted.as_ref().map(|a| a.code.as_str()), Some(code));
        // Die 7 Aufrufe der Phase Konto (seit T40 zuerst) beantwortet die Attrappe ohne Daten,
        // der erste eigene Aufruf beendet den Lauf.
        assert_eq!(s.calls, 7 + 1, "{code}");
        assert_eq!(store.garmin_settings().unwrap().last_sync, None);
        assert_eq!(s.new_activities, 0);
        assert!(!abort_message(s.aborted.as_ref().unwrap()).is_empty());
    }
}

#[test]
fn single_failures_are_counted_but_five_in_a_row_end_the_run() {
    // Ein Fehler bei einer Aktivität trifft nur sie.
    let dir = work_dir("single").join("fit");
    let mut fake = standard_fake();
    fake.fail_prefix = vec![("fit:1002".into(), GarminError::Http(500))];
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(s.aborted, None);
    assert_eq!(s.errors.get("network"), Some(&1));
    assert_eq!(s.new_activities, 3);
    assert!(dir.join("1001.fit").is_file() && !dir.join("1002.fit").exists());

    // Fünf Fehler in Folge: Abbruch, ohne last_sync.
    let dir = work_dir("five").join("fit");
    let mut fake = standard_fake();
    fake.fail_prefix = vec![("fit:".into(), GarminError::Http(500))];
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    // Vier Dateien und kein fünfter Fehler in der Liste: der Lauf endet in der Erholung nicht,
    // denn nach vier Fehlern kommt eine erfolgreiche Antwort (Profil) und setzt den Zähler zurück.
    assert_eq!(s.aborted, None);
    assert_eq!(s.errors.get("network"), Some(&4));

    let dir = work_dir("six").join("fit");
    let mut fake = standard_fake();
    for id in 2000..2004u64 {
        fake.activities.insert(0, summary(id, 4));
        fake.zips.insert(id, zip_of(&[("x.fit", b"x")]));
    }
    fake.fail_prefix = vec![("fit:".into(), GarminError::Http(503))];
    let mut store = store_with_backfill(10);
    let s = run(&mut store, &fake, &dir, d(2026, 5, 14));
    assert_eq!(s.aborted.as_ref().map(|a| a.code.as_str()), Some("network"));
    assert_eq!(fake.count("fit:"), 5);
    assert!(!s.last_sync_updated);
}

// --- Ort der Ablage (T23) -------------------------------------------------------------

/// Der feste Stamm wird bei Bedarf angelegt (er gehört Lastwerk), die Garmin-Dateien kommen in
/// `garmin/` darin. Ein zweiter Aufruf ändert nichts.
#[test]
fn fit_location_creates_the_fixed_root_with_a_garmin_subfolder() {
    let root = work_dir("location");
    let fit_root = root.join("daten").join("Lastwerk").join("fit");
    let loc = fit_location(&fit_root).unwrap();
    assert_eq!(loc.root, fit_root);
    assert_eq!(loc.download, fit_root.join(GARMIN_SUBDIR));
    assert!(fit_root.is_dir());
    // Unterordner `garmin/` legt erst der Abgleich an.
    assert!(!loc.download.exists());
    assert_eq!(fit_location(&fit_root).unwrap(), loc);
}

/// Garmin-Dateien liegen in `<FIT-Stamm>/garmin`, eingelesen wird der ganze Stamm: Alle
/// Quelldateien haben einen Pfad relativ zu derselben Wurzel, und ein Wiederaufbau mit dieser
/// Wurzel behält eigene und geholte Aktivitäten (Grenze der Ablage aus T22, gelöst in T23).
#[test]
fn one_root_keeps_own_and_garmin_files_through_a_rebuild() {
    let root = work_dir("one-root").join("fit");
    fs::create_dir_all(&root).unwrap();
    // Eine eigene Datei des Nutzers an einem anderen Tag als die der Attrappe.
    fs::write(root.join("eigene-fahrt.fit"), fit_bytes(9)).unwrap();
    let mut store = store_with_backfill(10);
    let loc = fit_location(&root).unwrap();
    let fake = standard_fake();
    let s = sync_into(
        &mut store,
        &fake,
        &loc.download,
        &loc.root,
        &SyncOptions::new(d(2026, 5, 14), now()),
        &mut no_progress(),
    )
    .unwrap();
    assert_eq!(s.new_activities, 4);
    // Vier geholte und die eigene Datei.
    assert_eq!(s.imported, 5);
    assert!(root.join("garmin").join("1001.fit").is_file());
    let mut paths: Vec<String> = store
        .source_files()
        .unwrap()
        .into_iter()
        .map(|f| f.row.rel_path)
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "eigene-fahrt.fit",
            "garmin/1001.fit",
            "garmin/1002.fit",
            "garmin/1003.fit",
            "garmin/1004.fit",
        ]
    );
    assert_eq!(store.activities().unwrap().len(), 5);

    // Wiederaufbau mit derselben Wurzel: nichts geht verloren.
    let report = store
        .rebuild_cache(&root, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();
    assert_eq!(report.new, 5);
    assert_eq!(store.activities().unwrap().len(), 5);

    // Ein zweiter Abgleich lädt nichts neu und legt keine zweite Zeile je Datei an.
    let mut store2 = store;
    store2.set_garmin_last_sync(d(2026, 5, 14)).unwrap();
    let s2 = sync_into(
        &mut store2,
        &fake,
        &loc.download,
        &loc.root,
        &SyncOptions::new(d(2026, 5, 14), now()),
        &mut no_progress(),
    )
    .unwrap();
    assert_eq!(s2.new_activities, 0);
    assert_eq!(s2.imported, 0);
    assert_eq!(store2.source_files().unwrap().len(), 5);
}

// --- Meldungen ----------------------------------------------------------------------

#[test]
fn reports_and_errors_contain_no_path_no_token_and_no_display_name() {
    let root = work_dir("secrecy");
    let dir = root.join("PFAD-MARKER").join("fit");
    let mut fake = standard_fake();
    fake.zips.insert(1002, zip_of(&[("readme.txt", b"x")]));
    let mut store = store_with_backfill(10);
    let mut out = Vec::new();
    let result = run_sync(
        &mut store,
        &fake,
        &dir,
        &dir,
        &SyncOptions::new(d(2026, 5, 14), now()),
        &mut out,
        &mut no_progress(),
    );
    assert!(result.is_ok());
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Abgleich abgeschlossen"), "{text}");
    assert!(text.contains("zip_no_fit"), "{text}");
    for forbidden in [
        "PFAD-MARKER",
        "ANZEIGENAME-MARKER",
        root.to_str().unwrap(),
        "/home",
        ".fit",
    ] {
        assert!(!text.contains(forbidden), "{forbidden} in:\n{text}");
    }

    // Bei einem Abbruch: Bericht auf stdout, Fehler mit Code 2 und ohne Pfad.
    let mut fake = standard_fake();
    fake.fail_from_call = Some((2, GarminError::Throttled));
    let mut store = store_with_backfill(10);
    let dir = root.join("PFAD-MARKER").join("fit2");
    let mut out = Vec::new();
    let err = run_sync(
        &mut store,
        &fake,
        &dir,
        &dir,
        &SyncOptions::new(d(2026, 5, 14), now()),
        &mut out,
        &mut no_progress(),
    )
    .unwrap_err();
    assert_eq!(err.exit_code(), 2);
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("unvollständig"), "{text}");
    for forbidden in ["PFAD-MARKER", "ANZEIGENAME-MARKER"] {
        assert!(!text.contains(forbidden) && !err.message().contains(forbidden));
    }
}

// --- Befehle status und logout --------------------------------------------------------

#[test]
fn status_and_logout_work_without_a_connection_and_without_network() {
    let root = work_dir("status");
    let tokens = std::sync::Arc::new(MemoryStore::new());
    let client = GarminClient::new(
        Endpoints::default(),
        tokens,
        root.join("PFAD-MARKER").join("lockout.json"),
    );
    let mut store = Store::open_in_memory().unwrap();

    let mut out = Vec::new();
    run_status(&client, Some(&store), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("nicht verbunden"), "{text}");
    assert!(text.contains("noch keiner"), "{text}");
    assert!(text.contains("365 Tage"), "{text}");
    assert!(text.contains("Abgleich beim Start der App: ja"), "{text}");

    store.set_garmin_last_sync(d(2026, 5, 14)).unwrap();
    store.set_garmin_sync_on_start(false).unwrap();
    let mut out = Vec::new();
    run_status(&client, Some(&store), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Letzter Abgleich: 2026-05-14"), "{text}");
    assert!(text.contains("Abgleich beim Start der App: nein"), "{text}");
    assert!(!text.contains("PFAD-MARKER"));

    // Ohne Datenbank gelten die Standardwerte.
    let mut out = Vec::new();
    run_status(&client, None, &mut out).unwrap();
    assert!(String::from_utf8(out).unwrap().contains("noch keiner"));

    let mut out = Vec::new();
    run_logout(&client, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("getrennt"), "{text}");
}

// --- Sicherung ----------------------------------------------------------------------

#[test]
fn synced_recovery_days_and_settings_survive_a_backup_roundtrip() {
    let dir = work_dir("backup").join("fit");
    let fake = standard_fake();
    let mut store = store_with_backfill(10);
    store.set_garmin_sync_on_start(false).unwrap();
    run(&mut store, &fake, &dir, d(2026, 5, 14));

    let export = store.export_user_data().unwrap();
    assert_eq!(export.recovery_days.len(), 5);
    assert_eq!(export.recovery_days[2].body_battery_high, Some(92));
    assert_eq!(export.recovery_days[2].source, RECOVERY_SOURCE);
    let json = export.to_json().unwrap();

    let mut fresh = Store::open_in_memory().unwrap();
    fresh
        .import_user_data(&UserDataExport::from_json(&json).unwrap())
        .unwrap();
    assert_eq!(fresh.export_user_data().unwrap(), export);
    let settings = fresh.garmin_settings().unwrap();
    assert_eq!(settings.last_sync, Some(d(2026, 5, 14)));
    assert!(!settings.sync_on_start);
    assert_eq!(settings.backfill_days, 10);
    assert_eq!(
        recovery(&fresh, d(2026, 5, 1), d(2026, 5, 31)),
        recovery(&store, d(2026, 5, 1), d(2026, 5, 31))
    );
}
