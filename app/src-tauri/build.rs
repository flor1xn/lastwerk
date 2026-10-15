//! Build-Skript: erzeugt aus der Befehlsliste die Berechtigungen `allow-<befehl>`.
//!
//! Ohne diese Liste dürfte das Frontend jeden Befehl der App aufrufen. Mit ihr gilt der
//! Grundsatz aus SPEC 10.4: Das Frontend darf genau die hier genannten Befehle und sonst
//! nichts (siehe `capabilities/default.json`). Die Liste muss zu `generate_handler!` in
//! `src/lib.rs` passen, ein Test dort prüft das.

/// Befehle, die das Frontend aufrufen darf.
const COMMANDS: &[&str] = &[
    "get_settings",
    "set_settings",
    "import_folder",
    "add_fit_files",
    "get_week_overview",
    "get_dashboard",
    "get_recovery",
    "get_performance",
    "get_progress",
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

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    if let Err(e) = tauri_build::try_build(attributes) {
        panic!("tauri-build fehlgeschlagen: {e:#}");
    }
}
