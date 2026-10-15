//! Befehl `pmc`: Verlauf von Fitness, Ermüdung und Form aus den Tageslasten (SPEC 8.2).

use std::collections::HashMap;
use std::io::Write;

use chrono::{NaiveDate, TimeDelta};
use lastwerk_core::{PmcParams, pmc};
use lastwerk_store::Store;

use crate::error::CliError;
use crate::report::{PmcRow, write_pmc_csv, write_pmc_json};

/// Längster Zeitraum eines Aufrufs in Tagen, rund 100 Jahre. Schutz vor Tippfehlern im Datum,
/// die sonst Millionen Zeilen erzeugen (Konvention).
const MAX_SPAN_DAYS: i64 = 36_600;

/// Ausgabeformat von `pmc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmcFormat {
    Csv,
    Json,
}

/// Angaben der Optionen `--tau-ctl`, `--tau-atl`, `--ctl0` und `--atl0`. Eine Angabe
/// überschreibt den Wert aus den Einstellungen `pmc.*`, ohne Angabe gilt die Einstellung
/// (Standard 42 und 7 Tage, Start 0). So rechnen `pmc`, die App und der MCP-Server gleich.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PmcOverrides {
    pub tau_ctl_d: Option<f32>,
    pub tau_atl_d: Option<f32>,
    pub ctl0: Option<f32>,
    pub atl0: Option<f32>,
}

impl PmcOverrides {
    /// Die Angaben über den Werten aus den Einstellungen.
    pub fn apply(self, base: PmcParams) -> PmcParams {
        PmcParams {
            tau_ctl_d: self.tau_ctl_d.unwrap_or(base.tau_ctl_d),
            tau_atl_d: self.tau_atl_d.unwrap_or(base.tau_atl_d),
            ctl0: self.ctl0.unwrap_or(base.ctl0),
            atl0: self.atl0.unwrap_or(base.atl0),
        }
    }
}

/// Auftrag von `pmc`.
#[derive(Debug)]
pub struct PmcRequest {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub format: PmcFormat,
    pub overrides: PmcOverrides,
}

/// Parameter des Verlaufs für den Auftrag: Einstellungen aus der Datenbank
/// (`Store::pmc_params`, dieselbe Auslese wie überall), darüber die Angaben der Optionen.
pub fn params(store: &Store, req: &PmcRequest) -> Result<PmcParams, CliError> {
    Ok(req.overrides.apply(store.pmc_params()?))
}

/// Zeilen des Verlaufs von `req.from` bis `req.to` und der erste Tag mit zählender Last.
///
/// Der Verlauf beginnt am ersten Tag mit Last, falls der vor `from` liegt, damit CTL und ATL
/// bei `from` schon eingeschwungen sind (SPEC 8.2: Standardstart 0 am ersten Tag mit Daten).
/// Ausgegeben werden nur Tage ab `from`. Tage ohne Last zählen als 0.
pub fn rows(
    store: &Store,
    req: &PmcRequest,
    params: &PmcParams,
) -> Result<(Vec<PmcRow>, Option<NaiveDate>), CliError> {
    if req.from > req.to {
        return Err(CliError::Input(
            "Das Anfangsdatum liegt nach dem Enddatum.".into(),
        ));
    }
    if (req.to - req.from).num_days() > MAX_SPAN_DAYS {
        return Err(CliError::Input(format!(
            "Der Zeitraum ist länger als {MAX_SPAN_DAYS} Tage."
        )));
    }
    let first = store.first_load_date()?;
    let start = first.filter(|f| *f < req.from).unwrap_or(req.from);
    let daily = store.daily_loads(start, req.to)?;
    let points = pmc(&daily, params, start, req.to);
    let loads: HashMap<NaiveDate, f32> = daily.iter().map(|d| (d.date, d.load)).collect();
    let rows = points
        .into_iter()
        .filter(|p| p.date >= req.from)
        .map(|p| PmcRow {
            date: p.date,
            load: loads.get(&p.date).copied().unwrap_or(0.0),
            ctl: p.ctl,
            atl: p.atl,
            tsb: p.tsb,
        })
        .collect();
    Ok((rows, first))
}

/// Gibt den Verlauf aus. Hinweise stehen auf `err`, damit `out` reines CSV oder JSON bleibt.
pub fn run(
    store: &Store,
    req: &PmcRequest,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let params = params(store, req)?;
    let (rows, first) = rows(store, req, &params)?;
    let io = |e: std::io::Error| CliError::Internal(format!("Ausgabefehler: {}", e.kind()));
    match req.format {
        PmcFormat::Csv => write_pmc_csv(out, &rows).map_err(io)?,
        PmcFormat::Json => write_pmc_json(out, &rows).map_err(io)?,
    }
    match first {
        Some(first) if first <= req.to => {
            let warm_up = TimeDelta::days(params.tau_ctl_d.ceil() as i64);
            if req.from - first < warm_up {
                writeln!(
                    err,
                    "Hinweis: Der Verlauf beginnt am {first} mit CTL = {} und ATL = {}. Die \
                     ersten Wochen sind nicht belastbar (SPEC 8.2).",
                    params.ctl0, params.atl0
                )
                .map_err(io)?;
            }
        }
        _ => {
            writeln!(
                err,
                "Hinweis: Im Zeitraum gibt es keine berechenbaren Belastungen. Schwellenwerte \
                 setzen und neu berechnen (siehe `lastwerk thresholds set`)."
            )
            .map_err(io)?;
        }
    }
    Ok(())
}
