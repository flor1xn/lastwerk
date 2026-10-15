//! Fehler beim Einlesen einer FIT-Datei (SPEC 5.4).

use thiserror::Error;

/// Grund, warum eine einzelne Datei nicht als Aktivität gelesen werden konnte.
///
/// Der Fehlercode ([`FitError::code`]) ist ein stabiler englischer String für Speicher,
/// Fehlerliste und Oberfläche. Die Anzeigetexte dieses Typs sind nur für Protokolle gedacht,
/// Texte für Nutzer kommen aus dem Katalog (`catalog/locales`).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FitError {
    /// Die Datei hat 0 Byte.
    #[error("Datei ist leer")]
    EmptyFile,
    /// Die Datei überschreitet das Größenlimit (SPEC 10.2).
    #[error("Datei ist mit {size} Byte größer als das Limit von {limit} Byte")]
    TooLarge { size: u64, limit: u64 },
    /// Kopflänge, Kennung `.FIT` oder Kopf-Prüfsumme sind unzulässig.
    #[error("Dateikopf ist ungültig")]
    InvalidHeader,
    /// Die Prüfsumme über den Dateiinhalt stimmt nicht.
    #[error("Prüfsumme stimmt nicht")]
    CrcMismatch,
    /// Die Datei ist kürzer, als der Kopf ankündigt.
    #[error("Datei ist abgeschnitten")]
    Truncated,
    /// Die Hauptversion des Protokolls ist weder 1 noch 2.
    #[error("Protokollversion {0} wird nicht unterstützt")]
    UnsupportedVersion(u8),
    /// Gültige FIT-Datei, aber keine Aktivität (zum Beispiel Monitoring).
    #[error("Datei enthält keine Aktivität")]
    NoActivity,
    /// Der Aufbau der Nachrichten ist in sich widersprüchlich.
    #[error("Datei ist fehlerhaft aufgebaut: {0}")]
    Malformed(String),
    /// Lesefehler des Dateisystems (nur `scan_dir`).
    #[error("Lesefehler: {0}")]
    Io(String),
}

impl FitError {
    /// Fehlercode als stabiler String. Die Codes `crc_mismatch`, `truncated`, `empty_file` und
    /// `no_activity` entsprechen `Expectation.error` der Fixtures aus `lastwerk-testkit`.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EmptyFile => "empty_file",
            Self::TooLarge { .. } => "too_large",
            Self::InvalidHeader => "invalid_header",
            Self::CrcMismatch => "crc_mismatch",
            Self::Truncated => "truncated",
            Self::UnsupportedVersion(_) => "unsupported_version",
            Self::NoActivity => "no_activity",
            Self::Malformed(_) => "malformed",
            Self::Io(_) => "io",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        let all = [
            (FitError::EmptyFile, "empty_file"),
            (FitError::TooLarge { size: 2, limit: 1 }, "too_large"),
            (FitError::InvalidHeader, "invalid_header"),
            (FitError::CrcMismatch, "crc_mismatch"),
            (FitError::Truncated, "truncated"),
            (FitError::UnsupportedVersion(3), "unsupported_version"),
            (FitError::NoActivity, "no_activity"),
            (FitError::Malformed("x".into()), "malformed"),
            (FitError::Io("x".into()), "io"),
        ];
        for (e, code) in all {
            assert_eq!(e.code(), code);
        }
    }
}
