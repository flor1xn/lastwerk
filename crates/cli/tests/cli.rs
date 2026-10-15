//! Aufrufe des Programms `lastwerk` als Prozess (SPEC 5.4, 12 M1). Eingaben sind die
//! synthetischen Fixtures aus `lastwerk-testkit`, alle Dateien liegen unter `target/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lastwerk_store::{
    ActivityRecord, LoadRow, NewThreshold, SETTING_ATHLETE_SEX, Store, ThresholdKind,
    ThresholdOrigin,
};
use lastwerk_testkit::fixtures;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli-tests")
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

/// Zeile `  <Name>: <Zahl>` aus dem Bericht.
fn count(report: &str, label: &str) -> usize {
    let line = report
        .lines()
        .find(|l| l.trim_start().starts_with(label))
        .unwrap_or_else(|| panic!("Zeile {label:?} fehlt in:\n{report}"));
    line.split_whitespace()
        .find_map(|w| w.parse().ok())
        .unwrap_or_else(|| panic!("keine Zahl in {line:?}"))
}

#[test]
fn import_reports_counts_and_exits_zero_despite_broken_files() {
    let dir = work_dir("import");
    let fix = write_fixtures(&dir);
    let db = dir.join("m1.db");
    let out = lastwerk(&[&"import", &"--dir", &fix, &"--db", &db]);
    let report = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{report}\n{}",
        text(&out.stderr)
    );
    assert_eq!(count(&report, "Dateien gefunden"), 19);
    assert_eq!(count(&report, "Neu"), 15);
    assert_eq!(count(&report, "Unverändert"), 0);
    assert_eq!(count(&report, "Ohne Aktivität"), 1);
    assert_eq!(count(&report, "Fehler"), 3);
    assert!(report.contains("Aktivitäten zählend:     14 (Duplikate: 1)"));
    for code in ["crc_mismatch", "truncated", "empty_file"] {
        assert!(report.contains(code), "{code} fehlt:\n{report}");
    }
    assert!(db.is_file());

    // Zweiter Lauf: alles unverändert.
    let again = text(&lastwerk(&[&"import", &"--dir", &fix, &"--db", &db]).stdout);
    assert_eq!(count(&again, "Unverändert"), 19);
    assert_eq!(count(&again, "Neu"), 0);
}

fn normalized(db: &Path) -> (Vec<ActivityRecord>, Vec<LoadRow>) {
    let store = Store::open(db).unwrap();
    let epoch = chrono::DateTime::from_timestamp(0, 0).unwrap();
    let loads = store
        .activity_loads()
        .unwrap()
        .into_iter()
        .map(|mut l| {
            l.computed_at = epoch;
            l
        })
        .collect();
    (store.activities().unwrap(), loads)
}

#[test]
fn import_and_rebuild_produce_identical_figures() {
    let dir = work_dir("rebuild");
    let fix = write_fixtures(&dir);
    let a = dir.join("m1.db");
    let b = dir.join("m1b.db");
    assert_eq!(
        lastwerk(&[&"import", &"--dir", &fix, &"--db", &a])
            .status
            .code(),
        Some(0)
    );
    let out = lastwerk(&[&"rebuild", &"--dir", &fix, &"--db", &b]);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).starts_with("Wiederaufbau abgeschlossen"));
    let (act_a, loads_a) = normalized(&a);
    let (act_b, loads_b) = normalized(&b);
    assert_eq!(act_a.len(), 15);
    assert_eq!(act_a, act_b);
    assert_eq!(loads_a, loads_b);
    assert_eq!(act_a.iter().filter(|x| x.is_primary).count(), 14);

    // Rebuild auf der bereits gefüllten Datenbank ändert die Kennzahlen nicht.
    lastwerk(&[&"rebuild", &"--dir", &fix, &"--db", &a]);
    assert_eq!(normalized(&a), (act_b, loads_b));
}

#[test]
fn missing_directory_is_an_input_error() {
    let dir = work_dir("missing_dir");
    let db = dir.join("x.db");
    let missing = dir.join("gibt_es_nicht");
    for cmd in ["import", "rebuild", "plausibility"] {
        let out = lastwerk(&[&cmd, &"--dir", &missing, &"--db", &db]);
        assert_eq!(out.status.code(), Some(2), "{cmd}");
        assert!(text(&out.stderr).starts_with("Fehler:"), "{cmd}");
    }
    // Es entsteht keine Datenbank für einen Lauf, der nicht anfängt.
    assert!(!db.exists());
}

#[test]
fn bad_arguments_and_bad_databases_are_input_errors() {
    let dir = work_dir("bad_args");
    let fix = write_fixtures(&dir);
    assert_eq!(lastwerk(&[&"import"]).status.code(), Some(2));
    assert_eq!(lastwerk(&[&"gibtsnicht"]).status.code(), Some(2));
    // Datenbankpfad ist ein Ordner.
    let out = lastwerk(&[&"import", &"--dir", &fix, &"--db", &dir]);
    assert_eq!(out.status.code(), Some(2));
    // Datei ohne SQLite-Inhalt.
    let junk = dir.join("junk.db");
    fs::write(
        &junk,
        b"das ist keine Datenbank, aber lang genug fuer einen Kopf.....",
    )
    .unwrap();
    let out = lastwerk(&[&"import", &"--dir", &fix, &"--db", &junk]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
}

/// Ordner, in dem die Kommandozeile ohne `--db` die Datenbank anlegt, wenn der Test `HOME`,
/// `XDG_DATA_HOME` und `LOCALAPPDATA` setzt. macOS nimmt wie die App
/// `~/Library/Application Support`, die übrigen Systeme den gesetzten Datenordner.
fn default_data_dir(data_home: &Path, home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else {
        data_home.to_path_buf()
    }
}

#[test]
fn default_database_path_is_used_without_db_argument() {
    let dir = work_dir("default_path");
    let fix = write_fixtures(&dir);
    let data_home = dir.join("data");
    let home = dir.join("home");
    let out = Command::new(env!("CARGO_BIN_EXE_lastwerk"))
        .args(["import", "--dir"])
        .arg(&fix)
        .env("XDG_DATA_HOME", &data_home)
        .env("LOCALAPPDATA", &data_home)
        .env("HOME", &home)
        .env_remove("LASTWERK_DB")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let base = default_data_dir(&data_home, &home);
    assert!(base.join("Lastwerk").join("lastwerk.db").is_file());
}

/// `LASTWERK_DB` ersetzt den Standardpfad wie in der App, `--db` geht beiden vor und ein
/// leerer Wert zählt als nicht gesetzt.
#[test]
fn lastwerk_db_overrides_the_default_path_and_db_overrides_both() {
    let dir = work_dir("env_db");
    let fix = write_fixtures(&dir);
    let data_home = dir.join("data");
    let home = dir.join("home");
    let base = default_data_dir(&data_home, &home);
    let from_env = dir.join("aus-env.db");
    let run = |env_db: &Path, db_arg: Option<&Path>| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lastwerk"));
        cmd.args(["import", "--dir"])
            .arg(&fix)
            .env("XDG_DATA_HOME", &data_home)
            .env("LOCALAPPDATA", &data_home)
            .env("HOME", &home)
            .env("LASTWERK_DB", env_db);
        if let Some(db) = db_arg {
            cmd.arg("--db").arg(db);
        }
        let out = cmd.output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    };

    run(&from_env, None);
    assert!(from_env.is_file());
    assert!(!base.join("Lastwerk").exists());

    let explicit = dir.join("explizit.db");
    let ignored = dir.join("ignoriert.db");
    run(&ignored, Some(&explicit));
    assert!(explicit.is_file());
    assert!(!ignored.exists());

    run(Path::new(""), None);
    assert!(base.join("Lastwerk").join("lastwerk.db").is_file());
}

#[test]
fn plausibility_prints_only_figures_and_stores_nothing() {
    let dir = work_dir("plausibility");
    let fix = write_fixtures(&dir);
    let before: Vec<_> = fs::read_dir(&fix)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let out = lastwerk(&[&"plausibility", &"--dir", &fix]);
    let report = text(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(report.contains("Dateien: 19 gefunden, 15 Aktivitäten, 1 ohne Aktivität, 3 Fehler"));
    assert!(report.contains("Duplikatpaare 1"));
    assert!(report.contains("Größter Startabstand eines Paars: 20 s"));
    assert!(report.contains("Nicht berechenbar: 100,0 %"));
    // Keine Dateinamen und keine Pfade in der Ausgabe.
    assert!(!report.contains(".fit"));
    assert!(!report.contains("pair_edge"));
    assert!(!report.contains(fix.to_str().unwrap()));
    // Im Ordner entsteht nichts.
    let after: Vec<_> = fs::read_dir(&fix)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let mut a = before;
    let mut b = after;
    a.sort();
    b.sort();
    assert_eq!(a, b);
    assert!(!dir.join("lastwerk.db").exists());
}

#[test]
fn plausibility_reads_thresholds_from_a_database() {
    let dir = work_dir("plausibility_db");
    let fix = write_fixtures(&dir);
    let db = dir.join("user.db");
    {
        let mut store = Store::open(&db).unwrap();
        store
            .insert_threshold(&NewThreshold {
                sport: Some(lastwerk_core::Sport::Cycling),
                kind: ThresholdKind::FtpW,
                value: 250.0,
                valid_from: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                origin: ThresholdOrigin::Manual,
                note: None,
            })
            .unwrap();
        store.set_setting(SETTING_ATHLETE_SEX, "male").unwrap();
    }
    let out = lastwerk(&[&"plausibility", &"--dir", &fix, &"--db", &db]);
    let report = text(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{report}");
    assert!(!report.contains("berechenbar bei 0 von"), "{report}");
    assert!(report.contains("Jahreslast: 2026:"), "{report}");
    // Die Datenbank des Nutzers bekommt dabei keinen Cache.
    let store = Store::open(&db).unwrap();
    assert!(store.activities().unwrap().is_empty());

    // Eine Datenbank, die es nicht gibt, ist ein Eingabefehler.
    let missing = dir.join("nope.db");
    let out = lastwerk(&[&"plausibility", &"--dir", &fix, &"--db", &missing]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn version_and_help_work() {
    let out = lastwerk(&[&"--version"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).starts_with("lastwerk "));
    let out = lastwerk(&[&"--help"]);
    assert_eq!(out.status.code(), Some(0));
    let help = text(&out.stdout);
    for cmd in [
        "import",
        "rebuild",
        "plausibility",
        "thresholds",
        "athlete",
        "pmc",
    ] {
        assert!(help.contains(cmd), "{cmd}");
    }
}

// --- T05: Schwellenwerte, Athlet, Vorschläge und Verlauf -------------------------------

fn ok(out: &Output) -> String {
    let stdout = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{stdout}\n{}",
        text(&out.stderr)
    );
    stdout
}

/// Zeilen des CSV ohne Kopf als Spalten.
fn csv_rows(csv: &str) -> Vec<Vec<String>> {
    let mut lines = csv.lines();
    assert_eq!(lines.next(), Some("date,load,ctl,atl,tsb"));
    lines
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect()
}

fn col(row: &[String], i: usize) -> f64 {
    row[i].parse().unwrap()
}

/// Ablauf wie in der Abnahme: importieren, FTP setzen und neu rechnen lassen, Verlauf lesen.
///
/// Handrechnung: `power_constant_200w_3600s` (11. Mai) hat bei FTP 250 die Last 64
/// (T-TSS-1). Die CTL des 11. Mai folgt aus der des 10. Mai:
/// `CTL = CTL_Vortag + (64 - CTL_Vortag) * 0,023528`.
#[test]
fn thresholds_set_recomputes_and_pmc_follows_the_formula() {
    let dir = work_dir("flow");
    let fix = write_fixtures(&dir);
    let db = dir.join("m2.db");
    ok(&lastwerk(&[&"import", &"--dir", &fix, &"--db", &db]));

    // Ohne Schwelle: der Verlauf ist leer (Nullen), die Ausgabe bleibt gültiges CSV.
    let out = lastwerk(&[
        &"pmc",
        &"--db",
        &db,
        &"--from",
        &"2026-05-10",
        &"--to",
        &"2026-05-12",
    ]);
    let csv = ok(&out);
    assert!(
        csv_rows(&csv)
            .iter()
            .all(|r| col(r, 1) == 0.0 && col(r, 2) == 0.0)
    );
    assert!(text(&out.stderr).contains("keine berechenbaren Belastungen"));

    // FTP setzen, ohne --dir: Hinweis, nichts wird neu gerechnet, Exit 0.
    let out = lastwerk(&[
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
    ]);
    let report = ok(&out);
    assert!(
        report.contains("Schwelle gespeichert: FTP (W) = 250"),
        "{report}"
    );
    assert!(report.contains("Hinweis"), "{report}");
    assert!(report.contains("rebuild"), "{report}");

    // Mit --dir wird neu gerechnet: elf Radfahrten ab 2026-01-01.
    let out = lastwerk(&[
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
        &"--dir",
        &fix,
    ]);
    let report = ok(&out);
    assert!(report.contains("Neu berechnet: 11 von 11"), "{report}");

    let csv = ok(&lastwerk(&[
        &"pmc",
        &"--db",
        &db,
        &"--from",
        &"2026-05-10",
        &"--to",
        &"2026-05-19",
        &"--format",
        &"csv",
    ]));
    let rows = csv_rows(&csv);
    assert_eq!(rows.len(), 10);
    assert_eq!(rows[0][0], "2026-05-10");
    assert_eq!(rows[1][0], "2026-05-11");
    assert_eq!(rows[1][1], "64.000", "T-TSS-1 über die ganze Kette");
    // Am 17. Mai zählt nur die primäre Fahrt des Paars (Edge, 64).
    assert_eq!(rows[7][0], "2026-05-17");
    assert_eq!(rows[7][1], "64.000");
    let (ctl_prev, ctl) = (col(&rows[0], 2), col(&rows[1], 2));
    let expected = ctl_prev + (64.0 - ctl_prev) * 0.023528;
    assert!((ctl - expected).abs() < 2e-3, "{ctl} gegen {expected}");
    // TSB mit den Werten des Vortags: TSB(11.) = CTL(10.) - ATL(10.).
    let tsb = col(&rows[1], 4);
    assert!((tsb - (ctl_prev - col(&rows[0], 3))).abs() < 2e-3);

    // Ein späterer Beginn des Ausgabezeitraums ändert die Werte nicht (Einschwingen vorher).
    let later = ok(&lastwerk(&[
        &"pmc",
        &"--db",
        &db,
        &"--from",
        &"2026-05-12",
        &"--to",
        &"2026-05-19",
    ]));
    assert_eq!(csv_rows(&later), rows[2..].to_vec());

    // JSON: eine Zeile je Tag, gleiche Zahlen.
    let json = ok(&lastwerk(&[
        &"pmc",
        &"--db",
        &db,
        &"--from",
        &"2026-05-10",
        &"--to",
        &"2026-05-11",
        &"--format",
        &"json",
    ]));
    assert!(json.starts_with("[\n"), "{json}");
    assert!(
        json.contains(r#"{"date":"2026-05-11","load":64.000,"#),
        "{json}"
    );
    assert_eq!(json.lines().count(), 4);
}

/// Eine Schwelle mit späterem Datum ändert frühere Tageslasten nicht, auch nicht über die CLI.
#[test]
fn a_later_threshold_does_not_change_earlier_days() {
    let dir = work_dir("later");
    let fix = write_fixtures(&dir);
    let db = dir.join("m2.db");
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
        &"--from",
        &"2026-01-01",
        &"--dir",
        &fix,
    ]));
    let args = |from: &'static str, to: &'static str| {
        ok(&lastwerk(&[
            &"pmc", &"--db", &db, &"--from", &from, &"--to", &to,
        ]))
    };
    let before = csv_rows(&args("2026-05-10", "2026-05-14"));
    let day_250 = col(&csv_rows(&args("2026-05-15", "2026-05-15"))[0], 1);
    ok(&lastwerk(&[
        &"thresholds",
        &"--db",
        &db,
        &"set",
        &"--sport",
        &"cycling",
        &"--ftp",
        &"300",
        &"--from",
        &"2026-05-15",
        &"--dir",
        &fix,
    ]));
    let after = csv_rows(&args("2026-05-10", "2026-05-14"));
    assert_eq!(before, after, "Tage vor dem 15. Mai bleiben gleich");
    // Am 15. Mai gilt die neue FTP: TSS ist proportional zu 1 / FTP^2, also neue Last = Last
    // bei FTP 250 * (250 / 300)^2 = Last * 0,69444 (Handrechnung, gleiche NP).
    let day = csv_rows(&args("2026-05-15", "2026-05-15"));
    assert!(day_250 > 0.0);
    assert!((col(&day[0], 1) - day_250 * (250.0_f64 / 300.0).powi(2)).abs() < 2e-3);
    let list = ok(&lastwerk(&[&"thresholds", &"--db", &db, &"list"]));
    assert!(list.contains("ab 2026-01-01"), "{list}");
    assert!(list.contains("ab 2026-05-15"), "{list}");
}

#[test]
fn suggestions_are_shown_but_never_stored() {
    let dir = work_dir("suggest");
    let fix = write_fixtures(&dir);
    let db = dir.join("m2.db");
    ok(&lastwerk(&[&"import", &"--dir", &fix, &"--db", &db]));

    let out = ok(&lastwerk(&[&"thresholds", &"--db", &db, &"suggest"]));
    assert!(out.contains("FTP: "), "{out}");
    assert!(
        out.contains("95 Prozent der besten 20-Minuten-Leistung"),
        "{out}"
    );
    assert!(out.contains("Warnung"), "{out}");
    assert!(out.contains("--from-suggestion"), "{out}");
    assert!(out.contains("LTHR: aus dem Cache nicht ableitbar"), "{out}");
    assert!(out.contains("CSS: aus den Daten nicht ableitbar"), "{out}");
    // Nichts gespeichert.
    let list = ok(&lastwerk(&[&"thresholds", &"--db", &db, &"list"]));
    assert!(list.contains("(keine gesetzt)"), "{list}");

    // CSS aus 400 m in 300 s und 200 m in 130 s: (400 - 200) / (300 - 130) = 1,176 m/s.
    let out = ok(&lastwerk(&[
        &"thresholds",
        &"--db",
        &db,
        &"suggest",
        &"--sport",
        &"swimming",
        &"--css-t400",
        &"300",
        &"--css-t200",
        &"130",
    ]));
    assert!(out.contains("CSS: 1,176 m/s"), "{out}");
    assert!(!out.contains("FTP"), "{out}");

    // Erst die ausdrückliche Übernahme speichert, mit Herkunft "Vorschlag übernommen".
    ok(&lastwerk(&[
        &"thresholds",
        &"--db",
        &db,
        &"set",
        &"--sport",
        &"swimming",
        &"--css",
        &"1.176",
        &"--from",
        &"2026-01-01",
        &"--from-suggestion",
    ]));
    let list = ok(&lastwerk(&[&"thresholds", &"--db", &db, &"list"]));
    assert!(list.contains("Vorschlag übernommen"), "{list}");

    // Ungültige Zeiten sind Eingabefehler.
    let out = lastwerk(&[
        &"thresholds",
        &"--db",
        &db,
        &"suggest",
        &"--css-t400",
        &"100",
        &"--css-t200",
        &"130",
    ]);
    assert_eq!(out.status.code(), Some(2));
}

/// Fehlt die maximale Herzfrequenz, zeigt `thresholds list` die Schätzung aus dem Alter und
/// kennzeichnet sie. Geschlecht und Geburtsjahr kommen über `athlete set`.
#[test]
fn athlete_settings_feed_the_estimated_hr_max() {
    let dir = work_dir("athlete");
    let db = dir.join("m2.db");
    let out = ok(&lastwerk(&[
        &"athlete",
        &"--db",
        &db,
        &"set",
        &"--sex",
        &"female",
        &"--birth-year",
        &"1986",
    ]));
    assert!(out.contains("Geschlecht gespeichert: weiblich"), "{out}");
    assert!(out.contains("Geburtsjahr gespeichert: 1986"), "{out}");
    ok(&lastwerk(&[
        &"thresholds",
        &"--db",
        &db,
        &"set",
        &"--sport",
        &"all",
        &"--hr-rest",
        &"50",
        &"--from",
        &"2026-01-01",
    ]));
    let list = ok(&lastwerk(&[&"thresholds", &"--db", &db, &"list"]));
    assert!(list.contains("Ruhepuls (bpm): 50"), "{list}");
    assert!(list.contains("geschätzt"), "{list}");
    assert!(list.contains("estimated"), "{list}");
    assert!(list.contains("weiblich"), "{list}");
    let store = Store::open(&db).unwrap();
    assert_eq!(
        store.get_setting(SETTING_ATHLETE_SEX).unwrap().as_deref(),
        Some("female")
    );

    // Ein gemessener Wert ersetzt die Schätzung.
    ok(&lastwerk(&[
        &"thresholds",
        &"--db",
        &db,
        &"set",
        &"--sport",
        &"all",
        &"--hr-max",
        &"190",
        &"--from",
        &"2026-01-01",
    ]));
    let list = ok(&lastwerk(&[&"thresholds", &"--db", &db, &"list"]));
    assert!(!list.contains("geschätzt"), "{list}");
}

#[test]
fn new_commands_report_input_errors_with_exit_code_2() {
    let dir = work_dir("t05_errors");
    let db = dir.join("x.db");
    let fix = write_fixtures(&dir);
    ok(&lastwerk(&[&"import", &"--dir", &fix, &"--db", &db]));

    let code = |args: &[&dyn AsRef<std::ffi::OsStr>]| lastwerk(args).status.code();
    // Anfang nach Ende.
    assert_eq!(
        code(&[
            &"pmc",
            &"--db",
            &db,
            &"--from",
            &"2026-02-01",
            &"--to",
            &"2026-01-01"
        ]),
        Some(2)
    );
    // Falsches Datum.
    assert_eq!(
        code(&[
            &"pmc",
            &"--db",
            &db,
            &"--from",
            &"morgen",
            &"--to",
            &"2026-01-01"
        ]),
        Some(2)
    );
    // Lesende Befehle legen keine Datenbank an.
    let missing = dir.join("gibt_es_nicht.db");
    for args in [
        vec!["pmc", "--from", "2026-01-01", "--to", "2026-01-31"],
        vec!["thresholds", "list"],
        vec!["thresholds", "suggest"],
    ] {
        let mut full: Vec<&dyn AsRef<std::ffi::OsStr>> = args.iter().map(|a| a as _).collect();
        full.push(&"--db");
        full.push(&missing);
        assert_eq!(code(&full), Some(2), "{args:?}");
    }
    assert!(!missing.exists());
    // FTP für Laufen, unplausible Werte, fehlende Angaben.
    assert_eq!(
        code(&[
            &"thresholds",
            &"--db",
            &db,
            &"set",
            &"--sport",
            &"running",
            &"--ftp",
            &"250",
            &"--from",
            &"2026-01-01"
        ]),
        Some(2)
    );
    assert_eq!(
        code(&[
            &"thresholds",
            &"--db",
            &db,
            &"set",
            &"--sport",
            &"cycling",
            &"--ftp",
            &"-5",
            &"--from",
            &"2026-01-01"
        ]),
        Some(2)
    );
    assert_eq!(
        code(&[
            &"thresholds",
            &"--db",
            &db,
            &"set",
            &"--sport",
            &"cycling",
            &"--from",
            &"2026-01-01"
        ]),
        Some(2)
    );
    assert_eq!(
        code(&[
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
            &"--dir",
            &dir.join("nope")
        ]),
        Some(2)
    );
    assert_eq!(code(&[&"athlete", &"--db", &db, &"set"]), Some(2));
    assert_eq!(
        code(&[&"athlete", &"--db", &db, &"set", &"--birth-year", &"1750"]),
        Some(2)
    );
    assert_eq!(
        code(&[&"athlete", &"--db", &db, &"set", &"--sex", &"unbekannt"]),
        Some(2)
    );
    // Nach den Fehlern ist nichts gespeichert.
    let store = Store::open(&db).unwrap();
    assert!(store.list_thresholds().unwrap().is_empty());
}
