// Seite "Leistung" der Attrappe (T33): erfundene, deterministische Bestwerte je Dauer. Die
// Kurve ist eine glatte, fallende Folge ohne physiologisches Modell (keine Quelle nötig, nur
// Anschauung). Der Vergleich mit dem Vorzeitraum folgt den Regeln von `core` (Band 2 Prozent).
//
// Varianten wie beim Dashboard: `empty` ohne Werte, `sparse` nur Dauern bis 20 min und ohne
// Vorzeitraum, `extreme` sehr hohe Werte, alle übrigen wie `rich`.

import { addDays } from '../dates';
import type {
  Performance,
  PerfFtpSuggestion,
  PerfPower,
  PerfRange,
  PerfThreshold,
  PowerCurveCode,
} from './types';
import type { DashboardVariant } from './mock-dashboard';

/** Raster wie `POWER_CURVE_DURATIONS_S` in `lastwerk-core`. */
export const MOCK_CURVE_DURATIONS = [
  1, 5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 300, 420, 600, 900, 1200, 1800, 2700, 3600, 5400,
  7200,
];

/** Erfundene Stützwerte (Watt), dazwischen geometrisch verbunden. */
const ANCHORS: [number, number][] = [
  [1, 1050],
  [5, 980],
  [30, 640],
  [60, 470],
  [300, 330],
  [1200, 282],
  [3600, 252],
  [7200, 225],
];

function shape(d: number): number {
  for (let i = 1; i < ANCHORS.length; i++) {
    const [d0, w0] = ANCHORS[i - 1];
    const [d1, w1] = ANCHORS[i];
    if (d <= d1) {
      const f = Math.log(d / d0) / Math.log(d1 / d0);
      return w0 * Math.pow(w1 / w0, f);
    }
  }
  return ANCHORS[ANCHORS.length - 1][1];
}

function trendOf(change: number | null): PowerCurveCode {
  if (change === null) return 'power_curve.no_data';
  if (change > 2) return 'power_curve.rising';
  if (change < -2) return 'power_curve.falling';
  return 'power_curve.flat';
}

export function buildPerformance(
  today: string,
  range: PerfRange,
  variant: DashboardVariant,
  activityIds: number[],
  runIds: number[],
  thresholds: PerfThreshold[],
  mulberry32: (seed: number) => () => number,
): Performance {
  const days = range === 'all' ? null : Number(range);
  const from = days === null ? null : addDays(today, -(days - 1));
  const previous_from = days === null || from === null ? null : addDays(from, -days);
  const rnd = mulberry32(range.length * 7919 + 3);
  // Längere Zeiträume enthalten mehr Maximalbelastungen: etwas höhere Werte.
  const level = days === null ? 1.04 : days >= 365 ? 1.02 : days >= 90 ? 1 : 0.97;
  const power: PerfPower[] = MOCK_CURVE_DURATIONS.map((duration_s, i) => {
    const has =
      variant !== 'empty' &&
      (variant !== 'sparse' || duration_s <= 1200) &&
      !(range === '42' && duration_s === 7200);
    const scale = variant === 'extreme' ? 2.1 : level;
    const value = has
      ? Math.round(shape(duration_s) * scale * (0.99 + rnd() * 0.02) * 10) / 10
      : null;
    const prev =
      value === null || days === null || variant === 'sparse'
        ? null
        : Math.round(value * (0.95 + rnd() * 0.08) * 10) / 10;
    const change =
      value !== null && prev !== null ? Math.round(((value - prev) / prev) * 1000) / 10 : null;
    const date = addDays(today, -Math.floor(rnd() * Math.min(days ?? 400, 400)));
    return {
      duration_s,
      current:
        value === null
          ? null
          : { value, date, activity_id: activityIds[i % Math.max(activityIds.length, 1)] ?? 1 },
      previous_w: prev,
      change_percent: change,
      trend: trendOf(change),
    };
  });
  // Bestzeiten im Laufen: erfundene Zeiten mit leicht fallendem Tempo über die Strecke.
  const RUN: [number, number][] = [
    [1000, 228],
    [5000, 1245],
    [10_000, 2610],
    [21_097, 5790],
    [42_195, 0],
  ];
  const run_bests = RUN.map(([distance_m, base], i) => {
    const has = base > 0 && variant !== 'empty' && (variant !== 'sparse' || distance_m <= 5000);
    const seconds = has
      ? Math.round(base * (variant === 'extreme' ? 0.55 : 1) * (0.99 + rnd() * 0.03))
      : null;
    const prev =
      seconds === null || days === null || variant === 'sparse'
        ? null
        : Math.round(seconds * (0.98 + rnd() * 0.05));
    return {
      distance_m,
      current:
        seconds === null
          ? null
          : {
              value: seconds,
              date: addDays(today, -Math.floor(rnd() * Math.min(days ?? 400, 400))),
              activity_id: runIds[i % Math.max(runIds.length, 1)] ?? 1,
            },
      previous_s: prev,
      change_percent:
        seconds !== null && prev !== null
          ? Math.round(((seconds - prev) / prev) * 1000) / 10
          : null,
    };
  });
  // FTP-Vorschläge: je Monat des Zeitraums (höchstens 12) 95 Prozent einer erfundenen besten
  // 20-Minuten-Leistung, die langsam steigt; Monate ohne Leistung fehlen.
  const ftp_suggestions: PerfFtpSuggestion[] = [];
  if (variant !== 'empty') {
    const months = Math.min(12, days === null ? 12 : Math.max(1, Math.ceil(days / 30)));
    for (let i = months - 1; i >= 0; i--) {
      if (variant === 'sparse' && i > 0) continue;
      const date = addDays(today, -i * 30 - Math.floor(rnd() * 10));
      if (from !== null && date < from) continue;
      if (rnd() < 0.15) continue;
      const best =
        Math.round(
          (262 + (months - i) * 1.6 + rnd() * 8) * (variant === 'extreme' ? 2.1 : 1) * 10,
        ) / 10;
      ftp_suggestions.push({
        month: date.slice(0, 7),
        value_w: Math.round(best * 0.95 * 10) / 10,
        best_20min_w: best,
        date,
        activity_id: activityIds[i % Math.max(activityIds.length, 1)] ?? 1,
      });
    }
  }
  const byMonth = new Map(ftp_suggestions.map((s) => [s.month, s]));
  return {
    today,
    range,
    from,
    previous_from,
    power,
    run_bests,
    thresholds: thresholds
      .filter((t) => ['ftp_w', 'lthr_bpm', 'run_speed_mps', 'css_mps'].includes(t.kind))
      .filter((t) => t.valid_from <= today)
      .sort((a, b) => a.kind.localeCompare(b.kind) || a.valid_from.localeCompare(b.valid_from)),
    ftp_suggestions: [...byMonth.values()].sort((a, b) => a.month.localeCompare(b.month)),
  };
}
