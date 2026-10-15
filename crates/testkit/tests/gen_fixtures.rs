//! Integrationstest: `gen-fixtures` schreibt in zwei Ordner byteidentische Dateien, und die
//! Dateien stimmen mit der Bibliotheks-API und dem Manifest überein (SPEC 11.3, Task T02).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use lastwerk_testkit::fixtures::{self, Manifest};

/// Ausgabeordner unter `target/`, damit nichts im Index landet (`*.fit` ist ignoriert).
fn out_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fixtures-test")
        .join(name)
}

fn run_gen(out: &Path) {
    if out.exists() {
        std::fs::remove_dir_all(out).expect("alten Ordner entfernen");
    }
    let status = Command::new(env!("CARGO_BIN_EXE_gen-fixtures"))
        .arg("--out")
        .arg(out)
        .status()
        .expect("gen-fixtures starten");
    assert!(
        status.success(),
        "gen-fixtures nach {} schlug fehl",
        out.display()
    );
}

fn read_dir_sorted(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .expect("Ordner lesen")
        .map(|e| {
            let e = e.unwrap();
            let name = e.file_name().into_string().unwrap();
            (name, std::fs::read(e.path()).unwrap())
        })
        .collect()
}

#[test]
fn two_runs_write_identical_files_matching_the_api() {
    let a = out_dir("a");
    let b = out_dir("b");
    run_gen(&a);
    run_gen(&b);

    let files_a = read_dir_sorted(&a);
    let files_b = read_dir_sorted(&b);
    assert_eq!(files_a, files_b, "beide Läufe müssen byteidentisch sein");

    let all = fixtures::all();
    assert_eq!(
        files_a.len(),
        all.len() + 1,
        "je Fixture eine Datei plus Manifest"
    );
    for f in &all {
        let on_disk = files_a
            .get(&f.file_name())
            .unwrap_or_else(|| panic!("{} fehlt", f.file_name()));
        assert_eq!(
            on_disk, &f.bytes,
            "{}: Bytes weichen von der API ab",
            f.name
        );
    }

    let manifest_text = String::from_utf8(files_a["manifest.json"].clone()).unwrap();
    assert_eq!(manifest_text, fixtures::manifest_json(&all));
    let manifest: Manifest = serde_json::from_str(&manifest_text).unwrap();
    for entry in &manifest.fixtures {
        let bytes = &files_a[&entry.file];
        assert_eq!(
            bytes.len(),
            entry.size_bytes,
            "{}: Größe im Manifest",
            entry.name
        );
    }
}

#[test]
fn help_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_gen-fixtures"))
        .arg("--help")
        .output()
        .expect("gen-fixtures starten");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("--out"));
}
