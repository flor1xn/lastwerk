// Typen zwischen Oberfläche und Rust-Teil. Sie spiegeln app/src-tauri/src/dto.rs.
// Konvention: Feldnamen in snake_case, Daten als YYYY-MM-DD, fehlende Werte sind null (nie 0).

export const SPORTS = ['cycling', 'running', 'swimming', 'strength', 'other'] as const;
export type Sport = (typeof SPORTS)[number];

export const LOAD_STATUSES = [
  'ok',
  'missing_threshold',
  'missing_channel',
  'implausible',
  'unknown',
] as const;
export type LoadStatus = (typeof LOAD_STATUSES)[number];

export const LOAD_METHODS = ['tss', 'hr_tss', 'r_tss', 's_tss', 'session_rpe'] as const;
export type LoadMethod = (typeof LOAD_METHODS)[number];

export const THRESHOLD_KINDS = [
  'ftp_w',
  'lthr_bpm',
  'hr_max_bpm',
  'hr_rest_bpm',
  'run_speed_mps',
  'css_mps',
] as const;
export type ThresholdKind = (typeof THRESHOLD_KINDS)[number];

export type Locale = 'de' | 'en';
export type Theme = 'dark' | 'light' | 'system';

/** Belastung einer Aktivität. `value` ist nur bei Status `ok` gesetzt. */
export interface LoadInfo {
  status: LoadStatus;
  value: number | null;
  method: LoadMethod | null;
  intensity_factor: number | null;
  /** rTSS, sTSS und Session-RPE sind Faustregeln und gelten als geschätzt (SPEC 8.1). */
  estimated: boolean;
  /** Bei `missing_threshold` die fehlende Art, sonst null (T39, SPEC 8.6). */
  missing: MissingKind | null;
}

/** Was einer Last fehlen kann: eine Schwellenart oder das Geschlecht (T39). */
export type MissingKind =
  'ftp_w' | 'lthr_bpm' | 'hr_max_bpm' | 'hr_rest_bpm' | 'run_speed_mps' | 'css_mps' | 'sex';

export interface ActivityItem {
  id: number;
  local_date: string;
  start_utc: string;
  sport: Sport;
  sub_sport: string | null;
  duration_s: number;
  distance_m: number | null;
  avg_hr: number | null;
  avg_power_w: number | null;
  load: LoadInfo;
  is_primary: boolean;
  duplicate_of: number | null;
}

export interface ActivityFilter {
  from: string;
  to: string;
  sport: Sport | null;
}

export interface ZoneSchemeTime {
  id: string;
  basis: string;
  seconds: number[];
  /** Zonen beruhen auf einer aus dem Alter geschätzten maximalen Herzfrequenz (SPEC 8.5). */
  estimated: boolean;
  source: string;
  status: string;
}

export interface BestPower {
  duration_s: number;
  watts: number;
}

export type SeriesStatus = 'ok' | 'file_missing' | 'file_changed' | 'unreadable' | 'no_channels';

export interface Series {
  status: SeriesStatus;
  t_s: number[];
  power_w: (number | null)[];
  hr_bpm: (number | null)[];
  /** Geschwindigkeit in m/s (T31), für das Tempo bei Läufen. */
  speed_mps: (number | null)[];
}

export type DecouplingState = 'decoupling.no_data' | 'decoupling.coupled' | 'decoupling.decoupled';

/** Aerobe Entkopplung (T31, Q16 nur sekundär). `percent` null: Reihe zu kurz oder Kanal fehlt. */
export interface DetailDecoupling {
  percent: number | null;
  state: DecouplingState;
}

/** Kennzahlen einer Runde (T31). Mittel nur über vorhandene Werte, sonst null. */
export interface LapItem {
  start_s: number;
  elapsed_s: number;
  timer_s: number | null;
  distance_m: number | null;
  avg_power_w: number | null;
  avg_hr_bpm: number | null;
  avg_speed_mps: number | null;
}

export interface ActivityDetail {
  item: ActivityItem;
  moving_s: number | null;
  max_hr: number | null;
  np_w: number | null;
  device_product: string | null;
  zones: ZoneSchemeTime[];
  best_power: BestPower[];
  series: Series;
  /** NP durch mittlere Herzfrequenz, nur Rad ab 45 min. */
  efficiency_factor: number | null;
  decoupling: DetailDecoupling;
  /** Leer bei weniger als zwei Runden oder ohne lesbare Datei. */
  laps: LapItem[];
  /** Garmins eigene Last, nur Anzeige, nie Rechengrundlage (E2, T39). */
  garmin_load: GarminLoadInfo | null;
}

/** Garmins Trainingslast und Trainingseffekt (Schlüssel wie `AEROBIC_BASE`). */
export interface GarminLoadInfo {
  training_load: number | null;
  training_effect: string | null;
}

export interface SportLoad {
  sport: Sport;
  load: number;
}

export interface DayLoad {
  date: string;
  by_sport: SportLoad[];
  /** Zählende Aktivitäten ohne berechenbare Last. Sie erscheinen nicht als 0. */
  not_computable: number;
}

export interface PmcPoint {
  date: string;
  load: number;
  ctl: number;
  atl: number;
  tsb: number;
}

export interface WeekOverview {
  week_start: string;
  week_end: string;
  days: DayLoad[];
  totals_by_sport: SportLoad[];
  activities: number;
  not_computable: number;
  pmc: PmcPoint[];
  pmc_first_load_date: string | null;
}

// --- Dashboard (T25 Teil B), spiegelt Dashboard in dto.rs ---------------------------------
// Zahlen sind gerundet. Fehlende Werte sind null, nie 0. Zustände sind Codes aus
// lastwerk_core::metrics, Texte entstehen in der Oberfläche aus den Sprachdateien.

export const FORM_CODES = [
  'form.fresh',
  'form.balanced',
  'form.loaded',
  'form.overloaded',
] as const;
export type FormCode = (typeof FORM_CODES)[number];

export const RAMP_CODES = [
  'ramp.no_data',
  'ramp.falling',
  'ramp.steady',
  'ramp.building',
  'ramp.steep',
] as const;
export type RampCode = (typeof RAMP_CODES)[number];

export const DISTRIBUTION_CODES = [
  'distribution.no_data',
  'distribution.easy_dominant',
  'distribution.intensity_heavy',
] as const;
export type DistributionCode = (typeof DISTRIBUTION_CODES)[number];

export const ADHERENCE_CODES = [
  'adherence.no_data',
  'adherence.under',
  'adherence.on_track',
  'adherence.over',
] as const;
export type AdherenceCode = (typeof ADHERENCE_CODES)[number];

export const EFFICIENCY_CODES = [
  'efficiency.no_data',
  'efficiency.falling',
  'efficiency.flat',
  'efficiency.rising',
] as const;
export type EfficiencyCode = (typeof EFFICIENCY_CODES)[number];

export const POWER_CURVE_CODES = [
  'power_curve.no_data',
  'power_curve.falling',
  'power_curve.flat',
  'power_curve.rising',
] as const;
export type PowerCurveCode = (typeof POWER_CURVE_CODES)[number];

export const READINESS_CODES = ['readiness.no_data', 'readiness.normal', 'readiness.low'] as const;
export type ReadinessCode = (typeof READINESS_CODES)[number];

export interface DashForm {
  tsb: number;
  ctl: number;
  atl: number;
  state: FormCode;
}

export interface DashPlan {
  id: string;
  first_day: string;
  last_day: string;
}

export interface DashRamp {
  current: number | null;
  state: RampCode;
  /** Obergrenze aus R1 in CTL-Punkten je Woche. */
  limit: number;
  series: { date: string; ramp: number }[];
}

export interface DashWeekShare {
  week_start: string;
  /** Anteil der Zeit in Zone 1 und 2 in Prozent, null ohne Zeit mit Zonen. */
  easy_percent: number | null;
  total_s: number;
}

export interface DashDistribution {
  window_easy_percent: number | null;
  window_total_s: number;
  /** Grenze aus R8 in Prozent. */
  min_easy_percent: number;
  state: DistributionCode;
  weeks: DashWeekShare[];
}

export interface DashWeekAdherence {
  week_start: string;
  planned_total: number;
  planned_to_date: number;
  done_to_date: number;
  complete: boolean;
  ratio_percent: number | null;
}

export interface DashAdherence {
  ratio_percent: number | null;
  state: AdherenceCode;
  weeks: DashWeekAdherence[];
}

export interface DashEfficiency {
  current: number | null;
  change_percent: number | null;
  trend: EfficiencyCode;
  sessions: number;
  series: { date: string; ef: number }[];
}

export interface DashPowerDuration {
  duration_s: number;
  current_w: number | null;
  previous_w: number | null;
  change_percent: number | null;
  trend: PowerCurveCode;
}

export interface DashRecoveryValue {
  value: number;
  baseline: number | null;
  /** Prozent der Basis, negativ heißt darunter. */
  deviation_percent: number | null;
}

export interface DashRecoveryPoint {
  date: string;
  hrv_ms: number | null;
  resting_hr_bpm: number | null;
  sleep_h: number | null;
}

export interface DashReadiness {
  state: ReadinessCode;
  last_date: string | null;
  date: string | null;
  hrv: DashRecoveryValue | null;
  resting_hr: DashRecoveryValue | null;
  sleep_h: DashRecoveryValue | null;
  /** Kommt mit T22, bis dahin immer null. */
  body_battery_high: number | null;
  body_battery_low: number | null;
  /** Der aktive Plan hat wegen der Erholungswerte eine Einheit ersetzt (R12). */
  plan_adjusted: boolean;
  series: DashRecoveryPoint[];
}

export interface Dashboard {
  today: string;
  form: DashForm | null;
  pmc: PmcPoint[];
  pmc_first_load_date: string | null;
  projection: PmcPoint[];
  plan: DashPlan | null;
  ramp: DashRamp;
  distribution: DashDistribution;
  adherence: DashAdherence;
  efficiency: DashEfficiency;
  power_curve: { durations: DashPowerDuration[] };
  readiness: DashReadiness;
  load_days: { date: string; load: number }[];
}

export interface PmcSettings {
  tau_ctl_d: number;
  tau_atl_d: number;
  ctl0: number;
  atl0: number;
}

export type ValueOrigin = 'garmin' | 'manual';

export interface Athlete {
  sex: 'male' | 'female' | null;
  birth_year: number | null;
  /** Herkunft der Werte (T39). */
  sex_origin: ValueOrigin | null;
  birth_year_origin: ValueOrigin | null;
}

/** Was `exchange/out` hergibt (SPEC 5.4). Nur Anzahlen und der jüngste Tag, nie ein Pfad. */
export interface BridgeStatus {
  has_data: boolean;
  recovery_days: number;
  activity_files: number;
  last_day: string | null;
}

/** Zahlen der einmaligen Übernahme des früher gewählten FIT-Ordners (T28). Nie ein Pfad. */
export interface FitMigration {
  /** Neu in den festen Stamm kopierte Dateien. */
  copied: number;
  /** Schon vorhanden und gleich, übersprungen. */
  identical: number;
  /** Am selben Pfad lag schon eine andere Datei, nichts überschrieben. */
  conflicts: number;
  /** Kopieren fehlgeschlagen, die App versucht es beim nächsten Start erneut. */
  failed: number;
  /** Der alte Ordner war nicht (mehr) da. */
  missing: boolean;
}

/** Ergebnis von `addFitFiles` (T28): nur Zahlen, nie ein Pfad oder Dateiname. */
export interface AddFitFilesResult {
  /** Der Nutzer hat den Dateidialog abgebrochen. */
  cancelled: boolean;
  selected: number;
  /** Neu im festen Stamm abgelegt (`manual/`). */
  copied: number;
  already_present: number;
  /** Falsche Endung, nicht lesbar oder über dem Größenlimit des Imports. */
  rejected: number;
  /** Import danach, falls etwas übernommen oder schon vorhanden war. */
  import: ImportSummary | null;
}

export interface Settings {
  /** Fester Stamm aller FIT-Dateien (T28). Der einzige Pfad, den die Oberfläche sieht, nur zur
   * Anzeige: Sie kann ihn nicht ändern. */
  fit_root: string;
  /** Ergebnis der einmaligen Übernahme des früheren FIT-Ordners, solange die App läuft, die sie
   * ausgeführt hat. Nach einem Neustart `null`. */
  fit_migration: FitMigration | null;
  locale: Locale;
  theme: Theme;
  pmc: PmcSettings;
  athlete: Athlete;
  /** Der MCP-Server darf Erholungswerte ausgeben (SPEC 9.6, O19). Standard aus. */
  mcp_recovery_enabled: boolean;
  /** Die Datenbank liegt in einem bekannten Sync-Ordner (SPEC 6.1). */
  db_in_sync_folder: boolean;
  /** Der Erststart-Assistent ist durchlaufen. Gespeichert in der Datenbank, nicht im Browser. */
  onboarded: boolean;
  /** Installierte Version der App (ohne Netzwerk aus den Paketdaten). */
  version: string;
  /** Abgleich mit Garmin beim Start der App (Standard an, T23). */
  garmin_sync_on_start: boolean;
  /** Hydration (S3) darf aus Garmin abgerufen werden (T39). Standard aus. */
  garmin_share_hydration: boolean;
  /** Freigaben der übrigen S3-Bereiche (T40, Entscheidung E7). Standard aus. */
  garmin_share_body: boolean;
  garmin_share_blood_pressure: boolean;
  garmin_share_nutrition: boolean;
  /** Abgeleitete Werte von einem älteren Programmstand (T33): beim Start einmal einlesen. */
  cache_outdated: boolean;
}

/** Einen Ordner gibt es nicht mehr (T28): Der FIT-Stamm ist fest. */
export interface SettingsPatch {
  locale?: Locale;
  theme?: Theme;
  pmc?: PmcSettings;
  /** Gibt Erholungswerte für den MCP-Server frei oder sperrt sie. Nur ein Wahrheitswert gilt. */
  mcp_recovery_enabled?: boolean;
  /** Merkt den Abschluss des Erststart-Assistenten. */
  onboarded?: boolean;
  /** Abgleich mit Garmin beim Start an oder aus. Nur ein Wahrheitswert gilt. */
  garmin_sync_on_start?: boolean;
  /** Gibt den Abruf der Hydration (S3) frei oder sperrt ihn (T39). */
  garmin_share_hydration?: boolean;
  /** Freigaben der übrigen S3-Bereiche (T40). */
  garmin_share_body?: boolean;
  garmin_share_blood_pressure?: boolean;
  garmin_share_nutrition?: boolean;
}

/** Bereiche sensibler Werte (S3), je Bereich freigegeben und gelöscht (T40, E7). */
export const SENSITIVE_CATEGORIES = ['body', 'blood_pressure', 'nutrition', 'hydration'] as const;
export type SensitiveCategory = (typeof SENSITIVE_CATEGORIES)[number];

/** Einstellung, die einen S3-Bereich freigibt. */
export function shareKey(
  c: SensitiveCategory,
): `garmin_share_${SensitiveCategory}` & keyof Settings & keyof SettingsPatch {
  return `garmin_share_${c}`;
}

export interface AthleteInput {
  sex?: 'male' | 'female';
  birth_year?: number;
}

/** Herkunft eines Schwellenwerts. `garmin` seit T39. */
export type ThresholdOrigin = 'manual' | 'suggested_accepted' | 'garmin';

export interface ThresholdItem {
  id: number;
  /** null gilt für alle Sportarten. */
  sport: Sport | null;
  kind: ThresholdKind;
  value: number;
  valid_from: string;
  /** Letzter Tag der Gültigkeit, null ohne späteren Wert. */
  valid_until: string | null;
  origin: ThresholdOrigin;
  note: string | null;
}

export interface ThresholdList {
  items: ThresholdItem[];
  /** Maximale Herzfrequenz aus dem Alter (Q7). Immer geschätzt. */
  hr_max_estimate: { bpm: number; estimated: boolean } | null;
}

export interface ThresholdInput {
  sport: Sport | null;
  kind: ThresholdKind;
  value: number;
  valid_from: string;
  note: string | null;
}

export interface Recompute {
  /** Noch keine FIT-Dateien im Stamm: Die Werte gelten erst für später eingelesene. */
  skipped: boolean;
  candidates: number;
  recomputed: number;
  file_missing: number;
  file_changed: number;
}

export interface ThresholdSet {
  id: number;
  recompute: Recompute;
}

export interface AthleteSet {
  settings: Settings;
  recompute: Recompute;
}

export interface ImportSummary {
  files_found: number;
  new: number;
  updated: number;
  unchanged: number;
  not_activity: number;
  error_count: number;
  /** Fehlerhafte Dateien nach Fehlercode. Dateipfade gehen nicht an die Oberfläche. */
  errors_by_code: { code: string; count: number }[];
  duplicates: number;
  primary: number;
  loads_ok: number;
  loads_missing_threshold: number;
  loads_missing_channel: number;
  loads_implausible: number;
}

/** Nummer (ab 1) der Datei in Arbeit und Gesamtzahl. */
export interface ImportProgress {
  done: number;
  total: number;
}

// --- Pläne, Ziele, Verfügbarkeit, Export (T09) ---------------------------------------------
// Der Plan hat die Form von schema/plan.schema.json. Texte stehen nie im Plan, nur Schlüssel
// (template_id, reason_keys, Warnungscodes). Die Oberfläche schlägt sie in den Sprachdateien
// nach und zeigt sie als Text, nie als HTML (SPEC 9.5).

export const PLAN_SPORTS = ['cycling', 'running', 'swimming', 'strength', 'rest'] as const;
export type PlanSport = (typeof PLAN_SPORTS)[number];

/** Ausdauersportarten, die ein Ziel oder ein Plan wählen darf. */
export const ENDURANCE_SPORTS = ['cycling', 'running', 'swimming'] as const;
export type EnduranceSport = (typeof ENDURANCE_SPORTS)[number];

export const PHASES = ['base', 'build', 'peak', 'taper', 'recovery', 'race'] as const;
export type Phase = (typeof PHASES)[number];

export type Priority = 'A' | 'B' | 'C';
export type PlanStatus = 'draft' | 'active' | 'archived';
export type Slot = 'am' | 'pm';

export interface PlanGoal {
  kind: 'event' | 'continuous';
  event_date?: string;
  sport?: PlanSport;
  priority?: Priority;
}

export interface PlanSession {
  slot: Slot;
  sport: PlanSport;
  template_id: string;
  duration_min: number;
  planned_load: number;
  hard: boolean;
  rpe?: number;
  reason_keys?: string[];
}

export interface PlanDay {
  date: string;
  sessions: PlanSession[];
}

export interface PlanWeek {
  week_start: string;
  phase: Phase;
  target_load: number;
  planned_load?: number;
  reason_keys?: string[];
  days: PlanDay[];
}

export interface PlanWarning {
  /** Regelnummer, zum Beispiel R03. Text und Quelle stehen unter rule.<code>.* */
  code: string;
  severity: 'info' | 'warn';
  date?: string;
}

export interface Plan {
  schema_version: 1;
  plan_id: string;
  created_at: string;
  locale: Locale;
  source: 'rules' | 'llm';
  rules_version: string;
  goal: PlanGoal;
  weeks: PlanWeek[];
  warnings: PlanWarning[];
  /** Freitext des Sprachmodells. Wird nur als Text angezeigt (SPEC 9.5). */
  explanation?: string;
}

export interface PlanListItem {
  id: string;
  created_at: string;
  status: PlanStatus;
  source: 'rules' | 'llm';
  locale: Locale;
  rules_version: string;
  goal_id: number | null;
  goal: PlanGoal;
  first_week: string | null;
  last_week: string | null;
  weeks: number;
}

export interface Violation {
  code: string;
  /** Hart: Der Plan würde abgelehnt. */
  hard: boolean;
  date: string | null;
  detail: Record<string, string>;
}

export interface PlanValidation {
  /** Schemaverletzungen, leer heißt gültig. */
  schema_errors: string[];
  violations: Violation[];
}

export interface PlanDetail {
  status: PlanStatus;
  plan: Plan;
  /** Fitness, Ermüdung und Form der Planwochen aus den geplanten Tageslasten. */
  projection: PmcPoint[];
  /** Fitness und Ermüdung am Tag vor dem Planbeginn. */
  ctl0: number;
  atl0: number;
  validation: PlanValidation;
}

export interface GeneratePlanInput {
  /** Montag der ersten Planwoche. */
  week_start: string;
  /** Überschreibt das aktive Ziel. */
  goal_date: string | null;
  goal_sport: EnduranceSport | null;
  /** 1 bis 26. Ohne Angabe bis zum Termin, sonst 8. */
  weeks: number | null;
  locale: Locale;
  /** Leer heißt: Sportarten der letzten zwölf Wochen. */
  sports: EnduranceSport[];
  /** 0 bis 2. */
  strength_per_week: number;
}

export interface GoalItem {
  id: number;
  kind: 'event' | 'continuous';
  event_date: string | null;
  sport: PlanSport | null;
  priority: Priority | null;
  label: string | null;
  active: boolean;
}

/** Mit Datum ein Termin, ohne ein fortlaufendes Ziel. */
export interface GoalInput {
  event_date: string | null;
  sport: EnduranceSport | null;
  priority: Priority | null;
  label: string | null;
}

/** 0 = Montag bis 6 = Sonntag. */
export interface AvailabilityDay {
  weekday: number;
  max_minutes: number;
  allow_hard: boolean;
  fixed_rest: boolean;
}

export interface PlanningInputs {
  /** Neueste zuerst, höchstens eines ist aktiv. */
  goals: GoalItem[];
  /** Sieben Einträge, Montag zuerst. Nicht gesetzte Tage tragen den Standard. */
  availability: AvailabilityDay[];
  availability_set: number;
  default_minutes: number;
}

export interface BackupCounts {
  thresholds: number;
  goals: number;
  availability: number;
  plans: number;
  manual_sessions: number;
  settings: number;
  recovery_days: number;
}

/** `saved` ist false, wenn der Nutzer den Dateidialog abbricht. Der Pfad bleibt im Rust-Teil. */
export interface ExportResult {
  saved: boolean;
  events: number | null;
  counts: BackupCounts | null;
}

export interface RestoreResult {
  restored: boolean;
  counts: BackupCounts | null;
}

/** Ergebnis der Suche nach einem Update. Weder URL noch Pfad erreichen die Oberfläche. */
export interface UpdateInfo {
  current: string;
  available: boolean;
  version: string | null;
}

// --- Garmin (T23, SPEC 5.6) ----------------------------------------------------------------
// Kein Typ trägt Token, Ticket, Anzeigenamen des Kontos oder einen Pfad.

/** Zustand der Verbindung, ohne Netzzugriff. */
export interface GarminStatus {
  /** Im Schlüsselbund liegen Anmeldedaten. */
  connected: boolean;
  /** Letzter vollständiger Abgleich (YYYY-MM-DD). */
  last_sync: string | null;
  sync_on_start: boolean;
  /** Ende einer Sperre nach wiederholter Drosselung (ISO 8601, UTC). */
  locked_until: string | null;
}

/** Ereignis `garmin-login`: `code` ist `connected`, `cancelled`, `unexpected_redirect` oder ein
 * Fehlercode von lastwerk-garmin. */
export interface GarminLoginResult {
  ok: boolean;
  code: string;
}

export const GARMIN_SYNC_PHASES = [
  'activities',
  'recovery',
  'account',
  'wellness',
  'import',
] as const;
export type GarminSyncPhase = (typeof GARMIN_SYNC_PHASES)[number];

/** Ereignis `garmin-sync-progress`. `total` fehlt, solange die Zahl nicht bekannt ist. */
export interface GarminSyncProgress {
  phase: GarminSyncPhase;
  done: number;
  total: number | null;
}

export interface GarminAbort {
  code: string;
  until: string | null;
}

export interface GarminSyncSummary {
  window_start: string;
  activities_listed: number;
  new_activities: number;
  known_activities: number;
  imported: number;
  import_errors: number;
  recovery_days: number;
  dropped_values: number;
  calls: number;
  /** Seit T39: geänderte Schwellen aus Garmin und danach neu gerechnete Aktivitäten. */
  thresholds_changed: number;
  recomputed: number;
  wellness_days: number;
  training_days: number;
  sensitive_days: number;
  garmin_loads: number;
  errors_by_code: { code: string; count: number }[];
  aborted: GarminAbort | null;
  last_sync_updated: boolean;
}

/** Klick des Nutzers oder einmal beim Start der App. */
export type GarminSyncTrigger = 'manual' | 'start';
// --- Planer mit drei Fragen und Gespräch mit dem Coach (T26 Teil B) --------------------------
// Spiegelt app/src-tauri/src/service/quick.rs und coach.rs. Texte des Modells (`reply`,
// `explanation`) sind Anzeigetext und erscheinen nur als Text, nie als HTML (SPEC 9.5).

/** Antworten auf die drei Fragen. */
export interface QuickPlanInput {
  goal: {
    kind: 'event' | 'continuous';
    /** Nur bei `event`. */
    date: string | null;
    sport: EnduranceSport | null;
  };
  /** 1 bis 30. */
  hours_per_week: number;
  /** 0 = Montag bis 6 = Sonntag, mindestens einer. */
  days: number[];
  /** Montag der ersten Planwoche. */
  week_start: string;
  locale: Locale;
}

export type CoachReason = 'no_server' | 'no_model' | 'bad_url' | 'not_configured';

export interface CoachStatus {
  /** Ein Modell ist erreichbar und gewählt. */
  available: boolean;
  provider: 'ollama' | 'openai_compatible';
  /** Adresse des Modellservers (nur Loopback). */
  base_url: string;
  model: string | null;
  models: string[];
  reason: CoachReason | null;
}

export interface WeekFigures {
  minutes: number;
  /** Harte Einheiten in der Woche. */
  hard: number;
  load: number | null;
}

export interface WeekComparison {
  week_start: string;
  before: WeekFigures | null;
  after: WeekFigures | null;
}

export interface EditLine {
  op: 'replace' | 'move' | 'remove';
  date: string;
  slot: Slot;
  template_id: string | null;
  duration_min: number | null;
  to_date: string | null;
  to_slot: Slot | null;
}

export interface ChangeLine {
  availability: AvailabilityDay[] | null;
  sports: PlanSport[] | null;
  strength_per_week: number | null;
  weeks: number | null;
  /** Montag, ab dem der neue Plan beginnt. */
  week_start: string;
}

export interface ProposalSummary {
  weeks: WeekComparison[];
  edits: EditLine[];
  change: ChangeLine | null;
  warnings: number;
}

export interface ProposalPreview {
  id: string;
  kind: 'plan_edit' | 'planning_change';
  explanation: string | null;
  summary: ProposalSummary;
}

export interface CoachReply {
  reply: string;
  proposal: ProposalPreview | null;
  /** Regelcodes eines verworfenen Vorschlags, zum Beispiel R03. */
  rejected_codes: string[];
}

export interface PromptPreview {
  provider: { name: string; base_url: string; model: string | null };
  messages: { role: 'system' | 'user' | 'assistant'; content: string }[];
}

// --- Einrichtung des lokalen Coachs (T29, SPEC 9.2) -----------------------------------------
// Spiegelt app/src-tauri/src/service/coach_setup.rs. Kein Typ trägt einen Pfad oder den Text
// einer Fehlermeldung von Ollama, PowerShell oder winget.

export type CoachFit = 'fits' | 'slow' | 'too_big';
/** `running`: Ollama antwortet. `installed`: gefunden, antwortet nicht. `missing`: nicht da. */
export type OllamaState = 'running' | 'installed' | 'missing';

export interface CoachCatalogEntry {
  /** Name bei Ollama, zum Beispiel `qwen3:4b`. */
  name: string;
  size_gb: number;
  /** Schlüssel des Anzeigetextes. */
  label_key: string;
  starter: boolean;
  fit: CoachFit;
  /** Die Empfehlung für diesen Rechner. */
  recommended: boolean;
  installed: boolean;
}

/** Inhalt der Zustandsdatei des Einrichtungsskripts, nur geprüfte Felder. */
export interface CoachSetupFileState {
  decision: 'yes' | 'no' | null;
  ollama_found: boolean;
  installed_via: 'existing' | 'winget' | 'download' | null;
  model: string | null;
  model_ready: boolean;
  /** Code des Skripts oder `unknown`. */
  error_code: string | null;
  updated_at: string | null;
}

/** Ein laufender Auftrag der App. */
export interface CoachBusy {
  kind: 'install' | 'pull';
  /** Modell bei `pull`. */
  name: string | null;
  phase: CoachPullPhase | null;
  completed: number;
  total: number;
}

export interface CoachSetupStatus {
  ollama: OllamaState;
  /** Die App kann Ollama selbst einrichten (Windows, Skript vorhanden). */
  can_install: boolean;
  /** Modelle, die Ollama lokal hat. */
  models: string[];
  /** Das Modell, mit dem das Gespräch liefe. */
  model: string | null;
  hardware: { ram_gb: number; vram_gb: number | null };
  catalog: CoachCatalogEntry[];
  recommended: string | null;
  state: CoachSetupFileState | null;
  busy: CoachBusy | null;
}

export type CoachSetupPhase = 'detect' | 'install' | 'start' | 'pull' | 'done' | 'error';

/** Ereignis `coach-setup-progress`: eine Zeile des Einrichtungsskripts. `value` ist ein fester
 * Wert der Phase, bei `pull` mit Prozent `progress`. */
export interface CoachSetupProgress {
  phase: CoachSetupPhase;
  value: string;
  percent: number | null;
}

export type CoachPullPhase = 'manifest' | 'download' | 'verify' | 'write' | 'done' | 'other';

/** Ereignis `coach-pull-progress`. `total` 0 heißt: noch unbekannt. Bytes. */
export interface CoachPullProgress {
  name: string;
  phase: CoachPullPhase;
  completed: number;
  total: number;
}

/** Fehler eines Befehls: `message_key` verweist auf einen Text in den Sprachdateien. */
export class BackendError extends Error {
  readonly code: string;
  readonly messageKey: string;

  constructor(code: string, messageKey: string) {
    super(code);
    this.name = 'BackendError';
    this.code = code;
    this.messageKey = messageKey;
  }
}

/** Alles, was die Oberfläche vom Rust-Teil braucht. Zwei Umsetzungen: Tauri und Attrappe. */
export const PERF_RANGES = ['42', '90', '365', 'all'] as const;
export type PerfRange = (typeof PERF_RANGES)[number];

/** Bestwert mit Herkunft (T33). */
export interface PerfBest {
  value: number;
  date: string;
  activity_id: number;
}

/** Eine Dauer der Leistungskurve im Zeitraum gegen den Vorzeitraum (T33). */
export interface PerfPower {
  duration_s: number;
  current: PerfBest | null;
  previous_w: number | null;
  change_percent: number | null;
  trend: PowerCurveCode;
}

/** Bestzeit über eine Strecke (T37): `current.value` und `previous_s` in Sekunden,
 * `change_percent` negativ heißt schneller. */
export interface PerfRunBest {
  distance_m: number;
  current: PerfBest | null;
  previous_s: number | null;
  change_percent: number | null;
}

/** Gesetzter Schwellenwert im Verlauf (T36). */
export interface PerfThreshold {
  kind: ThresholdKind;
  sport: Sport | null;
  value: number;
  valid_from: string;
  origin: ThresholdOrigin;
}

/** FTP-Vorschlag eines Monats aus der besten 20-Minuten-Leistung (T36, SPEC 8.6). */
export interface PerfFtpSuggestion {
  /** YYYY-MM */
  month: string;
  value_w: number;
  best_20min_w: number;
  date: string;
  activity_id: number;
}

/** Grundlage der Seite "Leistung" (T33). `from` fehlt beim Zeitraum `all`. */
export interface Performance {
  today: string;
  range: PerfRange;
  from: string | null;
  previous_from: string | null;
  power: PerfPower[];
  /** Bestzeiten im Laufen über 1 km bis Marathon (T37). */
  run_bests: PerfRunBest[];
  /** FTP, LTHR, Laufschwelle und CSS bis heute, nach Art und Datum (T36). */
  thresholds: PerfThreshold[];
  /** FTP-Vorschläge je Monat im Zeitraum, ältester zuerst (T36). */
  ftp_suggestions: PerfFtpSuggestion[];
}

export const RECOVERY_RANGES = [30, 90, 365] as const;
export type RecoveryRange = (typeof RECOVERY_RANGES)[number];

/** Ein Tag der Seite "Erholung" (T34). Basis und Untergrenze aus R12, `low`: R12 würde auslösen. */
export interface RecoveryPagePoint {
  date: string;
  hrv_ms: number | null;
  resting_hr_bpm: number | null;
  sleep_h: number | null;
  sleep_score: number | null;
  body_battery_high: number | null;
  body_battery_low: number | null;
  hrv_base: number | null;
  hrv_floor: number | null;
  resting_hr_base: number | null;
  sleep_base_h: number | null;
  sleep_floor_h: number | null;
  low: boolean;
  load: number | null;
}

export type RecoveryMetric = 'hrv' | 'resting_hr' | 'sleep' | 'sleep_score' | 'body_battery_high';

export interface RecoverySummary {
  metric: RecoveryMetric;
  recent: number | null;
  period: number | null;
}

/** Grundlage der Seite "Erholung" (T34). */
export interface RecoveryPage {
  today: string;
  days: RecoveryRange;
  state: ReadinessCode;
  last_date: string | null;
  low_days: number;
  hrv_drop_percent: number | null;
  sleep_deficit_percent: number | null;
  points: RecoveryPagePoint[];
  summary: RecoverySummary[];
}

export const HEALTH_RANGES = [30, 90, 365] as const;
export type HealthRange = (typeof HEALTH_RANGES)[number];

/** Ein Tag der Seite "Gesundheit" (T39). Fehlende Werte sind null, nie 0. */
export interface HealthPoint {
  date: string;
  steps: number | null;
  step_distance_km: number | null;
  hr_min_bpm: number | null;
  hr_max_bpm: number | null;
  respiration_waking_brpm: number | null;
  respiration_sleep_brpm: number | null;
  spo2_avg_pct: number | null;
  spo2_low_pct: number | null;
  intensity_moderate_min: number | null;
  intensity_vigorous_min: number | null;
  stress_avg: number | null;
  stress_max: number | null;
  readiness_score: number | null;
  /** Garmins Stufe, zum Beispiel `HIGH`. */
  readiness_level: string | null;
  /** Garmins akute Last, nur Anzeige (E2). */
  garmin_acute_load: number | null;
  fitness_age: number | null;
  vo2max_running: number | null;
  vo2max_cycling: number | null;
  endurance_score: number | null;
  hill_score: number | null;
  hydration_ml: number | null;
  // Seit T40.
  floors_up: number | null;
  floors_down: number | null;
  /** Garmins Trainingsstatus als Schlüssel, zum Beispiel `productive`. */
  training_status: string | null;
  garmin_chronic_load: number | null;
  garmin_acwr: number | null;
  load_aerobic_low: number | null;
  load_aerobic_low_min: number | null;
  load_aerobic_low_max: number | null;
  load_aerobic_high: number | null;
  load_aerobic_high_min: number | null;
  load_aerobic_high_max: number | null;
  load_anaerobic: number | null;
  load_anaerobic_min: number | null;
  load_anaerobic_max: number | null;
  /** Garmins Hinweis zum Belastungsfokus als Schlüssel. */
  load_balance: string | null;
  race_5k_s: number | null;
  race_10k_s: number | null;
  race_half_s: number | null;
  race_marathon_s: number | null;
  run_acute_distance_km: number | null;
  run_impact_load: number | null;
  run_tolerance_km: number | null;
  run_tolerance_level: string | null;
  weight_kg: number | null;
  bmi: number | null;
  body_fat_pct: number | null;
  body_water_pct: number | null;
  bone_kg: number | null;
  muscle_kg: number | null;
  systolic_mmhg: number | null;
  diastolic_mmhg: number | null;
  pulse_bpm: number | null;
  calories_kcal: number | null;
  carbs_g: number | null;
  fat_g: number | null;
  protein_g: number | null;
  calories_goal_kcal: number | null;
}

/** Textfelder eines Tages der Seite "Gesundheit" (Garmins Schlüssel). */
export type HealthTextKey =
  'readiness_level' | 'training_status' | 'load_balance' | 'run_tolerance_level';

/** Zahlenfelder eines Tages der Seite "Gesundheit". */
export type HealthNumberKey = Exclude<keyof HealthPoint, 'date' | HealthTextKey>;

/** Grundlage der Seite "Gesundheit" (T39). Nur Tage mit mindestens einem Wert, aufsteigend. */
export interface HealthPage {
  days: number;
  points: HealthPoint[];
  /** Mindestens ein S3-Bereich ist freigegeben. */
  sensitive_released: boolean;
  /** Freigegebene S3-Bereiche; die Werte der übrigen sind immer null (T40). */
  released: SensitiveCategory[];
}

/** Summen eines Tages und einer Sportart (T32). `load` nur aus berechenbaren Lasten. */
export interface ProgressEntry {
  date: string;
  sport: Sport;
  count: number;
  duration_s: number;
  distance_m: number;
  load: number;
  load_missing: number;
}

/** Grundlage der Ansicht "Fortschritt": Tageszeilen von `from` bis `today`. */
export interface Progress {
  today: string;
  from: string;
  entries: ProgressEntry[];
}

export interface Backend {
  readonly kind: 'tauri' | 'mock';
  getSettings(): Promise<Settings>;
  setSettings(patch: SettingsPatch): Promise<Settings>;
  /** Liest den festen FIT-Stamm ein ("Neu einlesen"). */
  importFolder(onProgress: (p: ImportProgress) => void): Promise<ImportSummary>;
  /** Öffnet die Dateiauswahl (nur `.fit`, mehrere), legt die Dateien im Stamm ab und liest sie
   * ein. Der Dialog läuft im Rust-Teil, die Oberfläche bekommt nur Zahlen (T28). */
  addFitFiles(onProgress: (p: ImportProgress) => void): Promise<AddFitFilesResult>;
  getWeekOverview(weekStart: string, pmcWeeks: number): Promise<WeekOverview>;
  /** Alle Kennzahlen der Startseite; `today` ist der lokale Tag. */
  getDashboard(today: string): Promise<Dashboard>;
  /** Tagessummen für "Fortschritt" (T32), Vorjahr und 24 Monate bis `today`. */
  getProgress(today: string): Promise<Progress>;
  /** Leistungskurve und Bestwerte für "Leistung" (T33). */
  getPerformance(today: string, range: PerfRange): Promise<Performance>;
  /** Erholungswerte mit Basislinie und Tageslast für "Erholung" (T34). */
  getRecovery(today: string, days: RecoveryRange): Promise<RecoveryPage>;
  /** Tages- und Trainingswerte aus Garmin für "Gesundheit" (T39). */
  getHealth(today: string, days: HealthRange): Promise<HealthPage>;
  /** Löscht die S3-Werte eines Bereichs, ohne Angabe aller Bereiche, und liefert die Zahl
   * gelöschter Tage (T39, je Bereich seit T40). */
  deleteSensitiveData(category?: SensitiveCategory): Promise<number>;
  listActivities(filter: ActivityFilter): Promise<ActivityItem[]>;
  getActivity(id: number): Promise<ActivityDetail>;
  listThresholds(): Promise<ThresholdList>;
  setThreshold(input: ThresholdInput): Promise<ThresholdSet>;
  setAthlete(input: AthleteInput): Promise<AthleteSet>;
  listPlans(): Promise<PlanListItem[]>;
  getPlan(id: string): Promise<PlanDetail>;
  /** Erzeugt einen Plan und speichert ihn als Entwurf. */
  generatePlan(input: GeneratePlanInput): Promise<PlanDetail>;
  setPlanStatus(id: string, status: PlanStatus): Promise<PlanListItem[]>;
  getPlanningInputs(): Promise<PlanningInputs>;
  setGoal(input: GoalInput): Promise<PlanningInputs>;
  setAvailability(days: AvailabilityDay[]): Promise<PlanningInputs>;
  /** Die drei Fragen: schreibt Ziel und Verfügbarkeit, erzeugt den Plan und aktiviert ihn. */
  quickPlan(input: QuickPlanInput): Promise<PlanDetail>;
  /** Ob ein lokales Modell erreichbar ist und welche es gibt. */
  coachStatus(): Promise<CoachStatus>;
  /** Merkt das Modell. Leer heißt: das Programm wählt eines. */
  coachSetModel(model: string): Promise<CoachStatus>;
  /** Der exakte Eingang der nächsten Nachricht (F24). Sendet nichts. */
  coachPreview(message: string): Promise<PromptPreview>;
  /** Ein Gesprächsschritt. Höchstens einer gleichzeitig (Fehler `busy`). */
  coachChat(message: string): Promise<CoachReply>;
  /** Übernimmt einen Vorschlag und aktiviert den neuen Plan. */
  coachApply(proposalId: string): Promise<PlanDetail>;
  /** Beginnt das Gespräch von vorn (Verlauf und offene Vorschläge weg). */
  coachReset(): Promise<void>;
  /** Bricht das laufende Gespräch ab, der Verlauf bleibt. */
  coachCancel(): Promise<void>;
  /** Zustand der Einrichtung: Ollama, Modelle, Hardware, Katalog mit Einstufung (T29). */
  coachSetupStatus(): Promise<CoachSetupStatus>;
  /** Richtet Ollama über das gebündelte Skript ein (nur Windows). Fehler `coach_setup_*`. */
  coachInstallOllama(onProgress: (p: CoachSetupProgress) => void): Promise<CoachSetupStatus>;
  /** Lädt ein Modell aus dem Katalog oder ein installiertes und merkt es als Modell des
   * Coachs. Fehler `coach_pull_*`, `coach_setup_busy`. */
  coachPullModel(
    name: string,
    onProgress: (p: CoachPullProgress) => void,
  ): Promise<CoachSetupStatus>;
  /** Bricht ein laufendes Laden ab. */
  coachPullCancel(): Promise<void>;
  /** Die folgenden vier öffnen einen Dateidialog im Rust-Teil. */
  exportPlanIcs(id: string): Promise<ExportResult>;
  exportPlanJson(id: string): Promise<ExportResult>;
  exportBackup(): Promise<ExportResult>;
  /** `confirmed` muss true sein: Die Wiederherstellung ersetzt alle Nutzerdaten. */
  restoreBackup(confirmed: boolean): Promise<RestoreResult>;
  /** Liest nur Dateinamen im Austauschordner: liefert das Zusatzprogramm Daten? */
  getBridgeStatus(): Promise<BridgeStatus>;
  /** Fragt nur auf Klick nach einer neuen Version. Kein Hintergrundcheck (SPEC 10.1). */
  checkUpdate(): Promise<UpdateInfo>;
  /** Lädt und installiert, danach startet die App neu. `confirmed` muss true sein. */
  installUpdate(confirmed: boolean): Promise<void>;
  // Garmin (T23)
  garminStatus(): Promise<GarminStatus>;
  /** Öffnet das Anmeldefenster von Garmin und wartet auf das Ergebnis. */
  garminLogin(): Promise<GarminLoginResult>;
  /** `null`, wenn der Abgleich beim Start nicht läuft (nicht verbunden, ausgeschaltet, schon
   * gelaufen). Läuft schon einer, Fehler `busy`. */
  garminSync(
    trigger: GarminSyncTrigger,
    onProgress: (p: GarminSyncProgress) => void,
  ): Promise<GarminSyncSummary | null>;
  /** Löscht die Anmeldedaten im Schlüsselbund. */
  garminLogout(): Promise<void>;
}
