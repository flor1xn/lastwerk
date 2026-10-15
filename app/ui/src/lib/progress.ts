// Fortschritt nach Woche, Monat und Jahr (T32). Reine Funktionen über die Tageszeilen aus
// `get_progress`: nur Summen und Gruppieren, keine Formel und keine Schwelle. Wochen beginnen am
// Montag (SPEC 6.2), der Vorjahresvergleich endet am gleichen Kalendertag des Vorjahres.

import { addDays, daysBetween, fromIso, mondayOf } from './dates';
import { SPORTS, type ProgressEntry, type Sport } from './backend/types';

export const PROGRESS_METRICS = ['duration', 'distance', 'load'] as const;
export type ProgressMetric = (typeof PROGRESS_METRICS)[number];

/** Wochen und Monate, die die Ansicht zeigt. */
export const PROGRESS_WEEKS = 52;
export const PROGRESS_MONTHS = 24;

export interface Totals {
  count: number;
  duration_s: number;
  distance_m: number;
  load: number;
  /** Einheiten ohne berechenbare Last: Sie fehlen in `load`, nie als 0 gezählt. */
  load_missing: number;
}

export function emptyTotals(): Totals {
  return { count: 0, duration_s: 0, distance_m: 0, load: 0, load_missing: 0 };
}

function add(t: Totals, e: ProgressEntry): void {
  t.count += e.count;
  t.duration_s += e.duration_s;
  t.distance_m += e.distance_m;
  t.load += e.load;
  t.load_missing += e.load_missing;
}

/** Wert einer Summe in der Einheit der Anzeige: Stunden, Kilometer oder Last. */
export function metricValue(t: Totals, metric: ProgressMetric): number {
  switch (metric) {
    case 'duration':
      return t.duration_s / 3600;
    case 'distance':
      return t.distance_m / 1000;
    case 'load':
      return t.load;
  }
}

/** Nur eine Sportart oder alle (`null`). */
export function filterSport(entries: ProgressEntry[], sport: Sport | null): ProgressEntry[] {
  return sport === null ? entries : entries.filter((e) => e.sport === sport);
}

/** Sportarten mit mindestens einer Einheit, in der festen Reihenfolge der App. */
export function sportsPresent(entries: ProgressEntry[]): Sport[] {
  const seen = new Set(entries.filter((e) => e.count > 0).map((e) => e.sport));
  return SPORTS.filter((s) => seen.has(s));
}

/** Gleicher Kalendertag im Vorjahr; der 29. Februar wird zum 28. */
export function sameDayPreviousYear(iso: string): string {
  const y = Number(iso.slice(0, 4)) - 1;
  const md = iso.slice(5);
  const leap = (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
  return `${String(y).padStart(4, '0')}-${md === '02-29' && !leap ? '02-28' : md}`;
}

function sumBetween(entries: ProgressEntry[], from: string, to: string): Totals {
  const t = emptyTotals();
  for (const e of entries) if (e.date >= from && e.date <= to) add(t, e);
  return t;
}

export interface YearToDate {
  year: number;
  current: Totals;
  /** Vorjahr vom 1. Januar bis zum gleichen Kalendertag. */
  previous: Totals;
}

export function yearToDate(entries: ProgressEntry[], today: string): YearToDate {
  const year = Number(today.slice(0, 4));
  const prevEnd = sameDayPreviousYear(today);
  return {
    year,
    current: sumBetween(entries, `${today.slice(0, 4)}-01-01`, today),
    previous: sumBetween(entries, `${prevEnd.slice(0, 4)}-01-01`, prevEnd),
  };
}

/** Veränderung in Prozent; ohne Vergleichswert (0 oder weniger) `null`, nie unendlich. */
export function changePercent(current: number, previous: number): number | null {
  if (!(previous > 0) || !Number.isFinite(current)) return null;
  return ((current - previous) / previous) * 100;
}

export interface WeekBucket {
  /** Montag der Woche. */
  start: string;
  total: Totals;
  bySport: Partial<Record<Sport, Totals>>;
}

/** Die letzten `weeks` Wochen bis zur Woche von `today`, älteste zuerst. */
export function weekly(
  entries: ProgressEntry[],
  today: string,
  weeks: number = PROGRESS_WEEKS,
): WeekBucket[] {
  const last = mondayOf(today);
  const first = addDays(last, -7 * (weeks - 1));
  const out: WeekBucket[] = [];
  for (let i = 0; i < weeks; i++) {
    out.push({ start: addDays(first, 7 * i), total: emptyTotals(), bySport: {} });
  }
  for (const e of entries) {
    if (e.date < first || e.date > today) continue;
    const bucket = out[Math.floor(daysBetween(first, e.date) / 7)];
    if (!bucket) continue;
    add(bucket.total, e);
    add((bucket.bySport[e.sport] ??= emptyTotals()), e);
  }
  return out;
}

export interface MonthBucket {
  /** YYYY-MM */
  month: string;
  total: Totals;
}

function monthKey(year: number, monthIndex: number): string {
  const y = year + Math.floor(monthIndex / 12);
  const m = ((monthIndex % 12) + 12) % 12;
  return `${String(y).padStart(4, '0')}-${String(m + 1).padStart(2, '0')}`;
}

/** Die letzten `months` Monate bis zum Monat von `today`, neueste zuerst. */
export function monthly(
  entries: ProgressEntry[],
  today: string,
  months: number = PROGRESS_MONTHS,
): MonthBucket[] {
  const year = Number(today.slice(0, 4));
  const month = Number(today.slice(5, 7)) - 1;
  const out: MonthBucket[] = [];
  const index = new Map<string, MonthBucket>();
  for (let i = 0; i < months; i++) {
    const b = { month: monthKey(year, month - i), total: emptyTotals() };
    out.push(b);
    index.set(b.month, b);
  }
  for (const e of entries) {
    if (e.date > today) continue;
    const b = index.get(e.date.slice(0, 7));
    if (b) add(b.total, e);
  }
  return out;
}

/**
 * Summenkurve eines Jahres: Wert am Tag i (0 = 1. Januar) ist die Summe vom 1. Januar bis
 * einschließlich dieses Tages. Die Reihe endet an `until` (höchstens am 31. Dezember).
 */
export function cumulative(
  entries: ProgressEntry[],
  year: number,
  until: string,
  metric: ProgressMetric,
): number[] {
  const start = `${String(year).padStart(4, '0')}-01-01`;
  const yearEnd = `${String(year).padStart(4, '0')}-12-31`;
  const end = until < yearEnd ? until : yearEnd;
  if (end < start) return [];
  const length = daysBetween(start, end) + 1;
  const daily = new Array<number>(length).fill(0);
  for (const e of entries) {
    if (e.date < start || e.date > end) continue;
    const t = emptyTotals();
    add(t, e);
    daily[daysBetween(start, e.date)] += metricValue(t, metric);
  }
  let acc = 0;
  return daily.map((v) => (acc += v));
}

/** Tag des Jahres, 0 = 1. Januar. */
export function dayOfYear(iso: string): number {
  return daysBetween(`${iso.slice(0, 4)}-01-01`, iso);
}

/** Zahl der Tage eines Jahres. */
export function daysInYear(year: number): number {
  return daysBetween(`${year}-01-01`, `${year + 1}-01-01`);
}

/** Monat (1 bis 12) eines Datums, für Achsenbeschriftungen. */
export function monthOf(iso: string): number {
  return fromIso(iso).getUTCMonth() + 1;
}
