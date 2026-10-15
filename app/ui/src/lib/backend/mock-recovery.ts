// Seite "Erholung" der Attrappe (T34): erfundene, deterministische Tageswerte über ein Jahr mit
// leichten Wellen, einzelnen Lücken und einigen schlechten Nächten. Basislinie und Untergrenze
// folgen den Regeln von R12 (7 Tage davor, mindestens 4 mit Wert, HRV ab 15 Prozent, Schlaf ab
// 20 Prozent darunter); maßgeblich bleibt `lastwerk-core`.
//
// Varianten wie beim Dashboard: `empty` und `gaps` ohne Erholungswerte, `sparse` nur die letzten
// zwei Wochen, `low` mit niedrigen letzten Tagen, `extreme` große Ausschläge.

import { addDays } from '../dates';
import type { DashboardVariant } from './mock-dashboard';
import type {
  ReadinessCode,
  RecoveryPage,
  RecoveryPagePoint,
  RecoveryRange,
  RecoverySummary,
} from './types';

const BASELINE_DAYS = 7;
const MIN_BASELINE_DAYS = 4;
const HRV_DROP = 0.15;
const SLEEP_DEFICIT = 0.2;

interface Raw {
  date: string;
  hrv: number | null;
  rhr: number | null;
  sleep: number | null;
  score: number | null;
  bbHigh: number | null;
  bbLow: number | null;
  load: number | null;
}

const r1 = (v: number) => Math.round(v * 10) / 10;
const r2 = (v: number) => Math.round(v * 100) / 100;

function rawDays(today: string, variant: DashboardVariant, rnd: () => number): Raw[] {
  const out: Raw[] = [];
  const total = 365 + BASELINE_DAYS;
  for (let i = total - 1; i >= 0; i--) {
    const date = addDays(today, -i);
    const wave = Math.sin(i / 11) * 0.06 + Math.sin(i / 47) * 0.04;
    const bad = rnd() < 0.05;
    const has = variant !== 'empty' && variant !== 'gaps' && (variant !== 'sparse' || i < 14);
    const missing = rnd() < 0.04;
    const amp = variant === 'extreme' ? 3 : 1;
    const lowTail = variant === 'low' && i < 2;
    if (!has || missing) {
      out.push({
        date,
        hrv: null,
        rhr: null,
        sleep: null,
        score: null,
        bbHigh: null,
        bbLow: null,
        load: null,
      });
      continue;
    }
    const hrv = 62 * (1 + wave * amp) * (bad || lowTail ? 0.78 : 1) + (rnd() - 0.5) * 3;
    const sleep = 7.3 * (1 + wave * 0.5) * (bad || lowTail ? 0.75 : 1) + (rnd() - 0.5) * 0.4;
    const bbHigh = Math.round(Math.min(100, 78 + wave * 80 + (rnd() - 0.5) * 10 - (bad ? 20 : 0)));
    out.push({
      date,
      hrv: r1(Math.max(10, hrv)),
      rhr: r1(48 * (1 - wave * 0.4) + (bad ? 4 : 0) + (rnd() - 0.5) * 2),
      sleep: r2(Math.max(2, sleep)),
      score: Math.round(Math.min(100, 72 + wave * 90 - (bad ? 25 : 0) + (rnd() - 0.5) * 8)),
      bbHigh,
      bbLow: Math.max(5, bbHigh - 45 - Math.round(rnd() * 20)),
      load: rnd() < 0.7 ? r1(40 + rnd() * 110) : null,
    });
  }
  return out;
}

function baseOf(rows: Raw[], index: number, pick: (r: Raw) => number | null): number | null {
  const values = rows
    .slice(Math.max(0, index - BASELINE_DAYS), index)
    .map(pick)
    .filter((v): v is number => v !== null && v > 0);
  if (values.length < MIN_BASELINE_DAYS) return null;
  return values.reduce((s, v) => s + v, 0) / values.length;
}

function mean(values: (number | null)[]): number | null {
  const v = values.filter((x): x is number => x !== null);
  return v.length === 0 ? null : v.reduce((s, x) => s + x, 0) / v.length;
}

export function buildRecovery(
  today: string,
  days: RecoveryRange,
  variant: DashboardVariant,
  mulberry32: (seed: number) => () => number,
): RecoveryPage {
  const rows = rawDays(today, variant, mulberry32(4242));
  const start = rows.length - days;
  const points: RecoveryPagePoint[] = rows.slice(start).map((r, k) => {
    const i = start + k;
    const hrvBase = baseOf(rows, i, (x) => x.hrv);
    const sleepBase = baseOf(rows, i, (x) => x.sleep);
    const hrvFloor = hrvBase === null ? null : hrvBase * (1 - HRV_DROP);
    const sleepFloor = sleepBase === null ? null : sleepBase * (1 - SLEEP_DEFICIT);
    const low =
      (r.hrv !== null && hrvFloor !== null && r.hrv <= hrvFloor) ||
      (r.sleep !== null && sleepFloor !== null && r.sleep <= sleepFloor);
    const rhrBase = baseOf(rows, i, (x) => x.rhr);
    return {
      date: r.date,
      hrv_ms: r.hrv,
      resting_hr_bpm: r.rhr,
      sleep_h: r.sleep,
      sleep_score: r.score,
      body_battery_high: r.bbHigh,
      body_battery_low: r.bbLow,
      hrv_base: hrvBase === null ? null : r1(hrvBase),
      hrv_floor: hrvFloor === null ? null : r1(hrvFloor),
      resting_hr_base: rhrBase === null ? null : r1(rhrBase),
      sleep_base_h: sleepBase === null ? null : r2(sleepBase),
      sleep_floor_h: sleepFloor === null ? null : r2(sleepFloor),
      low,
      load: r.load,
    };
  });
  const recent = points.slice(-7);
  const summary = (
    metric: RecoverySummary['metric'],
    pick: (p: RecoveryPagePoint) => number | null,
    digits: number,
  ): RecoverySummary => {
    const round = (v: number | null) =>
      v === null ? null : Math.round(v * 10 ** digits) / 10 ** digits;
    return { metric, recent: round(mean(recent.map(pick))), period: round(mean(points.map(pick))) };
  };
  const withValue = points.filter((p) => p.hrv_ms !== null || p.sleep_h !== null);
  const last = withValue[withValue.length - 1] ?? null;
  const fresh = last !== null && last.date >= addDays(today, -3);
  const state: ReadinessCode =
    !fresh || (last.hrv_base === null && last.sleep_base_h === null)
      ? 'readiness.no_data'
      : last.low
        ? 'readiness.low'
        : 'readiness.normal';
  return {
    today,
    days,
    state,
    last_date: last?.date ?? null,
    low_days: points.filter((p) => p.low).length,
    hrv_drop_percent: HRV_DROP * 100,
    sleep_deficit_percent: SLEEP_DEFICIT * 100,
    points,
    summary: [
      summary('hrv', (p) => p.hrv_ms, 1),
      summary('resting_hr', (p) => p.resting_hr_bpm, 1),
      summary('sleep', (p) => p.sleep_h, 2),
      summary('sleep_score', (p) => p.sleep_score, 0),
      summary('body_battery_high', (p) => p.body_battery_high, 0),
    ],
  };
}
