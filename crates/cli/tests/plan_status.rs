//! `plans` und `plan-status` als Prozess (SPEC 5.4: ohne diese Befehle gibt es in der Kommandozeile
//! nie einen aktiven Plan). Eingaben sind die synthetischen Fixtures aus
//! `lastwerk-testkit`, alle Dateien liegen unter `target/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lastwerk_store::{PlanStatus, Store};
use lastwerk_testkit::fixtures;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli-plan-status-tests")
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

/// Datenbank mit importierten Fixtures und zwei erzeugten Plänen (beide `draft`). Gibt die
/// Datenbank und die IDs in der Reihenfolge der Erzeugung zurück.
fn db_with_two_plans(dir: &Path) -> (PathBuf, String, String) {
    let fix = dir.join("fixtures");
    fs::create_dir_all(&fix).unwrap();
    for f in fixtures::all() {
        fs::write(fix.join(f.file_name()), &f.bytes).unwrap();
    }
    let db = dir.join("m1.db");
    ok(&lastwerk(&[&"import", &"--dir", &fix, &"--db", &db]));
    let mut ids = Vec::new();
    for name in ["a.json", "b.json"] {
        let plan = dir.join(name);
        ok(&lastwerk(&[
            &"plan",
            &"--db",
            &db,
            &"--week-start",
            &"2027-01-04",
            &"--weeks",
            &"2",
            &"--out",
            &plan,
        ]));
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&plan).unwrap()).unwrap();
        ids.push(value["plan_id"].as_str().unwrap().to_string());
    }
    let b = ids.pop().unwrap();
    let a = ids.pop().unwrap();
    (db, a, b)
}

fn status_of(db: &Path, id: &str) -> PlanStatus {
    Store::open(db)
        .unwrap()
        .list_plans()
        .unwrap()
        .into_iter()
        .find(|p| p.id == id)
        .unwrap()
        .status
}

fn set_status(db: &Path, id: &str, status: &str) -> Output {
    lastwerk(&[
        &"plan-status",
        &"--db",
        &db,
        &"--id",
        &id,
        &"--status",
        &status,
    ])
}

#[test]
fn plans_lists_id_status_date_and_weeks() {
    let dir = work_dir("list");
    let (db, a, b) = db_with_two_plans(&dir);

    let listing = ok(&lastwerk(&[&"plans", &"--db", &db]));
    assert_eq!(listing.lines().count(), 2, "{listing}");
    for id in [&a, &b] {
        let line = listing.lines().find(|l| l.contains(id.as_str())).unwrap();
        assert!(line.contains("draft"), "{line}");
        assert!(line.contains("Wochen: 2"), "{line}");
        // Erstellungsdatum ohne Uhrzeit: JJJJ-MM-TT. Die einzigen Doppelpunkte der Zeile
        // stehen hinter "erstellt" und "Wochen".
        let date = line.split("erstellt: ").nth(1).unwrap();
        let date = date.split_whitespace().next().unwrap();
        assert!(
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok(),
            "{line}"
        );
        assert_eq!(line.matches(':').count(), 2, "{line}");
    }

    ok(&set_status(&db, &a, "active"));
    let listing = ok(&lastwerk(&[&"plans", &"--db", &db]));
    let line = listing.lines().find(|l| l.contains(a.as_str())).unwrap();
    assert!(line.contains("active"), "{line}");
}

#[test]
fn plans_on_an_empty_database_says_so() {
    let dir = work_dir("empty");
    let db = dir.join("empty.db");
    drop(Store::open(&db).unwrap());
    let listing = ok(&lastwerk(&[&"plans", &"--db", &db]));
    assert!(listing.contains("Keine Pläne gespeichert"), "{listing}");
}

#[test]
fn plan_status_sets_every_status() {
    let dir = work_dir("set");
    let (db, a, _) = db_with_two_plans(&dir);
    assert_eq!(status_of(&db, &a), PlanStatus::Draft);
    let report = ok(&set_status(&db, &a, "active"));
    assert!(report.contains("Status active"), "{report}");
    assert_eq!(status_of(&db, &a), PlanStatus::Active);
    ok(&set_status(&db, &a, "archived"));
    assert_eq!(status_of(&db, &a), PlanStatus::Archived);
    ok(&set_status(&db, &a, "draft"));
    assert_eq!(status_of(&db, &a), PlanStatus::Draft);
}

/// Dieselbe Semantik wie `set_plan_status` der App (Store, 6.2): höchstens ein aktiver Plan,
/// das Aktivieren eines zweiten archiviert den ersten. Das Archivieren oder auf Entwurf setzen
/// berührt andere Pläne nicht.
#[test]
fn activating_a_second_plan_archives_the_first() {
    let dir = work_dir("second");
    let (db, a, b) = db_with_two_plans(&dir);
    ok(&set_status(&db, &a, "active"));
    let report = ok(&set_status(&db, &b, "active"));
    assert!(
        report.contains("jetzt archiviert") && report.contains(&a),
        "{report}"
    );
    assert_eq!(status_of(&db, &a), PlanStatus::Archived);
    assert_eq!(status_of(&db, &b), PlanStatus::Active);

    // Erneutes Aktivieren desselben Plans archiviert nichts anderes und meldet es nicht.
    let report = ok(&set_status(&db, &b, "active"));
    assert!(!report.contains("archiviert"), "{report}");
    assert_eq!(status_of(&db, &b), PlanStatus::Active);

    // Auf Entwurf setzen lässt den anderen Plan unverändert.
    ok(&set_status(&db, &a, "draft"));
    assert_eq!(status_of(&db, &a), PlanStatus::Draft);
    assert_eq!(status_of(&db, &b), PlanStatus::Active);
    let active = Store::open(&db).unwrap().active_plan().unwrap().unwrap();
    assert_eq!(active.plan_id, b);
}

#[test]
fn unknown_id_and_bad_arguments_are_input_errors() {
    let dir = work_dir("errors");
    let (db, a, _) = db_with_two_plans(&dir);

    let out = set_status(&db, "gibt-es-nicht", "active");
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("Einen solchen Plan gibt es nicht"));
    assert!(!text(&out.stderr).contains(dir.to_str().unwrap()));
    // Der Bestand bleibt unverändert.
    assert_eq!(status_of(&db, &a), PlanStatus::Draft);

    // Unbekannter Status: clap endet mit 2.
    let out = set_status(&db, &a, "unsinn");
    assert_eq!(out.status.code(), Some(2));

    // Fehlende Datenbank: Exit 2, es wird keine angelegt.
    let missing = dir.join("fehlt.db");
    for out in [
        lastwerk(&[&"plans", &"--db", &missing]),
        set_status(&missing, &a, "active"),
    ] {
        assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    }
    assert!(!missing.exists());
}
