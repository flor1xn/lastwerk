//! Garmin in der App (T23): Zustand, Abgleich mit Attrappe der Quelle, Ort der Ablage, Trennen,
//! Einstellung beim Start. Kein Netz, kein echter Schlüsselbund, nur synthetische Daten.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use lastwerk_app::dto::{ActivityFilter, SettingsPatch};
use lastwerk_app::service;
use lastwerk_cli::garmin::GarminSource;
use lastwerk_garmin::{
    ActivitySummary, Endpoints, GarminClient, GarminError, MemoryStore, Profile, TokenSet,
    TokenStore,
};
use lastwerk_store::Store;
use lastwerk_testkit::fixtures::{LOCAL_OFFSET_S, START_FIT};
use lastwerk_testkit::generators::{ActivityParams, activity};
use serde_json::Value;

const TOKEN_MARKER: &str = "TOKEN-MARKER-7f3a";
const NAME_MARKER: &str = "ANZEIGENAME-MARKER-91c2";

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("app-garmin-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap()
}

/// Synthetische Radfahrt am Tag `day` nach dem 10. Mai 2026 (`START_FIT`).
fn fit_bytes(day: u32) -> Vec<u8> {
    activity(&ActivityParams::cycling(
        START_FIT + day * 86_400,
        LOCAL_OFFSET_S,
        1800 + day * 60,
    ))
}

fn client(dir: &Path, connected: bool) -> GarminClient {
    let store = if connected {
        MemoryStore::with_tokens(TokenSet::for_tests(
            TOKEN_MARKER,
            Some(TOKEN_MARKER),
            "CLIENT-MARKER",
        ))
    } else {
        MemoryStore::new()
    };
    let store: Arc<dyn TokenStore> = Arc::new(store);
    // Echte Adressen, aber kein Test hier ruft einen Endpunkt auf: Zustand und Trennen arbeiten
    // ohne Netz, der Abgleich nimmt die Attrappe unten.
    GarminClient::new(Endpoints::default(), store, dir.join("lockout.json"))
}

/// Attrappe der Quelle: eine Aktivität in der Liste, keine Erholungswerte. Das Profil trägt
/// einen Marker, der nie die Oberfläche erreichen darf.
struct Fake {
    activities: Vec<ActivitySummary>,
    fail: Option<GarminError>,
}

impl Fake {
    fn one_activity(id: u64) -> Self {
        Self {
            activities: vec![ActivitySummary {
                activity_id: id,
                start_time_local: NaiveDateTime::new(
                    d(2026, 5, 10),
                    NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
                ),
                activity_type: "cycling".into(),
            }],
            fail: None,
        }
    }

    fn check(&self) -> Result<(), GarminError> {
        match &self.fail {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}

impl GarminSource for Fake {
    fn social_profile(&self) -> Result<Profile, GarminError> {
        self.check()?;
        Ok(Profile {
            display_name: NAME_MARKER.into(),
        })
    }
    fn activities(&self, start: u32, _limit: u32) -> Result<Vec<ActivitySummary>, GarminError> {
        self.check()?;
        Ok(if start == 0 {
            self.activities.clone()
        } else {
            Vec::new()
        })
    }
    fn activity_fit(&self, _id: u64) -> Result<Vec<u8>, GarminError> {
        // Die Datei liegt in den Tests schon in der Ablage; ein Download wäre ein Fehler.
        Err(GarminError::NotAvailable)
    }
    fn hrv_range(&self, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.check()?;
        Ok(Value::Null)
    }
    fn rhr_range(&self, _n: &str, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.check()?;
        Ok(Value::Null)
    }
    fn sleep_day(&self, _n: &str, _d: NaiveDate) -> Result<Value, GarminError> {
        Err(GarminError::NotAvailable)
    }
    fn body_battery(&self, _s: NaiveDate, _e: NaiveDate) -> Result<Value, GarminError> {
        self.check()?;
        Ok(Value::Null)
    }
}

fn patch_sync_on_start(on: bool) -> SettingsPatch {
    SettingsPatch {
        locale: None,
        theme: None,
        pmc: None,
        mcp_recovery_enabled: None,
        onboarded: None,
        garmin_sync_on_start: Some(on),
        garmin_share_hydration: None,
        garmin_share_body: None,
        garmin_share_blood_pressure: None,
        garmin_share_nutrition: None,
    }
}

/// Der Zustand kommt ohne Netz aus und nennt weder Token noch Anzeigenamen.
#[test]
fn status_reports_connection_last_sync_and_setting() {
    let dir = work_dir("status");
    let mut store = Store::open_in_memory().unwrap();
    let off = service::garmin::status(&store, &client(&dir, false)).unwrap();
    assert!(!off.connected);
    assert_eq!(off.last_sync, None);
    assert!(off.sync_on_start);
    assert_eq!(off.locked_until, None);

    store.set_garmin_last_sync(d(2026, 5, 12)).unwrap();
    store.set_garmin_sync_on_start(false).unwrap();
    let on = service::garmin::status(&store, &client(&dir, true)).unwrap();
    assert!(on.connected);
    assert_eq!(on.last_sync.as_deref(), Some("2026-05-12"));
    assert!(!on.sync_on_start);
}

/// Eine Sperre aus der Datei erscheint mit ihrem Ende in UTC.
#[test]
fn an_active_lockout_is_shown_with_its_end() {
    let dir = work_dir("lockout");
    let until = Utc::now().timestamp() + 3600;
    fs::write(
        dir.join("lockout.json"),
        format!(
            r#"{{"since_epoch_s":{},"until_epoch_s":{until},"http_status":429,"consecutive":3}}"#,
            until - 7200
        ),
    )
    .unwrap();
    let store = Store::open_in_memory().unwrap();
    let status = service::garmin::status(&store, &client(&dir, true)).unwrap();
    let shown = status.locked_until.expect("Sperre");
    assert_eq!(
        DateTime::parse_from_rfc3339(&shown).unwrap().timestamp(),
        until
    );
    assert!(shown.ends_with('Z'));
}

#[test]
fn logout_removes_the_tokens() {
    let dir = work_dir("logout");
    let store = Store::open_in_memory().unwrap();
    let c = client(&dir, true);
    assert!(service::garmin::status(&store, &c).unwrap().connected);
    service::garmin::logout(&c).unwrap();
    assert!(!service::garmin::status(&store, &c).unwrap().connected);
    // Zweimal trennen ist kein Fehler.
    service::garmin::logout(&c).unwrap();
}

/// Einstellung `garmin_sync_on_start` über `set_settings` und `get_settings`, Standard an. Der
/// Abgleich beim Start läuft nur, wenn eingeschaltet und verbunden.
#[test]
fn sync_on_start_is_a_setting_and_needs_a_connection() {
    let dir = work_dir("on-start");
    let mut store = Store::open_in_memory().unwrap();
    assert!(
        service::get_settings(&store, false)
            .unwrap()
            .garmin_sync_on_start
    );
    assert!(service::garmin::should_sync_on_start(&store, &client(&dir, true)).unwrap());
    assert!(!service::garmin::should_sync_on_start(&store, &client(&dir, false)).unwrap());

    let s = service::set_settings(&mut store, &patch_sync_on_start(false), false).unwrap();
    assert!(!s.garmin_sync_on_start);
    assert_eq!(
        store
            .get_setting("garmin.sync_on_start")
            .unwrap()
            .as_deref(),
        Some("false")
    );
    assert!(!service::garmin::should_sync_on_start(&store, &client(&dir, true)).unwrap());
    let s = service::set_settings(&mut store, &patch_sync_on_start(true), false).unwrap();
    assert!(s.garmin_sync_on_start);
    // Nur ein Wahrheitswert ist gültig.
    for bad in [
        serde_json::json!({ "garmin_sync_on_start": 1 }),
        serde_json::json!({ "garmin_sync_on_start": "true" }),
    ] {
        assert!(serde_json::from_value::<SettingsPatch>(bad).is_err());
    }
}

/// Entscheidung T23, T28: Garmin-Dateien liegen im Unterordner `garmin/` des festen FIT-Stamms,
/// eingelesen wird der ganze Stamm. Danach zeigt die Aktivitätsliste eigene und geholte Fahrten.
#[test]
fn sync_imports_the_whole_fit_root_with_the_garmin_subfolder() {
    let dir = work_dir("sync");
    let root = dir.join("Lastwerk").join("fit");
    fs::create_dir_all(root.join("garmin")).unwrap();
    fs::write(root.join("eigene.fit"), fit_bytes(3)).unwrap();
    // Schon geholt: wird nicht erneut geladen, aber eingelesen.
    fs::write(root.join("garmin").join("7001.fit"), fit_bytes(0)).unwrap();
    let mut store = Store::open(&dir.join("lastwerk.db")).unwrap();

    let mut phases = Vec::new();
    let summary = service::garmin::sync(
        &mut store,
        &Fake::one_activity(7001),
        &root,
        d(2026, 5, 20),
        now(),
        &mut |p| phases.push(p.phase),
    )
    .unwrap();
    assert_eq!(summary.activities_listed, 1);
    assert_eq!(summary.new_activities, 0);
    assert_eq!(summary.known_activities, 1);
    assert_eq!(summary.imported, 2);
    assert_eq!(summary.aborted, None);
    assert!(summary.last_sync_updated);
    assert!(phases.contains(&"import".to_owned()));
    // Der Abgleich schreibt keine Ordner-Einstellung mehr (T28).
    assert_eq!(store.get_setting("app.fit_dir").unwrap(), None);
    let items = service::list_activities(
        &store,
        &ActivityFilter {
            from: "2026-05-01".into(),
            to: "2026-05-31".into(),
            sport: None,
        },
    )
    .unwrap();
    assert_eq!(items.len(), 2);
    let mut paths: Vec<String> = store
        .source_files()
        .unwrap()
        .into_iter()
        .map(|f| f.row.rel_path)
        .collect();
    paths.sort();
    assert_eq!(paths, ["eigene.fit", "garmin/7001.fit"]);
    assert_eq!(
        service::garmin::status(&store, &client(&dir, true))
            .unwrap()
            .last_sync
            .as_deref(),
        Some("2026-05-20")
    );
}

/// Der erste Abgleich legt den festen Stamm und `garmin/` darin an, ohne dass ein Ordner gewählt
/// oder gemerkt wird (T28).
#[test]
fn the_first_sync_creates_the_fixed_root() {
    let dir = work_dir("assign");
    let root = dir.join("Lastwerk").join("fit");
    let mut store = Store::open_in_memory().unwrap();
    service::garmin::sync(
        &mut store,
        &Fake {
            activities: Vec::new(),
            fail: None,
        },
        &root,
        d(2026, 5, 20),
        now(),
        &mut |_| {},
    )
    .unwrap();
    assert!(root.join("garmin").is_dir());
    assert_eq!(store.get_setting("app.fit_dir").unwrap(), None);
}

/// Bricht Garmin ab, ist das ein Ergebnis mit Code, kein Fehler des Befehls.
#[test]
fn an_abort_by_garmin_is_part_of_the_summary() {
    let dir = work_dir("abort");
    let mut store = Store::open_in_memory().unwrap();
    let summary = service::garmin::sync(
        &mut store,
        &Fake {
            activities: Vec::new(),
            fail: Some(GarminError::NotConnected),
        },
        &dir.join("fit"),
        d(2026, 5, 20),
        now(),
        &mut |_| {},
    )
    .unwrap();
    let abort = summary.aborted.expect("Abbruch");
    assert_eq!(abort.code, "not_connected");
    assert!(!summary.last_sync_updated);
}

/// Markertest (SPEC 10.2): Kein Token, kein Anzeigename und kein Pfad der Ablage in einer
/// Antwort an die Oberfläche. Die Felder der Typen sind fest; ein neues Feld mit `token`,
/// `ticket`, `display_name` oder `path` im Namen fiele hier auf.
#[test]
fn no_token_no_display_name_and_no_path_reach_the_interface() {
    let dir = work_dir("marker").join("PFAD-MARKER");
    fs::create_dir_all(dir.join("fit").join("garmin")).unwrap();
    fs::write(
        dir.join("fit").join("garmin").join("7001.fit"),
        fit_bytes(0),
    )
    .unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let c = client(&dir, true);
    let status = serde_json::to_value(service::garmin::status(&store, &c).unwrap()).unwrap();
    let summary = service::garmin::sync(
        &mut store,
        &Fake::one_activity(7001),
        &dir.join("fit"),
        d(2026, 5, 20),
        now(),
        &mut |_| {},
    )
    .unwrap();
    let mut progress = Vec::new();
    service::garmin::sync(
        &mut store,
        &Fake::one_activity(7001),
        &dir.join("fit"),
        d(2026, 5, 20),
        now(),
        &mut |p| progress.push(serde_json::to_value(p).unwrap()),
    )
    .unwrap();
    let summary = serde_json::to_value(summary).unwrap();
    let events = serde_json::to_value(lastwerk_app::dto::GarminLoginEvent {
        ok: true,
        code: "connected".into(),
    })
    .unwrap();

    let mut keys = BTreeMap::new();
    for value in [&status, &summary, &events]
        .into_iter()
        .chain(progress.iter())
    {
        let text = value.to_string();
        for forbidden in [TOKEN_MARKER, NAME_MARKER, "CLIENT-MARKER", "PFAD-MARKER"] {
            assert!(!text.contains(forbidden), "{forbidden} in {text}");
        }
        collect_keys(value, &mut keys);
    }
    for key in keys.keys() {
        for word in ["token", "ticket", "display", "name", "path", "dir"] {
            assert!(!key.contains(word), "Feld {key}");
        }
    }
    assert_eq!(
        status.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["connected", "last_sync", "locked_until", "sync_on_start"]
    );
}

fn collect_keys(v: &Value, out: &mut BTreeMap<String, ()>) {
    match v {
        Value::Object(map) => {
            for (k, v) in map {
                out.insert(k.clone(), ());
                collect_keys(v, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_keys(v, out)),
        _ => {}
    }
}
