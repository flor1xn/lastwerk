//! Eigene FIT-Dateien ablegen (T28): Edge-Geräte, USB-Stick, Dateien ohne Garmin Connect.
//!
//! Der Dateidialog läuft im Rust-Teil (`add_fit_files` in `lib.rs`), die Oberfläche bekommt nie
//! einen Pfad. Diese Funktion kopiert die gewählten Dateien in den festen Stamm, nach
//! `<Stamm>/manual/<SHA-256-Präfix>.fit`. Der Name entsteht allein aus dem Inhalt: keine
//! Pfadteile aus der Eingabe, derselbe Inhalt ergibt denselben Namen, und eine Datei, die schon
//! da ist, wird nicht doppelt abgelegt.
//!
//! Das Größenlimit ist das des Imports (`lastwerk_fit::MAX_FILE_BYTES`, SPEC 10.2), gelesen mit
//! `read_fit_file`. Ob die Datei eine gültige Aktivität ist, prüft danach der Import und meldet
//! es je Datei mit Code.

use std::fs;
use std::path::{Path, PathBuf};

use lastwerk_cli::fit_migration::same_content;
use lastwerk_fit::read_fit_file;
use lastwerk_store::sha256_hex;

use crate::error::{AppError, Result};

/// Unterordner des Stamms für von Hand hinzugefügte Dateien.
pub const MANUAL_SUBDIR: &str = "manual";
/// Länge des SHA-256-Präfixes im Dateinamen in Hexzeichen (128 Bit: Kollisionen praktisch
/// ausgeschlossen, und eine trotzdem gleiche Benennung bei anderem Inhalt wird abgelehnt).
const NAME_PREFIX_HEX: usize = 32;

/// Zahlen des Ablegens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AddOutcome {
    pub selected: u32,
    pub copied: u32,
    pub already_present: u32,
    pub rejected: u32,
}

/// Legt die gewählten Dateien im Stamm ab. Ein Schreibfehler im Stamm (Platz, Rechte) bricht ab
/// und ist `fit_root_io`; eine einzelne unbrauchbare Eingabedatei zählt als abgelehnt.
pub fn add_fit_files(root: &Path, files: &[PathBuf]) -> Result<AddOutcome> {
    let mut out = AddOutcome {
        selected: u32::try_from(files.len()).unwrap_or(u32::MAX),
        ..AddOutcome::default()
    };
    let target_dir = root.join(MANUAL_SUBDIR);
    for file in files {
        let is_fit = file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("fit"));
        let Some(bytes) = is_fit.then(|| read_fit_file(file).ok()).flatten() else {
            out.rejected += 1;
            continue;
        };
        if bytes.is_empty() {
            out.rejected += 1;
            continue;
        }
        let hash = sha256_hex(&bytes);
        let name = format!("{}.fit", &hash[..NAME_PREFIX_HEX]);
        let target = target_dir.join(&name);
        if let Ok(meta) = fs::symlink_metadata(&target) {
            if meta.is_file() && same_content(file, &target) {
                out.already_present += 1;
            } else {
                // Gleicher Name bei anderem Inhalt oder keine gewöhnliche Datei: nichts anfassen.
                out.rejected += 1;
            }
            continue;
        }
        write_new(&target_dir, &target, &bytes)?;
        out.copied += 1;
    }
    Ok(out)
}

/// Schreibt über eine `.part`-Datei und benennt um, damit nie eine halbe `.fit`-Datei im Stamm
/// liegt (der Import liest nur `*.fit`).
fn write_new(dir: &Path, target: &Path, bytes: &[u8]) -> Result<()> {
    let io = |_: std::io::Error| AppError::new("fit_root_io");
    fs::create_dir_all(dir).map_err(io)?;
    let mut part = target.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    let result = fs::write(&part, bytes).and_then(|()| fs::rename(&part, target));
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result.map_err(io)
}
