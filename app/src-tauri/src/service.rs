//! Logik der Befehle ohne Tauri-Typen: dünne Schicht über `lastwerk-store`, `lastwerk-fit` und
//! `lastwerk-core`. Sie rechnet nichts Eigenes außer dem Zusammenfassen (Summen je Tag und
//! Sportart, Mittel beim Ausdünnen der Messreihe). Belastung, Zonen und Verlauf kommen aus den
//! Crates. Alles hier ist ohne Fenster testbar, die Uhrzeit kommt als Parameter.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Datelike, NaiveDate, SecondsFormat, TimeDelta, Utc};
use lastwerk_core::{LoadConfig, PmcParams, Sample, Sport, pmc};
use lastwerk_fit::{parse_activity, read_fit_file};
use lastwerk_store::{
    ImportOptions, ImportReport, LoadRow, LoadStatus, NewThreshold, SETTING_ATHLETE_BIRTH_YEAR,
    SETTING_ATHLETE_SEX, SETTING_MCP_RECOVERY_ENABLED, SETTING_PMC_ATL0, SETTING_PMC_CTL0,
    SETTING_PMC_TAU_ATL, SETTING_PMC_TAU_CTL, SourceFileRecord, Store, ThresholdKind,
    ThresholdOrigin, embedded_zone_catalog, sha256_hex,
};

use crate::dto::{
    ActivityDetail, ActivityFilter, ActivityItem, Athlete, AthleteInput, AthleteSet, BestPower,
    BridgeStatus, DayLoad, ErrorCount, HrMaxEstimate, ImportProgress, ImportSummary, LoadInfo,
    PmcPointDto, PmcSettings, Recompute, Series, Settings, SettingsPatch, SportLoad,
    ThresholdInput, ThresholdItem, ThresholdList, ThresholdSet, WeekOverview, ZoneSchemeTime,
};
use crate::dto::{DetailDecoupling, LapItem};
use crate::error::{AppError, Result};
use lastwerk_core::metrics::efficiency::{
    EfSession, aerobic_decoupling, decoupling_state, efficiency_factor,
};
use lastwerk_core::metrics::laps::{LapBounds, lap_stats};
use lastwerk_fit::RawActivity;

pub mod coach;
pub mod coach_setup;
pub mod dashboard;
pub mod fit_files;
pub mod garmin;
pub mod performance;
pub mod plans;
pub mod progress;
pub mod quick;
pub mod recovery;

/// Schlüssel in `setting`. Einstellungen der Oberfläche tragen das Präfix `app.`, die des
/// Verlaufs `pmc.` (SPEC 8.2: stehen im Backup), die des Athleten stehen im Store.
/// `app.fit_dir` (früherer Ordner des Nutzers) wird seit T28 nicht mehr geschrieben und nur noch
/// von der Übernahme gelesen (`lastwerk_cli::fit_migration`).
pub const KEY_LOCALE: &str = "app.locale";
pub const KEY_THEME: &str = "app.theme";
pub const KEY_ONBOARDED: &str = "app.onboarded";
pub const KEY_TAU_CTL: &str = SETTING_PMC_TAU_CTL;
pub const KEY_TAU_ATL: &str = SETTING_PMC_TAU_ATL;
pub const KEY_MCP_RECOVERY: &str = SETTING_MCP_RECOVERY_ENABLED;
pub const KEY_CTL0: &str = SETTING_PMC_CTL0;
pub const KEY_ATL0: &str = SETTING_PMC_ATL0;

pub const LOCALES: [&str; 2] = ["de", "en"];
pub const THEMES: [&str; 3] = ["dark", "light", "system"];

/// Höchstzahl Wochen im PMC-Fenster, rund zwei Jahre (Konvention).
pub const MAX_PMC_WEEKS: u32 = 104;
/// Höchstzahl Punkte einer Messreihe an die Oberfläche (Konvention, reicht für die Breite
/// eines Fensters).
pub const MAX_SERIES_POINTS: usize = 800;
/// Längste Notiz zu einer Schwelle in Zeichen (Konvention).
pub const MAX_NOTE_CHARS: usize = 200;
/// Längster Zeitraum der Aktivitätsliste in Tagen, rund 100 Jahre (Konvention, wie die CLI).
pub const MAX_LIST_DAYS: i64 = 36_600;

/// Sportarten in fester Reihenfolge der Anzeige.
const SPORT_ORDER: [Sport; 5] = [
    Sport::Cycling,
    Sport::Running,
    Sport::Swimming,
    Sport::Strength,
    Sport::Other,
];

fn sport_rank(s: Sport) -> usize {
    SPORT_ORDER
        .iter()
        .position(|x| *x == s)
        .unwrap_or(usize::MAX)
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| AppError::new("invalid_date"))
}

// --- Einstellungen -----------------------------------------------------------------

fn text_setting(store: &Store, key: &str) -> Result<Option<String>> {
    Ok(store.get_setting(key)?)
}

/// Parameter des Verlaufs aus den Einstellungen, sonst die Standardwerte aus `core`. Die
/// Auslese liegt im Speicher (`Store::pmc_params`), dieselbe nutzen die Kommandozeile und der
/// MCP-Server.
pub fn pmc_settings(store: &Store) -> Result<PmcSettings> {
    let p = store.pmc_params()?;
    Ok(PmcSettings {
        tau_ctl_d: p.tau_ctl_d,
        tau_atl_d: p.tau_atl_d,
        ctl0: p.ctl0,
        atl0: p.atl0,
    })
}

fn pmc_params(store: &Store) -> Result<PmcParams> {
    Ok(store.pmc_params()?)
}

pub fn get_settings(store: &Store, db_in_sync_folder: bool) -> Result<Settings> {
    let locale = text_setting(store, KEY_LOCALE)?
        .filter(|l| LOCALES.contains(&l.as_str()))
        .unwrap_or_else(|| "de".to_owned());
    // Dunkel zuerst (SPEC 2, F14).
    let theme = text_setting(store, KEY_THEME)?
        .filter(|t| THEMES.contains(&t.as_str()))
        .unwrap_or_else(|| "dark".to_owned());
    let sex = text_setting(store, SETTING_ATHLETE_SEX)?.filter(|s| s == "male" || s == "female");
    let birth_year =
        text_setting(store, SETTING_ATHLETE_BIRTH_YEAR)?.and_then(|t| t.trim().parse::<i32>().ok());
    Ok(Settings {
        // Beide füllt der Befehl (`lib.rs`), sie gehören dem Prozess und nicht der Datenbank.
        fit_root: String::new(),
        fit_migration: None,
        locale,
        theme,
        pmc: pmc_settings(store)?,
        athlete: Athlete { sex, birth_year },
        // Wie der MCP-Server: nur genau `true` gibt frei.
        mcp_recovery_enabled: text_setting(store, KEY_MCP_RECOVERY)?.as_deref() == Some("true"),
        db_in_sync_folder,
        onboarded: text_setting(store, KEY_ONBOARDED)?.as_deref() == Some("true"),
        // Vorgabe aus dem Paket, der Befehl ersetzt sie durch die Version der App.
        version: env!("CARGO_PKG_VERSION").to_owned(),
        garmin_sync_on_start: store.garmin_settings()?.sync_on_start,
        cache_outdated: store.cache_outdated()?,
    })
}

pub fn set_settings(
    store: &mut Store,
    patch: &SettingsPatch,
    db_in_sync_folder: bool,
) -> Result<Settings> {
    // Erst alles prüfen, dann schreiben: Ein ungültiger Teil ändert nichts.
    if let Some(l) = &patch.locale
        && !LOCALES.contains(&l.as_str())
    {
        return Err(AppError::new("invalid_setting"));
    }
    if let Some(t) = &patch.theme
        && !THEMES.contains(&t.as_str())
    {
        return Err(AppError::new("invalid_setting"));
    }
    if let Some(p) = &patch.pmc {
        // Zeitkonstanten müssen positiv sein, Startwerte nicht negativ (Konvention).
        let tau_ok = |v: f32| v.is_finite() && v > 0.0;
        let start_ok = |v: f32| v.is_finite() && v >= 0.0;
        if !(tau_ok(p.tau_ctl_d) && tau_ok(p.tau_atl_d) && start_ok(p.ctl0) && start_ok(p.atl0)) {
            return Err(AppError::new("invalid_value"));
        }
    }
    if let Some(l) = &patch.locale {
        store.set_setting(KEY_LOCALE, l)?;
    }
    if let Some(t) = &patch.theme {
        store.set_setting(KEY_THEME, t)?;
    }
    if let Some(enabled) = patch.mcp_recovery_enabled {
        store.set_setting(KEY_MCP_RECOVERY, if enabled { "true" } else { "false" })?;
    }
    if let Some(done) = patch.onboarded {
        store.set_setting(KEY_ONBOARDED, if done { "true" } else { "false" })?;
    }
    if let Some(on) = patch.garmin_sync_on_start {
        store.set_garmin_sync_on_start(on)?;
    }
    if let Some(p) = &patch.pmc {
        store.set_setting(KEY_TAU_CTL, &p.tau_ctl_d.to_string())?;
        store.set_setting(KEY_TAU_ATL, &p.tau_atl_d.to_string())?;
        store.set_setting(KEY_CTL0, &p.ctl0.to_string())?;
        store.set_setting(KEY_ATL0, &p.atl0.to_string())?;
    }
    get_settings(store, db_in_sync_folder)
}

/// Obergrenze gelesener Verzeichniseinträge: `exchange/out` ist nicht vertrauenswürdig
/// (SPEC 5.4), die Zahl begrenzt die Arbeit (Konvention).
const MAX_EXCHANGE_ENTRIES: usize = 50_000;

/// Zählt, was das Zusatzprogramm in `<dir>/out` abgelegt hat. Liest nur Dateinamen, nie den
/// Inhalt, und gibt nur Anzahlen und den jüngsten Tag zurück (SPEC 5.4, 10.2). Ein fehlender
/// Ordner ist kein Fehler, sondern "keine Daten".
pub fn bridge_status(dir: &Path) -> BridgeStatus {
    let mut recovery_days = 0u32;
    let mut last: Option<NaiveDate> = None;
    if let Ok(rd) = std::fs::read_dir(dir.join("out").join("recovery")) {
        for entry in rd.flatten().take(MAX_EXCHANGE_ENTRIES) {
            let name = entry.file_name();
            let Some(stem) = name.to_str().and_then(|n| n.strip_suffix(".json")) else {
                continue;
            };
            if let Ok(day) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                recovery_days += 1;
                last = last.max(Some(day));
            }
        }
    }
    let mut activity_files = 0u32;
    if let Ok(rd) = std::fs::read_dir(dir.join("out").join("activities")) {
        for entry in rd.flatten().take(MAX_EXCHANGE_ENTRIES) {
            let is_fit = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("fit"));
            if is_fit {
                activity_files += 1;
            }
        }
    }
    BridgeStatus {
        has_data: recovery_days > 0 || activity_files > 0,
        recovery_days,
        activity_files,
        last_day: last.map(|d| d.format("%Y-%m-%d").to_string()),
    }
}

/// Liegt der Pfad in einem bekannten Sync-Ordner (SPEC 6.1)? SQLite im WAL-Modus verträgt
/// Cloud-Sync schlecht (Beschädigungsrisiko). Erkennung nach Ordnernamen, Konvention: eine
/// Liste gängiger Dienste, keine Vollständigkeit.
pub fn in_sync_folder(path: &Path) -> bool {
    const MARKERS: [&str; 9] = [
        "onedrive",
        "dropbox",
        "icloud",
        "mobile documents",
        "google drive",
        "googledrive",
        "nextcloud",
        "owncloud",
        "pcloud",
    ];
    path.components().any(|c| match c {
        Component::Normal(name) => {
            let name = name.to_string_lossy().to_lowercase();
            MARKERS.iter().any(|m| name.starts_with(m))
        }
        _ => false,
    })
}

// --- Import ------------------------------------------------------------------------

/// Liest den festen FIT-Stamm ein (T28). Der Stamm wird bei Bedarf angelegt, ein frischer ist
/// einfach leer.
pub fn import(
    store: &mut Store,
    root: &Path,
    now: DateTime<Utc>,
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    std::fs::create_dir_all(root).map_err(|_| AppError::new("fit_root_io"))?;
    let report = store.import_dir(root, now, &ImportOptions::default(), &mut |p| {
        // Der Speicher meldet die 0-basierte Nummer der Datei in Arbeit. Die Anzeige zählt ab 1,
        // die letzte Meldung ist dann `total` von `total`.
        progress(ImportProgress {
            done: p.done as u32 + 1,
            total: p.total as u32,
        });
    })?;
    Ok(summarize(&report))
}

fn summarize(r: &ImportReport) -> ImportSummary {
    let mut by_code: BTreeMap<&str, u32> = BTreeMap::new();
    for e in &r.errors {
        *by_code.entry(e.code.as_str()).or_default() += 1;
    }
    ImportSummary {
        files_found: r.files_found as u32,
        new: r.new as u32,
        updated: r.updated as u32,
        unchanged: r.unchanged as u32,
        not_activity: r.not_activity as u32,
        error_count: r.error_count() as u32,
        errors_by_code: by_code
            .into_iter()
            .map(|(code, count)| ErrorCount {
                code: code.to_owned(),
                count,
            })
            .collect(),
        duplicates: r.duplicates as u32,
        primary: r.primary as u32,
        loads_ok: r.loads_ok as u32,
        loads_missing_threshold: r.loads_missing_threshold as u32,
        loads_missing_channel: r.loads_missing_channel as u32,
        loads_implausible: r.loads_implausible as u32,
    }
}

// --- Aktivitäten -------------------------------------------------------------------

const METHODS_ESTIMATED: [&str; 3] = ["r_tss", "s_tss", "session_rpe"];

fn load_info(rows: &[&LoadRow]) -> LoadInfo {
    // Ein Treffer mit Status ok gewinnt, sonst die erste Zeile. Der Import schreibt je
    // Aktivität genau eine Zeile.
    let row = rows
        .iter()
        .find(|l| l.status == LoadStatus::Ok)
        .or_else(|| rows.first());
    match row {
        Some(l) if l.status == LoadStatus::Ok => LoadInfo {
            status: "ok".into(),
            value: l.value,
            method: Some(l.method.clone()),
            intensity_factor: l.intensity_factor,
            estimated: METHODS_ESTIMATED.contains(&l.method.as_str()),
        },
        Some(l) => LoadInfo {
            status: l.status.as_str().into(),
            value: None,
            method: None,
            intensity_factor: None,
            estimated: false,
        },
        None => LoadInfo {
            status: "unknown".into(),
            value: None,
            method: None,
            intensity_factor: None,
            estimated: false,
        },
    }
}

fn item(a: &lastwerk_store::ActivityRecord, loads: &[&LoadRow]) -> ActivityItem {
    ActivityItem {
        id: a.id,
        local_date: a.row.local_date.to_string(),
        start_utc: a.row.start_utc.to_rfc3339_opts(SecondsFormat::Secs, true),
        sport: a.row.sport,
        sub_sport: a.row.sub_sport.clone(),
        duration_s: a.row.duration_s,
        distance_m: a.row.distance_m,
        avg_hr: a.row.avg_hr,
        avg_power_w: a.row.avg_power_w,
        load: load_info(loads),
        is_primary: a.is_primary,
        duplicate_of: a.duplicate_of,
    }
}

/// Aktivitäten des Zeitraums, neueste zuerst, mit Last und Status.
pub fn list_activities(store: &Store, filter: &ActivityFilter) -> Result<Vec<ActivityItem>> {
    let from = parse_date(&filter.from)?;
    let to = parse_date(&filter.to)?;
    if from > to || (to - from).num_days() > MAX_LIST_DAYS {
        return Err(AppError::new("invalid_date"));
    }
    let activities = store.activities_between(from, to)?;
    let loads = store.loads_between(from, to)?;
    let mut by_activity: BTreeMap<i64, Vec<&LoadRow>> = BTreeMap::new();
    for l in &loads {
        by_activity.entry(l.activity_id).or_default().push(l);
    }
    let mut items: Vec<ActivityItem> = activities
        .iter()
        .filter(|a| filter.sport.is_none_or(|s| s == a.row.sport))
        .map(|a| {
            let rows = by_activity.get(&a.id).map_or(&[][..], Vec::as_slice);
            item(a, rows)
        })
        .collect();
    items.reverse();
    Ok(items)
}

pub fn get_activity(store: &Store, id: i64, fit_root: &Path) -> Result<ActivityDetail> {
    let a = store
        .activity(id)?
        .ok_or_else(|| AppError::new("not_found"))?;
    let loads = store.activity_loads_of(id)?;
    let load_refs: Vec<&LoadRow> = loads.iter().collect();

    let catalog = embedded_zone_catalog();
    let zone_rows = store.zone_times_of(id)?;
    let mut zones: Vec<ZoneSchemeTime> = Vec::new();
    for scheme in &catalog.schemes {
        let rows: Vec<_> = zone_rows.iter().filter(|z| z.scheme == scheme.id).collect();
        if rows.is_empty() {
            // Nicht berechenbar für diese Aktivität: kein Eintrag statt Nullen.
            continue;
        }
        let mut seconds = vec![0u32; scheme.zone_count()];
        for z in &rows {
            if let Some(slot) = (z.zone as usize)
                .checked_sub(1)
                .and_then(|i| seconds.get_mut(i))
            {
                *slot = z.seconds;
            }
        }
        zones.push(ZoneSchemeTime {
            id: scheme.id.clone(),
            basis: basis_name(scheme.basis),
            seconds,
            estimated: rows.iter().any(|z| z.estimated),
            source: scheme.source.clone(),
            status: scheme.status.clone(),
        });
    }

    // Im Detail nur die Eckdauern; die ganze Kurve zeigt die Seite "Leistung" (T33).
    let best_power = store
        .best_powers_of(id)?
        .into_iter()
        .filter(|b| lastwerk_core::BEST_POWER_DURATIONS_S.contains(&b.duration_s))
        .map(|b| BestPower {
            duration_s: b.duration_s,
            watts: b.watts,
        })
        .collect();

    let read = match store.source_file(a.row.source_file_id)? {
        Some(file) => read_activity_file(fit_root, &file),
        None => Err("file_missing"),
    };
    let (series, decoupling_percent, laps) = match read {
        Ok((raw, cleaned)) => {
            let power: Vec<Option<f32>> = cleaned.iter().map(|s| s.power_w).collect();
            let hr: Vec<Option<f32>> = cleaned.iter().map(|s| s.hr_bpm).collect();
            (
                series_of(&cleaned),
                aerobic_decoupling(&power, &hr),
                laps_of(&raw, &cleaned),
            )
        }
        Err(status) => (empty_series(status), None, Vec::new()),
    };
    let ef = efficiency_factor(&EfSession {
        date: a.row.local_date,
        sport: a.row.sport,
        duration_s: u32::try_from(a.row.duration_s.max(0)).unwrap_or(u32::MAX),
        np_w: a.row.np_w.map(|v| v as f32),
        avg_hr_bpm: a.row.avg_hr.map(|v| v as f32),
    });

    Ok(ActivityDetail {
        item: item(&a, &load_refs),
        moving_s: a.row.moving_s,
        max_hr: a.row.max_hr,
        np_w: a.row.np_w,
        device_product: a.row.device_product.clone(),
        zones,
        best_power,
        series,
        efficiency_factor: ef.and_then(|v| round_digits(f64::from(v), 2)),
        decoupling: DetailDecoupling {
            percent: decoupling_percent.and_then(|v| round_digits(v, 1)),
            state: decoupling_state(decoupling_percent).code().to_owned(),
        },
        laps,
    })
}

/// Rundet auf `digits` Nachkommastellen, nicht endliche Werte fallen weg.
fn round_digits(v: f64, digits: i32) -> Option<f64> {
    v.is_finite().then(|| {
        let f = 10f64.powi(digits);
        (v * f).round() / f
    })
}

/// Runden der Datei mit Mitteln aus der bereinigten Reihe (T31). Weniger als zwei Runden
/// ergeben keine Tabelle: Eine einzige Runde wiederholt nur die Aktivität.
fn laps_of(raw: &RawActivity, cleaned: &[Sample]) -> Vec<LapItem> {
    if raw.laps.len() < 2 {
        return Vec::new();
    }
    let bounds: Vec<LapBounds> = raw
        .laps
        .iter()
        .map(|l| {
            let start_s = l
                .start_time
                .map(|t| (t - raw.start_utc).num_seconds().max(0))
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0);
            let elapsed = l
                .total_elapsed_time_s
                .or(l.total_timer_time_s)
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(0.0);
            LapBounds {
                start_s,
                elapsed_s: elapsed.ceil().min(f64::from(u32::MAX)) as u32,
                timer_s: l.total_timer_time_s.filter(|v| v.is_finite() && *v >= 0.0),
                distance_m: l.total_distance_m.filter(|v| v.is_finite() && *v >= 0.0),
            }
        })
        .collect();
    lap_stats(cleaned, &bounds)
        .into_iter()
        .map(|l| LapItem {
            start_s: l.start_s,
            elapsed_s: l.elapsed_s,
            timer_s: l.timer_s.and_then(|v| round_digits(v, 0)),
            distance_m: l.distance_m.and_then(|v| round_digits(v, 0)),
            avg_power_w: l.avg_power_w.and_then(|v| round_digits(f64::from(v), 0)),
            avg_hr_bpm: l.avg_hr_bpm.and_then(|v| round_digits(f64::from(v), 0)),
            avg_speed_mps: l.avg_speed_mps.and_then(|v| round_digits(f64::from(v), 3)),
        })
        .collect()
}

fn basis_name(b: lastwerk_core::ZoneBasis) -> String {
    use lastwerk_core::ZoneBasis::*;
    match b {
        Ftp => "ftp",
        Lthr => "lthr",
        HrReserve => "hr_reserve",
        RunThresholdSpeed => "run_threshold_speed",
        Css => "css",
    }
    .to_owned()
}

// --- Messreihe ---------------------------------------------------------------------

fn empty_series(status: &str) -> Series {
    Series {
        status: status.to_owned(),
        t_s: Vec::new(),
        power_w: Vec::new(),
        hr_bpm: Vec::new(),
        speed_mps: Vec::new(),
    }
}

/// Pfad einer Quelldatei unter `dir`. `rel_path` ist mit `/` getrennt und relativ. Pfade mit
/// `..`, Laufwerken oder Wurzel werden abgelehnt (wie in `lastwerk-store`).
fn source_path(dir: &Path, rel_path: &str) -> Option<PathBuf> {
    let mut path = dir.to_path_buf();
    for part in rel_path.split('/') {
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(c)), None) => path.push(c),
            _ => return None,
        }
    }
    Some(path)
}

/// Liest eine Aktivität erneut aus ihrer FIT-Datei (Konvention: Der Cache speichert keine
/// Messpunkte, SPEC 6.1). Stimmt die Prüfsumme nicht mehr, ist die Datei nicht mehr die
/// importierte, und es gibt keine Reihe. Ergebnis: die Rohdaten (Runden) und die Samples ohne
/// unplausible Werte (SPEC 6.5), oder der Zustand der Reihe als Fehler.
pub fn read_activity_file(
    fit_root: &Path,
    file: &SourceFileRecord,
) -> std::result::Result<(RawActivity, Vec<Sample>), &'static str> {
    let path = source_path(fit_root, &file.row.rel_path).ok_or("file_missing")?;
    let bytes = read_fit_file(&path).map_err(|_| "file_missing")?;
    if sha256_hex(&bytes) != file.row.sha256 {
        return Err("file_changed");
    }
    let raw = parse_activity(&bytes).map_err(|_| "unreadable")?;
    let cfg = LoadConfig::default();
    let cleaned: Vec<Sample> = raw
        .samples
        .iter()
        .map(|s| Sample {
            t_s: s.t_s,
            power_w: s
                .power_w
                .filter(|p| p.is_finite() && *p >= 0.0 && *p <= cfg.power_max_w),
            hr_bpm: s
                .hr_bpm
                .filter(|h| h.is_finite() && *h >= cfg.hr_min_bpm && *h <= cfg.hr_max_bpm),
            speed_mps: s.speed_mps.filter(|v| v.is_finite() && *v >= 0.0),
        })
        .collect();
    Ok((raw, cleaned))
}

/// Reihe für die Darstellung, `no_channels` ohne Leistung, Herzfrequenz und Geschwindigkeit.
fn series_of(cleaned: &[Sample]) -> Series {
    if !cleaned
        .iter()
        .any(|s| s.power_w.is_some() || s.hr_bpm.is_some() || s.speed_mps.is_some())
    {
        return empty_series("no_channels");
    }
    downsample(cleaned, MAX_SERIES_POINTS)
}

/// Gemittelte Messreihe einer Aktivität (siehe [`read_activity_file`]).
pub fn read_series(fit_root: &Path, file: &SourceFileRecord) -> Series {
    match read_activity_file(fit_root, file) {
        Ok((_, cleaned)) => series_of(&cleaned),
        Err(status) => empty_series(status),
    }
}

/// Fasst aufeinanderfolgende Samples zu höchstens `max_points` Punkten zusammen. Zeit ist der
/// Beginn des Blocks, Leistung und Herzfrequenz sind die Mittel der vorhandenen Werte im
/// Block (`null`, wenn der Block keinen hat). Mittel statt Auswahl, damit kurze Spitzen die
/// Darstellung nicht verzerren; die Kennzahlen kommen weiter aus dem Cache.
pub fn downsample(samples: &[Sample], max_points: usize) -> Series {
    let block = samples.len().div_ceil(max_points.max(1)).max(1);
    let mut out = empty_series("ok");
    for chunk in samples.chunks(block) {
        out.t_s.push(chunk[0].t_s);
        out.power_w
            .push(mean(chunk.iter().filter_map(|s| s.power_w)));
        out.hr_bpm.push(mean(chunk.iter().filter_map(|s| s.hr_bpm)));
        out.speed_mps
            .push(mean_speed(chunk.iter().filter_map(|s| s.speed_mps)));
    }
    out
}

/// Mittel der Geschwindigkeit auf drei Nachkommastellen (m/s), damit das Tempo in min/km nicht
/// um Sekunden springt.
fn mean_speed(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0f64, 0u32), |(s, n), v| (s + f64::from(v), n + 1));
    (n > 0).then(|| ((sum / f64::from(n)) * 1000.0).round() as f32 / 1000.0)
}

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0f64, 0u32), |(s, n), v| (s + f64::from(v), n + 1));
    (n > 0).then(|| ((sum / f64::from(n)) * 10.0).round() as f32 / 10.0)
}

// --- Woche und Verlauf -------------------------------------------------------------

pub fn week_overview(
    store: &Store,
    week_start: &str,
    pmc_weeks: u32,
    today: NaiveDate,
) -> Result<WeekOverview> {
    let start = parse_date(week_start)?;
    let end = start + TimeDelta::days(6);
    let weeks = pmc_weeks.clamp(1, MAX_PMC_WEEKS);

    let activities = store.activities_between(start, end)?;
    let loads = store.loads_between(start, end)?;
    let mut by_activity: BTreeMap<i64, Vec<&LoadRow>> = BTreeMap::new();
    for l in &loads {
        by_activity.entry(l.activity_id).or_default().push(l);
    }

    // Je Tag: Summe je Sportart und Zahl der zählenden Aktivitäten ohne berechenbare Last.
    let mut sums: BTreeMap<NaiveDate, BTreeMap<usize, f64>> = BTreeMap::new();
    let mut missing: BTreeMap<NaiveDate, u32> = BTreeMap::new();
    let mut counted = 0u32;
    for a in activities.iter().filter(|a| a.is_primary) {
        counted += 1;
        let info = load_info(by_activity.get(&a.id).map_or(&[][..], Vec::as_slice));
        match (info.status.as_str(), info.value) {
            ("ok", Some(v)) => {
                *sums
                    .entry(a.row.local_date)
                    .or_default()
                    .entry(sport_rank(a.row.sport))
                    .or_default() += v;
            }
            _ => *missing.entry(a.row.local_date).or_default() += 1,
        }
    }

    let mut totals: BTreeMap<usize, f64> = BTreeMap::new();
    let days: Vec<DayLoad> = (0..7)
        .map(|i| {
            let date = start + TimeDelta::days(i);
            let by_sport = sums
                .get(&date)
                .map(|m| {
                    m.iter()
                        .map(|(rank, load)| {
                            *totals.entry(*rank).or_default() += load;
                            SportLoad {
                                sport: SPORT_ORDER[*rank],
                                load: *load,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            DayLoad {
                date: date.to_string(),
                by_sport,
                not_computable: missing.get(&date).copied().unwrap_or(0),
            }
        })
        .collect();

    // Verlauf: letzte `weeks` Wochen bis zum Wochenende, aber nicht über heute hinaus (eine
    // Vorausberechnung ohne Plan wäre nur ein Abklingen und irreführend, SPEC 8.2).
    let window_start = end - TimeDelta::days(i64::from(weeks) * 7 - 1);
    let pmc_end = end.min(today);
    let first = store.first_load_date()?;
    // Ohne jede zählende Last gibt es keinen Verlauf: Nullen wären eine Aussage, die
    // niemand gemessen hat (SPEC 5.4).
    let pmc_points = if first.is_some() && pmc_end >= window_start {
        // Beginn am ersten Tag mit Last, falls der vor dem Fenster liegt, damit CTL und ATL
        // am Fensteranfang eingeschwungen sind (wie `lastwerk pmc`).
        let from = first.filter(|f| *f < window_start).unwrap_or(window_start);
        let daily = store.daily_loads(from, pmc_end)?;
        let load_of: BTreeMap<NaiveDate, f32> = daily.iter().map(|d| (d.date, d.load)).collect();
        pmc(&daily, &pmc_params(store)?, from, pmc_end)
            .into_iter()
            .filter(|p| p.date >= window_start)
            .map(|p| PmcPointDto {
                date: p.date.to_string(),
                load: load_of.get(&p.date).copied().unwrap_or(0.0),
                ctl: p.ctl,
                atl: p.atl,
                tsb: p.tsb,
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(WeekOverview {
        week_start: start.to_string(),
        week_end: end.to_string(),
        days,
        totals_by_sport: totals
            .into_iter()
            .map(|(rank, load)| SportLoad {
                sport: SPORT_ORDER[rank],
                load,
            })
            .collect(),
        activities: counted,
        not_computable: missing.values().sum(),
        pmc: pmc_points,
        pmc_first_load_date: first.map(|d| d.to_string()),
    })
}

// --- Schwellenwerte und Athlet -----------------------------------------------------

pub fn list_thresholds(store: &Store, today: NaiveDate) -> Result<ThresholdList> {
    let rows = store.list_thresholds()?;
    let mut items = Vec::with_capacity(rows.len());
    for t in &rows {
        items.push(ThresholdItem {
            id: t.id,
            sport: t.sport,
            kind: t.kind,
            value: t.value,
            valid_from: t.valid_from.to_string(),
            valid_until: store.threshold_valid_until(t.id)?.map(|d| d.to_string()),
            origin: t.origin.as_str().to_owned(),
            note: t.note.clone(),
        });
    }
    // Wie `lastwerk thresholds list`: Fehlt jede gemessene HR_max, steht die Schätzung aus
    // dem Alter dabei, immer als geschätzt (Q7).
    let hr_max_estimate = if rows.iter().any(|t| t.kind == ThresholdKind::HrMaxBpm) {
        None
    } else {
        store
            .effective_hr_max(Sport::Other, today)?
            .filter(|h| h.estimated)
            .map(|h| HrMaxEstimate {
                bpm: h.bpm,
                estimated: true,
            })
    };
    Ok(ThresholdList {
        items,
        hr_max_estimate,
    })
}

/// Prüft Wert und Art gegen die Plausibilitätsgrenzen aus SPEC 6.5 (`LoadConfig`) und gegen
/// die Sportart. Dieselben Regeln wie `lastwerk thresholds set`. Zahlen, die die SPEC nicht
/// nennt, werden nicht erfunden: Geschwindigkeiten müssen nur positiv sein.
fn check_threshold(input: &ThresholdInput, cfg: &LoadConfig) -> Result<()> {
    let value = input.value;
    if !(value.is_finite() && value > 0.0) {
        return Err(AppError::new("invalid_value"));
    }
    let hr_range = f64::from(cfg.hr_min_bpm)..=f64::from(cfg.hr_max_bpm);
    match input.kind {
        ThresholdKind::FtpW if value > f64::from(cfg.power_max_w) => {
            return Err(AppError::new("invalid_value"));
        }
        ThresholdKind::LthrBpm | ThresholdKind::HrMaxBpm | ThresholdKind::HrRestBpm
            if !hr_range.contains(&value) =>
        {
            return Err(AppError::new("invalid_value"));
        }
        _ => {}
    }
    let required = match input.kind {
        ThresholdKind::FtpW => Some(Sport::Cycling),
        ThresholdKind::RunSpeedMps => Some(Sport::Running),
        ThresholdKind::CssMps => Some(Sport::Swimming),
        _ => None,
    };
    if let (Some(needed), Some(sport)) = (required, input.sport)
        && sport != needed
    {
        return Err(AppError::new("invalid_sport"));
    }
    Ok(())
}

fn recompute(
    store: &mut Store,
    dir: &Path,
    sport: Option<Sport>,
    from: NaiveDate,
    to: Option<NaiveDate>,
    now: DateTime<Utc>,
) -> Result<Recompute> {
    // Ein Stamm, der noch nicht existiert, enthält keine Dateien: nichts neu zu rechnen.
    if !dir.is_dir() {
        return Ok(Recompute {
            skipped: true,
            ..Recompute::default()
        });
    }
    let r = store.recompute_loads(dir, sport, from, to, now, &ImportOptions::default())?;
    Ok(Recompute {
        skipped: false,
        candidates: r.candidates as u32,
        recomputed: r.recomputed as u32,
        file_missing: r.file_missing as u32,
        file_changed: r.file_changed as u32,
    })
}

/// Legt einen Schwellenwert an und rechnet die betroffenen Aktivitäten neu (Entscheidung T05,
/// wie `lastwerk thresholds set --dir`): ab dem Gültigkeitsdatum bis zur nächsten Schwelle
/// gleicher Art. Liegen im FIT-Stamm noch keine Dateien, bleibt es bei der gespeicherten Schwelle.
pub fn set_threshold(
    store: &mut Store,
    fit_root: &Path,
    input: &ThresholdInput,
    now: DateTime<Utc>,
) -> Result<ThresholdSet> {
    let valid_from = parse_date(&input.valid_from)?;
    check_threshold(input, &LoadConfig::default())?;
    let note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned);
    if note
        .as_ref()
        .is_some_and(|n| n.chars().count() > MAX_NOTE_CHARS)
    {
        return Err(AppError::new("invalid_value"));
    }
    let id = store.insert_threshold(&NewThreshold {
        sport: input.sport,
        kind: input.kind,
        value: input.value,
        valid_from,
        origin: ThresholdOrigin::Manual,
        note,
    })?;
    let until = store.threshold_valid_until(id)?;
    let recompute = recompute(store, fit_root, input.sport, valid_from, until, now)?;
    Ok(ThresholdSet { id, recompute })
}

/// Speichert Geschlecht (nur für die TRIMP-Konstanten) und Geburtsjahr (nur für die Schätzung
/// der maximalen Herzfrequenz, Q7) und rechnet alle Aktivitäten neu, wie `lastwerk athlete set
/// --dir`.
pub fn set_athlete(
    store: &mut Store,
    fit_root: &Path,
    input: &AthleteInput,
    now: DateTime<Utc>,
    db_in_sync_folder: bool,
) -> Result<AthleteSet> {
    if let Some(sex) = &input.sex
        && sex != "male"
        && sex != "female"
    {
        return Err(AppError::new("invalid_value"));
    }
    if let Some(year) = input.birth_year
        && !(1900..=now.year()).contains(&year)
    {
        return Err(AppError::new("invalid_year"));
    }
    if let Some(sex) = &input.sex {
        store.set_setting(SETTING_ATHLETE_SEX, sex)?;
    }
    if let Some(year) = input.birth_year {
        store.set_setting(SETTING_ATHLETE_BIRTH_YEAR, &year.to_string())?;
    }
    let from = NaiveDate::from_ymd_opt(1900, 1, 1).ok_or_else(|| AppError::new("internal"))?;
    let recompute = if input.sex.is_some() || input.birth_year.is_some() {
        recompute(store, fit_root, None, from, None, now)?
    } else {
        Recompute {
            skipped: true,
            ..Recompute::default()
        }
    };
    Ok(AthleteSet {
        settings: get_settings(store, db_in_sync_folder)?,
        recompute,
    })
}

#[cfg(test)]
mod tests;
