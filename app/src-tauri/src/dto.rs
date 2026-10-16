//! Typen zwischen Rust und Oberfläche. Sie spiegeln `app/ui/src/lib/backend/types.ts`.
//!
//! Konvention: Feldnamen in `snake_case` wie in der Datenbank, Daten als `YYYY-MM-DD`,
//! Zeitpunkte als ISO 8601 in UTC. Fehlende Werte sind `null`, nie 0 (SPEC 5.4). Keine Pfade,
//! außer dem festen FIT-Stamm zur Anzeige (`Settings::fit_root`, T28).

use lastwerk_core::Sport;
use lastwerk_store::ThresholdKind;
use serde::{Deserialize, Serialize};

/// Belastung einer Aktivität mit Status. `value` ist nur bei `ok` gesetzt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LoadInfo {
    /// `ok`, `missing_threshold`, `missing_channel`, `implausible`.
    pub status: String,
    pub value: Option<f64>,
    /// `tss`, `hr_tss`, `r_tss`, `s_tss`, `session_rpe`; bei einem Fehler `null`.
    pub method: Option<String>,
    pub intensity_factor: Option<f64>,
    /// rTSS, sTSS und Session-RPE sind Faustregeln und gelten als geschätzt (SPEC 8.1).
    pub estimated: bool,
    /// Bei `missing_threshold` die fehlende Art (`ftp_w`, `lthr_bpm`, `hr_max_bpm`,
    /// `hr_rest_bpm`, `run_speed_mps`, `css_mps`, `sex`), sonst `null` (T39, SPEC 8.6).
    pub missing: Option<String>,
}

/// Zeile der Aktivitätsliste.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActivityItem {
    pub id: i64,
    pub local_date: String,
    pub start_utc: String,
    pub sport: Sport,
    pub sub_sport: Option<String>,
    pub duration_s: i64,
    pub distance_m: Option<f64>,
    pub avg_hr: Option<f64>,
    pub avg_power_w: Option<f64>,
    pub load: LoadInfo,
    pub is_primary: bool,
    pub duplicate_of: Option<i64>,
}

/// Filter der Aktivitätsliste.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ActivityFilter {
    pub from: String,
    pub to: String,
    /// `None` heißt alle Sportarten.
    pub sport: Option<Sport>,
}

/// Zeit in Zonen eines Schemas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZoneSchemeTime {
    pub id: String,
    /// Bezugswert, zum Beispiel `ftp`.
    pub basis: String,
    pub seconds: Vec<u32>,
    /// Zonen beruhen auf einer aus dem Alter geschätzten maximalen Herzfrequenz (SPEC 8.5).
    pub estimated: bool,
    /// Quelle der Grenzen: Quellennummer aus SPEC Anhang A oder `Faustregel`.
    pub source: String,
    /// Prüfstatus: `Sekundärquelle` oder `Faustregel`.
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BestPower {
    pub duration_s: u32,
    pub watts: f64,
}

/// Verlauf von Leistung und Herzfrequenz, auf höchstens `MAX_SERIES_POINTS` Punkte gemittelt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Series {
    /// `ok`, `file_missing`, `file_changed`, `unreadable`, `no_channels`.
    pub status: String,
    pub t_s: Vec<u32>,
    pub power_w: Vec<Option<f32>>,
    pub hr_bpm: Vec<Option<f32>>,
    /// Geschwindigkeit in m/s (T31), für das Tempo bei Läufen.
    pub speed_mps: Vec<Option<f32>>,
}

/// Aerobe Entkopplung einer Aktivität (T31, Rechnung `core::metrics::efficiency`, Q16 nur
/// sekundär). `percent` ist `None`, wenn die Reihe zu kurz ist oder Leistung oder Herzfrequenz
/// fehlen; `state` ist dann `decoupling.no_data`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetailDecoupling {
    pub percent: Option<f64>,
    pub state: String,
}

/// Kennzahlen einer Runde (T31). Mittel nur über vorhandene Werte, sonst `None`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LapItem {
    pub start_s: u32,
    pub elapsed_s: u32,
    pub timer_s: Option<f64>,
    pub distance_m: Option<f64>,
    pub avg_power_w: Option<f64>,
    pub avg_hr_bpm: Option<f64>,
    pub avg_speed_mps: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActivityDetail {
    pub item: ActivityItem,
    pub moving_s: Option<i64>,
    pub max_hr: Option<f64>,
    pub np_w: Option<f64>,
    pub device_product: Option<String>,
    pub zones: Vec<ZoneSchemeTime>,
    pub best_power: Vec<BestPower>,
    pub series: Series,
    /// Effizienzfaktor (NP durch mittlere Herzfrequenz), nur Rad ab 45 min (T25, T31).
    pub efficiency_factor: Option<f64>,
    pub decoupling: DetailDecoupling,
    /// Runden aus der Datei, leer bei weniger als zwei Runden oder ohne lesbare Datei.
    pub laps: Vec<LapItem>,
    /// Garmins eigene Last, nur zur Anzeige neben der eigenen (Entscheidung E2, T39). `null`,
    /// wenn die Aktivität nicht aus Garmin stammt oder Garmin keine gemeldet hat.
    pub garmin_load: Option<GarminLoadInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GarminLoadInfo {
    pub training_load: Option<f64>,
    /// Garmins Schlüssel, zum Beispiel `AEROBIC_BASE`.
    pub training_effect: Option<String>,
}

/// Last eines Tages je Sportart (nur zählende Aktivitäten, SPEC 6.4).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DayLoad {
    pub date: String,
    pub by_sport: Vec<SportLoad>,
    /// Zählende Aktivitäten des Tages ohne berechenbare Last. Sie erscheinen nicht als 0.
    pub not_computable: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SportLoad {
    pub sport: Sport,
    pub load: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PmcPointDto {
    pub date: String,
    /// Last des Tages (0 an Tagen ohne zählende Last, wie im Verlauf gerechnet).
    pub load: f32,
    pub ctl: f32,
    pub atl: f32,
    pub tsb: f32,
}

/// Bestwert mit Herkunft (T33): Wert, Tag und Aktivität, damit die Oberfläche verlinken kann.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerfBest {
    pub value: f64,
    pub date: String,
    pub activity_id: i64,
}

/// Eine Dauer der Leistungskurve im Zeitraum gegen den Vorzeitraum (T33).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerfPower {
    pub duration_s: u32,
    pub current: Option<PerfBest>,
    pub previous_w: Option<f64>,
    pub change_percent: Option<f64>,
    /// `power_curve.*` aus `core`.
    pub trend: String,
}

/// Bestzeit über eine Strecke im Zeitraum gegen den Vorzeitraum (T37). `current.value` und
/// `previous_s` sind Sekunden; `change_percent` negativ heißt schneller.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerfRunBest {
    pub distance_m: u32,
    pub current: Option<PerfBest>,
    pub previous_s: Option<f64>,
    pub change_percent: Option<f64>,
}

/// Gesetzter Schwellenwert im Verlauf (T36).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerfThreshold {
    pub kind: ThresholdKind,
    /// `null` gilt für alle Sportarten.
    pub sport: Option<Sport>,
    pub value: f64,
    pub valid_from: String,
    /// `manual` oder `suggested_accepted`.
    pub origin: String,
}

/// FTP-Vorschlag eines Kalendermonats aus der besten 20-Minuten-Leistung (T36, SPEC 8.6:
/// 95 Prozent, Faustregel, Q16 nur sekundär). Wird nie übernommen, nur gezeigt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerfFtpSuggestion {
    /// `YYYY-MM`
    pub month: String,
    pub value_w: f64,
    pub best_20min_w: f64,
    pub date: String,
    pub activity_id: i64,
}

/// Grundlage der Seite "Leistung" (T33). `from` und `previous_from` fehlen beim Zeitraum `all`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Performance {
    pub today: String,
    /// `42`, `90`, `365` oder `all`.
    pub range: String,
    pub from: Option<String>,
    pub previous_from: Option<String>,
    pub power: Vec<PerfPower>,
    /// Bestzeiten im Laufen über `BEST_EFFORT_DISTANCES_M` (T37).
    pub run_bests: Vec<PerfRunBest>,
    /// Gesetzte FTP, LTHR, Laufschwelle und CSS bis heute, nach Art und Datum (T36). Unabhängig
    /// vom Zeitraum, damit der aktuelle Wert immer dabei ist.
    pub thresholds: Vec<PerfThreshold>,
    /// FTP-Vorschläge je Monat im Zeitraum, ältester zuerst (T36).
    pub ftp_suggestions: Vec<PerfFtpSuggestion>,
}

/// Ein Tag der Seite "Erholung" (T34). Fehlende Werte sind `None`, nie 0. Basislinie und
/// Untergrenze aus R12 (`core`), `low` heißt: HRV oder Schlaf auf oder unter der Untergrenze.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecoveryPagePoint {
    pub date: String,
    pub hrv_ms: Option<f64>,
    pub resting_hr_bpm: Option<f64>,
    pub sleep_h: Option<f64>,
    pub sleep_score: Option<u8>,
    pub body_battery_high: Option<u8>,
    pub body_battery_low: Option<u8>,
    pub hrv_base: Option<f64>,
    pub hrv_floor: Option<f64>,
    pub resting_hr_base: Option<f64>,
    pub sleep_base_h: Option<f64>,
    pub sleep_floor_h: Option<f64>,
    pub low: bool,
    /// Tageslast, nur Tage mit Last.
    pub load: Option<f64>,
}

/// Mittel der letzten sieben Tage gegen das Mittel des Zeitraums (beschreibend, T34).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecoverySummary {
    /// `hrv`, `resting_hr`, `sleep`, `sleep_score`, `body_battery_high`.
    pub metric: String,
    pub recent: Option<f64>,
    pub period: Option<f64>,
}

/// Grundlage der Seite "Erholung" (T34).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecoveryPage {
    pub today: String,
    /// 30, 90 oder 365.
    pub days: u32,
    /// `readiness.*` aus `core`, wie im Dashboard.
    pub state: String,
    /// Jüngster Tag mit Erholungswerten im Zeitraum.
    pub last_date: Option<String>,
    /// Tage im Zeitraum, an denen R12 auslösen würde.
    pub low_days: u32,
    /// Schwellen aus R12 in Prozent, für die Erläuterung.
    pub hrv_drop_percent: Option<f64>,
    pub sleep_deficit_percent: Option<f64>,
    pub points: Vec<RecoveryPagePoint>,
    pub summary: Vec<RecoverySummary>,
}

/// Summen eines Tages und einer Sportart (T32). Nur zählende Aktivitäten (SPEC 6.4). `load` summiert
/// nur berechenbare Lasten, `load_missing` zählt die übrigen, damit 0 nie für "fehlt" steht.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProgressEntry {
    pub date: String,
    pub sport: Sport,
    pub count: u32,
    pub duration_s: i64,
    pub distance_m: f64,
    pub load: f64,
    pub load_missing: u32,
}

/// Grundlage der Ansicht "Fortschritt" (T32): Tageszeilen von `from` bis `today`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Progress {
    pub today: String,
    pub from: String,
    pub entries: Vec<ProgressEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeekOverview {
    pub week_start: String,
    pub week_end: String,
    pub days: Vec<DayLoad>,
    pub totals_by_sport: Vec<SportLoad>,
    pub activities: u32,
    pub not_computable: u32,
    pub pmc: Vec<PmcPointDto>,
    /// Erster Tag mit zählender Last: Der Verlauf beginnt dort (SPEC 8.2).
    pub pmc_first_load_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PmcSettings {
    pub tau_ctl_d: f32,
    pub tau_atl_d: f32,
    pub ctl0: f32,
    pub atl0: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Athlete {
    /// `male`, `female` oder `null`.
    pub sex: Option<String>,
    pub birth_year: Option<i32>,
    /// Herkunft: `garmin`, `manual` oder `null` (T39).
    pub sex_origin: Option<String>,
    pub birth_year_origin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Settings {
    /// Fester Stamm aller FIT-Dateien (`<lokaler Datenordner>/Lastwerk/fit`, T28). Der einzige
    /// Pfad, den das Frontend sieht, nur zur Anzeige: Es kann ihn nicht ändern.
    pub fit_root: String,
    /// Ergebnis der einmaligen Übernahme des früher gewählten FIT-Ordners, solange die App
    /// läuft, die sie ausgeführt hat. Nach einem Neustart `null`.
    pub fit_migration: Option<FitMigrationDto>,
    /// `de` oder `en`.
    pub locale: String,
    /// `dark`, `light` oder `system`.
    pub theme: String,
    pub pmc: PmcSettings,
    pub athlete: Athlete,
    /// Der MCP-Server darf Erholungswerte (HRV, Ruhepuls, Schlafdauer) ausgeben
    /// (`mcp.recovery_enabled` genau `true`, SPEC 9.6, O19). Standard aus.
    pub mcp_recovery_enabled: bool,
    /// Die Datenbank liegt in einem bekannten Sync-Ordner (SPEC 6.1): Warnung anzeigen.
    pub db_in_sync_folder: bool,
    /// Der Erststart-Assistent ist durchlaufen (`app.onboarded` genau `true`). Wird als
    /// Einstellung gespeichert, nicht im Browser, damit Backup und Neuinstallation es mitnehmen.
    pub onboarded: bool,
    /// Installierte Version der App. Kommt ohne Netzwerk aus den Paketdaten.
    pub version: String,
    /// Abgleich mit Garmin beim Start der App (`garmin.sync_on_start`, Standard an, T23).
    pub garmin_sync_on_start: bool,
    /// Hydration (S3) darf abgerufen werden (`garmin.share.hydration` genau `true`, T39).
    /// Standard aus.
    pub garmin_share_hydration: bool,
    /// Die abgeleiteten Werte stammen von einem älteren Programmstand (T33): Die Oberfläche
    /// liest beim Start einmal ein, damit neue Kennzahlen auch für alte Dateien entstehen.
    pub cache_outdated: bool,
}

/// Änderung der Einstellungen. Einen Ordner gibt es nicht mehr (T28): Der FIT-Stamm ist fest.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SettingsPatch {
    pub locale: Option<String>,
    pub theme: Option<String>,
    pub pmc: Option<PmcSettings>,
    /// Gibt die Erholungswerte für den MCP-Server frei oder sperrt sie. Nur ein JSON-Wahrheitswert
    /// ist gültig, Zahlen und Texte lehnt die Deserialisierung ab.
    pub mcp_recovery_enabled: Option<bool>,
    /// Merkt den Abschluss des Erststart-Assistenten. Nur ein JSON-Wahrheitswert ist gültig.
    pub onboarded: Option<bool>,
    /// Abgleich mit Garmin beim Start an oder aus (T23). Nur ein JSON-Wahrheitswert ist gültig.
    pub garmin_sync_on_start: Option<bool>,
    /// Gibt den Abruf der Hydration (S3) frei oder sperrt ihn (T39). Nur ein Wahrheitswert.
    pub garmin_share_hydration: Option<bool>,
}

/// Was der Austauschordner `exchange/out` hergibt (SPEC 5.4). Nur Anzahlen und der jüngste Tag,
/// nie ein Pfad und nie Inhalt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BridgeStatus {
    /// Mindestens eine Erholungsdatei oder FIT-Datei liegt vor.
    pub has_data: bool,
    pub recovery_days: u32,
    pub activity_files: u32,
    /// Jüngster Tag einer Erholungsdatei (`YYYY-MM-DD`), falls vorhanden.
    pub last_day: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AthleteInput {
    pub sex: Option<String>,
    pub birth_year: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThresholdItem {
    pub id: i64,
    /// `null` gilt für alle Sportarten.
    pub sport: Option<Sport>,
    pub kind: ThresholdKind,
    pub value: f64,
    pub valid_from: String,
    /// Letzter Tag der Gültigkeit, `null` ohne späteren Wert.
    pub valid_until: Option<String>,
    /// `manual` oder `suggested_accepted`.
    pub origin: String,
    pub note: Option<String>,
}

/// Maximale Herzfrequenz aus dem Alter (Q7), wenn kein Messwert vorliegt. Immer geschätzt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HrMaxEstimate {
    pub bpm: f32,
    pub estimated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThresholdList {
    pub items: Vec<ThresholdItem>,
    pub hr_max_estimate: Option<HrMaxEstimate>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ThresholdInput {
    /// `null` gilt für alle Sportarten.
    pub sport: Option<Sport>,
    pub kind: ThresholdKind,
    pub value: f64,
    pub valid_from: String,
    pub note: Option<String>,
}

/// Ergebnis der Neuberechnung nach einer Änderung von Schwelle oder Athletendaten.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Recompute {
    /// Es gab keinen Ordner, also nichts neu zu berechnen. Die Werte gelten erst nach einem
    /// Import mit gewähltem Ordner.
    pub skipped: bool,
    pub candidates: u32,
    pub recomputed: u32,
    pub file_missing: u32,
    pub file_changed: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThresholdSet {
    pub id: i64,
    pub recompute: Recompute,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AthleteSet {
    pub settings: Settings,
    pub recompute: Recompute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorCount {
    pub code: String,
    pub count: u32,
}

/// Zahlen der einmaligen Übernahme des früher gewählten FIT-Ordners in den festen Stamm (T28).
/// Nie ein Pfad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FitMigrationDto {
    /// Neu in den Stamm kopierte Dateien.
    pub copied: u32,
    /// Schon vorhanden und gleich, übersprungen.
    pub identical: u32,
    /// Am selben Pfad lag schon eine andere Datei, nichts überschrieben.
    pub conflicts: u32,
    /// Kopieren fehlgeschlagen. Die App versucht es beim nächsten Start erneut.
    pub failed: u32,
    /// Der alte Ordner war nicht (mehr) da.
    pub missing: bool,
}

impl From<lastwerk_cli::fit_migration::FitMigration> for FitMigrationDto {
    fn from(m: lastwerk_cli::fit_migration::FitMigration) -> Self {
        Self {
            copied: m.copied,
            identical: m.identical,
            conflicts: m.conflicts,
            failed: m.failed,
            missing: m.missing,
        }
    }
}

/// Ergebnis von `add_fit_files` (T28): nur Zahlen, nie ein Pfad.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AddFitFilesResult {
    /// Der Nutzer hat den Dialog abgebrochen.
    pub cancelled: bool,
    /// Gewählte Dateien.
    pub selected: u32,
    /// Neu im Stamm abgelegt (`manual/`).
    pub copied: u32,
    /// Schon im Stamm, gleiche Datei.
    pub already_present: u32,
    /// Nicht übernommen: falsche Endung, nicht lesbar oder über dem Größenlimit.
    pub rejected: u32,
    /// Import danach, falls etwas übernommen oder schon vorhanden war.
    pub import: Option<ImportSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportSummary {
    pub files_found: u32,
    pub new: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub not_activity: u32,
    pub error_count: u32,
    /// Fehlerhafte Dateien nach Fehlercode. Dateipfade gehen nicht an das Frontend.
    pub errors_by_code: Vec<ErrorCount>,
    pub duplicates: u32,
    pub primary: u32,
    pub loads_ok: u32,
    pub loads_missing_threshold: u32,
    pub loads_missing_channel: u32,
    pub loads_implausible: u32,
}

/// Fortschrittsereignis `import-progress`: `done` ist die Nummer (ab 1) der Datei in Arbeit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ImportProgress {
    pub done: u32,
    pub total: u32,
}

// --- Pläne, Ziele, Verfügbarkeit, Export (T09) ---------------------------------------------

/// Sportart, Sprache, Priorität und Plan, wie `plan.schema.json` sie schreibt. Das Frontend
/// nimmt nur Ausdauersportarten an (die Prüfung steht in `service::plans`).
pub use lastwerk_core::plan::{Goal as PlanGoal, Locale as PlanLocale, Plan, PlanSport, Priority};

/// Zeile der Planliste.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanListItem {
    pub id: String,
    /// RFC 3339 in UTC.
    pub created_at: String,
    /// `draft`, `active` oder `archived`.
    pub status: String,
    /// `rules` oder `llm`.
    pub source: String,
    pub locale: PlanLocale,
    pub rules_version: String,
    pub goal_id: Option<i64>,
    pub goal: PlanGoal,
    pub first_week: Option<String>,
    pub last_week: Option<String>,
    pub weeks: u32,
}

/// Verletzung einer Leitplanke bei der Prüfung. Text und Quelle schlägt die Oberfläche über
/// `rule.<code>.name` und `rule.<code>.explanation` nach.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ViolationDto {
    pub code: String,
    /// `true` bei einer harten Regel (der Plan würde abgelehnt).
    pub hard: bool,
    pub date: Option<String>,
    /// Parameter der Verletzung, Zahlen als Zeichenketten.
    pub detail: std::collections::BTreeMap<String, String>,
}

/// Ergebnis der Prüfung eines Plans gegen das Schema und die Leitplanken (F10).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanValidation {
    /// Schemaverletzungen als Zeilen aus Pfad im Plan und Meldung der Bibliothek. Leer heißt
    /// gültig.
    pub schema_errors: Vec<String>,
    pub violations: Vec<ViolationDto>,
}

/// Ein Plan mit Status, Vorausberechnung und Prüfung.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanDetail {
    /// `draft`, `active` oder `archived`.
    pub status: String,
    /// Der Plan nach `plan.schema.json`. Texte stehen nie darin, nur Schlüssel.
    pub plan: Plan,
    /// Fitness, Ermüdung und Form der Planwochen aus den geplanten Tageslasten.
    pub projection: Vec<PmcPointDto>,
    /// CTL und ATL am Tag vor dem Planbeginn, Startwerte der Vorausberechnung.
    pub ctl0: f32,
    pub atl0: f32,
    pub validation: PlanValidation,
}

/// Eingaben von `generate_plan`. Entspricht den Optionen von `lastwerk plan`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GeneratePlanInput {
    /// Montag der ersten Planwoche.
    pub week_start: String,
    /// Überschreibt das aktive Ziel der Datenbank.
    pub goal_date: Option<String>,
    pub goal_sport: Option<PlanSport>,
    /// 1 bis 26. Ohne Angabe: bis zur Woche des Termins, sonst 8.
    pub weeks: Option<u32>,
    /// Sprache der Texte im Plan.
    pub locale: PlanLocale,
    /// Leer heißt: Sportarten der letzten zwölf Wochen.
    pub sports: Vec<PlanSport>,
    /// 0 bis 2.
    pub strength_per_week: u8,
}

/// Ein gespeichertes Ziel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GoalItem {
    pub id: i64,
    /// `event` oder `continuous`.
    pub kind: String,
    pub event_date: Option<String>,
    pub sport: Option<PlanSport>,
    pub priority: Option<Priority>,
    /// Freitext, bleibt lokal.
    pub label: Option<String>,
    pub active: bool,
}

/// Neues Ziel. Mit `event_date` ein Termin, ohne ein fortlaufendes Ziel.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GoalInput {
    pub event_date: Option<String>,
    pub sport: Option<PlanSport>,
    pub priority: Option<Priority>,
    pub label: Option<String>,
}

/// Verfügbarkeit eines Wochentags (0 = Montag bis 6 = Sonntag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailabilityDay {
    pub weekday: u8,
    pub max_minutes: u16,
    pub allow_hard: bool,
    pub fixed_rest: bool,
}

/// Eingaben des Planers: Ziele und Verfügbarkeit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanningInputs {
    pub goals: Vec<GoalItem>,
    /// Immer sieben Einträge, Montag zuerst. Nicht gesetzte Tage tragen den Standard des
    /// Planers.
    pub availability: Vec<AvailabilityDay>,
    /// Wie viele der sieben Tage gesetzt sind.
    pub availability_set: u8,
    /// Standardwert in Minuten für nicht gesetzte Tage.
    pub default_minutes: u16,
}

/// Zahl der Zeilen je Tabelle eines Backups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BackupCounts {
    pub thresholds: u32,
    pub goals: u32,
    pub availability: u32,
    pub plans: u32,
    pub manual_sessions: u32,
    pub settings: u32,
    /// Erholungstage (ab T14 Teil der Sicherung). Eine Sicherung von vor T14 leert die Tabelle
    /// beim Einspielen, die Zahl 0 macht das sichtbar.
    pub recovery_days: u32,
}

/// Ergebnis eines Exports. `saved` ist `false`, wenn der Nutzer den Dialog abbricht. Der Pfad
/// der Datei geht nie an die Oberfläche.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportResult {
    pub saved: bool,
    /// Zahl der Veranstaltungen beim ICS-Export.
    pub events: Option<u32>,
    /// Zeilen je Tabelle beim Backup.
    pub counts: Option<BackupCounts>,
}

/// Ergebnis der Wiederherstellung. `restored` ist `false`, wenn der Nutzer den Dialog abbricht.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RestoreResult {
    pub restored: bool,
    pub counts: Option<BackupCounts>,
}

/// Ergebnis der Suche nach einem Update. Weder URL noch Pfad gehen an die Oberfläche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateInfo {
    /// Installierte Version.
    pub current: String,
    pub available: bool,
    /// Neue Version, wenn `available`.
    pub version: Option<String>,
}

// --- Dashboard (T25 Teil B) ------------------------------------------------------------
//
// Zahlen sind auf eine Nachkommastelle gerundet (Effizienzfaktor auf zwei, weil er um 1,5
// liegt). Fehlende Werte sind `None`, nie 0 (SPEC 5.4). Zustände sind die Codes aus
// `lastwerk_core::metrics` (`ramp.steep` und so weiter), Texte entstehen in der Oberfläche.

/// Form am heutigen Tag.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashForm {
    pub tsb: f64,
    pub ctl: f64,
    pub atl: f64,
    /// `form.fresh`, `form.balanced`, `form.loaded` oder `form.overloaded`.
    pub state: String,
}

/// Der aktive Plan, soweit das Dashboard ihn braucht.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashPlan {
    pub id: String,
    /// Montag der ersten und Sonntag der letzten Planwoche.
    pub first_day: String,
    pub last_day: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashRampPoint {
    pub date: String,
    /// CTL-Punkte je Woche.
    pub ramp: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashRamp {
    /// Rampe heute, `None` im ersten Verlaufswochenende (kein Vergleichstag).
    pub current: Option<f64>,
    /// `ramp.no_data`, `ramp.falling`, `ramp.steady`, `ramp.building`, `ramp.steep`.
    pub state: String,
    /// Obergrenze aus R1 in CTL-Punkten je Woche.
    pub limit: f64,
    /// Tägliche Rampe der letzten zwölf Wochen.
    pub series: Vec<DashRampPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashWeekShare {
    pub week_start: String,
    /// Anteil der Zeit in Zone 1 und 2 in Prozent, `None` ohne Zeit mit Zonen.
    pub easy_percent: Option<f64>,
    pub total_s: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashDistribution {
    /// Anteil über die letzten 28 Tage in Prozent.
    pub window_easy_percent: Option<f64>,
    pub window_total_s: u64,
    /// Grenze aus R8 in Prozent.
    pub min_easy_percent: f64,
    /// `distribution.no_data`, `distribution.easy_dominant`, `distribution.intensity_heavy`.
    pub state: String,
    /// Acht Wochen, die letzte ist die laufende.
    pub weeks: Vec<DashWeekShare>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashWeekAdherence {
    pub week_start: String,
    pub planned_total: f64,
    pub planned_to_date: f64,
    pub done_to_date: f64,
    pub complete: bool,
    pub ratio_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashAdherence {
    /// Verhältnis der jüngsten Woche mit Plan bis gestern, in Prozent.
    pub ratio_percent: Option<f64>,
    /// `adherence.no_data`, `adherence.under`, `adherence.on_track`, `adherence.over`.
    pub state: String,
    /// Wochen des aktiven Plans, leer ohne Plan.
    pub weeks: Vec<DashWeekAdherence>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashEfPoint {
    pub date: String,
    pub ef: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashEfficiency {
    /// Mittel der letzten 28 Tage (Watt je Schlag pro Minute).
    pub current: Option<f64>,
    /// Änderung gegenüber den 28 Tagen davor in Prozent.
    pub change_percent: Option<f64>,
    /// `efficiency.no_data`, `efficiency.falling`, `efficiency.flat`, `efficiency.rising`.
    pub trend: String,
    /// Radeinheiten mit Effizienzfaktor in den letzten 90 Tagen.
    pub sessions: u32,
    /// Gleitendes 28-Tage-Mittel je Tag der letzten 90 Tage, wo berechenbar.
    pub series: Vec<DashEfPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashPowerDuration {
    pub duration_s: u32,
    pub current_w: Option<f64>,
    pub previous_w: Option<f64>,
    pub change_percent: Option<f64>,
    /// `power_curve.no_data`, `power_curve.falling`, `power_curve.flat`, `power_curve.rising`.
    pub trend: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashPowerCurve {
    /// Je Dauer (5 s, 1 min, 5 min, 20 min, 1 h) die letzten 90 Tage gegen die 90 davor.
    pub durations: Vec<DashPowerDuration>,
}

/// Ein Erholungswert gegen die eigene Basis.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashRecoveryValue {
    pub value: f64,
    /// Mittel der Tage davor (Basislinie aus R12), `None` bei zu wenig Tagen.
    pub baseline: Option<f64>,
    /// Abweichung in Prozent der Basis, negativ heißt darunter.
    pub deviation_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashRecoveryPoint {
    pub date: String,
    pub hrv_ms: Option<f64>,
    pub resting_hr_bpm: Option<f64>,
    pub sleep_h: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashReadiness {
    /// `readiness.no_data`, `readiness.normal`, `readiness.low`.
    pub state: String,
    /// Jüngster Erholungstag überhaupt (auch wenn zu alt für eine Deutung).
    pub last_date: Option<String>,
    /// Tag, auf den sich die Abweichungen beziehen. `None` ohne aktuelle Daten.
    pub date: Option<String>,
    pub hrv: Option<DashRecoveryValue>,
    pub resting_hr: Option<DashRecoveryValue>,
    pub sleep_h: Option<DashRecoveryValue>,
    /// Body Battery des jüngsten Tages mit Wert (Spalten aus T22), ohne Deutung.
    pub body_battery_high: Option<f64>,
    pub body_battery_low: Option<f64>,
    /// Der aktive Plan hat die erste harte Einheit wegen der Erholungswerte ersetzt (R12) und
    /// diese Einheit liegt heute oder in den letzten sechs Tagen.
    pub plan_adjusted: bool,
    /// Letzte 60 Tage, nur Tage mit Eintrag.
    pub series: Vec<DashRecoveryPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashLoadDay {
    pub date: String,
    pub load: f64,
}

/// Alles, was "Heute" zeigt, in einem Aufruf.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Dashboard {
    pub today: String,
    pub form: Option<DashForm>,
    /// Verlauf der letzten 120 Tage bis heute.
    pub pmc: Vec<PmcPointDto>,
    pub pmc_first_load_date: Option<String>,
    /// Vorausberechnung der Tage nach heute aus der geplanten Last des aktiven Plans.
    pub projection: Vec<PmcPointDto>,
    pub plan: Option<DashPlan>,
    pub ramp: DashRamp,
    pub distribution: DashDistribution,
    pub adherence: DashAdherence,
    pub efficiency: DashEfficiency,
    pub power_curve: DashPowerCurve,
    pub readiness: DashReadiness,
    /// Last je Tag der letzten 365 Tage, nur Tage mit zählender Last.
    pub load_days: Vec<DashLoadDay>,
}

// --- Garmin (T23, SPEC 5.6) ------------------------------------------------------------------
//
// Kein Typ hier trägt Token, Ticket, Anzeigenamen des Kontos oder einen Pfad (SPEC 10.2). Ein
// Test in `tests/garmin.rs` prüft das an der Serialisierung.

/// Zustand der Verbindung ohne Netzzugriff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GarminStatus {
    /// Im Schlüsselbund liegen lesbare Token. Sagt nichts darüber, ob Garmin sie noch annimmt.
    pub connected: bool,
    /// Datum des letzten vollständigen Abgleichs (`YYYY-MM-DD`).
    pub last_sync: Option<String>,
    /// Abgleich beim Start der App (Standard an).
    pub sync_on_start: bool,
    /// Ende einer aktiven Sperre nach wiederholtem 429 oder 403 (ISO 8601, UTC).
    pub locked_until: Option<String>,
}

/// Ereignis `garmin-login` nach dem Anmeldefenster. `code` ist `connected` bei Erfolg, sonst
/// `cancelled`, `unexpected_redirect` oder ein Fehlercode von `lastwerk-garmin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GarminLoginEvent {
    pub ok: bool,
    pub code: String,
}

/// Fortschrittsereignis `garmin-sync-progress`. `phase` ist `activities`, `recovery` oder
/// `import`; `total` fehlt, solange die Zahl nicht bekannt ist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GarminSyncProgress {
    pub phase: String,
    pub done: u32,
    pub total: Option<u32>,
}

/// Abbruch durch Garmin: Code von `lastwerk-garmin` und bei `locked` das Ende der Sperre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GarminAbort {
    pub code: String,
    /// ISO 8601, UTC.
    pub until: Option<String>,
}

/// Ergebnis eines Abgleichs, ohne Pfade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GarminSyncSummary {
    /// Erster Tag des Fensters (`YYYY-MM-DD`).
    pub window_start: String,
    pub activities_listed: u32,
    /// Neu geladene FIT-Dateien.
    pub new_activities: u32,
    pub known_activities: u32,
    /// Vom Import neu aufgenommene oder geänderte Aktivitäten.
    pub imported: u32,
    pub import_errors: u32,
    pub recovery_days: u32,
    pub dropped_values: u32,
    pub calls: u32,
    /// Seit T39: geänderte Schwellen aus Garmin, danach neu gerechnete Aktivitäten, neue oder
    /// geänderte Zeilen der Tages-, Trainings- und S3-Werte und von Garmins Last.
    pub thresholds_changed: u32,
    pub recomputed: u32,
    pub wellness_days: u32,
    pub training_days: u32,
    pub sensitive_days: u32,
    pub garmin_loads: u32,
    /// Fehler einzelner Aufrufe je Code.
    pub errors_by_code: Vec<ErrorCount>,
    pub aborted: Option<GarminAbort>,
    pub last_sync_updated: bool,
}

/// Seite "Gesundheit" (T39): Tages- und Trainingswerte aus Garmin über 30, 90 oder 365 Tage.
/// Ein Tag ohne einen einzigen Wert fehlt; fehlende Werte sind `null`, nie 0 (SPEC 5.4).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HealthPage {
    pub days: u32,
    pub points: Vec<HealthPoint>,
    /// Die Hydration (S3) ist freigegeben. Sonst ist `hydration_ml` immer `null`.
    pub sensitive_released: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HealthPoint {
    pub date: String,
    pub steps: Option<f64>,
    pub step_distance_km: Option<f64>,
    pub hr_min_bpm: Option<f64>,
    pub hr_max_bpm: Option<f64>,
    pub respiration_waking_brpm: Option<f64>,
    pub respiration_sleep_brpm: Option<f64>,
    pub spo2_avg_pct: Option<f64>,
    pub spo2_low_pct: Option<f64>,
    pub intensity_moderate_min: Option<f64>,
    pub intensity_vigorous_min: Option<f64>,
    pub stress_avg: Option<f64>,
    pub stress_max: Option<f64>,
    pub readiness_score: Option<f64>,
    /// Garmins Stufe, zum Beispiel `HIGH`.
    pub readiness_level: Option<String>,
    /// Garmins akute Last, nur Anzeige (E2).
    pub garmin_acute_load: Option<f64>,
    pub fitness_age: Option<f64>,
    pub vo2max_running: Option<f64>,
    pub vo2max_cycling: Option<f64>,
    pub endurance_score: Option<f64>,
    pub hill_score: Option<f64>,
    pub hydration_ml: Option<f64>,
}
