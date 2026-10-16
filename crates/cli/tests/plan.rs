//! `plan`, `validate`, `export-ics`, `backup` und `restore` als Prozess (SPEC 5.4, 12 M3) und
//! der Wiederherstellungstest aus SPEC 6.3. Eingaben sind die synthetischen Fixtures aus
//! `lastwerk-testkit`, alle Dateien liegen unter `target/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lastwerk_store::{Store, UserDataExport};
use lastwerk_testkit::fixtures;
use serde_json::Value;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli-plan-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_fixtures(dir: &Path) -> PathBuf {
    let out = dir.join("fixtures");
    fs::create_dir_all(&out).unwrap();
    for f in fixtures::all() {
        fs::write(out.join(f.file_name()), &f.bytes).unwrap();
    }
    out
}

fn lastwerk(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lastwerk"));
    for a in args {
        cmd.arg(a.as_ref());
    }
    // Kein Zugriff auf echte Nutzerordner, falls ein Test den Standardpfad nutzt.
    cmd.env_remove("XDG_DATA_HOME")
        .env_remove("LOCALAPPDATA")
        .env_remove("LASTWERK_DB");
    cmd.output().expect("lastwerk startet")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn ok(out: &Output) -> String {
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    text(&out.stdout)
}

/// Datenbank mit importierten Fixtures, Schwellen und Verfügbarkeit.
fn prepared_db(dir: &Path) -> (PathBuf, PathBuf) {
    let fix = write_fixtures(dir);
    let db = dir.join("m1.db");
    ok(&lastwerk(&[&"import", &"--dir", &fix, &"--db", &db]));
    ok(&lastwerk(&[
        &"thresholds",
        &"--db",
        &db,
        &"set",
        &"--sport",
        &"cycling",
        &"--ftp",
        &"250",
        &"--lthr",
        &"160",
        &"--from",
        &"2026-01-01",
        &"--dir",
        &fix,
    ]));
    ok(&lastwerk(&[
        &"athlete", &"--db", &db, &"set", &"--sex", &"male",
    ]));
    for (day, minutes) in [("mon", "60"), ("wed", "90"), ("sat", "240"), ("sun", "150")] {
        ok(&lastwerk(&[
            &"availability",
            &"--db",
            &db,
            &"set",
            &"--weekday",
            &day,
            &"--max-minutes",
            &minutes,
        ]));
    }
    ok(&lastwerk(&[
        &"availability",
        &"--db",
        &db,
        &"set",
        &"--weekday",
        &"fr",
        &"--max-minutes",
        &"0",
        &"--fixed-rest",
    ]));
    (fix, db)
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn session_count(plan: &Value) -> usize {
    plan["weeks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|w| w["days"].as_array().unwrap())
        .map(|d| d["sessions"].as_array().unwrap().len())
        .sum()
}

#[test]
fn plan_validate_export_and_backup_follow_the_acceptance_commands() {
    let dir = work_dir("acceptance");
    let fix = write_fixtures(&dir);
    let db = dir.join("m1.db");
    ok(&lastwerk(&[&"import", &"--dir", &fix, &"--db", &db]));
    let plan = dir.join("plan.json");
    let ics = dir.join("plan.ics");
    let backup = dir.join("backup.json");

    let out = lastwerk(&[
        &"plan",
        &"--db",
        &db,
        &"--week-start",
        &"2027-01-04",
        &"--goal-date",
        &"2027-05-01",
        &"--out",
        &plan,
    ]);
    let report = ok(&out);
    assert!(report.contains("17 Wochen"), "{report}");
    // Ohne gesetzte Verfügbarkeit und ohne Historie weist die Fehlerausgabe darauf hin.
    let hints = text(&out.stderr);
    assert!(hints.contains("Keine Verfügbarkeit gesetzt"), "{hints}");

    let value = read_json(&plan);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["source"], "rules");
    assert_eq!(value["goal"]["kind"], "event");
    assert_eq!(value["goal"]["event_date"], "2027-05-01");
    let weeks = value["weeks"].as_array().unwrap();
    assert_eq!(weeks.len(), 17);
    assert_eq!(weeks[0]["week_start"], "2027-01-04");
    assert_eq!(weeks[16]["week_start"], "2027-04-26");
    assert_eq!(weeks[16]["phase"], "race");
    let id = value["plan_id"].as_str().unwrap();
    assert_eq!(id.len(), 36);
    assert_eq!(id.as_bytes()[14], b'4', "UUID Version 4");

    // Der Plan liegt als Entwurf in der Datenbank.
    let store = Store::open(&db).unwrap();
    let plans = store.list_plans().unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].id, id);
    assert_eq!(plans[0].status, lastwerk_store::PlanStatus::Draft);
    drop(store);

    let out = lastwerk(&[&"validate", &plan, &"--db", &db]);
    let report = ok(&out);
    assert!(report.contains("Schema: in Ordnung"), "{report}");
    assert!(report.contains("keine harte Verletzung"), "{report}");
    // Weiche Verletzungen stehen als Warnungen in der Ausgabe und ändern den Exit-Code nicht.
    if !value["warnings"].as_array().unwrap().is_empty() {
        assert!(report.contains("Warnungen ("), "{report}");
    }

    let report = ok(&lastwerk(&[&"export-ics", &plan, &"--out", &ics]));
    let events = session_count(&value);
    assert!(
        report.contains(&format!("{events} Veranstaltungen")),
        "{report}"
    );
    let calendar = fs::read_to_string(&ics).unwrap();
    assert_eq!(calendar.matches("BEGIN:VEVENT").count(), events);
    assert!(!calendar.contains("LOCATION") && !calendar.contains("GEO"));

    ok(&lastwerk(&[&"backup", &"--db", &db, &"--out", &backup]));
    let b = read_json(&backup);
    assert_eq!(b["backup_version"], 1);
    assert!(b["exported_at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(b["plans"].as_array().unwrap().len(), 1);
}

#[test]
fn plan_uses_the_goal_and_the_availability_from_the_database() {
    let dir = work_dir("from_db");
    let (_, db) = prepared_db(&dir);
    ok(&lastwerk(&[
        &"goal",
        &"--db",
        &db,
        &"set",
        &"--date",
        &"2027-03-27",
        &"--sport",
        &"cycling",
        &"--priority",
        &"B",
        &"--label",
        &"Zielrennen",
    ]));
    let listing = ok(&lastwerk(&[&"goal", &"--db", &db, &"list"]));
    assert!(listing.contains("2027-03-27") && listing.contains("aktiv"));
    let listing = ok(&lastwerk(&[&"availability", &"--db", &db, &"list"]));
    assert!(listing.contains("Freitag") && listing.contains("fester Ruhetag: ja"));

    let plan = dir.join("plan.json");
    let out = lastwerk(&[
        &"plan",
        &"--db",
        &db,
        &"--week-start",
        &"2027-01-04",
        &"--out",
        &plan,
    ]);
    ok(&out);
    let value = read_json(&plan);
    assert_eq!(value["goal"]["event_date"], "2027-03-27");
    assert_eq!(value["goal"]["priority"], "B");
    assert_eq!(value["weeks"].as_array().unwrap().len(), 12);
    // Freitag ist ein fester Ruhetag, die gesetzten Zeiten gelten.
    for week in value["weeks"].as_array().unwrap() {
        let friday = &week["days"][4];
        assert!(friday["sessions"].as_array().unwrap().is_empty());
        let tuesday = &week["days"][1];
        let thursday = &week["days"][3];
        // Dienstag und Donnerstag haben keine Angabe: 90 Minuten.
        for day in [tuesday, thursday] {
            let minutes: u64 = day["sessions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["duration_min"].as_u64().unwrap())
                .sum();
            assert!(minutes <= 90, "{minutes}");
        }
    }
    // Das Ziel steht mit im Plan der Datenbank.
    let store = Store::open(&db).unwrap();
    assert!(store.list_plans().unwrap()[0].goal_id.is_some());
    // Für die Verfügbarkeit und das Ziel ist nichts mehr zu sagen: kein Hinweis.
    assert!(!text(&out.stderr).contains("Keine Verfügbarkeit gesetzt"));
    // Validierung gegen dieselbe Datenbank ist ohne harte Verletzung.
    ok(&lastwerk(&[&"validate", &plan, &"--db", &db]));
}

#[test]
fn continuous_plans_and_english_texts() {
    let dir = work_dir("continuous");
    let (_, db) = prepared_db(&dir);
    let plan = dir.join("plan.json");
    ok(&lastwerk(&[
        &"plan",
        &"--db",
        &db,
        &"--week-start",
        &"2027-01-04",
        &"--weeks",
        &"4",
        &"--locale",
        &"en",
        &"--sports",
        &"cycling,running",
        &"--strength",
        &"0",
        &"--out",
        &plan,
    ]));
    let value = read_json(&plan);
    assert_eq!(value["goal"], serde_json::json!({"kind": "continuous"}));
    assert_eq!(value["locale"], "en");
    assert_eq!(value["weeks"].as_array().unwrap().len(), 4);
    let ics = dir.join("plan.ics");
    ok(&lastwerk(&[&"export-ics", &plan, &"--out", &ics]));
    let calendar = fs::read_to_string(&ics).unwrap();
    assert!(calendar.contains("(Cycling)") || calendar.contains("(Running)"));
    assert!(!value.to_string().contains("strength"));
}

#[test]
fn input_errors_exit_with_2() {
    let dir = work_dir("input_errors");
    let (_, db) = prepared_db(&dir);
    let plan = dir.join("plan.json");
    // Kein Montag.
    let out = lastwerk(&[
        &"plan",
        &"--db",
        &db,
        &"--week-start",
        &"2027-01-05",
        &"--out",
        &plan,
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("kein Montag"));
    assert!(!plan.exists());
    // Termin vor dem Planbeginn.
    let out = lastwerk(&[
        &"plan",
        &"--db",
        &db,
        &"--week-start",
        &"2027-01-04",
        &"--goal-date",
        &"2026-12-01",
        &"--out",
        &plan,
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    // Fehlende Datenbank, fehlende Plandatei, kein JSON, falsches Datum.
    let missing = dir.join("gibt-es-nicht.db");
    let out = lastwerk(&[
        &"plan",
        &"--db",
        &missing,
        &"--week-start",
        &"2027-01-04",
        &"--out",
        &plan,
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!missing.exists());
    let out = lastwerk(&[&"validate", &dir.join("nope.json"), &"--db", &db]);
    assert_eq!(out.status.code(), Some(2));
    let garbage = dir.join("garbage.json");
    fs::write(&garbage, "kein json").unwrap();
    assert_eq!(
        lastwerk(&[&"validate", &garbage, &"--db", &db])
            .status
            .code(),
        Some(2)
    );
    let out = lastwerk(&[
        &"plan",
        &"--db",
        &db,
        &"--week-start",
        &"04.01.2027",
        &"--out",
        &plan,
    ]);
    assert_eq!(out.status.code(), Some(2));
    // Mehr als 26 Wochen.
    let out = lastwerk(&[
        &"plan",
        &"--db",
        &db,
        &"--week-start",
        &"2027-01-04",
        &"--weeks",
        &"27",
        &"--out",
        &plan,
    ]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn validate_rejects_schema_violations_and_hard_rule_violations_with_exit_1() {
    let dir = work_dir("validate");
    let (_, db) = prepared_db(&dir);
    let plan_path = dir.join("plan.json");
    ok(&lastwerk(&[
        &"plan",
        &"--db",
        &db,
        &"--week-start",
        &"2027-01-04",
        &"--weeks",
        &"3",
        &"--out",
        &plan_path,
    ]));
    let good = read_json(&plan_path);
    ok(&lastwerk(&[&"validate", &plan_path, &"--db", &db]));

    // Schemaverletzung: Pflichtfeld fehlt. Auch export-ics lehnt den Plan ab.
    let mut broken = good.clone();
    broken.as_object_mut().unwrap().remove("weeks");
    let bad = dir.join("no_weeks.json");
    fs::write(&bad, broken.to_string()).unwrap();
    let out = lastwerk(&[&"validate", &bad, &"--db", &db]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out.stderr).contains("Schema"),
        "{}",
        text(&out.stderr)
    );
    let ics = dir.join("bad.ics");
    let out = lastwerk(&[&"export-ics", &bad, &"--out", &ics]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!ics.exists());

    // Harte Regel: harte Einheiten an zwei aufeinanderfolgenden Tagen (R4).
    let mut hard = good.clone();
    let template = hard["weeks"][0]["days"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|d| d["sessions"].as_array().unwrap())
        .next()
        .expect("der Plan hat eine Einheit")
        .clone();
    for day in [0, 1] {
        let mut s = template.clone();
        s["hard"] = serde_json::json!(true);
        s["slot"] = serde_json::json!("am");
        hard["weeks"][0]["days"][day]["sessions"] = serde_json::json!([s]);
    }
    let path = dir.join("hard.json");
    fs::write(&path, hard.to_string()).unwrap();
    let out = lastwerk(&[&"validate", &path, &"--db", &db]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stdout));
    let err = text(&out.stderr);
    assert!(err.contains("harte Regeln") && err.contains("R04"), "{err}");

    // Der unveränderte Plan besteht weiterhin, weiche Verletzungen wären nur Warnungen.
    let out = lastwerk(&[&"validate", &plan_path, &"--db", &db]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn restore_rejects_unknown_versions_and_broken_files_with_exit_2() {
    let dir = work_dir("restore_errors");
    let (_, db) = prepared_db(&dir);
    let backup = dir.join("backup.json");
    ok(&lastwerk(&[&"backup", &"--db", &db, &"--out", &backup]));
    let before = Store::open(&db).unwrap().export_user_data().unwrap();

    let newer = fs::read_to_string(&backup)
        .unwrap()
        .replace("\"backup_version\": 1", "\"backup_version\": 2");
    let path = dir.join("newer.json");
    fs::write(&path, newer).unwrap();
    let out = lastwerk(&[&"restore", &"--db", &db, &"--in", &path]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let garbage = dir.join("garbage.json");
    fs::write(&garbage, "[1, 2").unwrap();
    assert_eq!(
        lastwerk(&[&"restore", &"--db", &db, &"--in", &garbage])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        lastwerk(&[&"restore", &"--db", &db, &"--in", &dir.join("nope.json")])
            .status
            .code(),
        Some(2)
    );
    // Der Bestand bleibt unverändert.
    assert_eq!(
        Store::open(&db).unwrap().export_user_data().unwrap(),
        before
    );
}

/// Wiederherstellung (SPEC 6.3, 12 M3): Datenbank mit Fixtures, Schwellen, Ziel, Verfügbarkeit
/// und Plan sichern, in eine neue leere Datenbank einspielen, den Cache aus den Fixtures neu
/// aufbauen. Dann stimmen Nutzerdaten und Kennzahlen überein.
#[test]
fn backup_restore_and_rebuild_reproduce_user_data_and_figures() {
    let dir = work_dir("restore");
    let (fix, a) = prepared_db(&dir);
    ok(&lastwerk(&[
        &"goal",
        &"--db",
        &a,
        &"set",
        &"--date",
        &"2027-03-27",
        &"--sport",
        &"cycling",
    ]));
    let plan = dir.join("plan.json");
    ok(&lastwerk(&[
        &"plan",
        &"--db",
        &a,
        &"--week-start",
        &"2027-01-04",
        &"--out",
        &plan,
    ]));
    let backup = dir.join("backup.json");
    ok(&lastwerk(&[&"backup", &"--db", &a, &"--out", &backup]));

    // Neue, leere Datenbank: Sicherung einspielen, dann den Cache aus den Fixtures aufbauen.
    let b = dir.join("restored.db");
    ok(&lastwerk(&[&"restore", &"--db", &b, &"--in", &backup]));
    ok(&lastwerk(&[&"rebuild", &"--dir", &fix, &"--db", &b]));

    let sa = Store::open(&a).unwrap();
    let sb = Store::open(&b).unwrap();
    // Schwellenwerte, Ziele, Verfügbarkeit, Pläne, Einstellungen: identisch.
    let (ea, eb): (UserDataExport, UserDataExport) = (
        sa.export_user_data().unwrap(),
        sb.export_user_data().unwrap(),
    );
    assert_eq!(ea, eb);
    assert_eq!(ea.thresholds.len(), 2);
    assert_eq!(ea.goals.len(), 1);
    assert_eq!(ea.availability.len(), 5);
    assert_eq!(ea.plans.len(), 1);
    // Geschlecht und seit T39 seine Herkunft (`athlete.sex_origin`, manuell).
    assert_eq!(ea.settings.len(), 2);
    assert_eq!(sa.list_plans().unwrap(), sb.list_plans().unwrap());
    let id = sa.list_plans().unwrap()[0].id.clone();
    assert_eq!(sa.load_plan(&id).unwrap(), sb.load_plan(&id).unwrap());
    assert!(sb.load_plan(&id).unwrap().is_some());

    // Kennzahlen: Aktivitäten und Belastungen (ohne Berechnungszeitpunkt) sowie der Verlauf.
    let epoch = chrono::DateTime::from_timestamp(0, 0).unwrap();
    let loads = |s: &Store| {
        s.activity_loads()
            .unwrap()
            .into_iter()
            .map(|mut l| {
                l.computed_at = epoch;
                l
            })
            .collect::<Vec<_>>()
    };
    assert!(!sa.activities().unwrap().is_empty());
    assert_eq!(sa.activities().unwrap(), sb.activities().unwrap());
    assert_eq!(loads(&sa), loads(&sb));
    assert!(loads(&sa).iter().any(|l| l.value.is_some()));
    let pmc = |db: &Path| {
        let out = lastwerk(&[
            &"pmc",
            &"--db",
            &db,
            &"--from",
            &"2026-05-01",
            &"--to",
            &"2026-06-30",
        ]);
        ok(&out)
    };
    assert_eq!(pmc(&a), pmc(&b));

    // Ein Plan aus der wiederhergestellten Datenbank lässt sich prüfen.
    ok(&lastwerk(&[&"validate", &plan, &"--db", &b]));
}
