// Hilfen für das Dashboard (T25 Teil B): Aussagen aus den Zustandscodes, Aufbereitung der Reihen
// für die Diagramme. Reine Funktionen ohne DOM, damit sie sich ohne Browser prüfen lassen.
//
// Texte entstehen nur aus den Sprachdateien (`ui.dash.*`). Die Codes kommen aus den Rechenkernen,
// die Oberfläche kennt keine Schwellen: Sie wählt Text und Richtung, rechnet aber nichts neu.
// Die Texte beschreiben Trainingsdaten und sagen nichts über Gesundheit (SPEC 2.3).

import type {
  AdherenceCode,
  DashAdherence,
  DashDecision,
  Dashboard,
  DashDistribution,
  DashEfficiency,
  DashPowerDuration,
  DashRamp,
  DashReadiness,
  FormCode,
  InsightCode,
  Locale,
  PmcPoint,
  PowerCurveCode,
} from './backend/types';
import { addDays, fromIso, mondayOf } from './dates';
import { formatDate, formatNumber, MISSING } from './format';
import type { Params } from './i18n';
import type { FormState } from './today';

export type Translate = (key: string, params?: Params) => string;

/** Schlüssel der Aussage zu einem Code, zum Beispiel `ramp.steep`. */
export function statementKey(code: string): string {
  return `ui.dash.statement.${code}`;
}

/** `form.fresh` wird `fresh`, der Teil der Schlüssel unter `ui.form.*`. */
export function formName(code: FormCode): FormState {
  return code.slice('form.'.length) as FormState;
}

/** Geschütztes Leerzeichen (U+00A0), hält Zahl und Prozentzeichen zusammen. */
export const NBSP = String.fromCharCode(0xa0);

/** Echtes Minuszeichen (U+2212), wie in `Metric`. */
const MINUS = '−';

/** Vorzeichenbehaftete Zahl: +3,2 oder −3,2 (nie -0). */
export function signed(value: number, locale: Locale, digits = 1): string {
  const text = formatNumber(Math.abs(value), locale, digits);
  const zero = Number(text.replace(',', '.')) === 0;
  if (zero) return text;
  return `${value < 0 ? MINUS : '+'}${text}`;
}

/** Abweichung in Prozent mit Vorzeichen und Leerzeichen vor dem Zeichen, zum Beispiel −12 %. */
export function signedPercent(value: number, locale: Locale, digits = 0): string {
  return `${signed(value, locale, digits)}${NBSP}%`;
}

export type Direction = 'up' | 'down' | 'flat';

/** Pfeilrichtung einer Abweichung. Unter einem halben Punkt der Anzeige gilt sie als flach. */
export function directionOf(value: number, digits = 0): Direction {
  const eps = 0.5 / 10 ** digits;
  if (Math.abs(value) < eps) return 'flat';
  return value > 0 ? 'up' : 'down';
}

// --- Aussagen -----------------------------------------------------------------------------

export function rampStatement(r: DashRamp, tr: Translate, locale: Locale): string {
  return tr(statementKey(r.state), {
    value: r.current === null ? MISSING : formatNumber(Math.abs(r.current), locale, 1),
    limit: formatNumber(r.limit, locale, 0),
  });
}

export function distributionStatement(d: DashDistribution, tr: Translate, locale: Locale): string {
  return tr(statementKey(d.state), {
    share: formatNumber(d.window_easy_percent, locale, 0),
    min: formatNumber(d.min_easy_percent, locale, 0),
  });
}

export function adherenceStatement(
  a: DashAdherence,
  hasPlan: boolean,
  tr: Translate,
  locale: Locale,
): string {
  if (!hasPlan) return tr('ui.dash.adherence.no_plan');
  return tr(statementKey(a.state satisfies AdherenceCode), {
    ratio: formatNumber(a.ratio_percent, locale, 0),
  });
}

export function efficiencyStatement(e: DashEfficiency, tr: Translate, locale: Locale): string {
  return tr(statementKey(e.trend), {
    change:
      e.change_percent === null ? MISSING : formatNumber(Math.abs(e.change_percent), locale, 1),
  });
}

/**
 * Gesamturteil der Leistungskurve: Überwiegen die gestiegenen Dauern, gilt "gestiegen", bei
 * Gleichstand oder nur gleichen Dauern "gleich". Ohne eine einzige vergleichbare Dauer fehlt die
 * Aussage. Die Deutung je Dauer kommt aus `core`, hier wird nur gezählt.
 */
export function powerCurveSummary(durations: DashPowerDuration[]): PowerCurveCode {
  const known = durations.filter((d) => d.trend !== 'power_curve.no_data');
  if (known.length === 0) return 'power_curve.no_data';
  const rising = known.filter((d) => d.trend === 'power_curve.rising').length;
  const falling = known.filter((d) => d.trend === 'power_curve.falling').length;
  if (rising > falling) return 'power_curve.rising';
  if (falling > rising) return 'power_curve.falling';
  return 'power_curve.flat';
}

export function powerCurveStatement(durations: DashPowerDuration[], tr: Translate): string {
  return tr(statementKey(powerCurveSummary(durations)));
}

/** Aussage der Bereitschaft. Unterscheidet "nie Daten", "zu alt" und "Basis fehlt". */
export function readinessStatement(r: DashReadiness, tr: Translate, locale: Locale): string {
  if (r.state !== 'readiness.no_data') return tr(statementKey(r.state));
  if (r.last_date === null) return tr('ui.dash.readiness.empty');
  if (r.date === null) {
    return tr('ui.dash.readiness.stale', { date: formatDate(r.last_date, locale) });
  }
  return tr('ui.dash.readiness.short_baseline');
}

export function calendarStatement(
  loadDays: { load: number }[],
  tr: Translate,
  locale: Locale,
): string {
  const days = loadDays.filter((d) => d.load > 0).length;
  if (days === 0) return tr('ui.dash.calendar.empty');
  const total = loadDays.reduce((s, d) => s + d.load, 0);
  return tr('ui.dash.calendar.statement', {
    days,
    total: formatNumber(total, locale, 0),
  });
}

// --- Kopfzeile: Form, Bereitschaft und Plan in einem Satz ----------------------------------

export type PlanToday = 'none' | 'rest' | 'easy' | 'hard' | 'lighten' | 'keep_easy';

/**
 * Der Plan von heute für die Ein-Satz-Aussage, gelesen aus der Tagesentscheidung (T44): Rät sie
 * zu leichter oder locker, sagt der Kopf das, sonst nennt er die geplante Einheit.
 */
export function planTodayOf(decision: DashDecision): PlanToday {
  switch (decision.state) {
    case 'decision.no_plan':
      return 'none';
    case 'decision.rest':
      return 'rest';
    case 'decision.lighten':
      return 'lighten';
    case 'decision.keep_easy':
      return 'keep_easy';
    case 'decision.already_adjusted':
      return 'easy';
    case 'decision.as_planned':
      return decision.main?.hard ? 'hard' : 'easy';
  }
}

/** Fügt Satzteile mit Komma und "und" vor dem letzten Teil zusammen. */
export function joinParts(parts: string[], and: string): string {
  if (parts.length <= 1) return parts[0] ?? '';
  return `${parts.slice(0, -1).join(', ')} ${and} ${parts[parts.length - 1]}`;
}

/**
 * Ein Satz, der Form, Bereitschaft und den Plan von heute verbindet. Fehlt die Form, bleibt es
 * bei der festen Aussage der Seite. Jeder Teil entfällt, wenn die Daten ihn nicht hergeben.
 */
export function leadSentence(
  form: FormCode | null,
  readiness: DashReadiness | null,
  plan: PlanToday,
  tr: Translate,
): string {
  if (form === null) return tr('ui.lead.today');
  const parts = [tr(`ui.dash.lead.${form}`)];
  if (readiness && readiness.state !== 'readiness.no_data') {
    parts.push(tr(`ui.dash.lead.${readiness.state}`));
  }
  if (plan !== 'none') parts.push(tr(`ui.dash.lead.plan_${plan}`));
  const sentence = joinParts(parts, tr('ui.dash.lead.and'));
  return `${sentence.charAt(0).toUpperCase()}${sentence.slice(1)}.`;
}

// --- Coach-Erkenntnisse (T44) ---------------------------------------------------------------

/**
 * Text einer Erkenntnis: dieselbe Aussage, die der Abschnitt der Kennzahl trägt
 * (`ui.dash.statement.<code>`, bei der Form der Hinweis des Formzustands).
 */
export function insightStatement(
  code: InsightCode,
  dash: Dashboard,
  tr: Translate,
  locale: Locale,
): string {
  switch (code) {
    case 'form.overloaded':
      return tr('ui.form.overloaded_hint');
    case 'readiness.low':
      return readinessStatement(dash.readiness, tr, locale);
    case 'ramp.steep':
    case 'ramp.building':
      return rampStatement(dash.ramp, tr, locale);
    case 'distribution.intensity_heavy':
      return distributionStatement(dash.distribution, tr, locale);
    case 'adherence.under':
    case 'adherence.over':
      return adherenceStatement(dash.adherence, true, tr, locale);
    case 'efficiency.rising':
      return efficiencyStatement(dash.efficiency, tr, locale);
    case 'power_curve.rising':
      return tr(statementKey(code));
  }
}

/** `decision.lighten` wird `lighten`, `decision.reason.form_fresh` wird `form_fresh`. */
export function lastPart(code: string): string {
  return code.slice(code.lastIndexOf('.') + 1);
}

// --- Diagrammdaten ------------------------------------------------------------------------

/** Verlauf und Prognose auf einer Zeitachse. Die Prognose beginnt am letzten Ist-Tag. */
export interface PmcSplit {
  dates: string[];
  ctl: (number | null)[];
  atl: (number | null)[];
  tsb: (number | null)[];
  ctlProjected: (number | null)[];
  atlProjected: (number | null)[];
  tsbProjected: (number | null)[];
  /** Index des ersten Prognosetags oder -1 ohne Prognose. */
  projectionStart: number;
}

export function splitPmc(history: PmcPoint[], projection: PmcPoint[]): PmcSplit {
  const all = [...history, ...projection];
  const h = history.length;
  const pick = (key: 'ctl' | 'atl' | 'tsb') => ({
    actual: all.map((p, i) => (i < h ? p[key] : null)),
    projected: all.map((p, i) => {
      // Der letzte Ist-Tag gehört zu beiden Reihen, damit die Linie ohne Lücke anschließt.
      if (i >= h || (i === h - 1 && projection.length > 0)) return p[key];
      return null;
    }),
  });
  const ctl = pick('ctl');
  const atl = pick('atl');
  const tsb = pick('tsb');
  return {
    dates: all.map((p) => p.date),
    ctl: ctl.actual,
    atl: atl.actual,
    tsb: tsb.actual,
    ctlProjected: ctl.projected,
    atlProjected: atl.projected,
    tsbProjected: tsb.projected,
    projectionStart: projection.length > 0 ? h : -1,
  };
}

/** Pfad einer Linie durch Werte mit Lücken (null). Jede Lücke beginnt ein neues Stück. */
export function sparkPath(
  values: (number | null)[],
  width: number,
  height: number,
  pad = 2,
  range?: { min: number; max: number },
): string {
  const known = values.filter((v): v is number => v !== null && Number.isFinite(v));
  if (known.length === 0 || values.length < 2) return '';
  let min = range?.min ?? Math.min(...known);
  let max = range?.max ?? Math.max(...known);
  if (max - min < 1e-9) {
    // Konstante Reihe: waagerecht in der Mitte statt durch 0 zu teilen.
    min -= 1;
    max += 1;
  }
  const x = (i: number) => pad + (i / (values.length - 1)) * (width - 2 * pad);
  const y = (v: number) => pad + (1 - (v - min) / (max - min)) * (height - 2 * pad);
  let d = '';
  let open = false;
  values.forEach((v, i) => {
    if (v === null || !Number.isFinite(v)) {
      open = false;
      return;
    }
    d += `${open ? 'L' : 'M'}${x(i).toFixed(1)} ${y(v).toFixed(1)}`;
    open = true;
  });
  return d;
}

/** Punkt (x, y) eines Index auf der Linie von [`sparkPath`], für den Endpunkt. */
export function sparkPoint(
  values: (number | null)[],
  index: number,
  width: number,
  height: number,
  pad = 2,
  range?: { min: number; max: number },
): { x: number; y: number } | null {
  const v = values[index];
  const known = values.filter((x): x is number => x !== null && Number.isFinite(x));
  if (v === null || v === undefined || known.length === 0 || values.length < 2) return null;
  let min = range?.min ?? Math.min(...known);
  let max = range?.max ?? Math.max(...known);
  if (max - min < 1e-9) {
    min -= 1;
    max += 1;
  }
  return {
    x: pad + (index / (values.length - 1)) * (width - 2 * pad),
    y: pad + (1 - (v - min) / (max - min)) * (height - 2 * pad),
  };
}

/** Reihe der Erholungswerte auf eine feste Zahl Tage, fehlende Tage als null. */
export function denseSeries<T extends { date: string }>(
  points: T[],
  pick: (p: T) => number | null,
  today: string,
  days: number,
): (number | null)[] {
  const byDate = new Map(points.map((p) => [p.date, pick(p)]));
  return Array.from(
    { length: days },
    (_, i) => byDate.get(addDays(today, -(days - 1 - i))) ?? null,
  );
}

// --- Belastungskalender -------------------------------------------------------------------

export interface HeatCell {
  date: string;
  /** Spalte (Woche) und Zeile (0 = Montag). */
  col: number;
  row: number;
  load: number | null;
  /** 0 = keine Last, 1 bis 4 aufsteigend. */
  level: 0 | 1 | 2 | 3 | 4;
}

export interface Heatmap {
  cells: HeatCell[];
  cols: number;
  /** Beginn jedes Monats: Spalte der ersten Woche, in der er anfängt, und der erste Tag. */
  months: { col: number; date: string }[];
  /** Obergrenzen der Stufen 1 bis 3 (Stufe 4 liegt darüber). */
  thresholds: [number, number, number];
}

/** Schwellen aus den Quartilen der Tage mit Last, damit Ausreißer die Skala nicht plattdrücken. */
export function heatThresholds(loads: number[]): [number, number, number] {
  const v = loads.filter((x) => x > 0 && Number.isFinite(x)).sort((a, b) => a - b);
  if (v.length === 0) return [1, 2, 3];
  const q = (p: number) => v[Math.min(v.length - 1, Math.floor(p * v.length))];
  return [q(0.25), q(0.5), q(0.75)];
}

export function levelOf(load: number | null, th: [number, number, number]): HeatCell['level'] {
  if (load === null || !(load > 0)) return 0;
  if (load <= th[0]) return 1;
  if (load <= th[1]) return 2;
  if (load <= th[2]) return 3;
  return 4;
}

/**
 * Raster der letzten `days` Tage in Wochenspalten (Montag oben). Die erste Spalte beginnt am
 * Montag der Woche des ältesten Tages, Tage davor bleiben leer und erscheinen nicht als Zelle.
 */
export function buildHeatmap(
  loadDays: { date: string; load: number }[],
  today: string,
  days = 365,
): Heatmap {
  const start = addDays(today, -(days - 1));
  const gridStart = mondayOf(start);
  const load = new Map(loadDays.map((d) => [d.date, d.load]));
  const th = heatThresholds(loadDays.map((d) => d.load));
  const cells: HeatCell[] = [];
  const months: Heatmap['months'] = [];
  let lastMonth = -1;
  for (let d = start; d <= today; d = addDays(d, 1)) {
    const offset = Math.round((fromIso(d).getTime() - fromIso(gridStart).getTime()) / 86_400_000);
    const col = Math.floor(offset / 7);
    const row = offset % 7;
    const value = load.get(d) ?? null;
    cells.push({ date: d, col, row, load: value, level: levelOf(value, th) });
    const month = fromIso(d).getUTCMonth();
    // Monatsmarke an der Spalte des ersten sichtbaren Tages des Monats.
    if (month !== lastMonth) {
      months.push({ col, date: d });
      lastMonth = month;
    }
  }
  const cols = cells.length === 0 ? 0 : cells[cells.length - 1].col + 1;
  // Liegt die nächste Marke zu nah (angeschnittener Monat am Anfang), entfällt die frühere.
  const labels = months.filter((m, i) => i === months.length - 1 || months[i + 1].col - m.col >= 3);
  return { cells, cols, months: labels, thresholds: th };
}

export interface CalendarStats {
  /** Tage mit Last größer als 0. */
  trainingDays: number;
  total: number;
  /** Höchste Wochenlast (Montag bis Sonntag) im Zeitraum. */
  bestWeek: number;
}

export function calendarStats(loadDays: { date: string; load: number }[]): CalendarStats {
  const weeks = new Map<string, number>();
  let total = 0;
  let trainingDays = 0;
  for (const d of loadDays) {
    if (!(d.load > 0)) continue;
    trainingDays++;
    total += d.load;
    const monday = mondayOf(d.date);
    weeks.set(monday, (weeks.get(monday) ?? 0) + d.load);
  }
  return { trainingDays, total, bestWeek: Math.max(0, ...weeks.values()) };
}

// --- Leistungskurve -----------------------------------------------------------------------

/** Dauer der Bestleistung: 5 s, 1 min, 20 min, 1 h. */
export function formatPowerDuration(seconds: number): string {
  if (seconds < 60) return `${seconds} s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)} min`;
  return `${Math.round((seconds / 3600) * 10) / 10} h`;
}
