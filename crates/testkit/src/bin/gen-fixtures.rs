//! gen-fixtures: schreibt alle synthetischen FIT-Fixtures samt `manifest.json` in einen Ordner.
//!
//! Aufruf: `cargo run -p lastwerk-testkit --bin gen-fixtures -- --out target/fixtures`
//! Ohne `--out` ist der Ordner `target/fixtures` relativ zum Arbeitsverzeichnis. Die Ausgabe
//! ist deterministisch: zweimaliges Erzeugen ergibt byteidentische Dateien (SPEC 11.3).
//! Die Fixtures werden nicht eingecheckt, der Ordner liegt unter `target/`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lastwerk_testkit::fixtures;

const DEFAULT_OUT: &str = "target/fixtures";

const USAGE: &str = "gen-fixtures [--out <ordner>]

Schreibt alle synthetischen FIT-Fixtures und manifest.json in den Ordner
(Standard: target/fixtures). Vorhandene Dateien gleichen Namens werden überschrieben.";

/// Wertet die Argumente aus. `Ok(None)` heißt Hilfe anzeigen.
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Option<PathBuf>, String> {
    let mut out: Option<PathBuf> = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "--out" => {
                let value = iter.next().ok_or("--out braucht einen Ordner")?;
                out = Some(PathBuf::from(value));
            }
            other => match other.strip_prefix("--out=") {
                Some(value) if !value.is_empty() => out = Some(PathBuf::from(value)),
                _ => return Err(format!("unbekanntes Argument: {other}")),
            },
        }
    }
    Ok(Some(out.unwrap_or_else(|| PathBuf::from(DEFAULT_OUT))))
}

/// Schreibt alle Fixtures und das Manifest. Liefert die Anzahl geschriebener Dateien.
pub fn write_all(out: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(out)?;
    let all = fixtures::all();
    for f in &all {
        std::fs::write(out.join(f.file_name()), &f.bytes)?;
    }
    std::fs::write(out.join("manifest.json"), fixtures::manifest_json(&all))?;
    Ok(all.len() + 1)
}

fn main() -> ExitCode {
    let out = match parse_args(std::env::args().skip(1)) {
        Ok(Some(out)) => out,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("gen-fixtures: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match write_all(&out) {
        Ok(n) => {
            println!("{n} Dateien nach {} geschrieben", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!(
                "gen-fixtures: Schreiben nach {} fehlgeschlagen: {e}",
                out.display()
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn default_out_dir_is_target_fixtures() {
        assert_eq!(
            parse_args(args(&[])).unwrap(),
            Some(PathBuf::from("target/fixtures"))
        );
    }

    #[test]
    fn out_accepts_separate_and_joined_form() {
        assert_eq!(
            parse_args(args(&["--out", "x/y"])).unwrap(),
            Some(PathBuf::from("x/y"))
        );
        assert_eq!(
            parse_args(args(&["--out=x/z"])).unwrap(),
            Some(PathBuf::from("x/z"))
        );
    }

    #[test]
    fn help_and_errors_are_recognised() {
        assert_eq!(parse_args(args(&["--help"])).unwrap(), None);
        assert!(parse_args(args(&["--out"])).is_err());
        assert!(parse_args(args(&["--nope"])).is_err());
    }
}
