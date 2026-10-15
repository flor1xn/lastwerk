//! lastwerk-fit: Einlesen von FIT-Dateien
//!
//! Eigener minimaler Decoder auf Basis des öffentlich dokumentierten FIT-Protokolls (Kopf,
//! Definitions- und Datennachrichten, Prüfsumme). Keine Garmin-Profildateien und keine
//! Fremdbibliothek mit solchen Dateien, siehe SPEC 14 (O2). Die benötigten Nachrichten- und
//! Feldnummern stehen als eigene Konstanten in [`profile`].
//!
//! - [`parse_activity`]: Bytes einer Datei zu einer [`RawActivity`] oder einem [`FitError`].
//! - [`scan_dir`]: ein Ordner, ein Ergebnis je Datei, Fehler brechen nichts ab.
//!
//! Die Schnittstellen stehen in docs/SPEC.md, Abschnitt 5.4. Das Crate hängt von
//! `lastwerk-core` nur wegen der gemeinsamen Typen (`Sport`, `Sample`) und der
//! Plausibilitätsregeln ab, nie umgekehrt.

mod activity;
mod decode;
mod error;
pub mod profile;
mod scan;

pub use activity::{
    Lap, MAX_FILE_BYTES, MAX_LAPS, MAX_RECORDS, RawActivity, SessionSummary, parse_activity,
};
pub use error::FitError;
pub use scan::{FitEntry, FitFile, ScanError, list_fit_files, read_fit_file, scan_dir};

/// Name des Crates, nur zum Prüfen des Gerüsts.
pub const CRATE_NAME: &str = "lastwerk-fit";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
