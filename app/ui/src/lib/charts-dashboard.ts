// Diagramme der Startseite (T25 Teil B): Formverlauf mit TSB-Bändern und Prognose, Rampe und
// Effizienzfaktor. Reine Funktionen ohne DOM-Zugriff, Farben kommen als Parameter. Die
// Grundlagen (Achsen, Farben, Epochen) stehen in charts.ts.

import uPlot from 'uplot';
import type { Locale } from './backend/types';
import {
  axisBase,
  timeAxis,
  epochOf,
  isoOfEpoch,
  type BandKey,
  type ChartColors,
  type PmcLabels,
} from './charts';
import type { PmcSplit } from './dashboard';
import { formatDate, formatNumber, MISSING } from './format';
import { FORM_BALANCED_FROM, FORM_FRESH_FROM, FORM_LOADED_FROM } from './today';

/** Farbe mit Deckkraft. Nur für #rrggbb, alles andere bleibt unverändert. */
export function withAlpha(color: string, alpha: number): string {
  if (!/^#[0-9a-f]{6}$/i.test(color)) return color;
  const a = Math.round(Math.min(1, Math.max(0, alpha)) * 255)
    .toString(16)
    .padStart(2, '0');
  return `${color}${a}`;
}

/** Zustandsbänder des TSB (SPEC 5.5), von oben nach unten. */
export const FORM_BANDS: { key: BandKey; from: number; to: number }[] = [
  { key: 'fresh', from: FORM_FRESH_FROM, to: Infinity },
  { key: 'balanced', from: FORM_BALANCED_FROM, to: FORM_FRESH_FROM },
  { key: 'loaded', from: FORM_LOADED_FROM, to: FORM_BALANCED_FROM },
  { key: 'overloaded', from: -Infinity, to: FORM_LOADED_FROM },
];

/** Zusätze für den Verlauf der Startseite: Prognose und TSB-Bänder der Formzustände. */
export interface DashPmcExtras {
  /** Die Daten tragen drei weitere Reihen mit der Prognose (gestrichelt). */
  projection: boolean;
  /** Namen der Bänder, als Text im Diagramm (die Farbe allein trägt nichts). */
  bands: Record<BandKey, string>;
  /** Trennlinie zwischen Verlauf und Prognose: Tag und Beschriftung. */
  today?: { date: string; label: string };
}

/** Breite des rechten Rands für die Namen der Bänder (Faustregel für zwölf Pixel Text). */
export const BAND_GUTTER = 84;

/**
 * Zeichnet die vier Formzustände als sehr leise Flächen hinter den Linien und schreibt ihre
 * Namen rechts neben die Zeichenfläche, damit sie keine Linie überdecken (Befund der
 * Bestandsaufnahme, docs/designsprache.md).
 */
function drawBands(u: uPlot, colors: ChartColors, names: Record<BandKey, string>): void {
  const yMin = u.scales.y.min;
  const yMax = u.scales.y.max;
  if (yMin == null || yMax == null) return;
  const { left, top, width, height } = u.bbox;
  const px = uPlot.pxRatio;
  const ctx = u.ctx;
  ctx.save();
  ctx.font = `500 ${12 * px}px ${colors.font}`;
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'left';
  for (const band of FORM_BANDS) {
    const hi = Math.min(band.to, yMax);
    const lo = Math.max(band.from, yMin);
    if (hi <= lo) continue;
    const y0 = Math.max(top, u.valToPos(hi, 'y', true));
    const y1 = Math.min(top + height, u.valToPos(lo, 'y', true));
    ctx.globalAlpha = 0.35;
    ctx.fillStyle = colors.bandFill[band.key];
    ctx.fillRect(left, y0, width, y1 - y0);
    if (y1 - y0 > 14 * px) {
      ctx.globalAlpha = 1;
      ctx.fillStyle = colors.bandText[band.key];
      ctx.fillText(names[band.key], left + width + 10 * px, (y0 + y1) / 2);
    }
  }
  ctx.restore();
}

/** Senkrechte gestrichelte Linie am heutigen Tag mit Beschriftung unten links der Linie. */
function drawToday(u: uPlot, colors: ChartColors, today: { date: string; label: string }): void {
  const x = u.valToPos(epochOf(today.date), 'x', true);
  const { left, top, width, height } = u.bbox;
  if (x < left || x > left + width) return;
  const px = uPlot.pxRatio;
  const ctx = u.ctx;
  ctx.save();
  ctx.strokeStyle = colors.muted;
  ctx.globalAlpha = 0.55;
  ctx.lineWidth = px;
  ctx.setLineDash([3 * px, 4 * px]);
  ctx.beginPath();
  ctx.moveTo(x, top);
  ctx.lineTo(x, top + height);
  ctx.stroke();
  ctx.setLineDash([]);
  ctx.globalAlpha = 1;
  ctx.fillStyle = colors.muted;
  ctx.font = `600 ${12 * px}px ${colors.font}`;
  ctx.textBaseline = 'bottom';
  ctx.textAlign = 'right';
  ctx.fillText(today.label, x - 6 * px, top + height - 4 * px);
  ctx.restore();
}

/** Verlauf mit Prognose: sechs Reihen, die drei letzten (gestrichelt) tragen nur die Prognose. */
export function pmcProjectionData(split: PmcSplit): uPlot.AlignedData {
  return [
    split.dates.map(epochOf),
    split.ctl,
    split.atl,
    split.tsb,
    split.ctlProjected,
    split.atlProjected,
    split.tsbProjected,
  ];
}

/** Optionen für Fitness, Ermüdung und Form mit TSB-Bändern und gestrichelter Prognose. */
export function dashPmcOptions(
  colors: ChartColors,
  locale: Locale,
  labels: PmcLabels,
  extras: DashPmcExtras,
): Omit<uPlot.Options, 'width' | 'height'> {
  const value = (_u: uPlot, v: number | null) =>
    v === null ? MISSING : formatNumber(v, locale, 1);
  const line = (label: string, stroke: string, dashed: boolean): uPlot.Series => ({
    label,
    stroke,
    width: 2,
    value,
    points: { show: false },
    ...(dashed ? { dash: [7, 5] } : {}),
  });
  const series: uPlot.Series[] = [
    {
      label: labels.date,
      value: (_u, v) => (v === null ? '' : formatDate(isoOfEpoch(v), locale)),
    },
    line(labels.ctl, colors.ctl, false),
    line(labels.atl, colors.atl, false),
    line(labels.tsb, colors.tsb, false),
  ];
  if (extras.projection) {
    series.push(
      line(labels.ctl, colors.ctl, true),
      line(labels.atl, colors.atl, true),
      line(labels.tsb, colors.tsb, true),
    );
  }
  const { today } = extras;
  return {
    tzDate: (ts) => uPlot.tzDate(new Date(ts * 1000), 'Etc/UTC'),
    scales: { x: { time: true } },
    cursor: { drag: { x: false, y: false } },
    legend: { show: false },
    padding: [null, BAND_GUTTER, null, null],
    hooks: {
      drawClear: [(u) => drawBands(u, colors, extras.bands)],
      ...(today ? { draw: [(u: uPlot) => drawToday(u, colors, today)] } : {}),
    },
    axes: [timeAxis(colors, locale), { ...axisBase(colors), size: 40 }],
    series,
  };
}

export interface RampLabels {
  date: string;
  ramp: string;
  limit: string;
}

export function rampData(series: { date: string; ramp: number }[]): uPlot.AlignedData {
  return [series.map((p) => epochOf(p.date)), series.map((p) => p.ramp)];
}

/** Rampe: CTL-Punkte je Woche als Fläche über null, dazu die Grenze aus R1 gestrichelt. */
export function rampOptions(
  colors: ChartColors,
  locale: Locale,
  labels: RampLabels,
  limit: number,
): Omit<uPlot.Options, 'width' | 'height'> {
  return {
    tzDate: (ts) => uPlot.tzDate(new Date(ts * 1000), 'Etc/UTC'),
    scales: {
      x: { time: true },
      // Null und die Grenze bleiben immer im Bild, sonst wirkt eine kleine Rampe steil.
      y: { range: (_u, min, max) => [Math.min(min, -1), Math.max(max, limit * 1.3)] },
    },
    cursor: { drag: { x: false, y: false } },
    legend: { show: false },
    hooks: {
      draw: [
        (u) => {
          const y = u.valToPos(limit, 'y', true);
          const { left, top, width, height } = u.bbox;
          if (y < top || y > top + height) return;
          const px = uPlot.pxRatio;
          const ctx = u.ctx;
          ctx.save();
          ctx.strokeStyle = colors.atl;
          ctx.lineWidth = px;
          ctx.setLineDash([5 * px, 4 * px]);
          ctx.beginPath();
          ctx.moveTo(left, y);
          ctx.lineTo(left + width, y);
          ctx.stroke();
          ctx.setLineDash([]);
          ctx.fillStyle = colors.atl;
          ctx.font = `600 ${12 * px}px ${colors.font}`;
          ctx.textBaseline = 'bottom';
          ctx.textAlign = 'right';
          ctx.fillText(labels.limit, left + width - 6 * px, y - 4 * px);
          ctx.restore();
        },
      ],
    },
    axes: [timeAxis(colors, locale), { ...axisBase(colors), size: 36 }],
    series: [
      {
        label: labels.date,
        value: (_u, v) => (v === null ? '' : formatDate(isoOfEpoch(v), locale)),
      },
      {
        label: labels.ramp,
        stroke: colors.ctl,
        width: 2,
        fill: withAlpha(colors.ctl, 0.14),
        fillTo: () => 0,
        points: { show: false },
        value: (_u, v) => (v === null ? MISSING : formatNumber(v, locale, 1)),
      },
    ],
  };
}

export interface EfLabels {
  date: string;
  ef: string;
}

export function efData(series: { date: string; ef: number }[]): uPlot.AlignedData {
  return [series.map((p) => epochOf(p.date)), series.map((p) => p.ef)];
}

/** Effizienzfaktor: gleitendes 28-Tage-Mittel als Linie. */
export function efOptions(
  colors: ChartColors,
  locale: Locale,
  labels: EfLabels,
): Omit<uPlot.Options, 'width' | 'height'> {
  return {
    tzDate: (ts) => uPlot.tzDate(new Date(ts * 1000), 'Etc/UTC'),
    scales: { x: { time: true } },
    cursor: { drag: { x: false, y: false } },
    legend: { show: false },
    axes: [
      timeAxis(colors, locale),
      {
        ...axisBase(colors),
        size: 44,
        values: (_u, splits) => splits.map((v) => formatNumber(v, locale, 2)),
      },
    ],
    series: [
      {
        label: labels.date,
        value: (_u, v) => (v === null ? '' : formatDate(isoOfEpoch(v), locale)),
      },
      {
        label: labels.ef,
        stroke: colors.power,
        width: 2,
        fill: withAlpha(colors.power, 0.1),
        points: { show: false },
        value: (_u, v) => (v === null ? MISSING : formatNumber(v, locale, 2)),
      },
    ],
  };
}
