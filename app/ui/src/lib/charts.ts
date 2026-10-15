// Optionen für die Diagramme (uPlot, Entscheidung O11). Reine Funktionen ohne DOM-Zugriff,
// damit sie sich ohne Canvas testen lassen. Farben kommen als Parameter aus den CSS-Variablen.

import uPlot from 'uplot';
import type { ActivityDetail, Locale, PmcPoint } from './backend/types';
import {
  formatDate,
  formatDayMonth,
  formatDuration,
  formatNumber,
  MIN_PACE_SPEED_MPS,
  MISSING,
} from './format';

export interface ChartColors {
  /** Schriftfamilie für Achsen, aus --font-sans. Canvas braucht eine fertige Zeichenkette. */
  font: string;
  text: string;
  muted: string;
  grid: string;
  ctl: string;
  atl: string;
  tsb: string;
  power: string;
  heart: string;
  accent: string;
  /** Flächen und Schrift der vier Formzustände (nur im Verlauf der Startseite). */
  bandFill: Record<BandKey, string>;
  bandText: Record<BandKey, string>;
}

export type BandKey = 'fresh' | 'balanced' | 'loaded' | 'overloaded';

/** Beschriftungen der Reihen, aus den Sprachdateien. */
export interface PmcLabels {
  date: string;
  ctl: string;
  atl: string;
  tsb: string;
}

export interface SeriesLabels {
  time: string;
  power: string;
  heart: string;
  pace?: string;
}

/** Sekunden seit 1970 für Mitternacht UTC eines Kalendertags. */
export function epochOf(isoDate: string): number {
  return Date.parse(`${isoDate}T00:00:00Z`) / 1000;
}

export function isoOfEpoch(seconds: number): string {
  return new Date(seconds * 1000).toISOString().slice(0, 10);
}

/** Achsenschrift: Cairo ist kleiner im Bild als eine Systemschrift, deshalb 13 statt 12 Pixel. */
const AXIS_FONT_SIZE = 13;

/**
 * Gemeinsame Achsen: Beschriftung in der Schrift der Oberfläche, Gitter nur als feine Linie,
 * keine Teilstriche und kein Rahmen (SPEC 5.5). Die Linien tragen die Daten, nicht das Gitter.
 */
export function axisBase(colors: ChartColors): uPlot.Axis {
  return {
    stroke: colors.muted,
    font: `${AXIS_FONT_SIZE}px ${colors.font}`,
    grid: { stroke: colors.grid, width: 1 },
    ticks: { show: false },
    border: { show: false },
    gap: 8,
  };
}

export function pmcData(points: PmcPoint[]): uPlot.AlignedData {
  return [
    points.map((p) => epochOf(p.date)),
    points.map((p) => p.ctl),
    points.map((p) => p.atl),
    points.map((p) => p.tsb),
  ];
}

export function pmcOptions(
  colors: ChartColors,
  locale: Locale,
  labels: PmcLabels,
): Omit<uPlot.Options, 'width' | 'height'> {
  const value = (_u: uPlot, v: number | null) =>
    v === null ? MISSING : formatNumber(v, locale, 1);
  const line = (label: string, stroke: string): uPlot.Series => ({
    label,
    stroke,
    width: 2,
    value,
    points: { show: false },
  });
  return {
    // Tage sind UTC-Mitternacht, Achsen sollen keine Ortszeit-Verschiebung zeigen.
    tzDate: (ts) => uPlot.tzDate(new Date(ts * 1000), 'Etc/UTC'),
    scales: { x: { time: true } },
    cursor: { drag: { x: false, y: false } },
    // Die Legende der Bibliothek zeigt Beschriftung und Wert in einer Zeile mit Doppelpunkt und
    // Platzhalter. Stattdessen steht unter dem Diagramm die eigene (PmcChart).
    legend: { show: false },
    axes: [
      {
        ...axisBase(colors),
        grid: { show: false },
        values: (_u, splits) => splits.map((s) => formatDayMonth(isoOfEpoch(s), locale)),
      },
      { ...axisBase(colors), size: 40 },
    ],
    series: [
      {
        label: labels.date,
        value: (_u, v) => (v === null ? '' : formatDate(isoOfEpoch(v), locale)),
      },
      line(labels.ctl, colors.ctl),
      line(labels.atl, colors.atl),
      line(labels.tsb, colors.tsb),
    ],
  };
}

/** Leistung (links) und Herzfrequenz (rechts) über der Zeit. Leere Reihen entfallen. */
/** Tempo in Sekunden je Kilometer, unter der Anzeigegrenze fehlend (Gehen, Stehen). */
export function paceSeconds(speed: (number | null)[]): (number | null)[] {
  return speed.map((v) => (v === null || v < MIN_PACE_SPEED_MPS ? null : 1000 / v));
}

export function activitySeriesData(d: ActivityDetail, pace = false): uPlot.AlignedData {
  const base: uPlot.AlignedData = [d.series.t_s, d.series.power_w, d.series.hr_bpm];
  return pace ? [...base, paceSeconds(d.series.speed_mps ?? [])] : base;
}

export function activitySeriesOptions(
  colors: ChartColors,
  locale: Locale,
  labels: SeriesLabels,
  channels: { power: boolean; heart: boolean; pace?: boolean },
): Omit<uPlot.Options, 'width' | 'height'> {
  const value = (_u: uPlot, v: number | null) =>
    v === null ? MISSING : formatNumber(v, locale, 0);
  const pace = channels.pace === true;
  // Tempo: kleinere Zahl ist schneller, die Achse läuft deshalb von oben nach unten (dir -1).
  const paceAxis: uPlot.Axis[] = pace
    ? [
        {
          ...axisBase(colors),
          scale: 'pace',
          side: channels.power ? 1 : 3,
          size: 52,
          stroke: colors.accent,
          grid: { show: false },
          values: (_u, splits) => splits.map((s) => formatDuration(s)),
        },
      ]
    : [];
  const paceSeries: uPlot.Series[] = pace
    ? [
        {
          label: labels.pace ?? '',
          scale: 'pace',
          stroke: colors.accent,
          width: 1.5,
          value: (_u, v) => (v === null ? MISSING : formatDuration(v)),
          points: { show: false },
        },
      ]
    : [];
  return {
    scales: { x: { time: false }, w: { auto: true }, bpm: { auto: true }, pace: { dir: -1 } },
    cursor: { drag: { x: false, y: false } },
    axes: [
      {
        ...axisBase(colors),
        grid: { show: false },
        values: (_u, splits) => splits.map((s) => formatDuration(s)),
      },
      { ...axisBase(colors), scale: 'w', size: 46, stroke: colors.power, show: channels.power },
      {
        ...axisBase(colors),
        scale: 'bpm',
        side: 1,
        size: 46,
        stroke: colors.heart,
        grid: { show: false },
        show: channels.heart,
      },
      ...paceAxis,
    ],
    series: [
      { label: labels.time, value: (_u, v) => (v === null ? '' : formatDuration(v)) },
      {
        label: labels.power,
        scale: 'w',
        stroke: colors.power,
        width: 1.5,
        value,
        show: channels.power,
        points: { show: false },
      },
      {
        label: labels.heart,
        scale: 'bpm',
        stroke: colors.heart,
        width: 1.5,
        value,
        show: channels.heart,
        points: { show: false },
      },
      ...paceSeries,
    ],
  };
}

/** Liest die Farben aus den CSS-Variablen des Wurzelelements. */
export function readChartColors(root: HTMLElement = document.documentElement): ChartColors {
  const style = getComputedStyle(root);
  const v = (name: string, fallback: string) => style.getPropertyValue(name).trim() || fallback;
  return {
    font: v('--font-sans', 'system-ui, sans-serif'),
    text: v('--text', '#dfe4ea'),
    muted: v('--muted', '#8b97a5'),
    grid: v('--grid', '#28303a'),
    ctl: v('--pmc-ctl', '#4f9de0'),
    atl: v('--pmc-atl', '#ee7aa2'),
    tsb: v('--pmc-tsb', '#7bc47f'),
    power: v('--power', '#e8a33d'),
    heart: v('--heart', '#ef6b63'),
    accent: v('--accent', '#c8f169'),
    bandFill: {
      fresh: v('--form-fresh-bg', '#142630'),
      balanced: v('--form-balanced-bg', '#22262a'),
      loaded: v('--form-loaded-bg', '#3a2e19'),
      overloaded: v('--form-overloaded-bg', '#3a1d1a'),
    },
    bandText: {
      fresh: v('--form-fresh', '#7cc4e8'),
      balanced: v('--form-balanced', '#d4d7da'),
      loaded: v('--form-loaded', '#f0b35a'),
      overloaded: v('--form-overloaded', '#f29486'),
    },
  };
}
