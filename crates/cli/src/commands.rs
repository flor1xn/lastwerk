//! Ausführung der Befehle. Die Uhrzeit und die Ausgabe kommen von außen, damit Tests
//! deterministisch sind.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use lastwerk_store::{ImportOptions, PlanStatus, Progress, Store};

use crate::chat::ChatRequest;
use crate::error::CliError;
use crate::planning::PlanRequest;
use crate::pmc::PmcRequest;
use crate::refine::RefineRequest;
use crate::thresholds::{AthleteRequest, SetRequest, SuggestRequest};
use crate::userdata::{AvailabilityRequest, GoalRequest};
use crate::{
    chat, garmin, ics, paths, planning, plans, pmc, recovery, refine, report, thresholds, userdata,
};

/// Ein Aufruf mit allen Argumenten.
#[derive(Debug)]
pub enum Invocation {
    Import {
        dir: PathBuf,
        db: Option<PathBuf>,
    },
    ImportRecovery {
        dir: PathBuf,
        db: Option<PathBuf>,
    },
    Rebuild {
        dir: PathBuf,
        db: Option<PathBuf>,
    },
    Plausibility {
        dir: PathBuf,
        db: Option<PathBuf>,
    },
    ThresholdsSet {
        db: Option<PathBuf>,
        request: SetRequest,
    },
    ThresholdsList {
        db: Option<PathBuf>,
    },
    ThresholdsSuggest {
        db: Option<PathBuf>,
        request: SuggestRequest,
    },
    AthleteSet {
        db: Option<PathBuf>,
        request: AthleteRequest,
    },
    Pmc {
        db: Option<PathBuf>,
        request: PmcRequest,
    },
    Plan {
        db: Option<PathBuf>,
        request: PlanRequest,
        out: PathBuf,
    },
    Plans {
        db: Option<PathBuf>,
    },
    PlanStatus {
        db: Option<PathBuf>,
        id: String,
        status: PlanStatus,
    },
    Validate {
        db: Option<PathBuf>,
        plan: PathBuf,
    },
    ExportIcs {
        plan: PathBuf,
        out: PathBuf,
    },
    Refine {
        db: Option<PathBuf>,
        request: RefineRequest,
    },
    Chat {
        db: Option<PathBuf>,
        request: ChatRequest,
    },
    Backup {
        db: Option<PathBuf>,
        out: PathBuf,
    },
    Restore {
        db: Option<PathBuf>,
        input: PathBuf,
    },
    GarminStatus {
        db: Option<PathBuf>,
    },
    GarminSync {
        db: Option<PathBuf>,
        /// Ersetzt das Fenster: so viele Tage vor heute.
        days: Option<u32>,
    },
    GarminLogout,
    GoalSet {
        db: Option<PathBuf>,
        request: GoalRequest,
    },
    GoalList {
        db: Option<PathBuf>,
    },
    AvailabilitySet {
        db: Option<PathBuf>,
        request: AvailabilityRequest,
    },
    AvailabilityList {
        db: Option<PathBuf>,
    },
}

/// Führt einen Aufruf aus. `out` bekommt das Ergebnis, `err` Hinweise, die das Ergebnis nicht
/// stören dürfen (zum Beispiel bei `pmc`, dessen Ausgabe reines CSV oder JSON bleibt).
pub fn run(
    inv: Invocation,
    now: DateTime<Utc>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    match inv {
        Invocation::Import { dir, db } => import(&dir, db.as_deref(), false, now, out),
        Invocation::ImportRecovery { dir, db } => import_recovery(&dir, db.as_deref(), now, out),
        Invocation::Rebuild { dir, db } => import(&dir, db.as_deref(), true, now, out),
        Invocation::Plausibility { dir, db } => plausibility(&dir, db.as_deref(), now, out),
        Invocation::ThresholdsSet { db, request } => {
            let mut store = open_store(db.as_deref())?;
            thresholds::set(&mut store, &request, now, out)
        }
        Invocation::ThresholdsList { db } => {
            let store = open_existing_store(db.as_deref())?;
            thresholds::list(&store, now, out)
        }
        Invocation::ThresholdsSuggest { db, request } => {
            let store = open_existing_store(db.as_deref())?;
            thresholds::suggest(&store, &request, out)
        }
        Invocation::AthleteSet { db, request } => {
            let mut store = open_store(db.as_deref())?;
            thresholds::athlete_set(&mut store, &request, now, out)
        }
        Invocation::Pmc { db, request } => {
            let store = open_existing_store(db.as_deref())?;
            pmc::run(&store, &request, out, err)
        }
        Invocation::Plan {
            db,
            request,
            out: out_path,
        } => {
            // Die Plan-ID ist eine UUID v4. Zufall und Uhr gehören in die CLI, `core` bekommt
            // beides als Parameter.
            let plan_id = uuid::Uuid::new_v4().to_string();
            let mut store = open_existing_store(db.as_deref())?;
            planning::run_plan(&mut store, &request, &out_path, plan_id, now, out, err)
        }
        Invocation::Plans { db } => {
            let store = open_existing_store(db.as_deref())?;
            plans::list(&store, out)
        }
        Invocation::PlanStatus { db, id, status } => {
            let mut store = open_existing_store(db.as_deref())?;
            plans::set_status(&mut store, &id, status, out)
        }
        Invocation::Validate { db, plan } => {
            let store = open_existing_store(db.as_deref())?;
            planning::run_validate(&store, &plan, out, err)
        }
        Invocation::ExportIcs {
            plan,
            out: out_path,
        } => ics::run(&plan, &out_path, out),
        Invocation::Refine { db, request } => {
            // Die Adresse wird vor allem anderen geprüft: Ein nicht erlaubter Host endet mit 2,
            // bevor die Datenbank oder der Plan gelesen werden.
            let provider = refine::provider_for(&request)?;
            let client = provider.client();
            let mut store = open_existing_store(db.as_deref())?;
            let stdin = std::io::stdin();
            let interactive = stdin.is_terminal();
            refine::run_refine(
                &mut store,
                &request,
                &provider,
                client.as_ref(),
                uuid::Uuid::new_v4().to_string(),
                now,
                &mut stdin.lock(),
                interactive,
                out,
                err,
            )
        }
        Invocation::Chat { db, request } => {
            // Erst die Nachricht, dann wie bei `refine` die Adresse (und bei Bedarf die
            // Modellliste), dann die Datenbank.
            chat::check_message(&request)?;
            let provider = chat::resolve_provider(&request)?;
            let client = provider.client();
            let store = open_existing_store(db.as_deref())?;
            chat::run_chat(&store, &request, &provider, client.as_ref(), out, err)
        }
        Invocation::Backup { db, out: out_path } => {
            let store = open_existing_store(db.as_deref())?;
            userdata::backup(&store, &out_path, now, out)
        }
        Invocation::Restore { db, input } => {
            let mut store = open_store(db.as_deref())?;
            userdata::restore(&mut store, &input, out)
        }
        Invocation::GarminStatus { db } => {
            // Ohne Datenbankdatei gibt es keinen Abgleich; sie wird dafür nicht angelegt.
            let path = db_path(db.as_deref())?;
            let store = if path.is_file() {
                Some(Store::open(&path)?)
            } else {
                None
            };
            garmin::run_status(&garmin::default_client()?, store.as_ref(), out)
        }
        Invocation::GarminSync { db, days } => {
            let client = garmin::default_client()?;
            let mut store = open_store(db.as_deref())?;
            // Wie die App (T28): fester Stamm, Ablage in `<Stamm>/garmin`, eingelesen wird der
            // ganze Stamm. `LASTWERK_FIT` verlegt den Stamm (Tests, Vorführungen).
            let loc = garmin::fit_location(&garmin::default_fit_root()?)?;
            // Ein früher gewählter FIT-Ordner wird einmalig in den Stamm übernommen.
            if let Some(m) = crate::fit_migration::migrate_legacy_fit_dir(&mut store, &loc.root)? {
                writeln!(
                    out,
                    "Früherer FIT-Ordner übernommen: {} kopiert, {} schon vorhanden, {} Konflikte, \
                     {} fehlgeschlagen{}.",
                    m.copied,
                    m.identical,
                    m.conflicts,
                    m.failed,
                    if m.missing {
                        ", Ordner nicht gefunden"
                    } else {
                        ""
                    }
                )
                .map_err(|e| io_error(&e))?;
            }
            let (download, root) = (loc.download, loc.root);
            // "Heute" ist das Datum am Ort des Nutzers (SPEC 6.1), das Fenster zählt Kalendertage.
            let today = now.with_timezone(&chrono::Local).date_naive();
            let opts = garmin::SyncOptions {
                days,
                ..garmin::SyncOptions::new(today, now)
            };
            garmin::run_sync(
                &mut store,
                &client,
                &download,
                &root,
                &opts,
                out,
                &mut sync_progress_printer(),
            )
        }
        Invocation::GarminLogout => garmin::run_logout(&garmin::default_client()?, out),
        Invocation::GoalSet { db, request } => {
            let mut store = open_store(db.as_deref())?;
            userdata::goal_set(&mut store, &request, out)
        }
        Invocation::GoalList { db } => {
            let store = open_existing_store(db.as_deref())?;
            userdata::goal_list(&store, out)
        }
        Invocation::AvailabilitySet { db, request } => {
            let mut store = open_store(db.as_deref())?;
            userdata::availability_set(&mut store, &request, out)
        }
        Invocation::AvailabilityList { db } => {
            let store = open_existing_store(db.as_deref())?;
            userdata::availability_list(&store, out)
        }
    }
}

fn io_error(e: &std::io::Error) -> CliError {
    // Nur die Art des Fehlers, nie der Pfad: Meldungen landen in Protokollen.
    CliError::Internal(format!("Ausgabefehler: {}", e.kind()))
}

fn check_dir(dir: &Path) -> Result<(), CliError> {
    if dir.is_dir() {
        Ok(())
    } else {
        Err(CliError::Input(
            "Der angegebene Ordner existiert nicht oder ist kein Ordner.".into(),
        ))
    }
}

/// Datenbankpfad aus `--db` oder dem Standardpfad.
fn db_path(db: Option<&Path>) -> Result<PathBuf, CliError> {
    match db {
        Some(p) => Ok(p.to_path_buf()),
        None => paths::default_db_path().ok_or_else(|| {
            CliError::Input(
                "Kein Standardpfad für die Datenbank ermittelbar. Bitte --db angeben.".into(),
            )
        }),
    }
}

/// Öffnet eine vorhandene Datenbank für lesende Befehle. Eine fehlende Datei ist ein
/// Eingabefehler: Ein Tippfehler im Pfad soll keine leere Datenbank anlegen.
fn open_existing_store(db: Option<&Path>) -> Result<Store, CliError> {
    let path = db_path(db)?;
    if !path.is_file() {
        return Err(CliError::Input(
            "Die Datenbankdatei existiert nicht. Bitte --db prüfen oder zuerst `import` \
             ausführen."
                .into(),
        ));
    }
    Ok(Store::open(&path)?)
}

/// Öffnet die Datenbank, legt den Ordner dafür an. Ohne `--db` gilt der Standardpfad.
fn open_store(db: Option<&Path>) -> Result<Store, CliError> {
    let path = db_path(db)?;
    if path.is_dir() {
        return Err(CliError::Input(
            "Der Datenbankpfad ist ein Ordner, erwartet wird eine Datei.".into(),
        ));
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| {
            CliError::Input(format!(
                "Der Ordner der Datenbank lässt sich nicht anlegen: {}",
                e.kind()
            ))
        })?;
    }
    Ok(Store::open(&path)?)
}

/// Fortschrittsanzeige auf der Fehlerausgabe, nur wenn dort ein Terminal hängt.
fn progress_printer() -> impl FnMut(Progress) {
    let show = std::io::stderr().is_terminal();
    move |p: Progress| {
        if show {
            eprint!("\r{}/{}   ", p.done + 1, p.total);
            if p.done + 1 == p.total {
                eprintln!();
            }
        }
    }
}

/// Fortschritt des Abgleichs auf der Fehlerausgabe, nur wenn dort ein Terminal hängt.
fn sync_progress_printer() -> impl FnMut(garmin::SyncProgress) {
    let show = std::io::stderr().is_terminal();
    move |p: garmin::SyncProgress| {
        if show {
            match p.total {
                Some(total) => eprint!("\r{} {}/{}   ", p.phase.as_str(), p.done, total),
                None => eprint!("\r{} {}   ", p.phase.as_str(), p.done),
            }
        }
    }
}

fn import(
    dir: &Path,
    db: Option<&Path>,
    rebuild: bool,
    now: DateTime<Utc>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    check_dir(dir)?;
    let mut store = open_store(db)?;
    let opts = ImportOptions::default();
    let mut progress = progress_printer();
    let (title, result) = if rebuild {
        (
            "Wiederaufbau abgeschlossen",
            store.rebuild_cache(dir, now, &opts, &mut progress)?,
        )
    } else {
        (
            "Import abgeschlossen",
            store.import_dir(dir, now, &opts, &mut progress)?,
        )
    };
    report::write_import_report(out, title, &result).map_err(|e| io_error(&e))
}

fn import_recovery(
    dir: &Path,
    db: Option<&Path>,
    now: DateTime<Utc>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    check_dir(dir)?;
    let mut store = open_store(db)?;
    let result = recovery::import_dir(&mut store, dir, now)?;
    recovery::write_report(out, &result).map_err(|e| io_error(&e))
}

fn plausibility(
    dir: &Path,
    db: Option<&Path>,
    now: DateTime<Utc>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    check_dir(dir)?;
    // Arbeitsdatenbank nur im Speicher: im Zielordner und auf der Platte entsteht nichts.
    let mut store = Store::open_in_memory()?;
    if let Some(path) = db {
        if !path.is_file() {
            return Err(CliError::Input(
                "Die Datenbankdatei existiert nicht.".into(),
            ));
        }
        // Nur die Nutzerdaten (Schwellenwerte) werden übernommen, nie der Cache.
        let source = Store::open(path)?;
        store.import_user_data(&source.export_user_data()?)?;
    }
    store.import_dir(dir, now, &ImportOptions::default(), &mut progress_printer())?;
    let summary = store.plausibility_summary()?;
    report::write_plausibility(out, &summary).map_err(|e| io_error(&e))
}
