//! `scan_dir` über einen Ordner unter `target/fit-scan-test/` (SPEC 5.4, 10.2). Die Dateien
//! entstehen aus den Fixtures des Testkits, `target/` ist nicht versioniert.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::code_of;
use lastwerk_fit::{MAX_FILE_BYTES, scan_dir};
use lastwerk_testkit::fixtures;

/// Frischer, leerer Ordner je Test unter `target/fit-scan-test/`. Gelöscht wird nur der eigene
/// Unterordner, nie ein Elternordner.
fn fresh_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fit-scan-test")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn scans_all_fixtures_with_errors_per_file() {
    let root = fresh_dir("all");
    let all = fixtures::all();
    // Hälfte im Wurzelordner, der Rest in Unterordnern, eine Endung groß geschrieben
    for (i, f) in all.iter().enumerate() {
        let (dir, ext) = match i % 3 {
            0 => (root.clone(), "fit"),
            1 => (root.join("rides"), "fit"),
            _ => (root.join("rides").join("deep"), "FIT"),
        };
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{}.{ext}", f.name)), &f.bytes).unwrap();
    }
    // Dateien ohne passende Endung zählen nicht
    fs::write(root.join("notes.txt"), b"keine FIT-Datei").unwrap();
    fs::write(root.join("rides").join("readme"), b"x").unwrap();

    let results: Vec<_> = scan_dir(&root).collect();
    assert_eq!(results.len(), all.len());

    // Sortiert nach relativem Pfad, Trenner immer "/"
    let paths: Vec<String> = results
        .iter()
        .map(|r| match r {
            Ok(e) => e.rel_path.clone(),
            Err(e) => e.rel_path.clone(),
        })
        .collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted);
    assert!(paths.iter().all(|p| !p.contains('\\')));
    assert!(paths.iter().any(|p| p.starts_with("rides/deep/")));

    // Je Datei das Ergebnis aus der Erwartung
    for (i, f) in all.iter().enumerate() {
        let ext = if i % 3 == 2 { "FIT" } else { "fit" };
        let suffix = format!("{}.{ext}", f.name);
        let hit = results
            .iter()
            .find(|r| match r {
                Ok(e) => e.rel_path.ends_with(&suffix),
                Err(e) => e.rel_path.ends_with(&suffix),
            })
            .unwrap_or_else(|| panic!("{}: kein Ergebnis", f.name));
        match (&f.expect.error, hit) {
            (None, Ok(entry)) => {
                assert_eq!(entry.bytes_len, f.bytes.len() as u64, "{}", f.name);
                assert_eq!(entry.activity.samples.len(), f.expect.record_count as usize);
            }
            (Some(code), Err(err)) => assert_eq!(err.code(), code_of(*code), "{}", f.name),
            (expected, got) => panic!("{}: erwartet {expected:?}, bekommen {got:?}", f.name),
        }
    }

    // Beschädigte Dateien erscheinen als Fehler, der Rest läuft durch
    let errors = results.iter().filter(|r| r.is_err()).count();
    let expected_errors = all.iter().filter(|f| f.expect.error.is_some()).count();
    assert_eq!(errors, expected_errors);
    assert!(errors >= 4);
}

#[test]
fn order_is_deterministic() {
    let root = fresh_dir("order");
    for name in ["b", "a", "c"] {
        fs::create_dir_all(root.join(name)).unwrap();
        fs::write(root.join(name).join("x.fit"), b"").unwrap();
    }
    fs::write(root.join("a.fit"), b"").unwrap();
    let run = || -> Vec<String> {
        scan_dir(&root)
            .map(|r| r.unwrap_err().rel_path)
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), vec!["a.fit", "a/x.fit", "b/x.fit", "c/x.fit"]);
    assert_eq!(run(), run());
}

#[test]
fn empty_file_and_garbage_are_errors_not_aborts() {
    let root = fresh_dir("garbage");
    fs::write(root.join("a_empty.fit"), b"").unwrap();
    fs::write(root.join("b_garbage.fit"), [0xFFu8; 40]).unwrap();
    let good = &fixtures::by_name("no_power").unwrap().bytes;
    fs::write(root.join("c_good.fit"), good).unwrap();
    let results: Vec<_> = scan_dir(&root).collect();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].as_ref().unwrap_err().code(), "empty_file");
    assert_eq!(results[1].as_ref().unwrap_err().code(), "invalid_header");
    assert!(results[2].is_ok());
}

#[test]
fn oversized_file_is_rejected_without_reading_it() {
    let root = fresh_dir("large");
    // Datei mit gesetzter Länge, ohne sie zu befüllen (dünn besetzt auf den meisten Systemen)
    let file = fs::File::create(root.join("huge.fit")).unwrap();
    file.set_len(MAX_FILE_BYTES + 1).unwrap();
    drop(file);
    let results: Vec<_> = scan_dir(&root).collect();
    assert_eq!(results.len(), 1);
    let err = results[0].as_ref().unwrap_err();
    assert_eq!(err.code(), "too_large");
    assert_eq!(err.rel_path, "huge.fit");
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_followed() {
    let root = fresh_dir("symlink");
    let outside = fresh_dir("symlink_outside");
    fs::write(
        outside.join("secret.fit"),
        &fixtures::by_name("no_power").unwrap().bytes,
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
    std::os::unix::fs::symlink(outside.join("secret.fit"), root.join("file.fit")).unwrap();
    assert_eq!(scan_dir(&root).count(), 0);
}

#[test]
fn empty_directory_yields_nothing() {
    let root = fresh_dir("empty");
    assert_eq!(scan_dir(&root).count(), 0);
}
