//! Laufzeit der Rechenphasen ohne Datenbank und ohne Dateisystem (T38, Anhang Messung).
//!
//! Erzeugt die Messdaten aus `lastwerk_testkit::bench` im Speicher und misst je Phase die
//! Zeit über den ganzen Satz mit `std::time::Instant`. Jede Phase läuft `--runs` Mal, die
//! Ausgabe nennt Median und Minimum in Millisekunden. Der Import als Ganzes (mit SQLite)
//! wird über die CLI mit hyperfine gemessen, siehe docs/tasks/T38-anhang-messung.md.
//!
//! ```text
//! cargo run --release -p lastwerk-store --example bench_phases -- --set long --count 10
//! cargo run --release -p lastwerk-store --example bench_phases -- --set history --years 5 --count 1500
//! cargo run --release -p lastwerk-store --example bench_phases -- --set long --count 50 --only parse --runs 21
//! ```

use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use lastwerk_core::{
    ActivityInput, BEST_POWER_DURATIONS_S, DailyLoad, LoadConfig, PmcParams, Sample, Sex,
    Thresholds, best_power, check_plausibility, compute_load, normalized_power, pmc, zone_seconds,
};
use lastwerk_fit::{RawActivity, parse_activity};
use lastwerk_store::{embedded_zone_catalog, sha256_hex};
use lastwerk_testkit::bench::{self, DEFAULT_SEED};
use lastwerk_testkit::generators::activity;

struct Args {
    set: String,
    count: Option<usize>,
    years: u32,
    runs: usize,
    /// Nur diese Phase messen, zum Beispiel `parse`.
    only: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args {
        set: "long".to_owned(),
        count: None,
        years: 5,
        runs: 7,
        only: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let v = it.next().expect("Wert zum Argument fehlt");
        match arg.as_str() {
            "--set" => a.set = v,
            "--count" => a.count = Some(v.parse().expect("--count: Zahl")),
            "--years" => a.years = v.parse().expect("--years: Zahl"),
            "--runs" => a.runs = v.parse().expect("--runs: Zahl"),
            "--only" => a.only = Some(v),
            other => panic!("unbekanntes Argument: {other}"),
        }
    }
    a
}

/// Führt `f` `runs` Mal aus und gibt Median und Minimum aus. Mit `only` läuft nur die
/// genannte Phase.
fn measure<T>(name: &str, runs: usize, only: Option<&str>, mut f: impl FnMut() -> T) {
    if only.is_some_and(|o| o != name) {
        return;
    }
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            black_box(f());
            t.elapsed()
        })
        .collect();
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!(
        "{name:<14} median {:>10.3} ms   min {:>10.3} ms",
        ms(times[times.len() / 2]),
        ms(times[0])
    );
}

/// Schwellen für alle Sportarten, damit jede Methode wirklich rechnet. Synthetische Werte.
fn thresholds() -> Thresholds {
    Thresholds {
        ftp_w: Some(250.0),
        lthr_bpm: Some(165.0),
        hr_max_bpm: Some(190.0),
        hr_rest_bpm: Some(50.0),
        run_threshold_speed_mps: Some(3.6),
        css_mps: Some(1.2),
        sex: Some(Sex::Male),
        ..Thresholds::empty(NaiveDate::from_ymd_opt(2020, 1, 1).expect("Datum"))
    }
}

fn input(raw: &RawActivity) -> ActivityInput {
    ActivityInput {
        sport: raw.sport,
        start_utc: raw.start_utc,
        local_offset_s: raw.local_offset_s,
        samples: raw.samples.clone(),
        rpe: None,
        pool_length_m: raw.session.pool_length_m,
    }
}

fn main() {
    let args = parse_args();
    let params = if args.set == "long" {
        bench::long_rides(args.count.unwrap_or(10), DEFAULT_SEED)
    } else {
        bench::history(args.years, args.count.unwrap_or(1500), DEFAULT_SEED)
    };
    let files: Vec<Vec<u8>> = params.iter().map(|(_, p)| activity(p)).collect();
    let bytes: usize = files.iter().map(Vec::len).sum();
    let raws: Vec<RawActivity> = files
        .iter()
        .map(|b| parse_activity(b).expect("synthetische Datei ist lesbar"))
        .collect();
    let samples: usize = raws.iter().map(|r| r.samples.len()).sum();
    let inputs: Vec<ActivityInput> = raws.iter().map(input).collect();
    let cfg = LoadConfig::default();
    let th = thresholds();
    let zones = embedded_zone_catalog();
    println!(
        "Satz {}: {} Dateien, {samples} Messpunkte, {bytes} Byte, {} Läufe je Phase",
        args.set,
        files.len(),
        args.runs
    );

    let runs = args.runs;
    let only = args.only.as_deref();
    measure("sha256", runs, only, || {
        files.iter().map(|b| sha256_hex(b)).collect::<Vec<_>>()
    });
    measure("parse", runs, only, || {
        files
            .iter()
            .map(|b| parse_activity(b).expect("lesbar"))
            .collect::<Vec<_>>()
    });
    measure("plausibility", runs, only, || {
        inputs
            .iter()
            .map(|a| check_plausibility(&a.samples, &cfg).expect("plausibel"))
            .collect::<Vec<_>>()
    });
    measure("np", runs, only, || {
        // Wie der Import: NP je Abschnitt aus der Reihe der gültigen Leistungswerte.
        inputs
            .iter()
            .flat_map(|a| {
                check_plausibility(&a.samples, &cfg)
                    .expect("plausibel")
                    .segments
                    .into_iter()
                    .map(|seg| {
                        let p: Vec<f32> = seg.iter().filter_map(|s: &Sample| s.power_w).collect();
                        normalized_power(&p)
                    })
            })
            .collect::<Vec<_>>()
    });
    measure("load", runs, only, || {
        inputs
            .iter()
            .map(|a| compute_load(a, &th, &cfg))
            .collect::<Vec<_>>()
    });
    measure("best_power", runs, only, || {
        inputs
            .iter()
            .map(|a| best_power(a, &cfg, &BEST_POWER_DURATIONS_S))
            .collect::<Vec<_>>()
    });
    measure("zones", runs, only, || {
        inputs
            .iter()
            .flat_map(|a| {
                zones
                    .for_sport(a.sport)
                    .map(|s| zone_seconds(a, s, &th, &cfg))
            })
            .collect::<Vec<_>>()
    });

    // PMC über die ganze Spanne der Daten: Tageslasten aus den berechneten Belastungen.
    let mut by_day: BTreeMap<NaiveDate, f32> = BTreeMap::new();
    for a in &inputs {
        if let Ok(l) = compute_load(a, &th, &cfg) {
            *by_day.entry(a.local_date()).or_insert(0.0) += l.value;
        }
    }
    let daily: Vec<DailyLoad> = by_day
        .into_iter()
        .map(|(date, load)| DailyLoad { date, load })
        .collect();
    if let (Some(first), Some(last)) = (daily.first(), daily.last()) {
        let (from, to) = (first.date, last.date);
        let p = PmcParams::default();
        println!(
            "pmc über {} Tage mit {} Lasttagen",
            (to - from).num_days() + 1,
            daily.len()
        );
        measure("pmc", runs, only, || pmc(&daily, &p, from, to));
    }
}
