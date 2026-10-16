// Attrappe des Backends mit synthetischen Daten. Aktiv, wenn die Oberfläche nicht in Tauri
// läuft (Browser, Vitest, Playwright). Die Daten sind frei erfunden und deterministisch
// (Zufallszahlen aus festem Startwert), sie stammen von keiner echten Person.
//
// Die Attrappe verhält sich wie der Rust-Teil in den Punkten, die die Oberfläche sichtbar
// macht: Ohne passenden Schwellenwert ist die Last "nicht berechenbar" (nie 0), eine neue
// Schwelle gilt ab ihrem Datum bis zur nächsten, Duplikate zählen nicht in die Wochenlast,
// geschätzte Werte tragen die Kennzeichnung. Die Formeln hier sind vereinfacht und nur für die
// Anzeige gedacht. Maßgeblich sind `lastwerk-core` und `lastwerk-store`.

import { addDays, daysBetween, fromIso, isIsoDate, localIso, mondayOf, toIso } from '../dates';
import { MockCoach } from './mock-coach';
import { MockCoachSetup, type MockSetupOptions, type OllamaVariant } from './mock-coach-setup';
import { buildDashboard, type DashboardVariant } from './mock-dashboard';
import { buildPerformance } from './mock-performance';
import { buildProgress } from './mock-progress';
import { buildRecovery } from './mock-recovery';
import { buildHealth } from './mock-health';
import { MockPlanning, type MockCall } from './mock-plans';
import {
  BackendError,
  PERF_RANGES,
  RECOVERY_RANGES,
  HEALTH_RANGES,
  SPORTS,
  type ActivityDetail,
  type ActivityFilter,
  type ActivityItem,
  type AthleteInput,
  type AddFitFilesResult,
  type AthleteSet,
  type AvailabilityDay,
  type Backend,
  type BestPower,
  type CoachPullProgress,
  type CoachReply,
  type CoachSetupProgress,
  type CoachSetupStatus,
  type CoachStatus,
  type DayLoad,
  type ExportResult,
  type FitMigration,
  type GeneratePlanInput,
  type GoalInput,
  type ImportProgress,
  type ImportSummary,
  type LoadInfo,
  type LapItem,
  type LoadMethod,
  type PlanDetail,
  type PlanListItem,
  type PlanStatus,
  type PlanningInputs,
  type PmcPoint,
  type PmcSettings,
  type Performance,
  type PerfRange,
  type Progress,
  type RecoveryPage,
  type RecoveryRange,
  type HealthPage,
  type HealthRange,
  type MissingKind,
  type ThresholdOrigin,
  type PromptPreview,
  type QuickPlanInput,
  type Recompute,
  type RestoreResult,
  type BridgeStatus,
  type Dashboard,
  type UpdateInfo,
  type Series,
  type Settings,
  type SettingsPatch,
  type Sport,
  type SportLoad,
  type ThresholdInput,
  type ThresholdItem,
  type ThresholdKind,
  type ThresholdList,
  type ThresholdSet,
  type WeekOverview,
  type ZoneSchemeTime,
  type GarminLoginResult,
  type GarminStatus,
  type GarminSyncProgress,
  type GarminSyncSummary,
  type GarminSyncTrigger,
} from './types';

export interface MockOptions {
  /** Bezugstag der Daten (YYYY-MM-DD). Standard: heute. */
  today?: string;
  /** Pause je Schritt des Imports in Millisekunden. Standard 0. */
  importStepMs?: number;
  /** Erststart schon durchlaufen? Standard ja, der Test des Assistenten setzt false. */
  onboarded?: boolean;
  /** Variante des Dashboards (siehe mock-dashboard.ts). Standard `rich`. */
  dashboard?: DashboardVariant;
  /** Mit Garmin verbunden? Standard nein (T23). */
  garminConnected?: boolean;
  /** Läuft ein lokales Modell? `false` ist der Zustand ohne Ollama. Standard ja. */
  coach?: boolean;
  /** Wartezeit je Antwort des Coachs in Millisekunden. Standard 0. */
  coachDelayMs?: number;
  /** Ergebnis der Übernahme des früheren FIT-Ordners, das die Einstellungen melden (T29: auch
   * die Einrichtung von Ollama). */
  fitMigration?: FitMigration;
  /** Zustand von Ollama in der Einrichtung (T29). Standard `running`, mit `coach: false`
   * `missing`. Überschreibt `coach`. */
  ollama?: OllamaVariant;
  /** Pause je Schritt der Einrichtung und des Ladens in Millisekunden. Standard 0. */
  setupStepMs?: number;
  /** Hardware des erfundenen Rechners für die Empfehlung. */
  setupHardware?: MockSetupOptions['hardware'];
  /** Kann die Attrappe Ollama selbst einrichten (Windows)? Standard ja. */
  setupCanInstall?: boolean;
  /** Abgeleitete Werte veraltet (T33): Die Oberfläche liest beim Start ein. Standard nein. */
  cacheOutdated?: boolean;
  /** Ein aktiver Plan, der vor drei Wochen begann (T35: Kalender mit erledigten und verpassten
   * Einheiten). Standard nein. */
  runningPlan?: boolean;
}

/** Zufallszahlen mit festem Startwert (mulberry32), damit die Attrappe deterministisch bleibt. */
export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Verlauf von Fitness, Ermüdung und Form wie in SPEC 8.2:
 * CTL_t = CTL_{t-1} + (L_t - CTL_{t-1}) * (1 - exp(-1 / tau)), TSB_t = CTL_{t-1} - ATL_{t-1}.
 * Tage ohne Eintrag in `daily` zählen als Last 0. Nur für die Attrappe, die Rechnung der App
 * steht in `lastwerk-core`.
 */
export function computePmc(
  daily: ReadonlyMap<string, number>,
  from: string,
  to: string,
  p: PmcSettings,
): PmcPoint[] {
  const weight = (tau: number) => (tau > 0 && Number.isFinite(tau) ? 1 - Math.exp(-1 / tau) : 1);
  const wc = weight(p.tau_ctl_d);
  const wa = weight(p.tau_atl_d);
  const out: PmcPoint[] = [];
  let ctl = p.ctl0;
  let atl = p.atl0;
  for (let d = from; d <= to; d = addDays(d, 1)) {
    const load = daily.get(d) ?? 0;
    const tsb = ctl - atl;
    ctl += (load - ctl) * wc;
    atl += (load - atl) * wa;
    out.push({ date: d, load, ctl, atl, tsb });
  }
  return out;
}

interface RawActivity {
  id: number;
  date: string;
  start_utc: string;
  sport: Sport;
  duration_s: number;
  distance_m: number | null;
  avg_hr: number | null;
  max_hr: number | null;
  avg_power_w: number | null;
  np_w: number | null;
  /** Messkanal, aus dem die Last folgt. */
  channel: 'power' | 'hr' | 'speed' | 'none';
  /** Geschwindigkeit oder Anteil, nur für die erfundenen Lastwerte. */
  speed_mps: number | null;
  is_primary: boolean;
  duplicate_of: number | null;
  implausible: boolean;
  seed: number;
}

interface MockThreshold {
  id: number;
  sport: Sport | null;
  kind: ThresholdKind;
  value: number;
  valid_from: string;
  note: string | null;
  origin: ThresholdOrigin;
}

const ZONE_META: Record<string, { source: string; status: string; basis: string; zones: number }> =
  {
    power7: { source: 'Q16', status: 'Sekundärquelle', basis: 'ftp', zones: 7 },
    hr_lthr5: { source: 'Q22', status: 'Faustregel', basis: 'lthr', zones: 5 },
    hr_karvonen: { source: 'Q14', status: 'Faustregel', basis: 'hr_reserve', zones: 5 },
    run_speed5: {
      source: 'Faustregel',
      status: 'Faustregel',
      basis: 'run_threshold_speed',
      zones: 5,
    },
    swim_css5: { source: 'Q15', status: 'Faustregel', basis: 'css', zones: 5 },
  };

const BEST_POWER_FACTORS: [number, number][] = [
  [5, 2.2],
  [60, 1.6],
  [300, 1.3],
  [1200, 1.12],
  [3600, 1.03],
];

const ESTIMATED_METHODS: LoadMethod[] = ['r_tss', 's_tss', 'session_rpe'];

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

function err(code: string): BackendError {
  return new BackendError(code, `ui.error.${code}`);
}

const round1 = (v: number) => Math.round(v * 10) / 10;

export class MockBackend implements Backend {
  readonly kind = 'mock' as const;

  private readonly today: string;
  private readonly importStepMs: number;
  private readonly activities: RawActivity[];
  private thresholds: MockThreshold[];
  private nextThresholdId = 1;
  private imported = false;
  private settings: Settings;
  private readonly planning: MockPlanning;
  private readonly coach: MockCoach;
  private readonly coachSetup: MockCoachSetup;
  /** Protokoll der Aufrufe mit Dateidialog (Export, Wiederherstellung). Tests lesen es aus. */
  readonly calls: MockCall[] = [];
  /** Welche erfundenen Dashboard-Daten `getDashboard` liefert. Tests setzen sie um. */
  dashboardVariant: DashboardVariant;

  constructor(options: MockOptions = {}) {
    this.today = options.today ?? localIso();
    this.importStepMs = options.importStepMs ?? 0;
    this.dashboardVariant = options.dashboard ?? 'rich';
    this.activities = buildActivities(this.today);
    this.thresholds = [];
    const year = Number(this.today.slice(0, 4));
    this.settings = {
      fit_root: 'attrappe/Lastwerk/fit',
      fit_migration: options.fitMigration ?? null,
      locale: 'de',
      theme: 'dark',
      pmc: { tau_ctl_d: 42, tau_atl_d: 7, ctl0: 0, atl0: 0 },
      athlete: {
        sex: 'male',
        birth_year: year - 38,
        sex_origin: 'garmin',
        birth_year_origin: 'garmin',
      },
      mcp_recovery_enabled: false,
      db_in_sync_folder: false,
      onboarded: options.onboarded ?? true,
      version: '0.0.0',
      garmin_sync_on_start: true,
      garmin_share_hydration: false,
      cache_outdated: options.cacheOutdated ?? false,
    };
    this.garmin.connected = options.garminConnected ?? false;
    // Rad hat einen FTP, der später steigt. Lauf, Schwimmen und Herzfrequenz-Schwelle fehlen
    // mit Absicht: So zeigt die Oberfläche den Zustand "nicht berechenbar". Der erste FTP und
    // der Ruhepuls stammen aus Garmin (T39), der spätere FTP ist ein eigener Wert.
    this.addThreshold('cycling', 'ftp_w', 250, addDays(this.today, -200), null, 'garmin');
    this.addThreshold('cycling', 'ftp_w', 260, addDays(this.today, -60), 'Test nach Aufbauphase');
    this.addThreshold(null, 'hr_rest_bpm', 50, addDays(this.today, -200), null, 'garmin');
    this.planning = new MockPlanning({
      today: this.today,
      pmcAt: (date) => this.pmcAt(date),
      pmcSettings: () => this.settings.pmc,
      thresholdCount: () => this.thresholds.length,
      settingCount: () => 5,
      computePmc,
      log: (call) => this.calls.push(call),
    });
    if (options.runningPlan) {
      const detail = this.planning.generate({
        week_start: addDays(mondayOf(this.today), -21),
        goal_date: null,
        goal_sport: null,
        weeks: 8,
        locale: 'de',
        sports: [],
        strength_per_week: 1,
      });
      this.planning.setStatus(detail.plan.plan_id, 'active');
    }
    this.coach = new MockCoach(this.planning, {
      available: options.coach ?? true,
      delayMs: options.coachDelayMs ?? 0,
      today: this.today,
      locale: () => this.settings.locale,
    });
    this.coachSetup = new MockCoachSetup(this.coach, {
      variant: options.ollama ?? ((options.coach ?? true) ? 'running' : 'missing'),
      stepMs: options.setupStepMs ?? 0,
      hardware: options.setupHardware,
      canInstall: options.setupCanInstall,
    });
  }

  /** Fitness und Ermüdung am Ende des Tages mit den Einstellungen des Verlaufs, wie der Planer. */
  private pmcAt(date: string): { ctl: number; atl: number } {
    const daily = new Map<string, number>();
    let first: string | null = null;
    for (const a of this.activities.filter((x) => x.is_primary)) {
      const load = this.loadOf(a);
      if (load.status !== 'ok' || load.value === null) continue;
      daily.set(a.date, (daily.get(a.date) ?? 0) + load.value);
      if (first === null || a.date < first) first = a.date;
    }
    if (first === null || date < first) return { ctl: 0, atl: 0 };
    const points = computePmc(daily, first, date, this.settings.pmc);
    const last = points[points.length - 1];
    return { ctl: last.ctl, atl: last.atl };
  }

  // --- Einstellungen -----------------------------------------------------------------

  async getSettings(): Promise<Settings> {
    return structuredClone(this.settings);
  }

  async setSettings(patch: SettingsPatch): Promise<Settings> {
    if (patch.locale !== undefined && patch.locale !== 'de' && patch.locale !== 'en') {
      throw err('invalid_setting');
    }
    if (
      patch.theme !== undefined &&
      patch.theme !== 'dark' &&
      patch.theme !== 'light' &&
      patch.theme !== 'system'
    ) {
      throw err('invalid_setting');
    }
    // Wie der Rust-Teil: Nur ein Wahrheitswert ist gültig (die Deserialisierung lehnt anderes ab).
    if (
      patch.mcp_recovery_enabled !== undefined &&
      typeof patch.mcp_recovery_enabled !== 'boolean'
    ) {
      throw err('invalid_setting');
    }
    if (patch.onboarded !== undefined && typeof patch.onboarded !== 'boolean') {
      throw err('invalid_setting');
    }
    if (
      patch.garmin_sync_on_start !== undefined &&
      typeof patch.garmin_sync_on_start !== 'boolean'
    ) {
      throw err('invalid_setting');
    }
    if (
      patch.garmin_share_hydration !== undefined &&
      typeof patch.garmin_share_hydration !== 'boolean'
    ) {
      throw err('invalid_setting');
    }
    if (patch.pmc) {
      const { tau_ctl_d, tau_atl_d, ctl0, atl0 } = patch.pmc;
      const tauOk = (v: number) => Number.isFinite(v) && v > 0;
      const startOk = (v: number) => Number.isFinite(v) && v >= 0;
      if (!(tauOk(tau_ctl_d) && tauOk(tau_atl_d) && startOk(ctl0) && startOk(atl0))) {
        throw err('invalid_value');
      }
    }
    if (patch.locale) this.settings.locale = patch.locale;
    if (patch.theme) this.settings.theme = patch.theme;
    if (patch.pmc) this.settings.pmc = { ...patch.pmc };
    if (patch.mcp_recovery_enabled !== undefined) {
      this.settings.mcp_recovery_enabled = patch.mcp_recovery_enabled;
    }
    if (patch.onboarded !== undefined) this.settings.onboarded = patch.onboarded;
    if (patch.garmin_sync_on_start !== undefined) {
      this.settings.garmin_sync_on_start = patch.garmin_sync_on_start;
    }
    if (patch.garmin_share_hydration !== undefined) {
      this.settings.garmin_share_hydration = patch.garmin_share_hydration;
    }
    return structuredClone(this.settings);
  }

  async importFolder(onProgress: (p: ImportProgress) => void): Promise<ImportSummary> {
    const files = this.activities.length + 3;
    const steps = 12;
    for (let i = 1; i <= steps; i++) {
      onProgress({ done: Math.ceil((i * files) / steps), total: files });
      if (this.importStepMs > 0) await sleep(this.importStepMs);
    }
    const first = !this.imported;
    this.imported = true;
    this.settings.cache_outdated = false;
    const results = this.activities.map((a) => this.loadOf(a));
    const count = (s: LoadInfo['status']) =>
      results.filter((l, i) => this.activities[i].is_primary && l.status === s).length;
    return {
      files_found: files,
      new: first ? this.activities.length : 0,
      updated: 0,
      unchanged: first ? 0 : this.activities.length,
      not_activity: 1,
      error_count: 2,
      errors_by_code: [
        { code: 'crc_mismatch', count: 1 },
        { code: 'truncated', count: 1 },
      ],
      duplicates: this.activities.filter((a) => !a.is_primary).length,
      primary: this.activities.filter((a) => a.is_primary).length,
      loads_ok: count('ok'),
      loads_missing_threshold: count('missing_threshold'),
      loads_missing_channel: count('missing_channel'),
      loads_implausible: count('implausible'),
    };
  }

  /** Wie viele Dateien die nächste Auswahl enthält (Tests setzen 0 für "abgebrochen"). */
  pickedFiles = 2;
  /** Schon abgelegt: dieselben Dateien ein zweites Mal ergeben "schon vorhanden". */
  private addedFiles = 0;

  async addFitFiles(onProgress: (p: ImportProgress) => void): Promise<AddFitFilesResult> {
    this.calls.push({ command: 'add_fit_files', args: {} });
    const selected = this.pickedFiles;
    if (selected === 0) {
      return {
        cancelled: true,
        selected: 0,
        copied: 0,
        already_present: 0,
        rejected: 0,
        import: null,
      };
    }
    // Eine Datei der Auswahl ist unbrauchbar (falsche Endung), der Rest ist neu oder schon da.
    const rejected = selected > 2 ? 1 : 0;
    const usable = selected - rejected;
    const fresh = Math.max(0, usable - this.addedFiles);
    this.addedFiles = Math.max(this.addedFiles, usable);
    const summary = await this.importFolder(onProgress);
    return {
      cancelled: false,
      selected,
      copied: fresh,
      already_present: usable - fresh,
      rejected,
      import: summary,
    };
  }

  // --- Last und Aktivitäten ----------------------------------------------------------

  /** Schwelle `kind` für die Sportart am Tag: der jüngste Eintrag mit Beginn bis zum Tag, bei
   * gleichem Beginn gewinnt der der Sportart (wie `thresholds_for`). */
  private thresholdAt(kind: ThresholdKind, sport: Sport, date: string): number | null {
    const candidates = this.thresholds
      .filter(
        (t) => t.kind === kind && t.valid_from <= date && (t.sport === sport || t.sport === null),
      )
      .sort(
        (a, b) =>
          b.valid_from.localeCompare(a.valid_from) ||
          Number(b.sport === sport) - Number(a.sport === sport) ||
          b.id - a.id,
      );
    return candidates[0]?.value ?? null;
  }

  private loadOf(a: RawActivity): LoadInfo {
    const hours = a.duration_s / 3600;
    const info = (status: LoadInfo['status'], extra: Partial<LoadInfo> = {}): LoadInfo => ({
      status,
      value: null,
      method: null,
      intensity_factor: null,
      estimated: false,
      missing: null,
      ...extra,
    });
    const lacking = (missing: MissingKind) => info('missing_threshold', { missing });
    const ok = (method: LoadMethod, factor: number, exponent: number): LoadInfo => ({
      status: 'ok',
      value: round1(hours * factor ** exponent * 100),
      method,
      intensity_factor: Math.round(factor * 100) / 100,
      estimated: ESTIMATED_METHODS.includes(method),
      missing: null,
    });
    if (a.implausible) return info('implausible');
    if (a.sport === 'strength' || a.channel === 'none') return info('missing_channel');
    if (a.channel === 'power' && a.np_w) {
      const ftp = this.thresholdAt('ftp_w', a.sport, a.date);
      return ftp ? ok('tss', a.np_w / ftp, 2) : lacking('ftp_w');
    }
    if (a.channel === 'speed' && a.speed_mps) {
      if (a.sport === 'running') {
        const v = this.thresholdAt('run_speed_mps', a.sport, a.date);
        return v ? ok('r_tss', a.speed_mps / v, 2) : lacking('run_speed_mps');
      }
      const css = this.thresholdAt('css_mps', a.sport, a.date);
      return css ? ok('s_tss', a.speed_mps / css, 3) : lacking('css_mps');
    }
    const lthr = this.thresholdAt('lthr_bpm', a.sport, a.date);
    if (!lthr) return lacking('lthr_bpm');
    return a.avg_hr ? ok('hr_tss', a.avg_hr / lthr, 2) : info('missing_threshold');
  }

  private itemOf(a: RawActivity): ActivityItem {
    return {
      id: a.id,
      local_date: a.date,
      start_utc: a.start_utc,
      sport: a.sport,
      sub_sport: null,
      duration_s: a.duration_s,
      distance_m: a.distance_m,
      avg_hr: a.avg_hr,
      avg_power_w: a.avg_power_w,
      load: this.loadOf(a),
      is_primary: a.is_primary,
      duplicate_of: a.duplicate_of,
    };
  }

  async listActivities(filter: ActivityFilter): Promise<ActivityItem[]> {
    if (!isIsoDate(filter.from) || !isIsoDate(filter.to) || filter.from > filter.to) {
      throw err('invalid_date');
    }
    return this.activities
      .filter((a) => a.date >= filter.from && a.date <= filter.to)
      .filter((a) => filter.sport === null || a.sport === filter.sport)
      .map((a) => this.itemOf(a))
      .reverse();
  }

  async getActivity(id: number): Promise<ActivityDetail> {
    const a = this.activities.find((x) => x.id === id);
    if (!a) throw err('not_found');
    return {
      item: this.itemOf(a),
      moving_s: Math.round(a.duration_s * 0.97),
      max_hr: a.max_hr,
      np_w: a.np_w,
      device_product: a.channel === 'power' ? 'Attrappe Radcomputer' : 'Attrappe Uhr',
      zones: this.zonesOf(a),
      best_power: bestPowerOf(a),
      series: seriesOf(a),
      ...detailExtrasOf(a),
      garmin_load: garminLoadOf(a),
    };
  }

  private zonesOf(a: RawActivity): ZoneSchemeTime[] {
    const out: ZoneSchemeTime[] = [];
    const rnd = mulberry32(a.seed ^ 0x9e3779b9);
    const add = (id: keyof typeof ZONE_META, estimated = false) => {
      const meta = ZONE_META[id];
      // Erfundene Verteilung: Gewichte, die auf die Dauer skaliert werden.
      const weights = Array.from({ length: meta.zones }, (_, i) => rnd() * (meta.zones - i));
      const sum = weights.reduce((s, w) => s + w, 0);
      const seconds = weights.map((w) => Math.floor((w / sum) * a.duration_s * 0.98));
      out.push({
        id,
        basis: meta.basis,
        seconds,
        estimated,
        source: meta.source,
        status: meta.status,
      });
    };
    if (
      a.sport === 'cycling' &&
      a.channel === 'power' &&
      this.thresholdAt('ftp_w', a.sport, a.date)
    ) {
      add('power7');
    }
    if (a.sport === 'running' && this.thresholdAt('run_speed_mps', a.sport, a.date))
      add('run_speed5');
    if (a.sport === 'swimming' && this.thresholdAt('css_mps', a.sport, a.date)) add('swim_css5');
    if (a.avg_hr !== null) {
      if (this.thresholdAt('lthr_bpm', a.sport, a.date)) add('hr_lthr5');
      const rest = this.thresholdAt('hr_rest_bpm', a.sport, a.date);
      const measured = this.thresholdAt('hr_max_bpm', a.sport, a.date);
      if (rest && (measured || this.hrMaxEstimate(a.date))) add('hr_karvonen', !measured);
    }
    return out;
  }

  /** Maximale Herzfrequenz aus dem Alter, 208 - 0,7 * Alter (Q7), immer geschätzt. */
  private hrMaxEstimate(onDate: string): number | null {
    const year = this.settings.athlete.birth_year;
    if (year === null) return null;
    const age = Number(onDate.slice(0, 4)) - year;
    return age >= 0 ? Math.round((208 - 0.7 * age) * 10) / 10 : null;
  }

  // --- Woche ------------------------------------------------------------------------

  async getWeekOverview(weekStart: string, pmcWeeks: number): Promise<WeekOverview> {
    if (!isIsoDate(weekStart)) throw err('invalid_date');
    const end = addDays(weekStart, 6);
    const weeks = Math.min(Math.max(Math.trunc(pmcWeeks), 1), 104);
    const inWeek = this.activities.filter(
      (a) => a.is_primary && a.date >= weekStart && a.date <= end,
    );

    const totals = new Map<Sport, number>();
    const days: DayLoad[] = [];
    let notComputable = 0;
    for (let i = 0; i < 7; i++) {
      const date = addDays(weekStart, i);
      const bySport = new Map<Sport, number>();
      let missing = 0;
      for (const a of inWeek.filter((x) => x.date === date)) {
        const load = this.loadOf(a);
        if (load.status === 'ok' && load.value !== null) {
          bySport.set(a.sport, (bySport.get(a.sport) ?? 0) + load.value);
          totals.set(a.sport, (totals.get(a.sport) ?? 0) + load.value);
        } else {
          missing++;
        }
      }
      notComputable += missing;
      days.push({
        date,
        by_sport: SPORTS.filter((s) => bySport.has(s)).map((s) => ({
          sport: s,
          load: bySport.get(s) ?? 0,
        })),
        not_computable: missing,
      });
    }

    const windowStart = addDays(end, -(weeks * 7 - 1));
    const pmcEnd = end < this.today ? end : this.today;
    const daily = new Map<string, number>();
    let first: string | null = null;
    for (const a of this.activities.filter((x) => x.is_primary)) {
      const load = this.loadOf(a);
      if (load.status !== 'ok' || load.value === null) continue;
      daily.set(a.date, (daily.get(a.date) ?? 0) + load.value);
      if (first === null || a.date < first) first = a.date;
    }
    let pmc: PmcPoint[] = [];
    if (first !== null && pmcEnd >= windowStart) {
      const from = first !== null && first < windowStart ? first : windowStart;
      pmc = computePmc(daily, from, pmcEnd, this.settings.pmc).filter((p) => p.date >= windowStart);
    }
    const totalsList: SportLoad[] = SPORTS.filter((s) => totals.has(s)).map((s) => ({
      sport: s,
      load: totals.get(s) ?? 0,
    }));
    return {
      week_start: weekStart,
      week_end: end,
      days,
      totals_by_sport: totalsList,
      activities: inWeek.length,
      not_computable: notComputable,
      pmc,
      pmc_first_load_date: first,
    };
  }

  /** Alle Kennzahlen der Startseite. Erfundene Werte, der aktive Plan kommt aus der Planung. */
  async getProgress(today: string): Promise<Progress> {
    if (!isIsoDate(today)) throw err('invalid_date');
    return buildProgress(today, this.dashboardVariant, mulberry32);
  }

  async getPerformance(today: string, range: PerfRange): Promise<Performance> {
    if (!isIsoDate(today)) throw err('invalid_date');
    if (!PERF_RANGES.includes(range)) throw err('invalid_value');
    const ids = (sport: Sport) => this.activities.filter((a) => a.sport === sport).map((a) => a.id);
    return buildPerformance(
      today,
      range,
      this.dashboardVariant,
      ids('cycling'),
      ids('running'),
      this.thresholds.map((t) => ({
        kind: t.kind,
        sport: t.sport,
        value: t.value,
        valid_from: t.valid_from,
        origin: t.origin,
      })),
      mulberry32,
    );
  }

  async getRecovery(today: string, days: RecoveryRange): Promise<RecoveryPage> {
    if (!isIsoDate(today)) throw err('invalid_date');
    if (!RECOVERY_RANGES.includes(days)) throw err('invalid_value');
    return buildRecovery(today, days, this.dashboardVariant, mulberry32);
  }

  /** Hydration (S3) liegt vor: nur nach Freigabe und einem Abgleich, bis zum Löschen. */
  private hydrationStored = false;

  async getHealth(today: string, days: HealthRange): Promise<HealthPage> {
    if (!isIsoDate(today)) throw err('invalid_date');
    if (!HEALTH_RANGES.includes(days)) throw err('invalid_value');
    return buildHealth(
      today,
      days,
      this.dashboardVariant,
      this.settings.garmin_share_hydration,
      this.hydrationStored,
      mulberry32,
    );
  }

  async deleteSensitiveData(): Promise<number> {
    const stored = this.hydrationStored
      ? buildHealth(this.today, 365, this.dashboardVariant, true, true, mulberry32)
      : null;
    this.hydrationStored = false;
    return stored ? stored.points.filter((p) => p.hydration_ml !== null).length : 0;
  }

  async getDashboard(today: string): Promise<Dashboard> {
    if (!isIsoDate(today)) throw err('invalid_date');
    const active = this.planning.list().find((p) => p.status === 'active');
    const plan = active ? this.planning.get(active.id) : null;
    return buildDashboard(today, this.dashboardVariant, plan, {
      mulberry32,
      computePmc,
      pmc: this.settings.pmc,
    });
  }

  // --- Schwellenwerte und Athlet ------------------------------------------------------

  private addThreshold(
    sport: Sport | null,
    kind: ThresholdKind,
    value: number,
    validFrom: string,
    note: string | null,
    origin: ThresholdOrigin = 'manual',
  ): number {
    const id = this.nextThresholdId++;
    this.thresholds.push({ id, sport, kind, value, valid_from: validFrom, note, origin });
    return id;
  }

  private validUntil(t: MockThreshold): string | null {
    const later = this.thresholds
      .filter(
        (o) =>
          o.kind === t.kind &&
          o.valid_from > t.valid_from &&
          (o.sport === t.sport || o.sport === null || t.sport === null),
      )
      .map((o) => o.valid_from)
      .sort()[0];
    return later ? addDays(later, -1) : null;
  }

  async listThresholds(): Promise<ThresholdList> {
    const items: ThresholdItem[] = [...this.thresholds]
      .sort((a, b) => a.kind.localeCompare(b.kind) || a.valid_from.localeCompare(b.valid_from))
      .map((t) => ({
        id: t.id,
        sport: t.sport,
        kind: t.kind,
        value: t.value,
        valid_from: t.valid_from,
        valid_until: this.validUntil(t),
        origin: t.origin,
        note: t.note,
      }));
    const measured = this.thresholds.some((t) => t.kind === 'hr_max_bpm');
    const bpm = measured ? null : this.hrMaxEstimate(this.today);
    return { items, hr_max_estimate: bpm === null ? null : { bpm, estimated: true } };
  }

  async setThreshold(input: ThresholdInput): Promise<ThresholdSet> {
    if (!isIsoDate(input.valid_from)) throw err('invalid_date');
    const v = input.value;
    if (!(Number.isFinite(v) && v > 0)) throw err('invalid_value');
    if (input.kind === 'ftp_w' && v > 2500) throw err('invalid_value');
    if (
      (input.kind === 'lthr_bpm' || input.kind === 'hr_max_bpm' || input.kind === 'hr_rest_bpm') &&
      (v < 30 || v > 230)
    ) {
      throw err('invalid_value');
    }
    const required: Partial<Record<ThresholdKind, Sport>> = {
      ftp_w: 'cycling',
      run_speed_mps: 'running',
      css_mps: 'swimming',
    };
    const needed = required[input.kind];
    if (needed && input.sport !== null && input.sport !== needed) throw err('invalid_sport');
    const note = input.note?.trim() || null;
    if (note && [...note].length > 200) throw err('invalid_value');
    const id = this.addThreshold(input.sport, input.kind, v, input.valid_from, note);
    const t = this.thresholds.find((x) => x.id === id) as MockThreshold;
    const until = this.validUntil(t);
    const affected = this.activities.filter(
      (a) =>
        a.date >= input.valid_from &&
        (until === null || a.date <= until) &&
        (input.sport === null || a.sport === input.sport),
    ).length;
    return { id, recompute: this.recompute(affected) };
  }

  async setAthlete(input: AthleteInput): Promise<AthleteSet> {
    if (input.sex !== undefined && input.sex !== 'male' && input.sex !== 'female') {
      throw err('invalid_value');
    }
    const thisYear = Number(this.today.slice(0, 4));
    if (
      input.birth_year !== undefined &&
      !(input.birth_year >= 1900 && input.birth_year <= thisYear)
    ) {
      throw err('invalid_year');
    }
    // Eigene Werte gewinnen gegen die aus Garmin (T39).
    if (input.sex !== undefined) {
      this.settings.athlete.sex = input.sex;
      this.settings.athlete.sex_origin = 'manual';
    }
    if (input.birth_year !== undefined) {
      this.settings.athlete.birth_year = input.birth_year;
      this.settings.athlete.birth_year_origin = 'manual';
    }
    const changed = input.sex !== undefined || input.birth_year !== undefined;
    return {
      settings: structuredClone(this.settings),
      recompute: changed ? this.recompute(this.activities.length) : this.recompute(0, true),
    };
  }

  // --- Pläne, Ziele, Export ----------------------------------------------------------

  async listPlans(): Promise<PlanListItem[]> {
    return this.planning.list();
  }

  async getPlan(id: string): Promise<PlanDetail> {
    return this.planning.get(id);
  }

  async generatePlan(input: GeneratePlanInput): Promise<PlanDetail> {
    return this.planning.generate(input);
  }

  async setPlanStatus(id: string, status: PlanStatus): Promise<PlanListItem[]> {
    return this.planning.setStatus(id, status);
  }

  async getPlanningInputs(): Promise<PlanningInputs> {
    return this.planning.inputs();
  }

  async setGoal(input: GoalInput): Promise<PlanningInputs> {
    return this.planning.setGoal(input);
  }

  async setAvailability(days: AvailabilityDay[]): Promise<PlanningInputs> {
    return this.planning.setAvailability(days);
  }

  async quickPlan(input: QuickPlanInput): Promise<PlanDetail> {
    return this.planning.quick(input);
  }

  async coachStatus(): Promise<CoachStatus> {
    return this.coach.status();
  }

  async coachSetModel(model: string): Promise<CoachStatus> {
    return this.coach.setModel(model);
  }

  async coachPreview(message: string): Promise<PromptPreview> {
    return this.coach.preview(message);
  }

  async coachChat(message: string): Promise<CoachReply> {
    return this.coach.chat(message);
  }

  async coachApply(proposalId: string): Promise<PlanDetail> {
    return this.coach.apply(proposalId);
  }

  async coachReset(): Promise<void> {
    this.coach.reset();
  }

  async coachSetupStatus(): Promise<CoachSetupStatus> {
    return this.coachSetup.status();
  }

  async coachInstallOllama(onProgress: (p: CoachSetupProgress) => void): Promise<CoachSetupStatus> {
    return this.coachSetup.install(onProgress);
  }

  async coachPullModel(
    name: string,
    onProgress: (p: CoachPullProgress) => void,
  ): Promise<CoachSetupStatus> {
    return this.coachSetup.pull(name, onProgress);
  }

  async coachPullCancel(): Promise<void> {
    this.coachSetup.cancelPull();
  }

  async coachCancel(): Promise<void> {
    this.coach.cancel();
  }

  async exportPlanIcs(id: string): Promise<ExportResult> {
    return this.planning.exportIcs(id);
  }

  async exportPlanJson(id: string): Promise<ExportResult> {
    return this.planning.exportJson(id);
  }

  async exportBackup(): Promise<ExportResult> {
    return this.planning.exportBackup();
  }

  async restoreBackup(confirmed: boolean): Promise<RestoreResult> {
    return this.planning.restore(confirmed);
  }

  /** Zustand des Austauschordners in der Attrappe (für Tests setzbar). */
  bridgeStatus: BridgeStatus = {
    has_data: false,
    recovery_days: 0,
    activity_files: 0,
    last_day: null,
  };

  async getBridgeStatus(): Promise<BridgeStatus> {
    return { ...this.bridgeStatus };
  }

  /** Version, die die Attrappe als verfügbar meldet. `null`: aktuell (für Tests setzbar). */
  availableUpdate: string | null = null;

  async checkUpdate(): Promise<UpdateInfo> {
    return {
      current: '0.0.0',
      available: this.availableUpdate !== null,
      version: this.availableUpdate,
    };
  }

  async installUpdate(confirmed: boolean): Promise<void> {
    if (!confirmed) throw err('unconfirmed');
    if (this.availableUpdate === null) throw err('no_update');
    this.availableUpdate = null;
  }

  // --- Garmin (T23) ----------------------------------------------------------------------
  // Synthetischer Ablauf ohne Netz: Anmeldung gelingt (oder endet mit `garminLoginCode`), der
  // Abgleich meldet Fortschritt in allen drei Phasen und findet beim ersten Lauf drei neue
  // Aktivitäten. Kein Token, kein Name, kein Pfad.

  /** Zustand der Attrappe, für Tests setzbar. */
  readonly garmin = {
    connected: false,
    lastSync: null as string | null,
    lockedUntil: null as string | null,
    syncs: 0,
    startDone: false,
    running: false,
    /** Ergebnis der nächsten Anmeldung: `connected` oder ein Fehlercode wie `cancelled`. */
    loginCode: 'connected',
    /** Abbruchcode des nächsten Abgleichs, zum Beispiel `locked`. */
    abortCode: null as string | null,
  };

  async garminStatus(): Promise<GarminStatus> {
    return {
      connected: this.garmin.connected,
      last_sync: this.garmin.lastSync,
      sync_on_start: this.settings.garmin_sync_on_start,
      locked_until: this.garmin.lockedUntil,
    };
  }

  async garminLogin(): Promise<GarminLoginResult> {
    if (this.importStepMs > 0) await sleep(this.importStepMs * 3);
    const ok = this.garmin.loginCode === 'connected';
    if (ok) this.garmin.connected = true;
    return { ok, code: this.garmin.loginCode };
  }

  async garminSync(
    trigger: GarminSyncTrigger,
    onProgress: (p: GarminSyncProgress) => void,
  ): Promise<GarminSyncSummary | null> {
    if (trigger === 'start') {
      if (this.garmin.startDone) return null;
      this.garmin.startDone = true;
      if (!this.garmin.connected || !this.settings.garmin_sync_on_start) return null;
    } else if (!this.garmin.connected) {
      throw err('garmin_not_connected');
    }
    if (this.garmin.running) throw err('busy');
    this.garmin.running = true;
    try {
      const first = this.garmin.syncs === 0;
      const fresh = first ? 3 : 0;
      const steps: GarminSyncProgress[] = [
        { phase: 'activities', done: 2, total: null },
        { phase: 'activities', done: fresh, total: fresh },
        ...[1, 2, 3, 4].map((done) => ({ phase: 'recovery' as const, done, total: 4 })),
        { phase: 'account', done: 1, total: 1 },
        ...[1, 2, 3].map((done) => ({ phase: 'wellness' as const, done, total: 3 })),
        ...[1, 2, 3].map((done) => ({ phase: 'import' as const, done, total: 3 })),
      ];
      for (const p of steps) {
        onProgress(p);
        if (this.importStepMs > 0) await sleep(this.importStepMs);
      }
      const abort = this.garmin.abortCode;
      this.garmin.syncs += 1;
      if (!abort) this.garmin.lastSync = this.today;
      // Hydration kommt nur mit Freigabe (S3, T39).
      const sensitive = !abort && this.settings.garmin_share_hydration;
      if (sensitive) this.hydrationStored = true;
      return {
        window_start: addDays(this.today, first ? -365 : -7),
        activities_listed: 2 + fresh,
        new_activities: fresh,
        known_activities: 2,
        imported: fresh,
        import_errors: 0,
        recovery_days: first ? 30 : 7,
        dropped_values: 0,
        calls: 12,
        // Beim ersten Abgleich meldet die Attrappe geänderte Schwellen aus Garmin, ohne die
        // erfundenen Schwellen zu ändern (die Lasten der Tests bleiben stabil).
        thresholds_changed: first ? 2 : 0,
        recomputed: first ? 41 : 0,
        wellness_days: first ? 28 : 7,
        training_days: first ? 28 : 7,
        sensitive_days: sensitive ? (first ? 28 : 7) : 0,
        garmin_loads: fresh,
        errors_by_code: [],
        aborted: abort ? { code: abort, until: null } : null,
        last_sync_updated: !abort,
      };
    } finally {
      this.garmin.running = false;
    }
  }

  async garminLogout(): Promise<void> {
    this.garmin.connected = false;
  }

  private recompute(count: number, skipped = false): Recompute {
    return {
      skipped,
      candidates: skipped ? 0 : count,
      recomputed: skipped ? 0 : count,
      file_missing: 0,
      file_changed: 0,
    };
  }
}

// --- Erfundene Daten ---------------------------------------------------------------------

/** Garmins eigene Last, nur für Läufe und Fahrten mit Leistung, erfunden (T39). */
function garminLoadOf(a: RawActivity): ActivityDetail['garmin_load'] {
  if (a.sport !== 'running' && !(a.sport === 'cycling' && a.channel === 'power')) return null;
  const rnd = mulberry32(a.seed ^ 0x1b873593);
  const effects = ['AEROBIC_BASE', 'TEMPO', 'LACTATE_THRESHOLD', 'VO2MAX', 'MIXED_EFFORT'];
  const hours = a.duration_s / 3600;
  return {
    training_load: rnd() < 0.1 ? null : Math.round(hours * (60 + rnd() * 90)),
    training_effect: effects[Math.floor(rnd() * effects.length)],
  };
}

/** Wochenmuster über etwa 20 Wochen bis `today`. Montag Ruhetag. */
function buildActivities(today: string): RawActivity[] {
  const list: RawActivity[] = [];
  let id = 1;
  let saturdays = 0;
  const push = (a: Omit<RawActivity, 'id'>) => list.push({ ...a, id: id++ });
  const total = 140;
  for (let back = total; back >= 0; back--) {
    const date = addDays(today, -back);
    const weekday = (fromIso(date).getUTCDay() + 6) % 7; // 0 = Montag
    const seed = daysBetween('2020-01-01', date) * 7919 + 17;
    const rnd = mulberry32(seed);
    const startHour = 6 + Math.floor(rnd() * 3);
    const start_utc = `${date}T${String(startHour).padStart(2, '0')}:${String(Math.floor(rnd() * 60)).padStart(2, '0')}:00Z`;
    const base = {
      date,
      start_utc,
      is_primary: true,
      duplicate_of: null,
      implausible: false,
      seed,
    };
    if (weekday === 1) {
      // Dienstag: Lauf, Geschwindigkeit und Herzfrequenz.
      const duration_s = 2400 + Math.floor(rnd() * 1200);
      const speed = 3.1 + rnd() * 0.5;
      push({
        ...base,
        sport: 'running',
        duration_s,
        distance_m: Math.round(speed * duration_s),
        avg_hr: Math.round(145 + rnd() * 12),
        max_hr: Math.round(165 + rnd() * 10),
        avg_power_w: null,
        np_w: null,
        channel: 'speed',
        speed_mps: round1(speed * 100) / 100,
      });
    } else if (weekday === 2) {
      // Mittwoch: Intervalle auf dem Rad mit Leistung.
      const duration_s = 3600 + Math.floor(rnd() * 900);
      const avg = 190 + rnd() * 30;
      push({
        ...base,
        sport: 'cycling',
        duration_s,
        distance_m: Math.round(8.2 * duration_s),
        avg_hr: Math.round(148 + rnd() * 10),
        max_hr: Math.round(172 + rnd() * 8),
        avg_power_w: Math.round(avg),
        np_w: Math.round(avg * 1.12),
        channel: 'power',
        speed_mps: null,
      });
    } else if (weekday === 3) {
      // Donnerstag: Schwimmen, Geschwindigkeit ohne Herzfrequenz.
      const duration_s = 1800 + Math.floor(rnd() * 900);
      const speed = 1.1 + rnd() * 0.15;
      push({
        ...base,
        sport: 'swimming',
        duration_s,
        distance_m: Math.round(speed * duration_s),
        avg_hr: null,
        max_hr: null,
        avg_power_w: null,
        np_w: null,
        channel: 'speed',
        speed_mps: Math.round(speed * 100) / 100,
      });
    } else if (weekday === 4 && rnd() < 0.7) {
      // Freitag: Kraft, ohne RPE gibt es keine Last.
      push({
        ...base,
        sport: 'strength',
        duration_s: 2700,
        distance_m: null,
        avg_hr: Math.round(108 + rnd() * 10),
        max_hr: Math.round(140 + rnd() * 10),
        avg_power_w: null,
        np_w: null,
        channel: 'none',
        speed_mps: null,
      });
    } else if (weekday === 5) {
      // Samstag: lange Fahrt mit Leistung, alle paar Wochen mit Duplikat der Uhr.
      const duration_s = 7200 + Math.floor(rnd() * 5400);
      const avg = 175 + rnd() * 30;
      const mainId = id;
      saturdays++;
      push({
        ...base,
        sport: 'cycling',
        duration_s,
        distance_m: Math.round(8.6 * duration_s),
        avg_hr: Math.round(138 + rnd() * 8),
        max_hr: Math.round(165 + rnd() * 8),
        avg_power_w: Math.round(avg),
        np_w: Math.round(avg * 1.08),
        channel: 'power',
        speed_mps: null,
        implausible: back === 33,
      });
      if (saturdays % 3 === 1) {
        push({
          ...base,
          start_utc: start_utc.replace(':00Z', ':20Z'),
          sport: 'cycling',
          duration_s: duration_s - 30,
          distance_m: Math.round(8.6 * duration_s),
          avg_hr: Math.round(138 + rnd() * 8),
          max_hr: Math.round(165 + rnd() * 8),
          avg_power_w: null,
          np_w: null,
          channel: 'hr',
          speed_mps: null,
          is_primary: false,
          duplicate_of: mainId,
        });
      }
    } else if (weekday === 6) {
      // Sonntag: lockere Fahrt nur mit Herzfrequenz (hrTSS braucht die Schwellen-HF).
      const duration_s = 5400 + Math.floor(rnd() * 1800);
      push({
        ...base,
        sport: 'cycling',
        duration_s,
        distance_m: Math.round(7.4 * duration_s),
        avg_hr: Math.round(125 + rnd() * 10),
        max_hr: Math.round(150 + rnd() * 10),
        avg_power_w: null,
        np_w: null,
        channel: 'hr',
        speed_mps: null,
      });
    }
  }
  return list;
}

function bestPowerOf(a: RawActivity): BestPower[] {
  if (a.channel !== 'power' || a.avg_power_w === null) return [];
  return BEST_POWER_FACTORS.filter(([d]) => d <= a.duration_s).map(([duration_s, factor]) => ({
    duration_s,
    watts: Math.round(a.avg_power_w! * factor * 10) / 10,
  }));
}

/** Erfundene Messreihe mit 600 Punkten, Leistung nur bei Kanal `power`. */
function seriesOf(a: RawActivity): Series {
  if (a.channel === 'speed' && a.avg_hr === null) {
    return { status: 'no_channels', t_s: [], power_w: [], hr_bpm: [], speed_mps: [] };
  }
  const points = 600;
  const step = a.duration_s / points;
  const rnd = mulberry32(a.seed ^ 0x51ed270b);
  const t_s: number[] = [];
  const power_w: (number | null)[] = [];
  const hr_bpm: (number | null)[] = [];
  const speed_mps: (number | null)[] = [];
  let hr = (a.avg_hr ?? 120) - 15;
  for (let i = 0; i < points; i++) {
    t_s.push(Math.round(i * step));
    const wave = Math.sin(i / 14) * 0.18 + Math.sin(i / 60) * 0.1;
    const power =
      a.channel === 'power' && a.avg_power_w !== null
        ? a.avg_power_w * (1 + wave) + (rnd() - 0.5) * 12
        : null;
    power_w.push(power === null ? null : Math.max(0, Math.round(power)));
    if (a.avg_hr !== null) {
      const target = a.avg_hr * (1 + wave * 0.35);
      hr += (target - hr) * 0.08 + (rnd() - 0.5) * 1.2;
      hr_bpm.push(Math.round(hr));
    } else {
      hr_bpm.push(null);
    }
    speed_mps.push(
      a.speed_mps === null
        ? null
        : Math.round(a.speed_mps * (1 + wave * 0.25 + (rnd() - 0.5) * 0.04) * 1000) / 1000,
    );
  }
  return { status: 'ok', t_s, power_w, hr_bpm, speed_mps };
}

/** Erfundene Werte für Entkopplung, Effizienz und Runden (T31). Die Regeln entsprechen denen
 * des Rust-Teils: Effizienz nur Rad ab 45 min, Entkopplung ab 45 min mit Leistung und
 * Herzfrequenz, Runden erst ab zwei. Die Zahlen sind erfunden, nicht gerechnet. */
function detailExtrasOf(
  a: RawActivity,
): Pick<ActivityDetail, 'efficiency_factor' | 'decoupling' | 'laps'> {
  const long = a.duration_s >= 45 * 60;
  const efficiency_factor =
    a.sport === 'cycling' && long && a.np_w !== null && a.avg_hr !== null
      ? Math.round((a.np_w / a.avg_hr) * 100) / 100
      : null;
  const rnd = mulberry32(a.seed ^ 0x2545f491);
  const percent =
    long && a.channel === 'power' && a.avg_hr !== null
      ? Math.round((rnd() * 9 - 1) * 10) / 10
      : null;
  const decoupling = {
    percent,
    state:
      percent === null
        ? ('decoupling.no_data' as const)
        : percent > 5
          ? ('decoupling.decoupled' as const)
          : ('decoupling.coupled' as const),
  };
  // Läufe in Kilometerrunden, Rad in Zehn-Minuten-Runden, höchstens 30.
  const laps: LapItem[] = [];
  const lapSeconds =
    a.sport === 'running' && a.speed_mps
      ? Math.round(1000 / a.speed_mps)
      : a.sport === 'cycling'
        ? 600
        : 0;
  if (lapSeconds > 0 && a.duration_s >= 2 * lapSeconds) {
    const count = Math.min(30, Math.ceil(a.duration_s / lapSeconds));
    for (let i = 0; i < count; i++) {
      const start = i * lapSeconds;
      const elapsed = Math.min(lapSeconds, a.duration_s - start);
      const drift = 1 + (rnd() - 0.5) * 0.08;
      const speed = a.speed_mps === null ? null : Math.round(a.speed_mps * drift * 1000) / 1000;
      laps.push({
        start_s: start,
        elapsed_s: elapsed,
        timer_s: elapsed,
        distance_m: speed === null ? null : Math.round(speed * elapsed),
        avg_power_w: a.avg_power_w === null ? null : Math.round(a.avg_power_w * drift),
        avg_hr_bpm: a.avg_hr === null ? null : Math.round(a.avg_hr * (0.96 + (i / count) * 0.06)),
        avg_speed_mps: speed,
      });
    }
  }
  return { efficiency_factor, decoupling, laps };
}

export { mondayOf, toIso };
