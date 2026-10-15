//! Eigene FIT-Dateien hinzufügen (T28): Ablage unter `fit/manual/<SHA-256-Präfix>.fit`, Größenlimit,
//! keine Pfade im Ergebnis, danach Import. Alle Dateien sind synthetisch und liegen unter
//! `target/`.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use lastwerk_app::dto::{AddFitFilesResult, FitMigrationDto};
use lastwerk_app::service;
use lastwerk_app::service::fit_files::{MANUAL_SUBDIR, add_fit_files};
use lastwerk_store::{Store, sha256_hex};
use lastwerk_testkit::fixtures;

fn work_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("app-fit-files-tests")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap()
}

/// Eine gültige synthetische Fahrt aus den Fixtures.
fn ride() -> Vec<u8> {
    fixtures::by_name("power_constant_200w_3600s")
        .expect("Fixture")
        .bytes
}

fn manual_files(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root.join(MANUAL_SUBDIR))
        .map(|d| {
            d.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Der Name entsteht allein aus dem Inhalt: 32 Hexzeichen des SHA-256 plus `.fit`, egal wie die
/// Quelldatei heißt oder wo sie liegt. Dieselbe Datei unter anderem Namen ergibt keine zweite
/// Kopie. Der Import nimmt sie danach mit dem relativen Pfad `manual/<Name>` auf.
#[test]
fn files_are_stored_under_their_hash_and_imported() {
    let dir = work_dir("hash");
    let root = dir.join("Lastwerk").join("fit");
    let src = dir.join("usb");
    fs::create_dir_all(&src).unwrap();
    let bytes = ride();
    fs::write(src.join("Meine Fahrt (1).FIT"), &bytes).unwrap();
    fs::write(src.join("zweite.fit"), &bytes).unwrap();
    let hash = sha256_hex(&bytes);
    let expected = format!("{}.fit", &hash[..32]);

    let out = add_fit_files(
        &root,
        &[src.join("Meine Fahrt (1).FIT"), src.join("zweite.fit")],
    )
    .unwrap();
    assert_eq!(
        (out.selected, out.copied, out.already_present, out.rejected),
        (2, 1, 1, 0)
    );
    assert_eq!(manual_files(&root), std::slice::from_ref(&expected));
    assert_eq!(
        fs::read(root.join("manual").join(&expected)).unwrap(),
        bytes
    );
    // Quelle bleibt unberührt.
    assert!(src.join("Meine Fahrt (1).FIT").is_file());

    let mut store = Store::open_in_memory().unwrap();
    let summary = service::import(&mut store, &root, now(), &mut |_| {}).unwrap();
    assert_eq!((summary.files_found, summary.new), (1, 1));
    let paths: Vec<String> = store
        .source_files()
        .unwrap()
        .into_iter()
        .map(|f| f.row.rel_path)
        .collect();
    assert_eq!(paths, [format!("manual/{expected}")]);

    // Ein zweiter Durchgang legt nichts Neues an.
    let again = add_fit_files(&root, &[src.join("zweite.fit")]).unwrap();
    assert_eq!((again.copied, again.already_present), (0, 1));
    assert_eq!(manual_files(&root).len(), 1);
}

/// Falsche Endung, nicht lesbare, leere und zu große Dateien werden abgelehnt und nicht
/// abgelegt; die übrigen der Auswahl kommen trotzdem durch. Das Limit ist das des Imports.
#[test]
fn wrong_extension_unreadable_empty_and_too_large_files_are_rejected() {
    let dir = work_dir("reject");
    let root = dir.join("fit");
    let src = dir.join("quelle");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("gut.fit"), ride()).unwrap();
    fs::write(src.join("notiz.txt"), b"keine FIT-Datei").unwrap();
    fs::write(src.join("leer.fit"), b"").unwrap();
    // Sparse-Datei knapp über dem Limit des Imports, ohne den Platz wirklich zu belegen.
    let big = File::create(src.join("riesig.fit")).unwrap();
    big.set_len(lastwerk_fit::MAX_FILE_BYTES + 1).unwrap();
    drop(big);

    let out = add_fit_files(
        &root,
        &[
            src.join("notiz.txt"),
            src.join("leer.fit"),
            src.join("riesig.fit"),
            src.join("gibt-es-nicht.fit"),
            src.join("gut.fit"),
        ],
    )
    .unwrap();
    assert_eq!(
        (out.selected, out.copied, out.already_present, out.rejected),
        (5, 1, 0, 4)
    );
    assert_eq!(manual_files(&root).len(), 1);
}

/// Namen aus der Eingabe (auch mit `..` oder Trennzeichen) werden nie zu einem Pfad: Es entsteht
/// nur `manual/<32 Hexzeichen>.fit`, sonst nichts im Stamm oder daneben.
#[test]
fn hostile_source_names_never_reach_the_target_path() {
    let dir = work_dir("hostile");
    let root = dir.join("fit");
    let src = dir.join("quelle");
    fs::create_dir_all(&src).unwrap();
    let odd = src.join("..;..;evil name\u{202e}.fit");
    fs::write(&odd, ride()).unwrap();
    add_fit_files(&root, &[odd]).unwrap();
    let names = manual_files(&root);
    assert_eq!(names.len(), 1);
    let stem = names[0].strip_suffix(".fit").unwrap();
    assert!(stem.len() == 32 && stem.chars().all(|c| c.is_ascii_hexdigit()));
    let entries: Vec<_> = fs::read_dir(&root).unwrap().collect();
    assert_eq!(entries.len(), 1, "nur manual/ im Stamm");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 2, "nur fit und quelle");
}

/// Ein Stamm, den es nicht gibt und nicht gibt anlegen kann (Datei statt Ordner im Weg), ist
/// `fit_root_io`, kein Absturz und kein Pfad im Fehler.
#[test]
fn a_blocked_root_is_an_error_without_a_path() {
    let dir = work_dir("blocked");
    let blocker = dir.join("fit");
    fs::write(&blocker, b"ich bin eine Datei").unwrap();
    let src = dir.join("a.fit");
    fs::write(&src, ride()).unwrap();
    let err = add_fit_files(&blocker, &[src]).unwrap_err();
    assert_eq!(err.code, "fit_root_io");
    assert!(!format!("{err:?}").contains("blocked"));
}

/// Das Ergebnis an die Oberfläche trägt nur Zahlen: kein Pfad, kein Dateiname.
#[test]
fn the_result_carries_only_counts() {
    let dir = work_dir("marker").join("PFAD-MARKER");
    let root = dir.join("fit");
    let src = dir.join("DATEI-MARKER.fit");
    fs::create_dir_all(&dir).unwrap();
    fs::write(&src, ride()).unwrap();
    let out = add_fit_files(&root, &[src]).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let import = service::import(&mut store, &root, now(), &mut |_| {}).unwrap();
    let result = AddFitFilesResult {
        cancelled: false,
        selected: out.selected,
        copied: out.copied,
        already_present: out.already_present,
        rejected: out.rejected,
        import: Some(import),
    };
    let json = serde_json::to_value(&result).unwrap().to_string();
    for forbidden in ["MARKER", ".fit", "/"] {
        assert!(!json.contains(forbidden), "{forbidden} in {json}");
    }
    let migration = serde_json::to_value(FitMigrationDto {
        copied: 1,
        identical: 2,
        conflicts: 3,
        failed: 4,
        missing: false,
    })
    .unwrap();
    assert_eq!(
        migration.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["conflicts", "copied", "failed", "identical", "missing"]
    );
}
