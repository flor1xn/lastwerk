//! lastwerk-app: Tauri-Hülle der Oberfläche (SPEC 5.1, 5.3, 10.4).
//!
//! Die Befehle sind eine dünne Schicht über `lastwerk-store`, `lastwerk-fit` und
//! `lastwerk-core` (siehe [`service`]). Das Frontend ruft nur die Befehle aus [`COMMAND_NAMES`]
//! auf, alles andere (Shell, HTTP, Dateisystem, Dialog) ist in den Capabilities nicht
//! freigegeben. Dateidialoge laufen im Rust-Teil: Das Frontend bekommt nie einen Pfad und kann
//! keinen Ordner lesen lassen. Alle FIT-Dateien liegen im festen Stamm `fit` neben der
//! Datenbank (T28), der Nutzer wählt keinen Ordner.
//!
//! Konventionen:
//! - Die Datenbank liegt unter `<lokaler Datenordner>/Lastwerk/lastwerk.db`, im selben Pfad wie
//!   bei der Kommandozeile (SPEC 6.1). Die Umgebungsvariable `LASTWERK_DB` überschreibt ihn
//!   (für Tests und Vorführungen mit synthetischen Daten).
//! - Der FIT-Stamm liegt unter `<lokaler Datenordner>/Lastwerk/fit`, `LASTWERK_FIT` überschreibt
//!   ihn (wie `LASTWERK_DB`).
//! - Jeder Befehl öffnet die Datenbank selbst. SQLite im WAL-Modus erlaubt dann Lesen,
//!   während ein Import schreibt.
//! - Fehler gehen als `{ code, message_key }` an das Frontend, ohne Pfade ([`error`]).

pub mod dto;
pub mod error;
pub mod garmin_login;
pub mod service;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Local, NaiveDate, Utc};
use lastwerk_store::Store;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;

use crate::dto::{
    ActivityDetail, ActivityFilter, ActivityItem, AddFitFilesResult, AthleteInput, AthleteSet,
    AvailabilityDay, BridgeStatus, Dashboard, ExportResult, FitMigrationDto, GeneratePlanInput,
    GoalInput, HealthPage, ImportSummary, Performance, PlanDetail, PlanListItem, PlanningInputs,
    Progress, RecoveryPage, RestoreResult, Settings, SettingsPatch, ThresholdInput, ThresholdList,
    ThresholdSet, UpdateInfo, WeekOverview,
};
use crate::dto::{ChatFocusDto, GarminStatus, GarminSyncSummary};
use crate::error::{AppError, Result};
use lastwerk_garmin::{Endpoints, GarminClient, KeyringStore, TokenStore};

use crate::service::coach::{self, CoachState};
use crate::service::coach_setup::{self, CoachSetupState, CoachSetupStatus, JobKind};
use crate::service::quick::{self, QuickPlanInput};

/// Befehle, die das Frontend aufrufen darf. Muss zu `generate_handler!` unten, zu `build.rs`
/// (Berechtigungen) und zu `capabilities/default.json` passen, ein Test prüft das.
pub const COMMAND_NAMES: [&str; 46] = [
    "get_settings",
    "set_settings",
    "import_folder",
    "add_fit_files",
    "get_week_overview",
    "get_dashboard",
    "get_progress",
    "get_performance",
    "get_recovery",
    "get_health",
    "delete_sensitive_data",
    "list_activities",
    "get_activity",
    "list_thresholds",
    "set_threshold",
    "set_athlete",
    "list_plans",
    "get_plan",
    "generate_plan",
    "set_plan_status",
    "get_planning_inputs",
    "set_goal",
    "set_availability",
    "export_plan_ics",
    "export_plan_json",
    "export_backup",
    "restore_backup",
    "check_update",
    "install_update",
    "get_bridge_status",
    // Garmin (T23)
    "garmin_status",
    "garmin_login",
    "garmin_sync",
    "garmin_logout",
    // Planer mit drei Fragen und Gespräch mit dem Coach (T26 Teil B).
    "quick_plan",
    "coach_status",
    "coach_set_model",
    "coach_preview",
    "coach_chat",
    "coach_apply",
    "coach_reset",
    "coach_cancel",
    // Einrichtung des lokalen Coachs (T29).
    "coach_setup_status",
    "coach_install_ollama",
    "coach_pull_model",
    "coach_pull_cancel",
];

/// Name des Ereignisses mit dem Importfortschritt.
pub const EVENT_IMPORT_PROGRESS: &str = "import-progress";
/// Name des Ereignisses mit dem Fortschritt des Abgleichs mit Garmin (T23).
pub const EVENT_GARMIN_SYNC_PROGRESS: &str = "garmin-sync-progress";
/// Fortschritt der Einrichtung von Ollama durch das gebündelte Skript (T29).
pub const EVENT_COACH_SETUP_PROGRESS: &str = "coach-setup-progress";
/// Fortschritt beim Laden eines Modells (T29).
pub const EVENT_COACH_PULL_PROGRESS: &str = "coach-pull-progress";

/// Unterordner und Dateiname der Datenbank unter dem lokalen Datenordner (SPEC 6.1).
const DB_DIR: &str = "Lastwerk";
const DB_FILE: &str = "lastwerk.db";

/// Zustand der App: die Pfade der Datenbank und des festen FIT-Stamms. Die Verbindung öffnet
/// jeder Befehl neu.
pub struct AppState {
    db_path: PathBuf,
    /// Fester Stamm aller FIT-Dateien (T28).
    fit_root: PathBuf,
    in_sync_folder: bool,
    /// Ergebnis der Übernahme des früheren FIT-Ordners beim Start dieses Prozesses. Bleibt für
    /// die Laufzeit stehen (die Oberfläche blendet es aus), nach einem Neustart ist es leer.
    fit_migration: Option<FitMigrationDto>,
}

impl AppState {
    pub fn new(db_path: PathBuf, fit_root: PathBuf) -> Self {
        let in_sync_folder = service::in_sync_folder(&db_path);
        Self {
            db_path,
            fit_root,
            in_sync_folder,
            fit_migration: None,
        }
    }

    pub fn with_migration(mut self, migration: Option<FitMigrationDto>) -> Self {
        self.fit_migration = migration;
        self
    }

    /// Setzt die Felder, die dem Prozess gehören und nicht der Datenbank (Version, Stamm,
    /// Übernahme).
    fn decorate(&self, app: &AppHandle, settings: &mut Settings) {
        // Der Release-Ablauf setzt die Version aus dem Tag nur in `tauri.conf.json`.
        settings.version = app.package_info().version.to_string();
        settings.fit_root = self.fit_root.to_string_lossy().into_owned();
        settings.fit_migration = self.fit_migration;
    }
}

/// Heutiger Kalendertag des Nutzers. Aktivitäten und Pläne tragen lokale Tage (SPEC 6.1), der
/// UTC-Tag wiche abends westlich und morgens östlich von Greenwich um einen Tag ab
/// (Integrationsprüfung, 14. Oktober 2026). Zeitstempel bleiben UTC.
fn local_today() -> NaiveDate {
    Local::now().date_naive()
}

/// Pfad der Datenbank: `LASTWERK_DB`, sonst `<lokaler Datenordner>/Lastwerk/lastwerk.db`.
fn database_path(app: &AppHandle) -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("LASTWERK_DB").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let base = app
        .path()
        .local_data_dir()
        .map_err(|_| AppError::new("database_io"))?;
    Ok(base.join(DB_DIR).join(DB_FILE))
}

/// Fester FIT-Stamm: `LASTWERK_FIT`, sonst `<lokaler Datenordner>/Lastwerk/fit` (T28). Derselbe
/// Ort wie in der Kommandozeile (`lastwerk_cli::paths::default_fit_root`). Nicht der
/// Programmordner der Installation: Den ersetzt jedes Update, die Deinstallation löscht ihn, und
/// unter `Program Files` darf die App nicht schreiben.
fn fit_root_path(app: &AppHandle) -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("LASTWERK_FIT").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let base = app
        .path()
        .local_data_dir()
        .map_err(|_| AppError::new("database_io"))?;
    Ok(base.join(DB_DIR).join(FIT_DIR))
}

/// Öffnet die Datenbank und legt den Ordner an, falls er fehlt.
fn open_store(path: &Path) -> Result<Store> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| AppError::new("database_io"))?;
    }
    Ok(Store::open(path)?)
}

/// Führt `f` mit einer frisch geöffneten Datenbank auf einem Arbeitsthread aus.
async fn with_store<T, F>(state: &AppState, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Store) -> Result<T> + Send + 'static,
{
    let path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut store = open_store(&path)?;
        f(&mut store)
    })
    .await
    .map_err(|_| AppError::new("internal"))?
}

#[tauri::command(rename_all = "snake_case")]
async fn get_settings(app: AppHandle, state: State<'_, AppState>) -> Result<Settings> {
    let sync = state.in_sync_folder;
    let mut settings = with_store(&state, move |store| service::get_settings(store, sync)).await?;
    state.decorate(&app, &mut settings);
    Ok(settings)
}

#[tauri::command(rename_all = "snake_case")]
async fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<Settings> {
    let sync = state.in_sync_folder;
    let mut settings = with_store(&state, move |store| {
        service::set_settings(store, &patch, sync)
    })
    .await?;
    state.decorate(&app, &mut settings);
    Ok(settings)
}

/// Austauschordner des Zusatzprogramms. Standard `local/exchange` (SPEC 5.4), mit
/// `LASTWERK_EXCHANGE` überschreibbar, weil eine installierte App kein Projektverzeichnis hat.
fn exchange_dir() -> PathBuf {
    std::env::var_os("LASTWERK_EXCHANGE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("local").join("exchange"))
}

#[tauri::command(rename_all = "snake_case")]
async fn get_bridge_status() -> Result<BridgeStatus> {
    tauri::async_runtime::spawn_blocking(|| service::bridge_status(&exchange_dir()))
        .await
        .map_err(|_| AppError::new("internal"))
}

/// Liest den festen FIT-Stamm ein und meldet den Fortschritt als Ereignis. Höchstens etwa 200
/// Ereignisse je Import, damit die Brücke zur Oberfläche nicht bei tausenden Dateien belastet
/// wird (Konvention).
fn import_with_progress(app: &AppHandle, store: &mut Store, root: &Path) -> Result<ImportSummary> {
    let mut last_emitted: Option<u32> = None;
    service::import(store, root, Utc::now(), &mut |p| {
        let step = (p.total / 200).max(1);
        if p.done == p.total || p.done.is_multiple_of(step) {
            if last_emitted == Some(p.done) {
                return;
            }
            last_emitted = Some(p.done);
            // Ein verlorenes Ereignis ist unkritisch, der Import läuft weiter.
            let _ = app.emit(EVENT_IMPORT_PROGRESS, p);
        }
    })
}

#[tauri::command(rename_all = "snake_case")]
async fn import_folder(app: AppHandle, state: State<'_, AppState>) -> Result<ImportSummary> {
    let root = state.fit_root.clone();
    with_store(&state, move |store| {
        import_with_progress(&app, store, &root)
    })
    .await
}

/// Fügt eigene FIT-Dateien hinzu (Edge, USB-Stick, T28): Dateidialog im Rust-Teil mit
/// Mehrfachauswahl, nur `.fit`. Die Dateien werden nach `fit/manual/<SHA-256-Präfix>.fit` kopiert
/// und danach eingelesen. Die Oberfläche bekommt nur Zahlen, nie einen Pfad.
#[tauri::command(rename_all = "snake_case")]
async fn add_fit_files(app: AppHandle, state: State<'_, AppState>) -> Result<AddFitFilesResult> {
    let picker = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let Some(files) = picker
            .dialog()
            .file()
            .add_filter("FIT", &["fit"])
            .blocking_pick_files()
        else {
            return Ok(None);
        };
        let mut paths = Vec::with_capacity(files.len());
        for file in files {
            paths.push(
                file.into_path()
                    .map_err(|_| AppError::new("dialog_failed"))?,
            );
        }
        Ok::<_, AppError>(Some(paths))
    })
    .await
    .map_err(|_| AppError::new("internal"))??;
    let Some(paths) = picked else {
        return Ok(AddFitFilesResult {
            cancelled: true,
            selected: 0,
            copied: 0,
            already_present: 0,
            rejected: 0,
            import: None,
        });
    };
    let root = state.fit_root.clone();
    with_store(&state, move |store| {
        let added = service::fit_files::add_fit_files(&root, &paths)?;
        let import = if added.copied + added.already_present > 0 {
            Some(import_with_progress(&app, store, &root)?)
        } else {
            None
        };
        Ok(AddFitFilesResult {
            cancelled: false,
            selected: added.selected,
            copied: added.copied,
            already_present: added.already_present,
            rejected: added.rejected,
            import,
        })
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn get_week_overview(
    state: State<'_, AppState>,
    week_start: String,
    pmc_weeks: u32,
) -> Result<WeekOverview> {
    with_store(&state, move |store| {
        service::week_overview(store, &week_start, pmc_weeks, local_today())
    })
    .await
}

/// Alle Kennzahlen der Startseite in einem Aufruf (T25). `today` ist der lokale Tag des
/// Nutzers aus der Oberfläche, wie bei `get_week_overview` bestimmt sie den Bezugstag.
#[tauri::command(rename_all = "snake_case")]
async fn get_dashboard(state: State<'_, AppState>, today: String) -> Result<Dashboard> {
    with_store(&state, move |store| {
        service::dashboard::dashboard(store, &today)
    })
    .await
}

/// Tagessummen für die Ansicht "Fortschritt" (T32). `today` ist der lokale Tag des Nutzers.
#[tauri::command(rename_all = "snake_case")]
async fn get_progress(state: State<'_, AppState>, today: String) -> Result<Progress> {
    with_store(&state, move |store| {
        service::progress::progress(store, &today)
    })
    .await
}

/// Leistungskurve und weitere Bestwerte für die Seite "Leistung" (T33). `range` ist `42`, `90`,
/// `365` oder `all`.
#[tauri::command(rename_all = "snake_case")]
async fn get_performance(
    state: State<'_, AppState>,
    today: String,
    range: String,
) -> Result<Performance> {
    with_store(&state, move |store| {
        service::performance::performance(store, &today, &range)
    })
    .await
}

/// Erholungswerte mit Basislinie und Tageslast für die Seite "Erholung" (T34). `days` ist 30, 90
/// oder 365.
#[tauri::command(rename_all = "snake_case")]
async fn get_recovery(
    state: State<'_, AppState>,
    today: String,
    days: u32,
) -> Result<RecoveryPage> {
    with_store(&state, move |store| {
        service::recovery::recovery(store, &today, days)
    })
    .await
}

/// Seite "Gesundheit" (T39): Tages- und Trainingswerte aus Garmin, `days` ist 30, 90 oder 365.
#[tauri::command(rename_all = "snake_case")]
async fn get_health(state: State<'_, AppState>, today: String, days: u32) -> Result<HealthPage> {
    with_store(&state, move |store| {
        service::health::health(store, &today, days)
    })
    .await
}

/// "Löschen" der S3-Werte (T39, je Bereich seit T40): ein Bereich oder ohne Angabe alle.
#[tauri::command(rename_all = "snake_case")]
async fn delete_sensitive_data(
    state: State<'_, AppState>,
    category: Option<String>,
) -> Result<u32> {
    with_store(&state, move |store| {
        service::health::delete_sensitive(store, category.as_deref())
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn list_activities(
    state: State<'_, AppState>,
    filter: ActivityFilter,
) -> Result<Vec<ActivityItem>> {
    with_store(&state, move |store| {
        service::list_activities(store, &filter)
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn get_activity(state: State<'_, AppState>, id: i64) -> Result<ActivityDetail> {
    let root = state.fit_root.clone();
    with_store(&state, move |store| service::get_activity(store, id, &root)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn list_thresholds(state: State<'_, AppState>) -> Result<ThresholdList> {
    with_store(&state, move |store| {
        service::list_thresholds(store, local_today())
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn set_threshold(state: State<'_, AppState>, input: ThresholdInput) -> Result<ThresholdSet> {
    let root = state.fit_root.clone();
    with_store(&state, move |store| {
        service::set_threshold(store, &root, &input, Utc::now())
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn set_athlete(
    app: AppHandle,
    state: State<'_, AppState>,
    input: AthleteInput,
) -> Result<AthleteSet> {
    let sync = state.in_sync_folder;
    let root = state.fit_root.clone();
    let mut set = with_store(&state, move |store| {
        service::set_athlete(store, &root, &input, Utc::now(), sync)
    })
    .await?;
    state.decorate(&app, &mut set.settings);
    Ok(set)
}

#[tauri::command(rename_all = "snake_case")]
async fn list_plans(state: State<'_, AppState>) -> Result<Vec<PlanListItem>> {
    with_store(&state, |store| service::plans::list_plans(store)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn get_plan(state: State<'_, AppState>, id: String) -> Result<PlanDetail> {
    with_store(&state, move |store| service::plans::get_plan(store, &id)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn generate_plan(state: State<'_, AppState>, input: GeneratePlanInput) -> Result<PlanDetail> {
    with_store(&state, move |store| {
        service::plans::generate(store, &input, uuid::Uuid::new_v4().to_string(), Utc::now())
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn set_plan_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> Result<Vec<PlanListItem>> {
    with_store(&state, move |store| {
        service::plans::set_plan_status(store, &id, &status)
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn get_planning_inputs(state: State<'_, AppState>) -> Result<PlanningInputs> {
    with_store(&state, |store| {
        service::plans::planning_inputs_at(store, local_today())
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
async fn set_goal(state: State<'_, AppState>, input: GoalInput) -> Result<PlanningInputs> {
    with_store(&state, move |store| service::plans::set_goal(store, &input)).await
}

#[tauri::command(rename_all = "snake_case")]
async fn set_availability(
    state: State<'_, AppState>,
    days: Vec<AvailabilityDay>,
) -> Result<PlanningInputs> {
    with_store(&state, move |store| {
        service::plans::set_availability(store, &days)
    })
    .await
}

// --- Planer mit drei Fragen und Gespräch mit dem Coach (T26 Teil B) ---------------------

/// Die drei Fragen: Ziel, Stunden je Woche, Trainingstage. Schreibt Ziel und Verfügbarkeit,
/// erzeugt den Plan mit dem Regelplaner und aktiviert ihn (SPEC 7.2, O31).
#[tauri::command(rename_all = "snake_case")]
async fn quick_plan(state: State<'_, AppState>, input: QuickPlanInput) -> Result<PlanDetail> {
    with_store(&state, move |store| {
        quick::quick_plan(store, &input, uuid::Uuid::new_v4().to_string(), Utc::now())
    })
    .await
}

/// Ob ein lokales Modell erreichbar ist und welche es gibt (Loopback, kurzes Zeitlimit).
#[tauri::command(rename_all = "snake_case")]
async fn coach_status(state: State<'_, AppState>) -> Result<coach::CoachStatus> {
    with_store(&state, |store| coach::status(store)).await
}

/// Merkt das Modell für das Gespräch. Leer heißt: das Programm wählt eines.
#[tauri::command(rename_all = "snake_case")]
async fn coach_set_model(state: State<'_, AppState>, model: String) -> Result<coach::CoachStatus> {
    with_store(&state, move |store| coach::set_model(store, &model)).await
}

/// Der exakte Eingang der nächsten Nachricht (F24). Sendet nichts.
#[tauri::command(rename_all = "snake_case")]
async fn coach_preview(
    state: State<'_, AppState>,
    coach_state: State<'_, CoachState>,
    message: String,
    focus: Option<ChatFocusDto>,
) -> Result<coach::PromptPreview> {
    let coach_state = coach_state.inner().clone();
    with_store(&state, move |store| {
        coach::preview(store, &coach_state, &message, focus.as_ref(), local_today())
    })
    .await
}

/// Ein Gesprächsschritt. Höchstens einer gleichzeitig (`busy`). Der Aufruf an das Modell
/// läuft auf einem Arbeitsthread, das Zeitlimit steht in `lastwerk-coach` (120 s je Aufruf).
#[tauri::command(rename_all = "snake_case")]
async fn coach_chat(
    state: State<'_, AppState>,
    coach_state: State<'_, CoachState>,
    message: String,
    focus: Option<ChatFocusDto>,
) -> Result<coach::CoachReply> {
    let coach_state = coach_state.inner().clone();
    let guard = coach_state
        .begin()
        .ok_or_else(|| AppError::new("coach_busy"))?;
    with_store(&state, move |store| {
        // Die Sperre gehört zum Arbeitsthread und fällt erst mit seinem Ende.
        let _guard = guard;
        coach::chat_with_provider(
            store,
            &coach_state,
            &message,
            focus.as_ref(),
            local_today(),
            Utc::now(),
            &|| uuid::Uuid::new_v4().to_string(),
        )
    })
    .await
}

/// Übernimmt einen Vorschlag des Coachs und aktiviert den neuen Plan.
#[tauri::command(rename_all = "snake_case")]
async fn coach_apply(
    state: State<'_, AppState>,
    coach_state: State<'_, CoachState>,
    proposal_id: String,
) -> Result<PlanDetail> {
    let coach_state = coach_state.inner().clone();
    with_store(&state, move |store| {
        coach::apply(
            store,
            &coach_state,
            &proposal_id,
            local_today(),
            Utc::now(),
            uuid::Uuid::new_v4().to_string(),
        )
    })
    .await
}

/// Beginnt das Gespräch von vorn: Verlauf und offene Vorschläge weg, ein laufendes Gespräch
/// stellt weitere Aufrufe ein (SPEC 9.5).
#[tauri::command(rename_all = "snake_case")]
async fn coach_reset(coach_state: State<'_, CoachState>) -> Result<()> {
    coach_state.reset();
    Ok(())
}

/// Bricht das laufende Gespräch ab, der Verlauf bleibt (SPEC 9.5).
#[tauri::command(rename_all = "snake_case")]
async fn coach_cancel(coach_state: State<'_, CoachState>) -> Result<()> {
    coach_state.cancel();
    Ok(())
}

// --- Einrichtung des lokalen Coachs (T29) -----------------------------------------------

/// Pfad des gebündelten Skripts, falls es da ist. Bleibt im Rust-Teil.
fn setup_script(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .resolve(coach_setup::SCRIPT_RESOURCE, BaseDirectory::Resource)
        .ok()
        .filter(|p| p.is_file())
}

/// Umstände des Status: Zustandsdatei neben der Datenbank (dasselbe Verzeichnis, das das
/// Skript als Standard nutzt, `%LOCALAPPDATA%\Lastwerk`), Ort von Ollama, Skript.
fn setup_environment(app: &AppHandle, state: &AppState) -> coach_setup::Environment {
    let state_path = state
        .db_path
        .parent()
        .map_or_else(PathBuf::new, Path::to_path_buf)
        .join(coach_setup::STATE_FILE);
    let exe_present = cfg!(windows)
        && app.path().local_data_dir().is_ok_and(|base| {
            base.join("Programs")
                .join("Ollama")
                .join("ollama.exe")
                .is_file()
        });
    coach_setup::Environment {
        state_path,
        exe_present,
        script_present: setup_script(app).is_some(),
    }
}

/// Zustand der Einrichtung: Ollama gefunden oder laufend, Modelle, Hardware, Katalog mit
/// Einstufung und Empfehlung, Inhalt der Zustandsdatei, laufender Auftrag.
#[tauri::command(rename_all = "snake_case")]
async fn coach_setup_status(
    app: AppHandle,
    state: State<'_, AppState>,
    setup: State<'_, CoachSetupState>,
) -> Result<CoachSetupStatus> {
    let env = setup_environment(&app, &state);
    let setup = setup.inner().clone();
    with_store(&state, move |store| {
        coach_setup::status(store, &setup, &env)
    })
    .await
}

/// Richtet Ollama über das gebündelte Skript ein (nur Windows). Fortschritt als Ereignis
/// `coach-setup-progress`, Ergebnis ist der neue Status.
#[tauri::command(rename_all = "snake_case")]
async fn coach_install_ollama(
    app: AppHandle,
    state: State<'_, AppState>,
    setup: State<'_, CoachSetupState>,
) -> Result<CoachSetupStatus> {
    if !cfg!(windows) {
        return Err(AppError::new("coach_setup_unsupported"));
    }
    let script = setup_script(&app).ok_or_else(|| AppError::new("coach_setup_missing_script"))?;
    let setup = setup.inner().clone();
    let guard = setup.begin(JobKind::Install, None)?;
    let env = setup_environment(&app, &state);
    with_store(&state, move |store| {
        let result = coach_setup::install_ollama(&script, &env.state_path, &mut |p| {
            // Ein verlorenes Ereignis ist unkritisch, das Skript läuft weiter.
            let _ = app.emit(EVENT_COACH_SETUP_PROGRESS, p);
        });
        drop(guard);
        result?;
        coach_setup::status(store, &setup, &env)
    })
    .await
}

/// Lädt ein Modell aus dem Katalog oder ein schon installiertes, Fortschritt als Ereignis
/// `coach-pull-progress`. Danach ist es das Modell des Coachs (`coach.model`). Ergebnis ist der
/// neue Status.
#[tauri::command(rename_all = "snake_case")]
async fn coach_pull_model(
    app: AppHandle,
    state: State<'_, AppState>,
    setup: State<'_, CoachSetupState>,
    name: String,
) -> Result<CoachSetupStatus> {
    if !lastwerk_coach::is_valid_model_name(&name) {
        return Err(AppError::new("coach_pull_invalid_model"));
    }
    let setup = setup.inner().clone();
    let guard = setup.begin(JobKind::Pull, Some(name.clone()))?;
    let env = setup_environment(&app, &state);
    with_store(&state, move |store| {
        let result = coach_setup::pull(store, &setup, &guard, &name, &mut |e| {
            let _ = app.emit(EVENT_COACH_PULL_PROGRESS, e);
        });
        drop(guard);
        result?;
        coach_setup::status(store, &setup, &env)
    })
    .await
}

/// Bricht ein laufendes Laden ab. Ohne laufendes Laden passiert nichts.
#[tauri::command(rename_all = "snake_case")]
async fn coach_pull_cancel(setup: State<'_, CoachSetupState>) -> Result<()> {
    setup.cancel_pull();
    Ok(())
}

/// Öffnet den Speichern-Dialog und schreibt `content` an den gewählten Ort. `false`, wenn der
/// Nutzer abbricht. Der Pfad bleibt im Rust-Teil (SPEC 10.4: kein Dateisystem im Frontend).
async fn save_with_dialog(
    app: &AppHandle,
    file_name: String,
    filter: &'static str,
    extension: &'static str,
    content: String,
) -> Result<bool> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let picked = app
            .dialog()
            .file()
            .add_filter(filter, &[extension])
            .set_file_name(&file_name)
            .blocking_save_file();
        let Some(picked) = picked else {
            return Ok(false);
        };
        let path = picked
            .into_path()
            .map_err(|_| AppError::new("dialog_failed"))?;
        std::fs::write(path, content).map_err(|_| AppError::new("export_failed"))?;
        Ok(true)
    })
    .await
    .map_err(|_| AppError::new("internal"))?
}

#[tauri::command(rename_all = "snake_case")]
async fn export_plan_ics(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<ExportResult> {
    let (text, events, name) =
        with_store(&state, move |store| service::plans::export_ics(store, &id)).await?;
    let saved = save_with_dialog(&app, name, "ICS", "ics", text).await?;
    Ok(ExportResult {
        saved,
        events: saved.then_some(events),
        counts: None,
    })
}

#[tauri::command(rename_all = "snake_case")]
async fn export_plan_json(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<ExportResult> {
    let (text, name) = with_store(&state, move |store| {
        service::plans::export_plan_json(store, &id)
    })
    .await?;
    let saved = save_with_dialog(&app, name, "JSON", "json", text).await?;
    Ok(ExportResult {
        saved,
        events: None,
        counts: None,
    })
}

#[tauri::command(rename_all = "snake_case")]
async fn export_backup(app: AppHandle, state: State<'_, AppState>) -> Result<ExportResult> {
    let (text, counts, name) = with_store(&state, |store| {
        service::plans::export_backup(store, Utc::now())
    })
    .await?;
    let saved = save_with_dialog(&app, name, "JSON", "json", text).await?;
    Ok(ExportResult {
        saved,
        events: None,
        counts: saved.then_some(counts),
    })
}

/// Öffnet den Öffnen-Dialog und spielt die gewählte Sicherung ein. `confirmed` ist die
/// Bestätigung des Nutzers: Die Wiederherstellung ersetzt alle Nutzerdaten (SPEC 6.3).
#[tauri::command(rename_all = "snake_case")]
async fn restore_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    confirmed: bool,
) -> Result<RestoreResult> {
    if !confirmed {
        return Err(AppError::new("unconfirmed"));
    }
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let Some(picked) = app
            .dialog()
            .file()
            .add_filter("JSON", &["json"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = picked
            .into_path()
            .map_err(|_| AppError::new("dialog_failed"))?;
        let size = std::fs::metadata(&path)
            .map_err(|_| AppError::new("restore_invalid"))?
            .len();
        if size > service::plans::MAX_BACKUP_BYTES {
            return Err(AppError::new("restore_invalid"));
        }
        let text = std::fs::read_to_string(path).map_err(|_| AppError::new("restore_invalid"))?;
        Ok(Some(text))
    })
    .await
    .map_err(|_| AppError::new("internal"))??;
    let Some(text) = picked else {
        return Ok(RestoreResult {
            restored: false,
            counts: None,
        });
    };
    let counts = with_store(&state, move |store| {
        service::plans::restore_backup(store, &text)
    })
    .await?;
    Ok(RestoreResult {
        restored: true,
        counts: Some(counts),
    })
}

/// Fragt auf Klick des Nutzers beim Release-Endpunkt aus `tauri.conf.json` nach einer neuen
/// Version (SPEC 10.1, Abweichung T17). Es gibt keinen Hintergrundcheck. Der Endpunkt ist fest,
/// das Frontend bekommt keine URL und kann keine andere angeben. Fehler tragen keine Meldung des
/// Netzwerks, nur den Code.
#[tauri::command(rename_all = "snake_case")]
async fn check_update(app: AppHandle) -> Result<UpdateInfo> {
    let current = app.package_info().version.to_string();
    let updater = app.updater().map_err(|_| AppError::new("update_failed"))?;
    let found = updater
        .check()
        .await
        .map_err(|_| AppError::new("update_failed"))?;
    Ok(UpdateInfo {
        current,
        available: found.is_some(),
        version: found.map(|u| u.version),
    })
}

/// Lädt die neue Version und installiert sie. Das Plugin prüft die Signatur gegen den
/// öffentlichen Schlüssel aus der Konfiguration, bevor es etwas ausführt. `confirmed` ist die
/// Bestätigung des Nutzers. Danach startet die App neu.
#[tauri::command(rename_all = "snake_case")]
async fn install_update(app: AppHandle, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(AppError::new("unconfirmed"));
    }
    let updater = app.updater().map_err(|_| AppError::new("update_failed"))?;
    let found = updater
        .check()
        .await
        .map_err(|_| AppError::new("update_failed"))?;
    let Some(update) = found else {
        return Err(AppError::new("no_update"));
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|_| AppError::new("update_failed"))?;
    app.restart()
}

// --- Garmin (T23, SPEC 5.6) ------------------------------------------------------------------

/// Unterordner unter dem lokalen Datenordner für den FIT-Stamm und die Sperrdatei.
const FIT_DIR: &str = "fit";
const GARMIN_DIR: &str = "garmin";
const LOCKOUT_FILE: &str = "lockout.json";

/// Zustand für Garmin: ein Client für die ganze App (eine Drossel, eine Sperre, kein
/// Parallelabruf), die Sperre gegen zwei Abgleiche und die Merkmarke für den Abgleich beim Start.
pub struct GarminState {
    client: Arc<GarminClient>,
    running: Arc<AtomicBool>,
    start_done: AtomicBool,
}

impl GarminState {
    pub fn new(client: GarminClient) -> Self {
        Self {
            client: Arc::new(client),
            running: Arc::new(AtomicBool::new(false)),
            start_done: AtomicBool::new(false),
        }
    }
}

/// Client mit den echten Adressen, dem Schlüsselbund des Systems und der Sperrdatei neben dem
/// FIT-Ordner (`<Datenordner>/Lastwerk/garmin/lockout.json`, dieselbe wie in der Kommandozeile).
fn garmin_state(app: &AppHandle) -> Result<GarminState> {
    let base = app
        .path()
        .local_data_dir()
        .map_err(|_| AppError::new("database_io"))?
        .join(DB_DIR);
    let lockout = base.join(GARMIN_DIR).join(LOCKOUT_FILE);
    let store: Arc<dyn TokenStore> = Arc::new(KeyringStore::new());
    let client = GarminClient::new(Endpoints::default(), store, lockout);
    Ok(GarminState::new(client))
}

#[tauri::command(rename_all = "snake_case")]
async fn garmin_status(
    state: State<'_, AppState>,
    garmin: State<'_, GarminState>,
) -> Result<GarminStatus> {
    let client = Arc::clone(&garmin.client);
    with_store(&state, move |store| service::garmin::status(store, &client)).await
}

/// Öffnet das Anmeldefenster und kehrt sofort zurück. Das Ergebnis kommt als Ereignis
/// `garmin-login`.
#[tauri::command(rename_all = "snake_case")]
async fn garmin_login(
    app: AppHandle,
    state: State<'_, AppState>,
    garmin: State<'_, GarminState>,
) -> Result<()> {
    let locale = with_store(&state, |store| Ok(store.get_setting(service::KEY_LOCALE)?)).await?;
    garmin_login::open(
        &app,
        Arc::clone(&garmin.client),
        locale.as_deref().unwrap_or("de"),
    )
}

/// Abgleich auf einem Arbeitsthread. `trigger` ist `manual` (Klick) oder `start` (einmal je
/// Programmstart, nur wenn verbunden und eingeschaltet, sonst `null`). Läuft schon einer, ist
/// das Ergebnis der Fehler `busy`.
#[tauri::command(rename_all = "snake_case")]
async fn garmin_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    garmin: State<'_, GarminState>,
    trigger: String,
) -> Result<Option<GarminSyncSummary>> {
    let trigger = service::garmin::Trigger::parse(&trigger)?;
    if trigger == service::garmin::Trigger::Start && garmin.start_done.swap(true, Ordering::AcqRel)
    {
        return Ok(None);
    }
    let guard = service::garmin::SyncGuard::try_acquire(&garmin.running)?;
    let client = Arc::clone(&garmin.client);
    let fit_root = state.fit_root.clone();
    with_store(&state, move |store| {
        let _guard = guard;
        if trigger == service::garmin::Trigger::Start {
            if !service::garmin::should_sync_on_start(store, &client)? {
                return Ok(None);
            }
        } else if !client.is_connected()? {
            return Err(AppError::new("garmin_not_connected"));
        }
        let mut last_import = None;
        let summary = service::garmin::sync(
            store,
            client.as_ref(),
            &fit_root,
            local_today(),
            Utc::now(),
            &mut |p| {
                if service::garmin::should_emit(&p, &mut last_import) {
                    // Ein verlorenes Ereignis ist unkritisch, der Abgleich läuft weiter.
                    let _ = app.emit(EVENT_GARMIN_SYNC_PROGRESS, p);
                }
            },
        )?;
        Ok(Some(summary))
    })
    .await
}

/// "Trennen": löscht die Token im Schlüsselbund.
#[tauri::command(rename_all = "snake_case")]
async fn garmin_logout(garmin: State<'_, GarminState>) -> Result<()> {
    let client = Arc::clone(&garmin.client);
    tauri::async_runtime::spawn_blocking(move || service::garmin::logout(&client))
        .await
        .map_err(|_| AppError::new("internal"))?
}

/// Startet die App.
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let path = database_path(app.handle()).map_err(|e| e.code)?;
            let fit_root = fit_root_path(app.handle()).map_err(|e| e.code)?;
            // Datenbank anlegen und migrieren, damit ein Fehler beim Start auffällt und nicht
            // erst beim ersten Befehl. Danach übernimmt die App einen früher gewählten
            // FIT-Ordner einmalig in den festen Stamm (T28); bei einem Fehler startet sie
            // trotzdem, die Übernahme versucht es beim nächsten Start erneut.
            let migration = open_store(&path).ok().and_then(|mut store| {
                lastwerk_cli::fit_migration::migrate_legacy_fit_dir(&mut store, &fit_root)
                    .ok()
                    .flatten()
                    .map(FitMigrationDto::from)
            });
            app.manage(AppState::new(path, fit_root).with_migration(migration));
            app.manage(garmin_state(app.handle()).map_err(|e| e.code)?);
            // Verlauf und offene Vorschläge des Coachs leben nur im Speicher (T26, SPEC 9.7).
            app.manage(CoachState::default());
            // Auftrag der Einrichtung (Ollama, Modell laden) und gemessene Hardware (T29).
            app.manage(CoachSetupState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            get_bridge_status,
            import_folder,
            add_fit_files,
            get_week_overview,
            get_dashboard,
            get_progress,
            get_performance,
            get_recovery,
            get_health,
            delete_sensitive_data,
            list_activities,
            get_activity,
            list_thresholds,
            set_threshold,
            set_athlete,
            list_plans,
            get_plan,
            generate_plan,
            set_plan_status,
            get_planning_inputs,
            set_goal,
            set_availability,
            export_plan_ics,
            export_plan_json,
            export_backup,
            restore_backup,
            check_update,
            install_update,
            garmin_status,
            garmin_login,
            garmin_sync,
            garmin_logout,
            quick_plan,
            coach_status,
            coach_set_model,
            coach_preview,
            coach_chat,
            coach_apply,
            coach_reset,
            coach_cancel,
            coach_setup_status,
            coach_install_ollama,
            coach_pull_model,
            coach_pull_cancel,
        ])
        .run(tauri::generate_context!());
    if result.is_err() {
        // Keine Meldung mit Pfaden oder Fremdtext, nur der Exit-Code.
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Namen innerhalb von `generate_handler![ ... ]` im Quelltext dieser Datei.
    fn handler_names() -> Vec<String> {
        let src = include_str!("lib.rs");
        let start =
            src.find("generate_handler![").expect("generate_handler") + "generate_handler![".len();
        let end = start + src[start..].find(']').expect("Ende");
        src[start..end]
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    }

    #[test]
    fn handler_list_matches_command_names() {
        let mut handler = handler_names();
        handler.sort();
        let mut names: Vec<String> = COMMAND_NAMES.iter().map(|s| (*s).to_owned()).collect();
        names.sort();
        assert_eq!(handler, names);
    }

    #[test]
    fn build_script_and_capability_list_exactly_the_commands() {
        let build = include_str!("../build.rs");
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let permissions: Vec<&str> = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        for name in COMMAND_NAMES {
            assert!(
                build.contains(&format!("\"{name}\"")),
                "{name} fehlt in build.rs"
            );
            let permission = format!("allow-{}", name.replace('_', "-"));
            assert!(
                permissions.contains(&permission.as_str()),
                "{permission} fehlt"
            );
        }
        // Nur diese Befehle plus das Hören auf Ereignisse, nichts sonst (SPEC 10.4).
        let own = permissions
            .iter()
            .filter(|p| p.starts_with("allow-"))
            .count();
        assert_eq!(own, COMMAND_NAMES.len());
        let others: Vec<&&str> = permissions
            .iter()
            .filter(|p| !p.starts_with("allow-"))
            .collect();
        assert_eq!(
            others,
            [&"core:event:allow-listen", &"core:event:allow-unlisten"]
        );
    }

    /// Das Anmeldefenster `garmin-login` bekommt keine Capability (SPEC 10.4, T23): Die einzige
    /// Capability-Datei gilt genau für `main`, ohne entfernte Adressen und ohne Platzhalter.
    #[test]
    fn only_the_main_window_has_a_capability() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert!(capability.get("webviews").is_none());
        assert!(capability.get("remote").is_none());
        assert!(capability.get("platforms").is_none());
        let files: Vec<String> =
            std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/capabilities"))
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
        assert_eq!(files, ["default.json"]);
        // Die Konfiguration nennt keine weitere Capability und kein zweites Fenster.
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert!(conf["app"]["security"].get("capabilities").is_none());
        let windows = conf["app"]["windows"].as_array().unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0]["label"], "main");
        assert_ne!(garmin_login::LOGIN_WINDOW, "main");
    }

    #[test]
    fn updater_has_fixed_https_endpoint_and_public_key() {
        // Update nur auf Klick, Endpunkt und Schlüssel fest in der Konfiguration (T17, SPEC 10.1).
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = &conf["plugins"]["updater"];
        let endpoints = updater["endpoints"].as_array().unwrap();
        assert_eq!(endpoints.len(), 1);
        assert!(
            endpoints[0]
                .as_str()
                .unwrap()
                .starts_with("https://github.com/flor1xn/lastwerk/releases/")
        );
        assert!(updater["pubkey"].as_str().unwrap().len() > 40);
    }

    #[test]
    fn config_has_no_remote_sources_and_no_global_tauri() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["app"]["withGlobalTauri"], false);
        let csp = conf["app"]["security"]["csp"].as_str().unwrap();
        assert!(!csp.contains("unsafe-eval"));
        assert!(!csp.contains("unsafe-inline"));
        assert!(!csp.contains("http://") || csp.contains("http://ipc.localhost"));
        assert!(!csp.contains("https:"));
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("script-src 'self'"));
        assert!(csp.contains("connect-src 'self' ipc: http://ipc.localhost"));
    }

    #[test]
    fn every_error_code_has_a_text_in_both_languages() {
        for (name, src) in [
            ("de", include_str!("../../../catalog/locales/de.json")),
            ("en", include_str!("../../../catalog/locales/en.json")),
        ] {
            let catalog: serde_json::Value = serde_json::from_str(src).unwrap();
            for code in error::ERROR_CODES {
                let text = catalog["ui"]["error"][*code].as_str();
                assert!(
                    text.is_some_and(|t| !t.is_empty()),
                    "{name}: ui.error.{code} fehlt"
                );
            }
        }
    }

    #[test]
    fn every_catalog_label_and_phase_has_a_text_in_both_languages() {
        for (name, src) in [
            ("de", include_str!("../../../catalog/locales/de.json")),
            ("en", include_str!("../../../catalog/locales/en.json")),
        ] {
            let catalog: serde_json::Value = serde_json::from_str(src).unwrap();
            let text = |key: &str| {
                key.split('.')
                    .try_fold(&catalog, |v, k| v.get(k))
                    .and_then(|v| v.as_str())
                    .is_some_and(|t| !t.is_empty())
            };
            for model in lastwerk_coach::embedded_catalog() {
                assert!(text(&model.label_key), "{name}: {} fehlt", model.label_key);
            }
            for phase in ["manifest", "download", "verify", "write", "done", "other"] {
                let key = format!("ui.coach_setup.pull_phase.{phase}");
                assert!(text(&key), "{name}: {key} fehlt");
            }
            for key in [
                "detect_found",
                "detect_missing",
                "detect_running",
                "install_winget",
                "install_download",
                "install_verify_ok",
                "install_skipped",
                "start_ok",
                "start_timeout",
                "pull_start",
                "pull_progress",
                "pull_done",
                "pull_skipped",
                "done_ok",
                "error_unknown",
            ] {
                let key = format!("ui.coach_setup.install_phase.{key}");
                assert!(text(&key), "{name}: {key} fehlt");
            }
        }
    }
}
