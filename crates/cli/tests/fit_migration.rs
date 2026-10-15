//! Übernahme des früher gewählten FIT-Ordners in den festen Stamm (T28). Alle Dateien sind
//! synthetisch und liegen unter `target/`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use lastwerk_cli::fit_migration::{FitMigration, LEGACY_FIT_DIR_SETTING, migrate_legacy_fit_dir};
use lastwerk_store::{ImportOptions, Store};
use lastwerk_testkit::fixtures::{LOCAL_OFFSET_S, START_FIT};
use lastwerk_testkit::generators::{ActivityParams, activity};

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli-fit-migration-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap()
}

fn fit_bytes(day: u32) -> Vec<u8> {
    activity(&ActivityParams::cycling(
        START_FIT + day * 86_400,
        LOCAL_OFFSET_S,
        1800 + day * 60,
    ))
}

fn store_with_legacy(dir: &Path) -> Store {
    let mut store = Store::open_in_memory().unwrap();
    store
        .set_setting(LEGACY_FIT_DIR_SETTING, dir.to_str().unwrap())
        .unwrap();
    store
}

/// Ohne die alte Einstellung gibt es nichts zu tun, und der Stamm wird nicht angelegt.
#[test]
fn without_the_old_setting_nothing_happens() {
    let root = work_dir("none").join("fit");
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(migrate_legacy_fit_dir(&mut store, &root).unwrap(), None);
    assert!(!root.exists());
    // Ein leerer Rest der Einstellung fällt weg.
    store.set_setting(LEGACY_FIT_DIR_SETTING, "  ").unwrap();
    assert_eq!(migrate_legacy_fit_dir(&mut store, &root).unwrap(), None);
    assert_eq!(store.get_setting(LEGACY_FIT_DIR_SETTING).unwrap(), None);
}

/// Alle `*.fit` (auch in Unterordnern, auch `.FIT`) landen mit gleichem relativen Pfad im
/// Stamm, andere Dateien nicht, der alte Ordner bleibt unverändert, die Einstellung entfällt.
#[test]
fn copies_fit_files_with_their_relative_paths_and_drops_the_setting() {
    let base = work_dir("copy");
    let old = base.join("alt");
    let root = base.join("daten").join("fit");
    fs::create_dir_all(old.join("2026").join("mai")).unwrap();
    fs::write(old.join("a.fit"), fit_bytes(0)).unwrap();
    fs::write(old.join("2026").join("mai").join("b.FIT"), fit_bytes(1)).unwrap();
    fs::write(old.join("notiz.txt"), b"keine FIT-Datei").unwrap();
    let mut store = store_with_legacy(&old);

    let m = migrate_legacy_fit_dir(&mut store, &root).unwrap().unwrap();
    assert_eq!(
        m,
        FitMigration {
            copied: 2,
            ..FitMigration::default()
        }
    );
    assert_eq!(fs::read(root.join("a.fit")).unwrap(), fit_bytes(0));
    assert_eq!(
        fs::read(root.join("2026").join("mai").join("b.FIT")).unwrap(),
        fit_bytes(1)
    );
    assert!(!root.join("notiz.txt").exists());
    // Alt bleibt, wie es war, und es liegt keine halbe Datei im Stamm.
    assert!(old.join("a.fit").is_file());
    assert!(old.join("2026").join("mai").join("b.FIT").is_file());
    assert!(!root.join("a.fit.part").exists());
    assert_eq!(store.get_setting(LEGACY_FIT_DIR_SETTING).unwrap(), None);

    // Ein zweiter Lauf findet die Einstellung nicht mehr.
    assert_eq!(migrate_legacy_fit_dir(&mut store, &root).unwrap(), None);
}

/// Gleiche Dateien werden übersprungen, abweichende am selben Pfad nie überschrieben.
#[test]
fn skips_identical_and_never_overwrites() {
    let base = work_dir("skip");
    let old = base.join("alt");
    let root = base.join("fit");
    fs::create_dir_all(&old).unwrap();
    fs::create_dir_all(&root).unwrap();
    fs::write(old.join("gleich.fit"), fit_bytes(0)).unwrap();
    fs::write(root.join("gleich.fit"), fit_bytes(0)).unwrap();
    fs::write(old.join("anders.fit"), fit_bytes(1)).unwrap();
    fs::write(root.join("anders.fit"), fit_bytes(2)).unwrap();
    fs::write(old.join("neu.fit"), fit_bytes(3)).unwrap();
    // Eigene Datei im Stamm, die der alte Ordner nicht kennt, bleibt.
    fs::write(root.join("nur-im-stamm.fit"), fit_bytes(4)).unwrap();
    let mut store = store_with_legacy(&old);

    let m = migrate_legacy_fit_dir(&mut store, &root).unwrap().unwrap();
    assert_eq!(
        (m.copied, m.identical, m.conflicts, m.failed, m.missing),
        (1, 1, 1, 0, false)
    );
    assert_eq!(fs::read(root.join("anders.fit")).unwrap(), fit_bytes(2));
    assert_eq!(
        fs::read(root.join("nur-im-stamm.fit")).unwrap(),
        fit_bytes(4)
    );
    assert_eq!(fs::read(root.join("neu.fit")).unwrap(), fit_bytes(3));
}

/// Ein fehlender alter Ordner wird gemeldet und übersprungen, die Einstellung entfällt.
#[test]
fn a_missing_old_folder_is_reported_and_skipped() {
    let base = work_dir("missing");
    let gone = base.join("usb-stick").join("fit");
    let root = base.join("fit");
    let mut store = store_with_legacy(&gone);
    let m = migrate_legacy_fit_dir(&mut store, &root).unwrap().unwrap();
    assert_eq!(
        m,
        FitMigration {
            missing: true,
            ..FitMigration::default()
        }
    );
    assert_eq!(store.get_setting(LEGACY_FIT_DIR_SETTING).unwrap(), None);
    assert!(!gone.exists());
}

/// Zeigt die Einstellung auf den Stamm selbst, wird nur die Einstellung entfernt. Liegt der
/// Stamm im alten Ordner, wird er nicht auf sich selbst kopiert.
#[test]
fn the_root_itself_or_inside_the_old_folder_is_not_copied_onto_itself() {
    let base = work_dir("self");
    let root = base.join("fit");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.fit"), fit_bytes(0)).unwrap();
    let mut store = store_with_legacy(&root);
    assert_eq!(migrate_legacy_fit_dir(&mut store, &root).unwrap(), None);
    assert_eq!(store.get_setting(LEGACY_FIT_DIR_SETTING).unwrap(), None);
    assert_eq!(fs::read(root.join("a.fit")).unwrap(), fit_bytes(0));

    // Alter Ordner ist der Elternordner des Stamms.
    let mut store = store_with_legacy(&base);
    fs::write(base.join("b.fit"), fit_bytes(1)).unwrap();
    let m = migrate_legacy_fit_dir(&mut store, &root).unwrap().unwrap();
    assert_eq!((m.copied, m.identical, m.conflicts), (1, 0, 0));
    assert!(root.join("b.fit").is_file());
    assert!(!root.join("fit").exists());
}

/// Verknüpfungen werden nicht verfolgt (keine Schleife, kein Ausbruch aus dem alten Ordner).
#[cfg(unix)]
#[test]
fn symbolic_links_are_not_followed() {
    let base = work_dir("links");
    let old = base.join("alt");
    let outside = base.join("woanders");
    let root = base.join("fit");
    fs::create_dir_all(&old).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("fremd.fit"), fit_bytes(5)).unwrap();
    fs::write(old.join("a.fit"), fit_bytes(0)).unwrap();
    std::os::unix::fs::symlink(&outside, old.join("verweis")).unwrap();
    std::os::unix::fs::symlink(old.join("a.fit"), old.join("b.fit")).unwrap();
    let mut store = store_with_legacy(&old);
    let m = migrate_legacy_fit_dir(&mut store, &root).unwrap().unwrap();
    assert_eq!(m.copied, 1);
    assert!(root.join("a.fit").is_file());
    assert!(!root.join("b.fit").exists());
    assert!(!root.join("verweis").exists());
}

/// Der Zweck der Übernahme: Eine Aktivität, die aus dem alten Ordner importiert wurde, hat
/// denselben relativen Pfad im Stamm. Ein Import des Stamms ändert nichts und verliert nichts.
#[test]
fn import_of_the_root_after_migration_matches_the_old_import() {
    let base = work_dir("import");
    let old = base.join("alt");
    let root = base.join("fit");
    fs::create_dir_all(old.join("unter")).unwrap();
    fs::write(old.join("a.fit"), fit_bytes(0)).unwrap();
    fs::write(old.join("unter").join("b.fit"), fit_bytes(1)).unwrap();
    let mut store = store_with_legacy(&old);
    store
        .import_dir(&old, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();
    let mut before: Vec<String> = store
        .source_files()
        .unwrap()
        .into_iter()
        .map(|f| f.row.rel_path)
        .collect();
    before.sort();

    migrate_legacy_fit_dir(&mut store, &root).unwrap().unwrap();
    let report = store
        .import_dir(&root, now(), &ImportOptions::default(), &mut |_| {})
        .unwrap();
    assert_eq!((report.new, report.updated, report.unchanged), (0, 0, 2));
    let mut after: Vec<String> = store
        .source_files()
        .unwrap()
        .into_iter()
        .map(|f| f.row.rel_path)
        .collect();
    after.sort();
    assert_eq!(before, after);
    assert_eq!(store.activities().unwrap().len(), 2);
}
