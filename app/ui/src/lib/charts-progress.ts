// Diagramm der Ansicht "Fortschritt" (T32): Summenkurve dieses Jahres gegen das Vorjahr über
// den Tag des Jahres. Reine Funktionen ohne DOM, Farben aus den CSS-Variablen.

import type uPlot from 'uplot';
import type { Locale } from './backend/types';
import { axisBase, type ChartColors } from './charts';
import { withAlpha } from './charts-dashboard';
import { formatNumber, MISSING } from './format';
import { intlTag } from './i18n';

export interface CumulativeLabels {
  day: string;
  current: string;
  previous: string;
}

/** Daten: x = Tag des Jahres, dann dieses Jahr und das Vorjahr (fehlende Tage null). */
export function cumulativeData(current: number[], previous: number[]): uPlot.AlignedData {
  const length = Math.max(current.length, previous.length);
  const x = Array.from({ length }, (_, i) => i);
  const pad = (v: number[]) => x.map((i) => (i < v.length ? v[i] : null));
  return [x, pad(current), pad(previous)];
}

/** Erster Tag jedes Monats als Tag des Jahres (Achsenteilung), für ein Nicht-Schaltjahr. */
const MONTH_STARTS = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

function monthName(index: number, locale: Locale): string {
  return new Intl.DateTimeFormat(intlTag(locale), { month: 'short', timeZone: 'UTC' }).format(
    new Date(Date.UTC(2021, index, 1)),
  );
}

export function cumulativeOptions(
  colors: ChartColors,
  locale: Locale,
  labels: CumulativeLabels,
  digits: number,
): Omit<uPlot.Options, 'width' | 'height'> {
  const value = (_u: uPlot, v: number | null) =>
    v === null ? MISSING : formatNumber(v, locale, digits);
  return {
    scales: { x: { time: false, range: [0, 365] }, y: { range: (_u, _min, max) => [0, max || 1] } },
    cursor: { drag: { x: false, y: false } },
    legend: { show: false },
    axes: [
      {
        ...axisBase(colors),
        grid: { show: false },
        // Schmal (unter 480 px) nur Quartalsanfänge, sonst überlappen die Monatsnamen.
        splits: (u) => (u.width < 480 ? MONTH_STARTS.filter((_, i) => i % 3 === 0) : MONTH_STARTS),
        values: (_u, splits) => splits.map((s) => monthName(MONTH_STARTS.indexOf(s), locale)),
      },
      {
        ...axisBase(colors),
        size: 52,
        values: (_u, splits) => splits.map((v) => formatNumber(v, locale, 0)),
      },
    ],
    series: [
      { label: labels.day },
      {
        label: labels.current,
        stroke: colors.accent,
        width: 2.5,
        fill: withAlpha(colors.accent, 0.08),
        points: { show: false },
        value,
      },
      {
        label: labels.previous,
        stroke: colors.muted,
        width: 1.5,
        dash: [5, 4],
        points: { show: false },
        value,
      },
    ],
  };
}
