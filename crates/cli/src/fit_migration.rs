//! Übernahme des früher gewählten FIT-Ordners in den festen Stamm (SPEC 5.4, T28).
//!
//! Bis T27 wählte der Nutzer einen FIT-Ordner (Einstellung `app.fit_dir`). Seit T28 liegen alle
//! FIT-Dateien im festen Stamm `<Datenordner>/Lastwerk/fit` ([`crate::paths::default_fit_root`]).
//! Damit `source_file.rel_path` gültig bleibt und `rebuild_cache` nichts verliert, kopiert
//! [`migrate_legacy_fit_dir`] einmalig alle `*.fit` des alten Ordners (rekursiv) mit demselben
//! relativen Pfad in den Stamm.
//!
//! Regeln:
//! - Es wird nie etwas im alten Ordner geändert und nie eine Datei im Stamm überschrieben. Eine
//!   gleiche Datei zählt als übersprungen, eine abweichende am selben Pfad als Konflikt und bleibt
//!   unberührt.
//! - Kopiert wird über eine `.part`-Datei und Umbenennen, damit nie eine halbe `.fit`-Datei im
//!   Stamm liegt (der Import liest nur `*.fit`).
//! - Symbolische Verknüpfungen werden nicht verfolgt (keine Schleifen, kein Ausbruch aus dem
//!   Ordner).
//! - Nach dem Lauf wird `app.fit_dir` entfernt. Ausnahme: Schlug ein Kopieren fehl, bleibt die
//!   Einstellung, und der nächste Start versucht es erneut (gleiche Dateien werden übersprungen).
//!   Ein fehlender alter Ordner (Laufwerk nicht eingesteckt) wird gemeldet und die Einstellung
//!   entfernt: Ein Dauerzustand "Ordner nicht da" soll nicht bei jedem Start melden. Die Dateien
//!   bleiben dort, der Nutzer kann sie über "FIT-Dateien hinzufügen" nachreichen.
//!
//! Das Ergebnis trägt nur Zahlen, nie einen Pfad.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use lastwerk_store::{Store, StoreError};

/// Die frühere Einstellung mit dem Ordner des Nutzers. Wird nicht mehr geschrieben und nur noch
/// für die Übernahme gelesen.
pub const LEGACY_FIT_DIR_SETTING: &str = "app.fit_dir";

/// Zahlen der Übernahme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FitMigration {
    /// Neu in den Stamm kopierte Dateien.
    pub copied: u32,
    /// Übersprungen, weil dieselbe Datei schon am selben Pfad im Stamm liegt.
    pub identical: u32,
    /// Übersprungen, weil am selben Pfad schon eine andere Datei liegt (nichts überschrieben).
    pub conflicts: u32,
    /// Kopieren fehlgeschlagen (Berechtigung, Platz). Die Einstellung bleibt dann erhalten.
    pub failed: u32,
    /// Der alte Ordner war nicht (mehr) da.
    pub missing: bool,
}

/// Führt die Übernahme aus, falls `app.fit_dir` gesetzt ist, und liefert die Zahlen. `None`,
/// wenn es nichts zu übernehmen gab (Einstellung fehlt oder zeigt auf den Stamm selbst).
pub fn migrate_legacy_fit_dir(
    store: &mut Store,
    root: &Path,
) -> Result<Option<FitMigration>, StoreError> {
    let Some(raw) = store.get_setting(LEGACY_FIT_DIR_SETTING)? else {
        return Ok(None);
    };
    if raw.trim().is_empty() {
        // Eine leere Einstellung ist ein Rest, sie fällt weg.
        store.delete_setting(LEGACY_FIT_DIR_SETTING)?;
        return Ok(None);
    }
    let old = PathBuf::from(raw);
    if !old.is_dir() {
        store.delete_setting(LEGACY_FIT_DIR_SETTING)?;
        return Ok(Some(FitMigration {
            missing: true,
            ..FitMigration::default()
        }));
    }
    if fs::create_dir_all(root).is_err() {
        // Der Stamm lässt sich nicht anlegen: Einstellung behalten, der nächste Start versucht es.
        return Ok(Some(FitMigration {
            failed: 1,
            ..FitMigration::default()
        }));
    }
    let same = matches!(
        (fs::canonicalize(&old), fs::canonicalize(root)),
        (Ok(a), Ok(b)) if a == b
    );
    if same {
        store.delete_setting(LEGACY_FIT_DIR_SETTING)?;
        return Ok(None);
    }
    let result = copy_tree(&old, root);
    if result.failed == 0 {
        store.delete_setting(LEGACY_FIT_DIR_SETTING)?;
    }
    Ok(Some(result))
}

fn is_fit(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("fit"))
}

fn copy_tree(old: &Path, root: &Path) -> FitMigration {
    let mut out = FitMigration::default();
    // Liegt der Stamm im alten Ordner, wird er nicht erneut durchsucht.
    let root_canon = fs::canonicalize(root).ok();
    let mut stack = vec![old.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            out.failed += 1;
            continue;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                out.failed += 1;
                continue;
            };
            // `file_type` folgt keiner Verknüpfung: Verknüpfungen fallen hier heraus.
            let Ok(kind) = entry.file_type() else {
                out.failed += 1;
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                if root_canon.is_some() && fs::canonicalize(&path).ok() == root_canon {
                    continue;
                }
                stack.push(path);
            } else if kind.is_file() && is_fit(&path) {
                let Ok(rel) = path.strip_prefix(old) else {
                    continue;
                };
                copy_one(&path, &root.join(rel), &mut out);
            }
        }
    }
    out
}

fn copy_one(from: &Path, to: &Path, out: &mut FitMigration) {
    // `symlink_metadata`: ein Ziel, das eine Verknüpfung ist, zählt als Konflikt und wird nie
    // durchschrieben.
    if let Ok(meta) = fs::symlink_metadata(to) {
        if meta.is_file() && same_content(from, to) {
            out.identical += 1;
        } else {
            out.conflicts += 1;
        }
        return;
    }
    let copied = (|| -> std::io::Result<()> {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut part = to.as_os_str().to_owned();
        part.push(".part");
        let part = PathBuf::from(part);
        let result = fs::copy(from, &part).and_then(|_| fs::rename(&part, to));
        if result.is_err() {
            let _ = fs::remove_file(&part);
        }
        result
    })();
    if copied.is_ok() {
        out.copied += 1;
    } else {
        out.failed += 1;
    }
}

/// Gleiche Länge und gleicher Inhalt, in Blöcken gelesen (FIT-Dateien können groß sein).
pub fn same_content(a: &Path, b: &Path) -> bool {
    let (Ok(mut fa), Ok(mut fb)) = (File::open(a), File::open(b)) else {
        return false;
    };
    match (fa.metadata(), fb.metadata()) {
        (Ok(ma), Ok(mb)) if ma.len() == mb.len() => {}
        _ => return false,
    }
    let (mut ba, mut bb) = (vec![0u8; 64 * 1024], vec![0u8; 64 * 1024]);
    loop {
        let Ok(na) = read_full(&mut fa, &mut ba) else {
            return false;
        };
        let Ok(nb) = read_full(&mut fb, &mut bb) else {
            return false;
        };
        if na != nb || ba[..na] != bb[..nb] {
            return false;
        }
        if na == 0 {
            return true;
        }
    }
}

fn read_full(f: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        let n = f.read(&mut buf[total..])?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}
