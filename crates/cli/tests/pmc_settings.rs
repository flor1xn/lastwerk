//! `lastwerk pmc` und die Einstellungen `pmc.*` (SPEC 5.4, Nachtrag vom 14. Oktober 2026): ohne
//! Option gilt die Einstellung der Datenbank, eine Option überschreibt sie. Eingaben sind die
//! synthetischen Fixtures aus `lastwerk-testkit`, alle Dateien liegen unter `target/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use lastwerk_store::Store;
use lastwerk_testkit::fixtures;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli-pmc-settings-tests")
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

fn ok(out: &Output) -> String {
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Datenbank mit importierten Fixtures und FTP 250: Die erste Last liegt am 10. Mai 2026.
fn database(dir: &Path) -> PathBuf {
    let fix = dir.join("fixtures");
    fs::create_dir_all(&fix).unwrap();
    for f in fixtures::all() {
        fs::write(fix.join(f.file_name()), &f.bytes).unwrap();
    }
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
    db
}

/// CTL und ATL des 10. Mai 2026, dem ersten Tag mit Last (30,933), aus `lastwerk pmc`.
fn first_day(db: &Path, extra: &[&str]) -> (f64, f64) {
    let mut args: Vec<&dyn AsRef<std::ffi::OsStr>> = vec![
        &"pmc",
        &"--db",
        &db,
        &"--from",
        &"2026-05-10",
        &"--to",
        &"2026-05-10",
    ];
    for e in extra {
        args.push(e);
    }
    let csv = ok(&lastwerk(&args));
    let row: Vec<&str> = csv.lines().nth(1).unwrap().split(',').collect();
    assert_eq!(row[0], "2026-05-10");
    assert_eq!(row[1], "30.933");
    (row[2].parse().unwrap(), row[3].parse().unwrap())
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-3, "{a} statt {b}");
}

/// Handrechnung (SPEC 8.2): Der Verlauf beginnt am 10. Mai mit der Last 30,933 aus den
/// Startwerten.
/// Standard: CTL = 0 + 30,933 * (1 - exp(-1/42)) = 30,933 * 0,0235283 = 0,72782,
/// ATL = 30,933 * (1 - exp(-1/7)) = 30,933 * 0,1331221 = 4,11794.
/// Einstellungen `pmc.tau_ctl_d` = 50 und `pmc.ctl0` = 20: CTL = 20 + (30,933 - 20) *
/// (1 - exp(-1/50)) = 20 + 10,933 * 0,0198013 = 20,21649, ATL unverändert.
/// Option `--ctl0 0` überschreibt die Einstellung: CTL = 30,933 * 0,0198013 = 0,61252.
/// Option `--tau-ctl 42` mit ctl0 20 aus der Einstellung: 20 + 10,933 * 0,0235283 = 20,25724.
#[test]
fn pmc_reads_the_settings_and_the_options_override_them() {
    let dir = work_dir("settings");
    let db = database(&dir);

    let (ctl, atl) = first_day(&db, &[]);
    close(ctl, 0.727_82);
    close(atl, 4.117_94);

    {
        let mut store = Store::open(&db).unwrap();
        store.set_setting("pmc.tau_ctl_d", "50").unwrap();
        store.set_setting("pmc.ctl0", "20").unwrap();
    }
    let (ctl, atl) = first_day(&db, &[]);
    close(ctl, 20.216_49);
    close(atl, 4.117_94);

    // Eine Option überschreibt nur ihren Wert, die übrigen bleiben bei den Einstellungen.
    let (ctl, atl) = first_day(&db, &["--ctl0", "0"]);
    close(ctl, 0.612_52);
    close(atl, 4.117_94);
    let (ctl, _) = first_day(&db, &["--tau-ctl", "42"]);
    close(ctl, 20.257_24);
}
