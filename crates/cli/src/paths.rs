//! Standardpfad der Datenbank (SPEC 6.1).
//!
//! Die Kommandozeile und die App (Tauri `local_data_dir`) nutzen dieselbe Datei:
//! `<lokaler Datenordner>/Lastwerk/lastwerk.db`. Die CLI setzt das ohne zusätzliche Crate um,
//! mit denselben Ordnern wie Tauri: unter Windows `%LOCALAPPDATA%`, unter macOS
//! `~/Library/Application Support`, sonst `$XDG_DATA_HOME` und, wenn die fehlt,
//! `~/.local/share`. Lokal statt Roaming, weil die Datei neu aufbaubar ist.
//!
//! Die Umgebungsvariable `LASTWERK_DB` (nicht leer) hat Vorrang, wie in der App (für Tests und
//! Vorführungen mit synthetischen Daten). Die Option `--db` geht beiden vor.

use std::ffi::OsString;
use std::path::PathBuf;

/// Unterordner und Dateiname unter dem Datenordner.
const APP_DIR: &str = "Lastwerk";
const DB_FILE: &str = "lastwerk.db";
/// Ordner des Abgleichs mit Garmin unter `Lastwerk` (Sperrdatei, T22) und fester FIT-Stamm
/// (T23, T28).
const GARMIN_DIR: &str = "garmin";
const FIT_DIR: &str = "fit";
const LOCKOUT_FILE: &str = "lockout.json";

/// Betriebssystem, soweit es den Datenordner bestimmt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    /// Linux und andere Unix-Systeme (XDG).
    Other,
}

impl Platform {
    /// Das System, für das das Programm gebaut wurde.
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Other
        }
    }
}

/// Lokaler Datenordner des Systems ohne den Ordner `Lastwerk` (wie Tauri `local_data_dir`).
/// Leere Werte zählen als nicht gesetzt (so verlangt es die XDG-Spezifikation für
/// `XDG_DATA_HOME`).
fn data_base(
    platform: Platform,
    local_app_data: Option<OsString>,
    xdg_data_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    let non_empty = |v: Option<OsString>| v.filter(|s| !s.is_empty()).map(PathBuf::from);
    match platform {
        Platform::Windows => non_empty(local_app_data),
        Platform::MacOs => Some(non_empty(home)?.join("Library").join("Application Support")),
        Platform::Other => match non_empty(xdg_data_home) {
            Some(x) => Some(x),
            None => Some(non_empty(home)?.join(".local").join("share")),
        },
    }
}

/// Standardpfad aus den Umgebungswerten. Die Werte sind Parameter, damit sich die Regel ohne
/// Eingriff in die echte Umgebung prüfen lässt. Leere Werte zählen als nicht gesetzt (so
/// verlangt es die XDG-Spezifikation für `XDG_DATA_HOME`). `lastwerk_db` ist ein ganzer
/// Dateipfad und überschreibt alles andere.
pub fn default_db_path_from(
    platform: Platform,
    lastwerk_db: Option<OsString>,
    local_app_data: Option<OsString>,
    xdg_data_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(file) = lastwerk_db.filter(|s| !s.is_empty()).map(PathBuf::from) {
        return Some(file);
    }
    let base = data_base(platform, local_app_data, xdg_data_home, home)?;
    Some(base.join(APP_DIR).join(DB_FILE))
}

/// Fester Stamm aller FIT-Dateien (SPEC 5.4, 6.1, T28): `<Datenordner>/Lastwerk/fit`, neben der
/// Datenbank. Der Nutzer wählt ihn nicht. Nicht der Programmordner der Installation: Den ersetzt
/// jedes Update, die Deinstallation löscht ihn, und unter `Program Files` darf die App nicht
/// schreiben. `lastwerk_fit` (`LASTWERK_FIT`, nicht leer) ist der ganze Ordnerpfad und geht vor
/// (für Tests und Vorführungen mit synthetischen Daten, wie `LASTWERK_DB`). Eigene Dateien liegen
/// direkt darin oder in `manual/`, der Abgleich mit Garmin legt seine in `garmin/` ab
/// (`garmin::fit_location`), damit ein einziger Stamm beide Quellen trägt.
pub fn default_fit_root_from(
    platform: Platform,
    lastwerk_fit: Option<OsString>,
    local_app_data: Option<OsString>,
    xdg_data_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(dir) = lastwerk_fit.filter(|s| !s.is_empty()).map(PathBuf::from) {
        return Some(dir);
    }
    let base = data_base(platform, local_app_data, xdg_data_home, home)?;
    Some(base.join(APP_DIR).join(FIT_DIR))
}

/// Datei der Sperre nach 429 oder 403 (SPEC 5.6): `<Datenordner>/Lastwerk/garmin/lockout.json`.
/// Liegt neben dem FIT-Ordner, nicht darin, damit der Import sie nie ansieht.
pub fn default_lockout_file_from(
    platform: Platform,
    local_app_data: Option<OsString>,
    xdg_data_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    let base = data_base(platform, local_app_data, xdg_data_home, home)?;
    Some(base.join(APP_DIR).join(GARMIN_DIR).join(LOCKOUT_FILE))
}

/// Standardpfad für das laufende System.
pub fn default_db_path() -> Option<PathBuf> {
    default_db_path_from(
        Platform::current(),
        std::env::var_os("LASTWERK_DB"),
        std::env::var_os("LOCALAPPDATA"),
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("HOME"),
    )
}

/// Fester FIT-Stamm für das laufende System.
pub fn default_fit_root() -> Option<PathBuf> {
    default_fit_root_from(
        Platform::current(),
        std::env::var_os("LASTWERK_FIT"),
        std::env::var_os("LOCALAPPDATA"),
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("HOME"),
    )
}

/// Standardpfad der Sperrdatei für das laufende System.
pub fn default_lockout_file() -> Option<PathBuf> {
    default_lockout_file_from(
        Platform::current(),
        std::env::var_os("LOCALAPPDATA"),
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("HOME"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> Option<OsString> {
        Some(OsString::from(s))
    }

    #[test]
    fn windows_uses_local_app_data() {
        let p = default_db_path_from(
            Platform::Windows,
            None,
            os("C:/Users/x/AppData/Local"),
            os("/xdg"),
            os("/home/x"),
        );
        assert_eq!(
            p,
            Some(PathBuf::from(
                "C:/Users/x/AppData/Local/Lastwerk/lastwerk.db"
            ))
        );
        assert_eq!(
            default_db_path_from(Platform::Windows, None, None, os("/xdg"), os("/h")),
            None
        );
    }

    /// Wie Tauri `local_data_dir` unter macOS: `~/Library/Application Support`, die
    /// XDG-Variable gilt dort nicht.
    #[test]
    fn macos_uses_application_support_under_home() {
        assert_eq!(
            default_db_path_from(Platform::MacOs, None, None, os("/xdg"), os("/Users/x")),
            Some(PathBuf::from(
                "/Users/x/Library/Application Support/Lastwerk/lastwerk.db"
            ))
        );
        assert_eq!(
            default_db_path_from(Platform::MacOs, None, None, os("/xdg"), os("")),
            None
        );
        assert_eq!(
            default_db_path_from(Platform::MacOs, None, None, None, None),
            None
        );
    }

    #[test]
    fn other_systems_prefer_xdg_data_home_then_home() {
        let other = |xdg, home| default_db_path_from(Platform::Other, None, None, xdg, home);
        assert_eq!(
            other(os("/data"), os("/home/x")),
            Some(PathBuf::from("/data/Lastwerk/lastwerk.db"))
        );
        assert_eq!(
            other(None, os("/home/x")),
            Some(PathBuf::from("/home/x/.local/share/Lastwerk/lastwerk.db"))
        );
        // Ein leerer Wert zählt als nicht gesetzt.
        assert_eq!(
            other(os(""), os("/home/x")),
            Some(PathBuf::from("/home/x/.local/share/Lastwerk/lastwerk.db"))
        );
        assert_eq!(other(None, None), None);
    }

    /// `LASTWERK_DB` ist der ganze Dateipfad und geht auf jedem System vor, ein leerer Wert
    /// gilt wie nicht gesetzt (wie in der App).
    #[test]
    fn lastwerk_db_wins_on_every_platform() {
        for platform in [Platform::Windows, Platform::MacOs, Platform::Other] {
            assert_eq!(
                default_db_path_from(
                    platform,
                    os("/tmp/demo/eigene.db"),
                    os("C:/Local"),
                    os("/xdg"),
                    os("/home/x")
                ),
                Some(PathBuf::from("/tmp/demo/eigene.db")),
                "{platform:?}"
            );
            // Auch ohne alle anderen Werte ergibt sich ein Pfad.
            assert_eq!(
                default_db_path_from(platform, os("/tmp/x.db"), None, None, None),
                Some(PathBuf::from("/tmp/x.db"))
            );
        }
        assert_eq!(
            default_db_path_from(Platform::Other, os(""), None, os("/data"), None),
            Some(PathBuf::from("/data/Lastwerk/lastwerk.db"))
        );
    }

    #[test]
    fn fit_root_sits_beside_the_database_and_lockout_file_under_garmin() {
        let fit = |p, local, xdg, home| default_fit_root_from(p, None, local, xdg, home);
        assert_eq!(
            fit(Platform::Windows, os("C:/L"), os("/xdg"), os("/h")),
            Some(PathBuf::from("C:/L/Lastwerk/fit"))
        );
        assert_eq!(
            fit(Platform::MacOs, None, os("/xdg"), os("/Users/x")),
            Some(PathBuf::from(
                "/Users/x/Library/Application Support/Lastwerk/fit"
            ))
        );
        assert_eq!(
            fit(Platform::Other, None, os("/data"), os("/home/x")),
            Some(PathBuf::from("/data/Lastwerk/fit"))
        );
        assert_eq!(
            fit(Platform::Other, None, None, os("/home/x")),
            Some(PathBuf::from("/home/x/.local/share/Lastwerk/fit"))
        );
        assert_eq!(fit(Platform::Other, None, None, None), None);
        assert_eq!(
            default_lockout_file_from(Platform::Other, None, os("/data"), None),
            Some(PathBuf::from("/data/Lastwerk/garmin/lockout.json"))
        );
    }

    /// `LASTWERK_FIT` ist der ganze Ordnerpfad und geht auf jedem System vor, ein leerer Wert
    /// gilt wie nicht gesetzt (wie `LASTWERK_DB`).
    #[test]
    fn lastwerk_fit_is_the_whole_folder_and_wins() {
        for platform in [Platform::Windows, Platform::MacOs, Platform::Other] {
            assert_eq!(
                default_fit_root_from(
                    platform,
                    os("/tmp/demo/fit"),
                    os("C:/L"),
                    os("/x"),
                    os("/h")
                ),
                Some(PathBuf::from("/tmp/demo/fit")),
                "{platform:?}"
            );
            assert_eq!(
                default_fit_root_from(platform, os("/tmp/x"), None, None, None),
                Some(PathBuf::from("/tmp/x"))
            );
        }
        assert_eq!(
            default_fit_root_from(Platform::Other, os(""), None, os("/data"), None),
            Some(PathBuf::from("/data/Lastwerk/fit"))
        );
    }
}
