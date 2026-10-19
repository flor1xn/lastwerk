// Diagramme der Seite "Erholung" (T34): je Größe der Tageswert mit eigener Basislinie und
// Untergrenze aus R12, niedrige Tage als eigene Punkte, Body Battery als Spanne Tief bis Hoch,
// die Tageslast als Balken. Alle teilen die Zeitachse und den Zeiger (uPlot `cursor.sync`).
// Reine Funktionen ohne DOM.

import uPlot from 'uplot';
import type { Locale, RecoveryPagePoint } from './backend/types';
import { axisBase, epochOf, isoOfEpoch, timeAxis, type ChartColors } from './charts';
import { withAlpha } from './charts-dashboard';
import { formatDate, formatNumber, MISSING } from './format';

/** Gemeinsamer Schlüssel: Der Zeiger steht in allen Diagrammen der Seite am selben Tag. */
export const RECOVERY_SYNC = 'lastwerk-recovery';

export type RecoveryChartKey = 'hrv' | 'resting_hr' | 'sleep';

interface Picker {
  value: (p: RecoveryPagePoint) => number | null;
  base: (p: RecoveryPagePoint) => number | null;
  floor: (p: RecoveryPagePoint) => number | null;
}

const PICKS: Record<RecoveryChartKey, Picker> = {
  hrv: { value: (p) => p.hrv_ms, base: (p) => p.hrv_base, floor: (p) => p.hrv_floor },
  resting_hr: {
    value: (p) => p.resting_hr_bpm,
    base: (p) => p.resting_hr_base,
    floor: () => null,
  },
  sleep: { value: (p) => p.sleep_h, base: (p) => p.sleep_base_h, floor: (p) => p.sleep_floor_h },
};

/** x, Wert, Basislinie, Untergrenze, Wert nur an niedrigen Tagen. */
export function metricData(points: RecoveryPagePoint[], key: RecoveryChartKey): uPlot.AlignedData {
  const p = PICKS[key];
  return [
    points.map((x) => epochOf(x.date)),
    points.map(p.value),
    points.map(p.base),
    points.map(p.floor),
    points.map((x) => (x.low && key !== 'resting_hr' ? p.value(x) : null)),
  ];
}

export interface MetricLabels {
  date: string;
  value: string;
  base: string;
  floor: string;
  low: string;
}

function base(
  colors: ChartColors,
  locale: Locale,
  date: string,
): Pick<uPlot.Options, 'tzDate' | 'cursor' | 'legend'> & { dateSeries: uPlot.Series } {
  return {
    tzDate: (ts) => uPlot.tzDate(new Date(ts * 1000), 'Etc/UTC'),
    cursor: { drag: { x: false, y: false }, sync: { key: RECOVERY_SYNC } },
    legend: { show: false },
    dateSeries: {
      label: date,
      value: (_u, v) => (v === null ? '' : formatDate(isoOfEpoch(v), locale)),
    },
  };
}

export function metricOptions(
  colors: ChartColors,
  locale: Locale,
  labels: MetricLabels,
  digits: number,
  color: string,
): Omit<uPlot.Options, 'width' | 'height'> {
  const fmt = (_u: uPlot, v: number | null) =>
    v === null ? MISSING : formatNumber(v, locale, digits);
  const { dateSeries, ...common } = base(colors, locale, labels.date);
  return {
    ...common,
    scales: { x: { time: true } },
    axes: [
      timeAxis(colors, locale),
      {
        ...axisBase(colors),
        size: 44,
        values: (_u, splits) => splits.map((v) => formatNumber(v, locale, digits > 1 ? 1 : 0)),
      },
    ],
    series: [
      dateSeries,
      {
        label: labels.value,
        stroke: withAlpha(color, 0.55),
        width: 1,
        points: { show: true, size: 4, fill: color, stroke: color },
        value: fmt,
      },
      { label: labels.base, stroke: colors.text, width: 1.5, points: { show: false }, value: fmt },
      {
        label: labels.floor,
        stroke: colors.bandText.overloaded,
        width: 1.5,
        dash: [5, 4],
        points: { show: false },
        value: fmt,
      },
      {
        label: labels.low,
        stroke: colors.bandText.overloaded,
        width: 0,
        paths: () => null,
        points: {
          show: true,
          size: 9,
          fill: colors.bandText.overloaded,
          stroke: colors.bandText.overloaded,
        },
        value: fmt,
      },
    ],
  };
}

/** Body Battery: Hoch und Tief des Tages, die Fläche dazwischen ist die Spanne. */
export function batteryData(points: RecoveryPagePoint[]): uPlot.AlignedData {
  return [
    points.map((x) => epochOf(x.date)),
    points.map((x) => x.body_battery_high),
    points.map((x) => x.body_battery_low),
  ];
}

export function batteryOptions(
  colors: ChartColors,
  locale: Locale,
  labels: { date: string; high: string; low: string },
): Omit<uPlot.Options, 'width' | 'height'> {
  const fmt = (_u: uPlot, v: number | null) => (v === null ? MISSING : formatNumber(v, locale, 0));
  const { dateSeries, ...common } = base(colors, locale, labels.date);
  return {
    ...common,
    scales: { x: { time: true }, y: { range: [0, 100] } },
    axes: [timeAxis(colors, locale), { ...axisBase(colors), size: 44 }],
    bands: [{ series: [1, 2], fill: withAlpha(colors.text, 0.1) }],
    series: [
      dateSeries,
      {
        label: labels.high,
        stroke: colors.text,
        width: 1.5,
        points: { show: false },
        value: fmt,
      },
      { label: labels.low, stroke: colors.muted, width: 1, points: { show: false }, value: fmt },
    ],
  };
}

export function loadData(points: RecoveryPagePoint[]): uPlot.AlignedData {
  return [points.map((x) => epochOf(x.date)), points.map((x) => x.load)];
}

export function loadOptions(
  colors: ChartColors,
  locale: Locale,
  labels: { date: string; load: string },
): Omit<uPlot.Options, 'width' | 'height'> {
  const { dateSeries, ...common } = base(colors, locale, labels.date);
  return {
    ...common,
    scales: { x: { time: true }, y: { range: (_u, _min, max) => [0, Math.max(10, max)] } },
    axes: [timeAxis(colors, locale), { ...axisBase(colors), size: 44 }],
    series: [
      dateSeries,
      {
        label: labels.load,
        stroke: colors.muted,
        fill: withAlpha(colors.muted, 0.5),
        width: 0,
        paths: uPlot.paths.bars!({ size: [0.7, 10] }),
        points: { show: false },
        value: (_u, v) => (v === null ? MISSING : formatNumber(v, locale, 0)),
      },
    ],
  };
}
