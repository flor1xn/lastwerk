// Diagramm der Seite "Leistung" (T33): Leistungskurve über eine logarithmische Zeitachse, der
// Zeitraum als volle Linie, der Vorzeitraum gestrichelt. Reine Funktionen ohne DOM.

import uPlot from 'uplot';
import type { Locale, PerfFtpSuggestion, PerfPower, PerfThreshold } from './backend/types';
import { axisBase, epochOf, isoOfEpoch, type ChartColors } from './charts';
import { withAlpha } from './charts-dashboard';
import { formatDate, formatDayMonth, formatNumber, MISSING } from './format';
import { formatCurveDuration, stepValueOn } from './performance';

export interface CurveLabels {
  duration: string;
  current: string;
  previous: string;
}

export function curveData(power: PerfPower[]): uPlot.AlignedData {
  return [
    power.map((p) => p.duration_s),
    power.map((p) => p.current?.value ?? null),
    power.map((p) => p.previous_w),
  ];
}

/** Beschriftete Dauern der Achse (Konvention). */
const TICKS = [1, 5, 15, 60, 300, 1200, 3600, 7200];
const NARROW_TICKS = [1, 15, 300, 3600];

export function curveOptions(
  colors: ChartColors,
  locale: Locale,
  labels: CurveLabels,
  withPrevious: boolean,
): Omit<uPlot.Options, 'width' | 'height'> {
  const watts = (_u: uPlot, v: number | null) =>
    v === null ? MISSING : `${formatNumber(v, locale, 0)} W`;
  return {
    scales: {
      x: { time: false, distr: 3, log: 10, range: [1, 7200] },
      y: { range: (_u, _min, max) => [0, Math.max(100, Math.ceil((max * 1.08) / 50) * 50)] },
    },
    cursor: { drag: { x: false, y: false } },
    legend: { show: false },
    axes: [
      {
        ...axisBase(colors),
        grid: { show: false },
        // Feste Teilstriche; der Standardfilter der Logarithmusachse ließe nur Zehnerpotenzen stehen.
        space: 16,
        filter: (_u, splits) => splits,
        splits: (u) => (u.width < 520 ? NARROW_TICKS : TICKS),
        // Auf der logarithmischen Achse lässt uPlot Teilstriche ohne Platz leer (null).
        values: (_u, splits) => splits.map((s) => (s == null ? '' : formatCurveDuration(s))),
      },
      {
        ...axisBase(colors),
        size: 52,
        values: (_u, splits) => splits.map((v) => formatNumber(v, locale, 0)),
      },
    ],
    series: [
      { label: labels.duration, value: (_u, v) => (v === null ? '' : formatCurveDuration(v)) },
      {
        label: labels.current,
        stroke: colors.power,
        width: 2.5,
        fill: withAlpha(colors.power, 0.12),
        points: { show: true, size: 5, fill: colors.power },
        value: watts,
      },
      {
        label: labels.previous,
        stroke: colors.muted,
        width: 1.5,
        dash: [5, 4],
        points: { show: false },
        value: watts,
        show: withPrevious,
      },
    ],
  };
}

export interface FtpLabels {
  date: string;
  ftp: string;
  suggestion: string;
}

/**
 * FTP als Stufenlinie über die Zeit und die Monatsvorschläge als Punkte (T36). Die x-Achse sind
 * alle Tage mit einer Änderung oder einem Vorschlag, dazu `today`; der Wert der Stufe an jedem
 * Tag ist der dann gültige (Nachschlagen, keine Rechnung).
 */
export function ftpData(
  thresholds: PerfThreshold[],
  suggestions: PerfFtpSuggestion[],
  today: string,
): uPlot.AlignedData {
  const steps = thresholds.filter((t) => t.kind === 'ftp_w');
  const days = new Set<string>([today]);
  for (const s of steps) days.add(s.valid_from);
  for (const s of suggestions) days.add(s.date);
  const sorted = [...days].sort();
  const byDate = new Map(suggestions.map((s) => [s.date, s.value_w]));
  return [
    sorted.map(epochOf),
    sorted.map((d) => stepValueOn(steps, d)),
    sorted.map((d) => byDate.get(d) ?? null),
  ];
}

export function ftpOptions(
  colors: ChartColors,
  locale: Locale,
  labels: FtpLabels,
): Omit<uPlot.Options, 'width' | 'height'> {
  const watts = (_u: uPlot, v: number | null) =>
    v === null ? MISSING : `${formatNumber(v, locale, 0)} W`;
  return {
    tzDate: (ts) => uPlot.tzDate(new Date(ts * 1000), 'Etc/UTC'),
    scales: {
      x: { time: true },
      y: {
        range: (_u, min, max) => [
          Math.max(0, Math.floor((min * 0.9) / 10) * 10),
          Math.ceil((max * 1.06) / 10) * 10,
        ],
      },
    },
    cursor: { drag: { x: false, y: false } },
    legend: { show: false },
    axes: [
      {
        ...axisBase(colors),
        grid: { show: false },
        values: (_u, splits) => splits.map((s) => formatDayMonth(isoOfEpoch(s), locale)),
      },
      {
        ...axisBase(colors),
        size: 52,
        values: (_u, splits) => splits.map((v) => formatNumber(v, locale, 0)),
      },
    ],
    series: [
      {
        label: labels.date,
        value: (_u, v) => (v === null ? '' : formatDate(isoOfEpoch(v), locale)),
      },
      {
        label: labels.ftp,
        stroke: colors.power,
        width: 2.5,
        paths: uPlot.paths.stepped!({ align: 1 }),
        points: { show: false },
        spanGaps: true,
        value: watts,
      },
      {
        label: labels.suggestion,
        stroke: colors.accent,
        width: 0,
        paths: () => null,
        points: { show: true, size: 9, fill: colors.accent, stroke: colors.accent },
        value: watts,
      },
    ],
  };
}
