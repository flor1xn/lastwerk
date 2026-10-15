//! Ordner einlesen: alle FIT-Dateien unter einem Wurzelordner (SPEC 5.4, 10.2).

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::activity::{MAX_FILE_BYTES, RawActivity, parse_activity};
use crate::error::FitError;

/// Tiefste Ordnerverschachtelung unter der Wurzel. Konvention: schützt vor absurd tiefen
/// Bäumen; Symlinks werden ohnehin nicht verfolgt.
const MAX_DEPTH: usize = 32;

/// Eine gelesene Datei.
#[derive(Debug, Clone, PartialEq)]
pub struct FitEntry {
    /// Pfad relativ zur Wurzel mit `/` als Trenner, unabhängig vom Betriebssystem.
    pub rel_path: String,
    pub bytes_len: u64,
    pub activity: RawActivity,
}

/// Fehler bei einer Datei oder einem Ordner, mit dem Pfad, damit die Fehlerliste weiß, wer
/// betroffen ist.
///
/// Abweichung von SPEC 5.4, die `Result<FitEntry, FitError>` nennt: ohne Pfad ließe sich ein
/// Fehler im Import keiner Datei zuordnen (vermerkt in SPEC 5.4).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{rel_path}: {error}")]
pub struct ScanError {
    pub rel_path: String,
    #[source]
    pub error: FitError,
}

impl ScanError {
    /// Fehlercode des zugrunde liegenden [`FitError`].
    pub const fn code(&self) -> &'static str {
        self.error.code()
    }
}

/// Eintrag der Dateiliste vor dem Lesen.
enum Job {
    File { rel_path: String, path: PathBuf },
    Failed(ScanError),
}

impl Job {
    fn rel_path(&self) -> &str {
        match self {
            Job::File { rel_path, .. } => rel_path,
            Job::Failed(e) => &e.rel_path,
        }
    }
}

fn io_error(rel_path: &str, e: &std::io::Error) -> ScanError {
    ScanError {
        rel_path: rel_path.to_owned(),
        // Nur die Art des Fehlers, nie den absoluten Pfad: Meldungen landen in Protokollen.
        error: FitError::Io(e.kind().to_string()),
    }
}

fn is_fit_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("fit"))
}

/// Sammelt alle Dateien mit Endung `.fit` oder `.FIT` unter `root`. Ordner werden rekursiv
/// durchsucht, Symlinks nicht verfolgt (kein Ausbruch aus dem Ordner, keine Schleifen).
fn collect(root: &Path) -> Vec<Job> {
    let mut jobs = Vec::new();
    let mut stack: Vec<(PathBuf, String, usize)> = vec![(root.to_path_buf(), String::new(), 0)];
    while let Some((dir, prefix, depth)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                jobs.push(Job::Failed(io_error(&prefix, &e)));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    jobs.push(Job::Failed(io_error(&prefix, &e)));
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel_path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            // file_type folgt Symlinks nicht: ein Symlink ist weder Datei noch Ordner.
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    jobs.push(Job::Failed(io_error(&rel_path, &e)));
                    continue;
                }
            };
            if file_type.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((entry.path(), rel_path, depth + 1));
                } else {
                    jobs.push(Job::Failed(ScanError {
                        rel_path,
                        error: FitError::Io("directory depth limit".to_owned()),
                    }));
                }
            } else if file_type.is_file() && is_fit_file(&name) {
                jobs.push(Job::File {
                    rel_path,
                    path: entry.path(),
                });
            }
        }
    }
    // Deterministische Reihenfolge, unabhängig von der Ordnerreihenfolge des Dateisystems.
    jobs.sort_by(|a, b| a.rel_path().cmp(b.rel_path()));
    jobs
}

/// Liest eine Datei mit Größenlimit. Die Größe wird vor dem Lesen geprüft; das Limit gilt
/// zusätzlich beim Lesen, falls die Datei zwischen Prüfung und Lesen wächst.
fn read_limited(path: &Path) -> Result<Vec<u8>, FitError> {
    let file = File::open(path).map_err(|e| FitError::Io(e.kind().to_string()))?;
    let size = file
        .metadata()
        .map_err(|e| FitError::Io(e.kind().to_string()))?
        .len();
    if size > MAX_FILE_BYTES {
        return Err(FitError::TooLarge {
            size,
            limit: MAX_FILE_BYTES,
        });
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| FitError::Io(e.kind().to_string()))?;
    let read = bytes.len() as u64;
    if read > MAX_FILE_BYTES {
        return Err(FitError::TooLarge {
            size: read,
            limit: MAX_FILE_BYTES,
        });
    }
    Ok(bytes)
}

/// Eine gefundene Datei, noch nicht gelesen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FitFile {
    /// Pfad relativ zur Wurzel mit `/` als Trenner.
    pub rel_path: String,
    /// Pfad im Dateisystem, zum Lesen mit [`read_fit_file`].
    pub path: PathBuf,
}

/// Listet alle FIT-Dateien unter `root`, ohne sie zu lesen (gleiche Reihenfolge und gleiche
/// Regeln wie [`scan_dir`]). Der Speicher braucht die Liste, um unveränderte Dateien über
/// die Prüfsumme zu überspringen, ohne sie zu dekodieren (T04). Fehler beim Durchsuchen
/// stehen als `Err` an ihrer Stelle in der Liste.
pub fn list_fit_files(root: &Path) -> Vec<Result<FitFile, ScanError>> {
    collect(root)
        .into_iter()
        .map(|job| match job {
            Job::Failed(e) => Err(e),
            Job::File { rel_path, path } => Ok(FitFile { rel_path, path }),
        })
        .collect()
}

/// Liest die Bytes einer Datei mit dem Größenlimit aus [`MAX_FILE_BYTES`].
pub fn read_fit_file(path: &Path) -> Result<Vec<u8>, FitError> {
    read_limited(path)
}

/// Liest alle FIT-Dateien unter `root`, rekursiv und in sortierter Reihenfolge der relativen
/// Pfade (Vergleich der Pfadtexte, daher auch unter verschiedenen Betriebssystemen gleich).
///
/// Jede Datei liefert ein eigenes Ergebnis; ein Fehler bei einer Datei beendet die Iteration
/// nicht. Dateien werden erst beim Weiterschalten des Iterators gelesen, es liegt also immer
/// höchstens eine Datei im Speicher. Ist `root` nicht lesbar, kommt ein einzelner Fehler mit
/// leerem `rel_path`.
pub fn scan_dir(root: &Path) -> impl Iterator<Item = Result<FitEntry, ScanError>> {
    collect(root).into_iter().map(|job| match job {
        Job::Failed(e) => Err(e),
        Job::File { rel_path, path } => {
            let wrap = |error: FitError| ScanError {
                rel_path: rel_path.clone(),
                error,
            };
            let bytes = read_limited(&path).map_err(wrap)?;
            let activity = parse_activity(&bytes).map_err(|error| ScanError {
                rel_path: rel_path.clone(),
                error,
            })?;
            Ok(FitEntry {
                bytes_len: bytes.len() as u64,
                rel_path,
                activity,
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_check_ignores_case_and_needs_a_dot() {
        assert!(is_fit_file("a.fit"));
        assert!(is_fit_file("a.FIT"));
        assert!(is_fit_file("a.Fit"));
        assert!(!is_fit_file("fit"));
        assert!(!is_fit_file("a.fit.txt"));
        assert!(!is_fit_file("a.fits"));
    }

    #[test]
    fn missing_root_yields_one_io_error() {
        let results: Vec<_> = scan_dir(Path::new("/nonexistent/lastwerk-fit-test")).collect();
        assert_eq!(results.len(), 1);
        let err = results[0].as_ref().unwrap_err();
        assert_eq!(err.code(), "io");
        assert_eq!(err.rel_path, "");
    }

    #[test]
    fn listing_a_missing_root_yields_one_error_and_reads_nothing() {
        let list = list_fit_files(Path::new("/nonexistent/lastwerk-fit-test"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].as_ref().unwrap_err().code(), "io");
    }
}
