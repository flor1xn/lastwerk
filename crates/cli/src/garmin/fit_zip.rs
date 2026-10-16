//! FIT-Datei aus dem ZIP eines Garmin-Downloads holen (SPEC 5.6, T22).
//!
//! Garmin liefert das Original einer Aktivität als ZIP. Die Bytes stammen aus dem Netz und
//! sind nicht vertrauenswürdig. Deshalb gilt:
//!
//! - **Zip-Slip ausgeschlossen durch Bauart.** Kein Name aus dem Archiv wird je zu einem Pfad.
//!   Der Name des Eintrags entscheidet nur, ob er als FIT-Datei gilt (Endung `.fit`), die
//!   Zieldatei bestimmt allein der Aufrufer aus der Aktivitätskennung (eine Zahl).
//! - **Größe.** Höchstens `max_bytes` entpackt (SPEC 5.6: 256 MiB). Die im Archiv angegebene
//!   Größe wird vorab geprüft, gezählt wird trotzdem beim Lesen, weil die Angabe lügen kann
//!   (ZIP-Bombe).
//! - **Atomar.** Geschrieben wird in `<Ziel>.part`, dann umbenannt. Ein Abbruch hinterlässt nie
//!   eine halbe `.fit`-Datei, die der Import als Aktivität läse.
//! - Kein Entschlüsseln (die Funktion `aes-crypto` des Crates ist aus) und nur Deflate und
//!   Stored. Alles andere ist ein beschädigtes Archiv.

use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

/// So viele Einträge sieht sich die Suche höchstens an (Faustregel: Garmin legt genau eine Datei
/// ins Archiv, 64 lassen reichlich Platz und begrenzen die Arbeit bei einem erfundenen
/// Archiv mit Millionen Einträgen).
pub const MAX_ENTRIES_SCANNED: usize = 64;

/// Puffer beim Kopieren (Faustregel).
const COPY_BUFFER: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractError {
    /// Kein lesbares ZIP, oder ein Eintrag ist beschädigt (Prüfsumme, Verfahren).
    NotZip,
    /// Das Archiv enthält keine Datei mit der Endung `.fit`.
    NoFit,
    /// Die FIT-Datei wäre entpackt größer als das Limit.
    TooLarge,
    /// Schreiben oder Umbenennen scheiterte (nur die Art, nie ein Pfad).
    Io(io::ErrorKind),
}

impl ExtractError {
    /// Stabiler Code für die Zählung im Bericht.
    pub fn code(self) -> &'static str {
        match self {
            Self::NotZip => "zip_invalid",
            Self::NoFit => "zip_no_fit",
            Self::TooLarge => "fit_too_large",
            Self::Io(_) => "io",
        }
    }
}

impl From<io::Error> for ExtractError {
    fn from(e: io::Error) -> Self {
        // Ein Lesefehler im Strom des Archivs (falsche Prüfsumme, abgeschnittene Daten) ist
        // `InvalidData` oder `UnexpectedEof`. Das ist ein beschädigtes Archiv, kein Fehler
        // des Dateisystems.
        match e.kind() {
            io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => Self::NotZip,
            kind => Self::Io(kind),
        }
    }
}

/// Kopiert höchstens `max_bytes` von `from` nach `to` und meldet `TooLarge`, sobald der
/// Strom mehr liefert. Gelesen wird nie mehr als `max_bytes + 1` Byte.
pub fn copy_limited(
    from: &mut dyn Read,
    to: &mut dyn Write,
    max_bytes: u64,
) -> Result<u64, ExtractError> {
    let mut buf = vec![0u8; COPY_BUFFER];
    let mut total: u64 = 0;
    loop {
        let want =
            usize::try_from((max_bytes + 1 - total).min(COPY_BUFFER as u64)).unwrap_or(COPY_BUFFER);
        let n = from.read(&mut buf[..want])?;
        if n == 0 {
            return Ok(total);
        }
        total += n as u64;
        if total > max_bytes {
            return Err(ExtractError::TooLarge);
        }
        to.write_all(&buf[..n])?;
    }
}

fn part_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// Schreibt die erste `.fit`-Datei des Archivs atomar nach `target` und gibt ihre Größe zurück.
/// `target` bestimmt der Aufrufer, nie das Archiv.
pub fn extract_fit(zip_bytes: &[u8], target: &Path, max_bytes: u64) -> Result<u64, ExtractError> {
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes)).map_err(|_| ExtractError::NotZip)?;
    let scan = archive.len().min(MAX_ENTRIES_SCANNED);
    for index in 0..scan {
        // `by_index` und nie `by_name`: Der Name ist Daten des Absenders.
        let mut entry = match archive.by_index(index) {
            Ok(e) => e,
            // Verschlüsselt oder mit unbekanntem Verfahren: nicht lesbar, nächster Eintrag.
            Err(_) => continue,
        };
        // Seit zip 9 kann der Name ungültig kodiert sein: dann kein FIT, nächster Eintrag.
        let is_fit = entry.name().is_ok_and(|name| is_fit_name(&name));
        if entry.is_dir() || !is_fit {
            continue;
        }
        if entry.size() > max_bytes {
            return Err(ExtractError::TooLarge);
        }
        let part = part_path(target);
        let result = write_part(&mut entry, &part, max_bytes);
        let written = match result {
            Ok(n) => n,
            Err(e) => {
                let _ = fs::remove_file(&part);
                return Err(e);
            }
        };
        if let Err(e) = fs::rename(&part, target) {
            let _ = fs::remove_file(&part);
            return Err(e.into());
        }
        return Ok(written);
    }
    Err(ExtractError::NoFit)
}

fn write_part(entry: &mut dyn Read, part: &Path, max_bytes: u64) -> Result<u64, ExtractError> {
    let mut file = File::create(part)?;
    let written = copy_limited(entry, &mut file, max_bytes)?;
    file.flush()?;
    // Auf die Platte, bevor umbenannt wird: Nach einem Absturz soll keine leere `.fit` stehen.
    file.sync_all()?;
    Ok(written)
}

/// Endung `.fit` ohne Beachtung der Groß- und Kleinschreibung. Mehr tut der Name nicht.
fn is_fit_name(name: &str) -> bool {
    name.len() > 4
        && name
            .get(name.len() - 4..)
            .is_some_and(|e| e.eq_ignore_ascii_case(".fit"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::CompressionMethod;
    use zip::write::{SimpleFileOptions, ZipWriter};

    fn zip_with(entries: &[(&str, &[u8], CompressionMethod)]) -> Vec<u8> {
        let mut w = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data, method) in entries {
            w.start_file(
                *name,
                SimpleFileOptions::default().compression_method(*method),
            )
            .unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn dir(name: &str) -> PathBuf {
        // Unit-Tests kennen `CARGO_TARGET_TMPDIR` nicht; der Prozessname hält Läufe auseinander.
        let d = std::env::temp_dir()
            .join(format!("lastwerk-garmin-zip-unit-{}", std::process::id()))
            .join(name);
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn name_check_is_case_insensitive_and_needs_a_stem() {
        assert!(is_fit_name("a.fit"));
        assert!(is_fit_name("A.FIT"));
        assert!(is_fit_name("dir/x.Fit"));
        assert!(!is_fit_name(".fit"));
        assert!(!is_fit_name("a.fit.txt"));
        assert!(!is_fit_name("fit"));
        assert!(!is_fit_name("ä.fi"));
    }

    #[test]
    fn copy_limited_stops_one_byte_over_the_limit() {
        let data = vec![7u8; 1000];
        let mut out = Vec::new();
        assert_eq!(copy_limited(&mut &data[..], &mut out, 1000), Ok(1000));
        assert_eq!(out.len(), 1000);
        let mut out = Vec::new();
        assert_eq!(
            copy_limited(&mut &data[..], &mut out, 999),
            Err(ExtractError::TooLarge)
        );
        // Mehr als das Limit wurde nie geschrieben.
        assert!(out.len() <= 999);
    }

    #[test]
    fn copy_limited_never_reads_more_than_the_limit_plus_one() {
        struct Endless(u64);
        impl Read for Endless {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.0 += buf.len() as u64;
                buf.fill(0);
                Ok(buf.len())
            }
        }
        let mut src = Endless(0);
        let mut sink = io::sink();
        assert_eq!(
            copy_limited(&mut src, &mut sink, 100_000),
            Err(ExtractError::TooLarge)
        );
        assert_eq!(src.0, 100_001);
    }

    #[test]
    fn extracts_deflated_and_stored_entries_atomically() {
        for method in [CompressionMethod::Deflated, CompressionMethod::Stored] {
            let d = dir(&format!("ok-{method:?}"));
            let bytes = zip_with(&[("12345_ACTIVITY.fit", b"FITDATA", method)]);
            let target = d.join("12345.fit");
            assert_eq!(extract_fit(&bytes, &target, 1024), Ok(7));
            assert_eq!(fs::read(&target).unwrap(), b"FITDATA");
            // Keine Zwischendatei bleibt liegen.
            let names: Vec<_> = fs::read_dir(&d)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            assert_eq!(names, ["12345.fit"]);
        }
    }

    #[test]
    fn a_zip_without_a_fit_entry_is_no_fit_and_writes_nothing() {
        let d = dir("nofit");
        let bytes = zip_with(&[
            ("readme.txt", b"x", CompressionMethod::Stored),
            ("fit", b"x", CompressionMethod::Stored),
        ]);
        let target = d.join("1.fit");
        assert_eq!(extract_fit(&bytes, &target, 1024), Err(ExtractError::NoFit));
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
        // Leeres Archiv.
        let empty = zip_with(&[]);
        assert_eq!(extract_fit(&empty, &target, 1024), Err(ExtractError::NoFit));
    }

    #[test]
    fn bytes_that_are_not_a_zip_are_refused() {
        let d = dir("notzip");
        let target = d.join("1.fit");
        for bytes in [&b""[..], b"not a zip at all", &[0u8; 64][..]] {
            assert_eq!(extract_fit(bytes, &target, 1024), Err(ExtractError::NotZip));
        }
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
    }

    #[test]
    fn an_oversized_entry_is_refused_by_its_declared_size_and_writes_nothing() {
        let d = dir("big");
        // 100 KiB Nullen komprimieren auf wenige Bytes: so sieht eine ZIP-Bombe aus.
        let bytes = zip_with(&[("1.fit", &vec![0u8; 100 * 1024], CompressionMethod::Deflated)]);
        assert!(bytes.len() < 1024);
        let target = d.join("1.fit");
        assert_eq!(
            extract_fit(&bytes, &target, 100 * 1024 - 1),
            Err(ExtractError::TooLarge)
        );
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
        // Genau am Limit geht durch.
        assert_eq!(extract_fit(&bytes, &target, 100 * 1024), Ok(100 * 1024));
    }

    #[test]
    fn archive_names_never_become_paths() {
        let d = dir("slip");
        let inner = d.join("fit");
        fs::create_dir_all(&inner).unwrap();
        for (i, name) in [
            "../escape.fit",
            "../../escape.fit",
            "/abs/escape.fit",
            "C:\\escape.fit",
            "a/../../escape.fit",
            "..\\escape.fit",
            "ok/../x.fit",
        ]
        .iter()
        .enumerate()
        {
            let bytes = zip_with(&[(name, b"FITDATA", CompressionMethod::Stored)]);
            let target = inner.join(format!("{}.fit", 100 + i));
            assert_eq!(extract_fit(&bytes, &target, 1024), Ok(7), "{name}");
        }
        // Alle sieben Dateien liegen im Zielordner, unter der Kennung. Nichts daneben, nichts
        // darüber.
        let mut names: Vec<_> = fs::read_dir(&inner)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "100.fit", "101.fit", "102.fit", "103.fit", "104.fit", "105.fit", "106.fit"
            ]
        );
        let beside: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(beside, ["fit"]);
    }

    #[test]
    fn a_corrupt_entry_is_a_broken_archive_and_leaves_no_part_file() {
        let d = dir("corrupt");
        let mut bytes = zip_with(&[("1.fit", b"FITDATA-FITDATA", CompressionMethod::Stored)]);
        // Ein Byte im Inhalt ändern: Die Prüfsumme (CRC32) stimmt nicht mehr.
        let pos = bytes.windows(7).position(|w| w == b"FITDATA").unwrap();
        bytes[pos] ^= 0xFF;
        let target = d.join("1.fit");
        assert_eq!(
            extract_fit(&bytes, &target, 1024),
            Err(ExtractError::NotZip)
        );
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
    }

    #[test]
    fn the_first_fit_entry_wins_and_directories_are_skipped() {
        let d = dir("first");
        let mut w = ZipWriter::new(Cursor::new(Vec::new()));
        w.add_directory("folder/", SimpleFileOptions::default())
            .unwrap();
        for (name, data) in [
            ("notes.txt", &b"no"[..]),
            ("a.fit", b"AAA"),
            ("b.fit", b"BBB"),
        ] {
            w.start_file(name, SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        let bytes = w.finish().unwrap().into_inner();
        let target = d.join("9.fit");
        assert_eq!(extract_fit(&bytes, &target, 1024), Ok(3));
        assert_eq!(fs::read(&target).unwrap(), b"AAA");
    }
}
