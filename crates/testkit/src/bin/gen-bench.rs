//! gen-bench: schreibt große synthetische FIT-Datensätze für Laufzeitmessungen (T38).
//!
//! Aufruf (Ordner zum Beispiel aus `mktemp -d`, nie im Repository):
//!
//! ```text
//! cargo run --release -p lastwerk-testkit --bin gen-bench -- --out DIR --set long --count 10
//! cargo run --release -p lastwerk-testkit --bin gen-bench -- --out DIR --set history --years 5 --count 1500
//! ```
//!
//! Die Ausgabe ist deterministisch (fester Startwert, änderbar mit `--seed`). Messbefehle und
//! Ergebnisse stehen in docs/tasks/T38-anhang-messung.md.

use std::path::PathBuf;
use std::process::ExitCode;

use lastwerk_testkit::bench::{self, DEFAULT_SEED};
use lastwerk_testkit::generators::activity;

const USAGE: &str = "gen-bench --out <ordner> --set long|history [--count N] [--years Y] [--seed S]

long:    N Radfahrten zu je 5 Stunden mit 1 Hz (Standard N = 10)
history: N Einheiten über Y Jahre, mehrere Sportarten (Standard Y = 5, N = 1500)";

#[derive(Debug, PartialEq)]
struct Args {
    out: PathBuf,
    set: String,
    count: Option<usize>,
    years: u32,
    seed: u64,
}

fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Option<Args>, String> {
    let mut out = None;
    let mut set = None;
    let mut count = None;
    let mut years = 5;
    let mut seed = DEFAULT_SEED;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| iter.next().ok_or(format!("{name} braucht einen Wert"));
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--set" => set = Some(value("--set")?),
            "--count" => {
                count = Some(
                    value("--count")?
                        .parse()
                        .map_err(|_| "--count: keine Zahl")?,
                );
            }
            "--years" => {
                years = value("--years")?
                    .parse()
                    .map_err(|_| "--years: keine Zahl")?
            }
            "--seed" => seed = value("--seed")?.parse().map_err(|_| "--seed: keine Zahl")?,
            other => return Err(format!("unbekanntes Argument: {other}")),
        }
    }
    let out = out.ok_or("--out fehlt")?;
    let set = set.ok_or("--set fehlt")?;
    if set != "long" && set != "history" {
        return Err(format!("unbekannter Satz: {set}"));
    }
    Ok(Some(Args {
        out,
        set,
        count,
        years,
        seed,
    }))
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(Some(a)) => a,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("gen-bench: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let files = if args.set == "long" {
        bench::long_rides(args.count.unwrap_or(10), args.seed)
    } else {
        bench::history(args.years, args.count.unwrap_or(1500), args.seed)
    };
    if let Err(e) = std::fs::create_dir_all(&args.out) {
        eprintln!("gen-bench: Ordner nicht anlegbar: {}", e.kind());
        return ExitCode::FAILURE;
    }
    let mut bytes_total = 0usize;
    let mut records_total = 0u64;
    for (name, p) in &files {
        let bytes = activity(p);
        bytes_total += bytes.len();
        records_total += u64::from(p.record_count());
        if let Err(e) = std::fs::write(args.out.join(name), &bytes) {
            eprintln!("gen-bench: Schreiben fehlgeschlagen: {}", e.kind());
            return ExitCode::FAILURE;
        }
    }
    println!(
        "{} Dateien, {records_total} Messpunkte, {bytes_total} Byte nach {}",
        files.len(),
        args.out.display()
    );
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_required_and_optional_arguments() {
        let a = parse_args(args(&["--out", "x", "--set", "history", "--years", "2"]))
            .unwrap()
            .unwrap();
        assert_eq!(a.out, PathBuf::from("x"));
        assert_eq!(a.years, 2);
        assert_eq!(a.count, None);
        assert_eq!(a.seed, DEFAULT_SEED);
    }

    #[test]
    fn rejects_missing_or_unknown_arguments() {
        assert!(parse_args(args(&["--set", "long"])).is_err());
        assert!(parse_args(args(&["--out", "x", "--set", "huge"])).is_err());
        assert!(parse_args(args(&["--out", "x", "--set", "long", "--count", "z"])).is_err());
        assert_eq!(parse_args(args(&["--help"])).unwrap(), None);
    }
}
