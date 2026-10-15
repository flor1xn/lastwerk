// Dashboard der Attrappe (T25 Teil B): frei erfundene, deterministische Werte für alle
// Abschnitte. Sie hängen nicht an den Aktivitäten der Attrappe, sondern bilden eine eigene
// kleine Geschichte (Aufbau in Vierwochenblöcken, eine Urlaubslücke, leichter Trainingszuwachs).
// Die Schwellen entsprechen den Regeln der Rechenkerne (R1, R8, R12 und die Bänder der
// Kennzahlen), maßgeblich bleiben `lastwerk-core` und der Service in `app/src-tauri`.
//
// Varianten für Tests und die Sichtprüfung (im Browser mit `?dashboard=<name>`):
// - rich: alles gefüllt, Erholung unauffällig
// - low: wie rich, Erholungswerte deutlich unter der Basis (löst R12 aus)
// - sparse: wenig Daten, die meisten Abschnitte sagen, was fehlt
// - empty: nichts importiert, keine Erholungswerte
// - extreme: sehr große Werte, tägliche Last in jedem Kalendertag, steile Rampe
// - gaps: Daten, aber ohne Erholungswerte und ohne Zeit in Zonen

import { addDays, daysBetween, mondayOf } from '../dates';
import type {
  AdherenceCode,
  DashAdherence,
  DashEfficiency,
  DashPowerDuration,
  DashRamp,
  DashReadiness,
  DashRecoveryPoint,
  DashRecoveryValue,
  Dashboard,
  DistributionCode,
  EfficiencyCode,
  FormCode,
  PlanDetail,
  PmcPoint,
  PmcSettings,
  PowerCurveCode,
  RampCode,
  ReadinessCode,
} from './types';

export const DASHBOARD_VARIANTS = ['rich', 'low', 'sparse', 'empty', 'extreme', 'gaps'] as const;
export type DashboardVariant = (typeof DASHBOARD_VARIANTS)[number];

export function isDashboardVariant(v: unknown): v is DashboardVariant {
  return typeof v === 'string' && (DASHBOARD_VARIANTS as readonly string[]).includes(v);
}

/** Was die Erzeugung von der übrigen Attrappe braucht (vermeidet einen Import im Kreis). */
export interface DashboardDeps {
  mulberry32(seed: number): () => number;
  computePmc(
    daily: ReadonlyMap<string, number>,
    from: string,
    to: string,
    p: PmcSettings,
  ): PmcPoint[];
  pmc: PmcSettings;
}

// Grenzen wie in den Regeln (catalog/rules.default.json) und den Kennzahlen (Faustregeln).
const R1_MAX_RISE = 5;
const R8_MIN_EASY = 70;
const MIN_ZONE_S = 3600;
const STEADY_BAND = 1;
const TREND_BAND = 2;
const R12_HRV_DROP = 15;
const R12_SLEEP_DROP = 20;

const r1 = (v: number) => Math.round(v * 10) / 10;
const r2 = (v: number) => Math.round(v * 100) / 100;

function formCode(tsb: number): FormCode {
  if (tsb >= 5) return 'form.fresh';
  if (tsb >= -10) return 'form.balanced';
  if (tsb >= -30) return 'form.loaded';
  return 'form.overloaded';
}

function rampCode(ramp: number | null): RampCode {
  if (ramp === null) return 'ramp.no_data';
  if (ramp > R1_MAX_RISE + 1e-3) return 'ramp.steep';
  if (ramp >= STEADY_BAND) return 'ramp.building';
  if (ramp <= -STEADY_BAND) return 'ramp.falling';
  return 'ramp.steady';
}

function trendCode<P extends 'efficiency' | 'power_curve'>(
  prefix: P,
  change: number | null,
): `${P}.${'no_data' | 'falling' | 'flat' | 'rising'}` {
  if (change === null) return `${prefix}.no_data`;
  if (change >= TREND_BAND - 1e-9) return `${prefix}.rising`;
  if (change <= -TREND_BAND + 1e-9) return `${prefix}.falling`;
  return `${prefix}.flat`;
}

/** Tageslast mit Aufbauwochen, einer Erholungswoche je vier Wochen und einer Urlaubslücke. */
function syntheticLoads(
  today: string,
  days: number,
  rnd: () => number,
  scale: number,
  dense: boolean,
): Map<string, number> {
  const out = new Map<string, number>();
  const perWeekday = [0, 48, 78, 40, 26, 118, 66];
  const block = [0.85, 1.0, 1.15, 0.6];
  for (let back = days - 1; back >= 0; back--) {
    const date = addDays(today, -back);
    const weekday = (new Date(`${date}T00:00:00Z`).getUTCDay() + 6) % 7;
    const index = days - 1 - back;
    // Urlaubslücke: elf Tage ohne Training, rund 150 Tage vor heute.
    if (!dense && back >= 150 && back < 161) continue;
    const week = Math.floor(index / 7);
    const season = 0.75 + 0.25 * Math.min(1, index / days);
    let base = perWeekday[weekday] * block[week % 4] * season * scale;
    if (dense && base === 0) base = 20 * scale;
    if (base === 0) continue;
    if (!dense && rnd() < 0.1) continue;
    out.set(date, r1(base * (0.85 + rnd() * 0.3)));
  }
  return out;
}

function rampBlock(pmc: PmcPoint[], today: string, limit = R1_MAX_RISE): DashRamp {
  const byDate = new Map(pmc.map((p) => [p.date, p.ctl]));
  const series: { date: string; ramp: number }[] = [];
  for (let back = 83; back >= 0; back--) {
    const d = addDays(today, -back);
    const now = byDate.get(d);
    const then = byDate.get(addDays(d, -7));
    if (now !== undefined && then !== undefined) series.push({ date: d, ramp: r1(now - then) });
  }
  const current =
    series.length > 0 && series[series.length - 1].date === today
      ? series[series.length - 1].ramp
      : null;
  return { current, state: rampCode(current), limit, series };
}

function distributionBlock(today: string, rnd: () => number, variant: DashboardVariant) {
  const first = addDays(mondayOf(today), -49);
  const weeks = Array.from({ length: 8 }, (_, i) => {
    const total_s = Math.round((4 + rnd() * 5) * 3600);
    // Eine Woche mit viel Intensität, sonst überwiegend locker.
    const easy = i === 4 ? 56 : 74 + rnd() * 12;
    return {
      week_start: addDays(first, i * 7),
      easy_percent: r1(variant === 'extreme' ? 100 : easy),
      total_s,
    };
  });
  const last4 = weeks.slice(-4);
  const total = last4.reduce((s, w) => s + w.total_s, 0);
  const easy = last4.reduce((s, w) => s + (w.easy_percent ?? 0) * w.total_s, 0) / total;
  const state: DistributionCode =
    total < MIN_ZONE_S
      ? 'distribution.no_data'
      : easy >= R8_MIN_EASY - 0.1
        ? 'distribution.easy_dominant'
        : 'distribution.intensity_heavy';
  return {
    window_easy_percent: r1(easy),
    window_total_s: total,
    min_easy_percent: R8_MIN_EASY,
    state,
    weeks,
  };
}

function adherenceBlock(
  today: string,
  plan: PlanDetail | null,
  done: ReadonlyMap<string, number>,
): DashAdherence {
  if (!plan || plan.plan.weeks.length === 0) {
    return { ratio_percent: null, state: 'adherence.no_data', weeks: [] };
  }
  const planned = new Map<string, number>();
  for (const w of plan.plan.weeks) {
    for (const d of w.days) {
      const sum = d.sessions.reduce((s, x) => s + x.planned_load, 0);
      if (sum > 0) planned.set(d.date, sum);
    }
  }
  const asOf = addDays(today, -1);
  const sum = (m: ReadonlyMap<string, number>, from: string, to: string) => {
    let s = 0;
    for (let d = from; d <= to; d = addDays(d, 1)) s += m.get(d) ?? 0;
    return s;
  };
  const weeks = plan.plan.weeks.map((w) => {
    const end = addDays(w.week_start, 6);
    const upto = end < asOf ? end : asOf;
    const planned_to_date = upto >= w.week_start ? sum(planned, w.week_start, upto) : 0;
    const done_to_date = upto >= w.week_start ? sum(done, w.week_start, upto) : 0;
    return {
      week_start: w.week_start,
      planned_total: r1(sum(planned, w.week_start, end)),
      planned_to_date: r1(planned_to_date),
      done_to_date: r1(done_to_date),
      complete: asOf >= end,
      ratio_percent: planned_to_date > 0 ? r1((done_to_date / planned_to_date) * 100) : null,
    };
  });
  const current = [...weeks]
    .reverse()
    .find((w) => w.week_start <= today && w.ratio_percent !== null);
  const ratio = current?.ratio_percent ?? null;
  const state: AdherenceCode =
    ratio === null
      ? 'adherence.no_data'
      : ratio < 80 - 1e-4
        ? 'adherence.under'
        : ratio > 120 + 1e-4
          ? 'adherence.over'
          : 'adherence.on_track';
  return { ratio_percent: ratio, state, weeks };
}

function efficiencyBlock(today: string, rnd: () => number, drift: number): DashEfficiency {
  // Radeinheiten ab 45 Minuten, etwa zwei je Woche, Effizienzfaktor leicht steigend.
  const points: { date: string; ef: number }[] = [];
  for (let back = 130; back >= 0; back -= 3 + Math.floor(rnd() * 3)) {
    points.push({
      date: addDays(today, -back),
      ef: r2(1.5 + drift * ((130 - back) / 130) * 0.12 + (rnd() - 0.5) * 0.06),
    });
  }
  const mean = (end: string, days = 28) => {
    const from = addDays(end, -(days - 1));
    const v = points.filter((p) => p.date >= from && p.date <= end).map((p) => p.ef);
    return v.length >= 2 ? v.reduce((s, x) => s + x, 0) / v.length : null;
  };
  const series: { date: string; ef: number }[] = [];
  for (let back = 89; back >= 0; back--) {
    const d = addDays(today, -back);
    const m = mean(d);
    if (m !== null) series.push({ date: d, ef: r2(m) });
  }
  const current = mean(today);
  const previous = mean(addDays(today, -28));
  const change =
    current !== null && previous !== null ? r1(((current - previous) / previous) * 100) : null;
  const trend: EfficiencyCode = trendCode('efficiency', change);
  return {
    current: current === null ? null : r2(current),
    change_percent: change,
    trend,
    sessions: points.filter((p) => p.date >= addDays(today, -89)).length,
    series,
  };
}

function powerBlock(scale: number, changes: (number | null)[]): DashPowerDuration[] {
  const base = [1050, 520, 340, 285, 262];
  const durations = [5, 60, 300, 1200, 3600];
  return durations.map((duration_s, i) => {
    const change = changes[i];
    const current = base[i] * scale;
    const previous = change === null ? null : current / (1 + change / 100);
    const pct = change === null ? null : r1(change);
    const trend: PowerCurveCode = trendCode('power_curve', pct);
    return {
      duration_s,
      current_w: r1(current),
      previous_w: previous === null ? null : r1(previous),
      change_percent: pct,
      trend,
    };
  });
}

function readinessBlock(
  today: string,
  rnd: () => number,
  mode: 'normal' | 'low' | 'none' | 'short',
  plan: PlanDetail | null,
): DashReadiness {
  const empty: DashReadiness = {
    state: 'readiness.no_data',
    last_date: null,
    date: null,
    hrv: null,
    resting_hr: null,
    sleep_h: null,
    body_battery_high: null,
    body_battery_low: null,
    plan_adjusted: false,
    series: [],
  };
  if (mode === 'none') return empty;
  const days = mode === 'short' ? 2 : 60;
  const series: DashRecoveryPoint[] = [];
  for (let back = days - 1; back >= 0; back--) {
    // Ein paar Tage ohne Messung, das Ende (heute) ist immer da.
    if (back > 0 && back % 17 === 5) continue;
    const d = addDays(today, -back);
    let hrv = 58 + Math.sin(back / 6) * 4 + (rnd() - 0.5) * 6;
    let rhr = 48 + Math.cos(back / 8) * 1.5 + (rnd() - 0.5) * 2;
    let sleep = 7.3 + (rnd() - 0.5) * 1.1;
    if (back === 0 && mode === 'low') {
      hrv = 44;
      sleep = 5.8;
      rhr = 53;
    }
    series.push({ date: d, hrv_ms: r1(hrv), resting_hr_bpm: r1(rhr), sleep_h: r1(sleep) });
  }
  const latest = series[series.length - 1];
  const base = (pick: (p: DashRecoveryPoint) => number | null): number | null => {
    const v = series
      .filter((p) => p.date < latest.date && p.date >= addDays(latest.date, -7))
      .map(pick)
      .filter((x): x is number => x !== null);
    return v.length >= 4 ? v.reduce((s, x) => s + x, 0) / v.length : null;
  };
  const value = (v: number | null, b: number | null): DashRecoveryValue | null =>
    v === null
      ? null
      : {
          value: v,
          baseline: b === null ? null : r1(b),
          deviation_percent: b === null ? null : r1(((v - b) / b) * 100),
        };
  const hrv = value(
    latest.hrv_ms,
    base((p) => p.hrv_ms),
  );
  const sleep = value(
    latest.sleep_h,
    base((p) => p.sleep_h),
  );
  const rhr = value(
    latest.resting_hr_bpm,
    base((p) => p.resting_hr_bpm),
  );
  const known = hrv?.deviation_percent != null || sleep?.deviation_percent != null;
  const low =
    (hrv?.deviation_percent != null && -hrv.deviation_percent >= R12_HRV_DROP - 1e-9) ||
    (sleep?.deviation_percent != null && -sleep.deviation_percent >= R12_SLEEP_DROP - 1e-9);
  const state: ReadinessCode = !known
    ? 'readiness.no_data'
    : low
      ? 'readiness.low'
      : 'readiness.normal';
  return {
    state,
    last_date: latest.date,
    date: latest.date,
    hrv,
    resting_hr: rhr,
    sleep_h: sleep,
    body_battery_high: null,
    body_battery_low: null,
    plan_adjusted: state === 'readiness.low' && plan !== null,
    series,
  };
}

/** Das Dashboard der Attrappe für den Tag `today`. `plan` ist der aktive Plan oder null. */
export function buildDashboard(
  today: string,
  variant: DashboardVariant,
  plan: PlanDetail | null,
  deps: DashboardDeps,
): Dashboard {
  const rnd = deps.mulberry32(daysBetween('2020-01-01', today) * 31 + 7);
  const emptyRamp: DashRamp = {
    current: null,
    state: 'ramp.no_data',
    limit: R1_MAX_RISE,
    series: [],
  };
  if (variant === 'empty') {
    return {
      today,
      form: null,
      pmc: [],
      pmc_first_load_date: null,
      projection: [],
      plan: plan ? planInfo(plan) : null,
      ramp: emptyRamp,
      distribution: {
        window_easy_percent: null,
        window_total_s: 0,
        min_easy_percent: R8_MIN_EASY,
        state: 'distribution.no_data',
        weeks: Array.from({ length: 8 }, (_, i) => ({
          week_start: addDays(mondayOf(today), -49 + i * 7),
          easy_percent: null,
          total_s: 0,
        })),
      },
      adherence: {
        ratio_percent: null,
        state: 'adherence.no_data',
        weeks: adherenceBlock(today, plan, new Map()).weeks,
      },
      efficiency: {
        current: null,
        change_percent: null,
        trend: 'efficiency.no_data',
        sessions: 0,
        series: [],
      },
      power_curve: {
        durations: [5, 60, 300, 1200, 3600].map((duration_s) => ({
          duration_s,
          current_w: null,
          previous_w: null,
          change_percent: null,
          trend: 'power_curve.no_data' as const,
        })),
      },
      readiness: readinessBlock(today, rnd, 'none', plan),
      load_days: [],
    };
  }

  const sparse = variant === 'sparse';
  const extreme = variant === 'extreme';
  const days = sparse ? 12 : 365;
  const scale = extreme ? 9 : variant === 'gaps' ? 0.9 : 1;
  const daily = syntheticLoads(today, days, rnd, scale, extreme);
  const first = [...daily.keys()].sort()[0];
  const planned = new Map<string, number>();
  let planEnd: string | null = null;
  if (plan && plan.plan.weeks.length > 0) {
    for (const w of plan.plan.weeks) {
      for (const d of w.days) {
        const sum = d.sessions.reduce((s, x) => s + x.planned_load, 0);
        if (sum > 0) planned.set(d.date, sum);
      }
    }
    const lastWeek = plan.plan.weeks[plan.plan.weeks.length - 1];
    planEnd = addDays(lastWeek.week_start, 6);
  }
  const withPlan = new Map(daily);
  for (const [d, v] of planned) if (d > today) withPlan.set(d, v);
  const endAll = planEnd !== null && planEnd > today ? planEnd : today;
  const all = deps.computePmc(withPlan, first, endAll, deps.pmc);
  const upToToday = all.filter((p) => p.date <= today);
  const round = (p: PmcPoint): PmcPoint => ({
    date: p.date,
    load: r1(withPlan.get(p.date) ?? 0),
    ctl: r1(p.ctl),
    atl: r1(p.atl),
    tsb: r1(p.tsb),
  });
  const windowStart = addDays(today, -119);
  const pmc = upToToday.filter((p) => p.date >= windowStart).map(round);
  const projection = all.filter((p) => p.date > today).map(round);
  const last = upToToday[upToToday.length - 1];
  const done = new Map(daily);

  const changes: (number | null)[] = extreme
    ? [18, 14, 9, 7, 3]
    : sparse
      ? [null, null, null, null, null]
      : [3.1, 1.2, 2.6, 4.4, -0.8];

  return {
    today,
    form: last
      ? {
          tsb: r1(last.tsb),
          ctl: r1(last.ctl),
          atl: r1(last.atl),
          state: formCode(last.tsb),
        }
      : null,
    pmc,
    pmc_first_load_date: first ?? null,
    projection: plan ? projection : [],
    plan: plan ? planInfo(plan) : null,
    ramp: rampBlock(upToToday, today),
    distribution:
      variant === 'gaps' || sparse
        ? {
            window_easy_percent: null,
            window_total_s: 0,
            min_easy_percent: R8_MIN_EASY,
            state: 'distribution.no_data',
            weeks: Array.from({ length: 8 }, (_, i) => ({
              week_start: addDays(mondayOf(today), -49 + i * 7),
              easy_percent: null,
              total_s: 0,
            })),
          }
        : distributionBlock(today, rnd, variant),
    adherence: adherenceBlock(today, plan, done),
    efficiency: sparse
      ? {
          current: null,
          change_percent: null,
          trend: 'efficiency.no_data',
          sessions: 1,
          series: [],
        }
      : efficiencyBlock(today, rnd, extreme ? 6 : 1),
    power_curve: { durations: powerBlock(extreme ? 4 : 1, changes) },
    readiness: readinessBlock(
      today,
      rnd,
      variant === 'low' ? 'low' : variant === 'gaps' ? 'none' : sparse ? 'short' : 'normal',
      plan,
    ),
    load_days: [...daily.entries()]
      .filter(([d]) => d >= addDays(today, -364))
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([date, load]) => ({ date, load })),
  };
}

function planInfo(plan: PlanDetail) {
  const weeks = plan.plan.weeks;
  return {
    id: plan.plan.plan_id,
    first_day: weeks[0]?.week_start ?? '',
    last_day: weeks.length > 0 ? addDays(weeks[weeks.length - 1].week_start, 6) : '',
  };
}
