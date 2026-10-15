//! lastwerk-testkit: FIT-Encoder und Generatoren für synthetische Testdaten (nur Entwicklung)
//!
//! Das Crate liefert kleine, deterministische FIT-Dateien mit bekanntem Inhalt, damit der
//! Parser (T03) und der Import (T04) ohne echte Daten testbar sind (SPEC 11.3). Es hängt
//! bewusst nicht von `lastwerk-core` oder `lastwerk-fit` ab: `lastwerk-fit` nutzt dieses Crate
//! als Entwicklungsabhängigkeit, eine Abhängigkeit in Gegenrichtung wäre ein Zirkel.
//!
//! # Module
//!
//! - [`profile`]: die benötigten Nachrichten- und Feldnummern nach öffentlicher
//!   FIT-Protokoll- und Profilbeschreibung, keine Profildateien (SPEC 14, O2).
//! - [`encoder`]: [`FitWriter`](encoder::FitWriter) schreibt Kopf, Definitionen, Daten und
//!   Prüfsummen.
//! - [`generators`]: Aktivitäts- und Monitoring-Dateien als reine Funktionen ihrer Parameter.
//! - [`fixtures`]: die feste Sammlung mit Erwartungen und das Manifest.
//! - [`bench`]: große Datensätze für Laufzeitmessungen (T38), Binary `gen-bench`.
//!
//! # Nutzung aus T03 (`lastwerk-fit`)
//!
//! In `crates/fit/Cargo.toml` unter `[dev-dependencies]`:
//! `lastwerk-testkit = { path = "../testkit" }`. Danach alle Fixtures durchlaufen und die
//! Erwartung gegen das Parserergebnis prüfen:
//!
//! ```
//! use lastwerk_testkit::fixtures::{self, ErrorCode, FixtureKind};
//!
//! for f in fixtures::all() {
//!     match f.expect.kind {
//!         FixtureKind::Activity => {
//!             // parse_activity(&f.bytes) muss Ok liefern; dann Sportart, Dauer,
//!             // Startzeit (f.expect.start_unix_s), lokaler Versatz, Anzahl Samples
//!             // (f.expect.record_count plus Lücken) und Kanäle vergleichen.
//!             assert_eq!(f.expect.activities, 1);
//!         }
//!         FixtureKind::Monitoring => assert_eq!(f.expect.error, Some(ErrorCode::NoActivity)),
//!         FixtureKind::Empty => assert_eq!(f.expect.error, Some(ErrorCode::EmptyFile)),
//!         FixtureKind::Corrupt => assert!(f.expect.error.is_some()),
//!     }
//! }
//! ```
//!
//! Einzelne Fixtures sind über [`fixtures::by_name`] erreichbar, zum Beispiel
//! `power_constant_200w_3600s` für T-NP-1. Eigene Varianten entstehen über
//! [`generators::activity`] mit angepassten [`generators::ActivityParams`].
//!
//! # Nutzung aus T04 (`lastwerk-store`, `lastwerk-cli`)
//!
//! Der Import liest einen Ordner. Dafür schreibt das Binary die Dateien unter `target/`:
//!
//! ```text
//! cargo run -p lastwerk-testkit --bin gen-fixtures -- --out target/fixtures
//! ```
//!
//! `manifest.json` im Ordner enthält je Datei dieselbe [`fixtures::Expectation`] wie die
//! API. Der Importtest vergleicht seinen Bericht damit: je lesbarer Datei eine Aktivität,
//! das Paar `pair_edge`/`pair_watch` (Feld `pair`) ergibt genau eine primäre Aktivität
//! (SPEC 6.4), die beschädigten Dateien erscheinen als Fehler mit dem Code aus `error`, der
//! Import läuft zu Ende (SPEC 12, M1). Im Testcode geht es ohne Dateien: ein temporärer
//! Ordner unter `target/`, gefüllt aus [`fixtures::all`].
//!
//! Die Fixtures werden nicht eingecheckt. `*.fit` bleibt in `.gitignore`, der Ausgabeordner
//! liegt unter `target/`.

pub mod bench;
pub mod encoder;
pub mod fixtures;
pub mod generators;
pub mod profile;
#[cfg(test)]
pub(crate) mod readback;

/// Name des Crates, nur zum Prüfen des Gerüsts.
pub const CRATE_NAME: &str = "lastwerk-testkit";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
