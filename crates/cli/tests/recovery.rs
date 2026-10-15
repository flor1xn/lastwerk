//! `import-recovery` als Prozess (SPEC 5.4, 7.3, 12 M4d, Task T14) und der Weg bis zur
//! Planregel R12. Die Dateien entstehen unter `target/`, alle Werte sind erfunden. Dateien aus
//! `exchange/out` sind nicht vertrauenswürdig: gültige und ungültige liegen gemischt im Ordner,
//! der Import läuft zu Ende und zählt.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use chrono::{Datelike, NaiveDate, TimeDelta, Weekday};
use lastwerk_store::Store;
use lastwerk_testkit::fixtures::{LOCAL_OFFSET_S, START_FIT};
use lastwerk_testkit::generators::{ActivityParams, activity};
use serde_json::Value;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli-recovery-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
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

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// Gültige Datei nach SPEC 7.3.
fn day_json(date: &str, hrv: f64, sleep_s: u32) -> String {
    format!(
        r#"{{"schema_version":1,"date":"{date}","hrv_overnight_ms":{hrv},"resting_hr_bpm":49.0,
"sleep_duration_s":{sleep_s},"sleep_score":81,"source":"garmin-bridge 0.1.0",
"fetched_at":"2026-10-19T05:00:00Z"}}"#
    )
}

fn write(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

/// Zeile `  <Name>: <Zahl>` aus dem Bericht.
fn count(report: &str, label: &str) -> usize {
    let line = report
        .lines()
        .find(|l| l.trim_start().starts_with(label))
        .unwrap_or_else(|| panic!("Zeile {label:?} fehlt in:\n{report}"));
    line.split(':')
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|w| w.parse().ok())
        .unwrap_or_else(|| panic!("keine Zahl in {line:?}"))
}

fn rows(db: &Path) -> Vec<lastwerk_store::RecoveryDayRecord> {
    Store::open(db)
        .unwrap()
        .recovery_days(date("2000-01-01"), date("2100-01-01"))
        .unwrap()
}

/// Ordner mit zwei gültigen Dateien und je einem Beispiel für jede Ablehnung.
fn mixed_dir(dir: &Path) -> PathBuf {
    let rec = dir.join("recovery");
    fs::create_dir_all(&rec).unwrap();
    write(
        &rec,
        "2026-10-10.json",
        &day_json("2026-10-10", 62.5, 27000),
    );
    write(
        &rec,
        "2026-10-11.json",
        &day_json("2026-10-11", 58.0, 25200),
    );
    // Name ist kein Datum oder nicht `JJJJ-MM-TT.json`.
    write(
        &rec,
        "2026-10-12-extra.json",
        &day_json("2026-10-12", 60.0, 25200),
    );
    write(
        &rec,
        "2026-02-30.json",
        &day_json("2026-02-30", 60.0, 25200),
    );
    write(
        &rec,
        "2026-10-13.JSON",
        &day_json("2026-10-13", 60.0, 25200),
    );
    // Das Feld date nennt einen anderen Tag.
    write(
        &rec,
        "2026-10-14.json",
        &day_json("2026-10-15", 60.0, 25200),
    );
    // Schema: Wert außerhalb des Bereichs, unbekanntes Feld, Pflichtfeld fehlt, falscher Typ,
    // falsche Version, kein Datumsformat.
    write(
        &rec,
        "2026-10-15.json",
        &day_json("2026-10-15", 400.5, 25200),
    );
    write(
        &rec,
        "2026-10-16.json",
        &day_json("2026-10-16", 60.0, 25200).replace(
            r#""source""#,
            r#""path":"../../etc/passwd","note":"GEHEIMER-MARKER","source""#,
        ),
    );
    write(
        &rec,
        "2026-10-17.json",
        r#"{"schema_version":1,"date":"2026-10-17","fetched_at":"2026-10-19T05:00:00Z"}"#,
    );
    write(
        &rec,
        "2026-10-18.json",
        &day_json("2026-10-18", 60.0, 25200).replace("25200", r#""lang""#),
    );
    write(
        &rec,
        "2026-10-19.json",
        &day_json("2026-10-19", 60.0, 25200)
            .replace(r#""schema_version":1"#, r#""schema_version":2"#),
    );
    write(
        &rec,
        "2026-10-20.json",
        &day_json("2026-10-20", 60.0, 25200).replace("2026-10-19T05:00:00Z", "gestern"),
    );
    // Kein JSON, leere Datei, Wurzel kein Objekt.
    write(&rec, "2026-10-21.json", "{ das ist kein json");
    write(&rec, "2026-10-22.json", "");
    write(&rec, "2026-10-23.json", "[1,2,3]");
    // Zu groß: gültiges JSON mit viel Leerraum, über dem Limit von 64 KiB.
    let padded = format!(
        "{}{}",
        day_json("2026-10-24", 60.0, 25200),
        " ".repeat(70_000)
    );
    write(&rec, "2026-10-24.json", &padded);
    // Steuerzeichen im Dateinamen dürfen das Terminal nicht erreichen. Windows erlaubt
    // solche Namen im Dateisystem nicht, dort entfällt der Fall.
    #[cfg(not(windows))]
    write(&rec, "2026-10-25\u{1b}[31mROT.json", "{}");
    // Übersprungen: Unterordner, andere Endungen, temporäre Datei des Zusatzprogramms.
    fs::create_dir_all(rec.join("unterordner")).unwrap();
    write(&rec, "notizen.txt", "nichts");
    write(
        &rec,
        ".2026-10-26.json.tmp",
        &day_json("2026-10-26", 60.0, 25200),
    );
    #[cfg(unix)]
    {
        // Ein Link auf eine gültige Datei außerhalb des Ordners wird nicht gefolgt.
        let outside = dir.join("draussen.json");
        fs::write(&outside, day_json("2026-10-27", 60.0, 25200)).unwrap();
        std::os::unix::fs::symlink(&outside, rec.join("2026-10-27.json")).unwrap();
    }
    rec
}

#[test]
fn valid_files_are_imported_and_invalid_ones_are_rejected_and_counted() {
    let dir = work_dir("mixed");
    let rec = mixed_dir(&dir);
    let db = dir.join("m4.db");
    let out = lastwerk(&[&"import-recovery", &"--dir", &rec, &"--db", &db]);
    let report = ok(&out);

    assert_eq!(count(&report, "gültig"), 2, "{report}");
    assert!(
        report.contains("neu 2, aktualisiert 0, unverändert 0"),
        "{report}"
    );
    let unix = usize::from(cfg!(unix));
    // Ungültig: 15 Beispiele ohne Link, dazu der Link unter Unix.
    let control = usize::from(!cfg!(windows));
    assert_eq!(count(&report, "ungültig"), 14 + control + unix, "{report}");
    // Übersprungen: Unterordner, notizen.txt, .tmp.
    assert_eq!(count(&report, "übersprungen"), 3, "{report}");

    for code in [
        "bad_name",
        "date_mismatch",
        "schema",
        "not_json",
        "too_large",
    ] {
        assert!(
            report.contains(&format!("[{code}]")),
            "{code} fehlt:\n{report}"
        );
    }
    if cfg!(unix) {
        assert!(report.contains("[not_regular_file]"), "{report}");
    }
    // Die Schemaverletzung nennt Pfad und Schlüsselwort, nie Werte oder fremde Feldnamen.
    assert!(
        report.contains("2026-10-15.json: verletzt das Schema (/hrv_overnight_ms: maximum)"),
        "{report}"
    );
    assert!(
        report.contains("2026-10-17.json: verletzt das Schema (/: required)"),
        "{report}"
    );
    for forbidden in ["GEHEIMER-MARKER", "passwd", "400.5", "\u{1b}"] {
        assert!(
            !report.contains(forbidden),
            "{forbidden:?} im Bericht:\n{report}"
        );
    }
    assert!(
        !report.contains(rec.to_str().unwrap()),
        "der Bericht nennt den Ordnerpfad"
    );

    // In der Datenbank stehen genau die beiden gültigen Tage mit ihren Werten.
    let stored = rows(&db);
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].date, date("2026-10-10"));
    assert_eq!(stored[0].hrv_overnight_ms, Some(62.5));
    assert_eq!(stored[0].resting_hr_bpm, Some(49.0));
    assert_eq!(stored[0].sleep_duration_s, Some(27000));
    assert_eq!(stored[0].sleep_score, Some(81));
    assert_eq!(stored[0].source, "garmin-bridge 0.1.0");
    assert_eq!(stored[1].date, date("2026-10-11"));
    assert_eq!(stored[1].sleep_duration_s, Some(25200));
}

#[test]
fn a_second_run_changes_nothing_and_a_revised_file_updates_the_row() {
    let dir = work_dir("repeat");
    let rec = dir.join("recovery");
    fs::create_dir_all(&rec).unwrap();
    write(
        &rec,
        "2026-10-10.json",
        &day_json("2026-10-10", 62.5, 27000),
    );
    write(
        &rec,
        "2026-10-11.json",
        &day_json("2026-10-11", 58.0, 25200),
    );
    let db = dir.join("m4.db");
    ok(&lastwerk(&[
        &"import-recovery",
        &"--dir",
        &rec,
        &"--db",
        &db,
    ]));
    let before = rows(&db);
    assert_eq!(before.len(), 2);

    let again = ok(&lastwerk(&[
        &"import-recovery",
        &"--dir",
        &rec,
        &"--db",
        &db,
    ]));
    assert!(
        again.contains("neu 0, aktualisiert 0, unverändert 2"),
        "{again}"
    );
    assert_eq!(rows(&db), before, "ein zweiter Lauf schreibt nichts");

    // Garmin finalisiert nachträglich: andere Werte ersetzen die Zeile.
    write(
        &rec,
        "2026-10-11.json",
        &day_json("2026-10-11", 55.0, 24000),
    );
    let third = ok(&lastwerk(&[
        &"import-recovery",
        &"--dir",
        &rec,
        &"--db",
        &db,
    ]));
    assert!(
        third.contains("neu 0, aktualisiert 1, unverändert 1"),
        "{third}"
    );
    let after = rows(&db);
    assert_eq!(after[0], before[0]);
    assert_eq!(after[1].hrv_overnight_ms, Some(55.0));
    assert_eq!(after[1].sleep_duration_s, Some(24000));
}

#[test]
fn a_missing_folder_is_an_input_error_and_creates_no_database() {
    let dir = work_dir("missing");
    let db = dir.join("m4.db");
    let out = lastwerk(&[
        &"import-recovery",
        &"--dir",
        &dir.join("gibt-es-nicht"),
        &"--db",
        &db,
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(!db.exists());
}

#[test]
fn an_empty_folder_is_fine() {
    let dir = work_dir("empty");
    let rec = dir.join("recovery");
    fs::create_dir_all(&rec).unwrap();
    let db = dir.join("m4.db");
    let report = ok(&lastwerk(&[
        &"import-recovery",
        &"--dir",
        &rec,
        &"--db",
        &db,
    ]));
    assert_eq!(count(&report, "gültig"), 0);
    assert_eq!(count(&report, "ungültig"), 0);
    assert!(rows(&db).is_empty());
}

#[test]
fn a_file_at_the_size_limit_is_accepted_and_one_byte_more_is_not() {
    let dir = work_dir("limit");
    let rec = dir.join("recovery");
    fs::create_dir_all(&rec).unwrap();
    let base = day_json("2026-10-10", 60.0, 25200);
    let pad = |name: &str, bytes: usize| {
        write(
            &rec,
            name,
            &format!("{base}{}", " ".repeat(bytes - base.len())),
        );
    };
    pad("2026-10-10.json", 64 * 1024);
    let second = day_json("2026-10-11", 60.0, 25200);
    write(
        &rec,
        "2026-10-11.json",
        &format!("{second}{}", " ".repeat(64 * 1024 + 1 - second.len())),
    );
    let db = dir.join("m4.db");
    let report = ok(&lastwerk(&[
        &"import-recovery",
        &"--dir",
        &rec,
        &"--db",
        &db,
    ]));
    assert_eq!(count(&report, "gültig"), 1, "{report}");
    assert_eq!(count(&report, "ungültig"), 1, "{report}");
    assert!(report.contains("2026-10-11.json"), "{report}");
    assert!(report.contains("[too_large]"), "{report}");
}

/// Baseline: HRV 60, 62, 58, 61, 59, 60, 60 am 11. bis 17. Oktober (Mittel 60 ms), 7 Stunden
/// Schlaf. Der jüngste Tag vor dem Montag 19. Oktober ist der 18., sein HRV-Wert ist `last`.
fn write_week(rec: &Path, last_hrv: f64) {
    let hrv = [60.0, 62.0, 58.0, 61.0, 59.0, 60.0, 60.0];
    for (i, v) in hrv.iter().enumerate() {
        let day = format!("2026-10-{:02}", 11 + i);
        write(rec, &format!("{day}.json"), &day_json(&day, *v, 25200));
    }
    write(
        rec,
        "2026-10-18.json",
        &day_json("2026-10-18", last_hrv, 25200),
    );
}

/// Sechs Wochen Radfahrten (eine Stunde, 200 W, außer freitags) bis zum Sonntag vor dem
/// 19. Oktober. Mit FTP 250 sind das je Fahrt rund 64 Punkte Belastung, die Fitness steigt
/// auf einen Wert, bei dem der Planer harte Einheiten ansetzt.
fn write_history(dir: &Path) -> PathBuf {
    let hist = dir.join("history");
    fs::create_dir_all(&hist).unwrap();
    let origin = date("2026-05-10"); // Starttag der Fixtures (START_FIT)
    let first = date("2026-09-07");
    for i in 0..42 {
        let day = first + TimeDelta::days(i);
        if day.weekday() == Weekday::Fri {
            continue;
        }
        let offset = u32::try_from((day - origin).num_days()).unwrap();
        let params = ActivityParams::cycling(START_FIT + offset * 86_400, LOCAL_OFFSET_S, 3600);
        fs::write(hist.join(format!("ride-{day}.fit")), activity(&params)).unwrap();
    }
    hist
}

/// Datenbank mit FTP 250 und der Historie. Die Schwelle steht vor dem Import, damit die
/// Belastung beim Import entsteht.
fn db_with_history(dir: &Path, name: &str, recovery: Option<&Path>) -> PathBuf {
    let db = dir.join(name);
    ok(&lastwerk(&[
        &"thresholds",
        &"--db",
        &db,
        &"set",
        &"--sport",
        &"cycling",
        &"--ftp",
        &"250",
        &"--from",
        &"2026-01-01",
    ]));
    ok(&lastwerk(&[
        &"import",
        &"--dir",
        &write_history(dir),
        &"--db",
        &db,
    ]));
    if let Some(rec) = recovery {
        ok(&lastwerk(&[
            &"import-recovery",
            &"--dir",
            &rec,
            &"--db",
            &db,
        ]));
    }
    db
}

fn plan_json(db: &Path, out: &Path) -> Value {
    ok(&lastwerk(&[
        &"plan",
        &"--db",
        &db,
        &"--week-start",
        &"2026-10-19",
        &"--weeks",
        &"4",
        &"--sports",
        &"cycling",
        &"--out",
        &out,
    ]));
    serde_json::from_str(&fs::read_to_string(out).unwrap()).unwrap()
}

fn first_hard(plan: &Value) -> Option<(String, String)> {
    for w in plan["weeks"].as_array().unwrap() {
        for d in w["days"].as_array().unwrap() {
            for s in d["sessions"].as_array().unwrap() {
                if s["hard"].as_bool() == Some(true) {
                    return Some((
                        d["date"].as_str().unwrap().to_string(),
                        s["template_id"].as_str().unwrap().to_string(),
                    ));
                }
            }
        }
    }
    None
}

#[test]
fn imported_recovery_data_swaps_the_first_hard_session_in_the_plan() {
    let dir = work_dir("r12");
    let healthy = dir.join("healthy");
    let tired = dir.join("tired");
    fs::create_dir_all(&healthy).unwrap();
    fs::create_dir_all(&tired).unwrap();
    // 62 ms am letzten Tag: über der Basislinie. 51 ms: genau 15 Prozent darunter (9 / 60).
    write_week(&healthy, 62.0);
    write_week(&tired, 51.0);
    let db_healthy = db_with_history(&dir, "healthy.db", Some(&healthy));
    let db_tired = db_with_history(&dir, "tired.db", Some(&tired));
    // Ohne Erholungsdaten.
    let db_none = db_with_history(&dir, "none.db", None);

    let strip = |mut v: Value| {
        let o = v.as_object_mut().unwrap();
        o.remove("plan_id");
        o.remove("created_at");
        v
    };
    let none = plan_json(&db_none, &dir.join("none.json"));
    let ok_plan = plan_json(&db_healthy, &dir.join("healthy.json"));
    let swapped = plan_json(&db_tired, &dir.join("tired.json"));

    // Ohne Signal ist der Plan derselbe wie ohne Daten.
    assert_eq!(strip(none.clone()), strip(ok_plan));

    // Mit Signal ist genau die erste harte Einheit locker geworden.
    let (date0, template0) = first_hard(&none).expect("harte Einheit im Plan ohne Daten");
    let (date1, _) = first_hard(&swapped).expect("weitere harte Einheiten bleiben");
    assert!(date1 > date0, "{date1} nach {date0}");
    let day = swapped["weeks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|w| w["days"].as_array().unwrap())
        .find(|d| d["date"] == Value::String(date0.clone()))
        .unwrap();
    let session = day["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["template_id"] != Value::String(template0.clone()))
        .expect("die Einheit des Tages wurde ersetzt");
    assert_eq!(session["hard"], Value::Bool(false));
    assert_eq!(
        session["reason_keys"],
        serde_json::json!(["reason.session.recovery_swap"])
    );
    let warnings = swapped["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w["code"] == "R12" && w["severity"] == "info" && w["date"] == date0.as_str()),
        "{warnings:?}"
    );

    // validate: Der erzeugte Plan erfüllt R12, der Plan ohne Daten verletzt sie weich.
    let good = ok(&lastwerk(&[
        &"validate",
        &dir.join("tired.json"),
        &"--db",
        &db_tired,
    ]));
    assert!(!good.contains("R12"), "{good}");
    let soft = ok(&lastwerk(&[
        &"validate",
        &dir.join("none.json"),
        &"--db",
        &db_tired,
    ]));
    assert!(soft.contains("R12 (weich)"), "{soft}");
    assert!(soft.contains("Erholungsdaten"), "{soft}");
    assert!(soft.contains(&date0), "{soft}");
}

#[test]
fn recovery_days_are_part_of_backup_and_restore() {
    let dir = work_dir("backup");
    let rec = dir.join("recovery");
    fs::create_dir_all(&rec).unwrap();
    write_week(&rec, 51.0);
    let db = dir.join("a.db");
    ok(&lastwerk(&[
        &"import-recovery",
        &"--dir",
        &rec,
        &"--db",
        &db,
    ]));
    let backup = dir.join("backup.json");
    let out = ok(&lastwerk(&[&"backup", &"--db", &db, &"--out", &backup]));
    assert!(out.contains("8 Erholungstage"), "{out}");
    let other = dir.join("b.db");
    ok(&lastwerk(&[&"restore", &"--db", &other, &"--in", &backup]));
    assert_eq!(rows(&other), rows(&db));
}

/// T22: Body Battery ist ein optionales Feld des Schemas, der Import übernimmt es und lehnt
/// Werte außerhalb von 0 bis 100 ab.
#[test]
fn import_takes_body_battery_and_refuses_out_of_range_values() {
    let dir = work_dir("body-battery");
    let rec = dir.join("recovery");
    fs::create_dir_all(&rec).unwrap();
    let with_bb = |d: &str, high: i64, low: i64| {
        day_json(d, 60.0, 25200).replace(
            r#""source""#,
            &format!(r#""body_battery_high":{high},"body_battery_low":{low},"source""#),
        )
    };
    write(&rec, "2026-10-10.json", &with_bb("2026-10-10", 96, 8));
    write(&rec, "2026-10-11.json", &with_bb("2026-10-11", 101, 8));
    write(&rec, "2026-10-12.json", &with_bb("2026-10-12", 90, -1));
    let db = dir.join("lastwerk.db");
    let out = lastwerk(&[&"import-recovery", &"--dir", &rec, &"--db", &db]);
    let report = ok(&out);
    assert_eq!(count(&report, "ungültig"), 2, "{report}");
    let stored = rows(&db);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].body_battery_high, Some(96));
    assert_eq!(stored[0].body_battery_low, Some(8));
}
