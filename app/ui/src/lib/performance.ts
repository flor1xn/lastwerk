// Seite "Leistung" (T33): Deutung und Formatierung ohne Rechnung. Die Werte und Codes kommen aus
// `get_performance` (Rechnung in `lastwerk-core`), hier wird nur gezählt und beschriftet.

import type {
  Locale,
  PerfFtpSuggestion,
  PerfPower,
  PerfRunBest,
  PerfThreshold,
  PowerCurveCode,
  ThresholdKind,
} from './backend/types';
import { formatDuration, formatNumber, formatPace } from './format';

/** Dauern der Tabelle: Eckpunkte der Kurve, die man im Training kennt. Konvention. */
export const TABLE_DURATIONS = [1, 5, 15, 30, 60, 120, 300, 600, 1200, 1800, 3600, 7200];

/** Gesamtdeutung: mehr Dauern gestiegen als gesunken und umgekehrt, wie im Dashboard. */
export function curveSummary(power: PerfPower[]): PowerCurveCode {
  const known = power.filter((p) => p.trend !== 'power_curve.no_data');
  if (known.length === 0) return 'power_curve.no_data';
  const rising = known.filter((p) => p.trend === 'power_curve.rising').length;
  const falling = known.filter((p) => p.trend === 'power_curve.falling').length;
  if (rising > falling) return 'power_curve.rising';
  if (falling > rising) return 'power_curve.falling';
  return 'power_curve.flat';
}

/** Zahl der Dauern mit Wert und davon gestiegene (für die Aussage). */
export function curveCounts(power: PerfPower[]): { known: number; rising: number } {
  return {
    known: power.filter((p) => p.current !== null).length,
    rising: power.filter((p) => p.trend === 'power_curve.rising').length,
  };
}

/** Dauer einer Leistungskurve: 45 s, 1:30 min, 20 min, 1:30 h. */
export function formatCurveDuration(seconds: number): string {
  if (seconds < 60) return `${seconds} s`;
  if (seconds < 3600) {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return s === 0 ? `${m} min` : `${m}:${String(s).padStart(2, '0')} min`;
  }
  const h = Math.floor(seconds / 3600);
  const m = Math.round((seconds % 3600) / 60);
  return m === 0 ? `${h} h` : `${h}:${String(m).padStart(2, '0')} h`;
}

/** Strecken mit Wert und davon schneller als im Vorzeitraum (T37). */
export function runCounts(bests: PerfRunBest[]): {
  known: number;
  compared: number;
  faster: number;
} {
  const compared = bests.filter((b) => b.current !== null && b.previous_s !== null);
  return {
    known: bests.filter((b) => b.current !== null).length,
    compared: compared.length,
    faster: compared.filter((b) => b.current!.value < b.previous_s!).length,
  };
}

/** Zeitunterschied mit Vorzeichen (U+2212 für minus), zum Beispiel −0:12 oder +1:05:00. */
export function formatTimeDiff(seconds: number): string {
  if (!Number.isFinite(seconds)) return '';
  const sign = seconds < 0 ? '\u2212' : seconds > 0 ? '+' : '±';
  return `${sign}${formatDuration(Math.abs(seconds))}`;
}

/** Schlüssel der Streckenbezeichnung in `ui.perf.run.distance`. */
export function distanceKey(meters: number): string {
  return `ui.perf.run.distance.d${meters}`;
}

/** Zustand des Abschnitts "Schwellen" (T36), nur aus den gelieferten Werten. */
export type ThresholdView =
  | { kind: 'empty' }
  | { kind: 'above'; ftp: PerfThreshold; suggestion: PerfFtpSuggestion }
  | { kind: 'current'; ftp: PerfThreshold }
  | { kind: 'no_ftp'; suggestion: PerfFtpSuggestion }
  | { kind: 'others' };

/** Gültiger FTP heute: der jüngste gesetzte Wert (die Liste ist nach Art und Datum sortiert). */
export function currentFtp(thresholds: PerfThreshold[]): PerfThreshold | null {
  const ftps = thresholds.filter((t) => t.kind === 'ftp_w');
  return ftps.length === 0 ? null : ftps[ftps.length - 1];
}

/** Liegt der jüngste Vorschlag über dem gültigen FTP, weist die Seite darauf hin. Ein reiner
 * Vergleich zweier Zahlen, keine Schwelle. */
export function thresholdView(
  thresholds: PerfThreshold[],
  suggestions: PerfFtpSuggestion[],
): ThresholdView {
  const ftp = currentFtp(thresholds);
  const latest = suggestions.length === 0 ? null : suggestions[suggestions.length - 1];
  if (ftp && latest && latest.value_w > ftp.value)
    return { kind: 'above', ftp, suggestion: latest };
  if (ftp) return { kind: 'current', ftp };
  if (latest) return { kind: 'no_ftp', suggestion: latest };
  return thresholds.length > 0 ? { kind: 'others' } : { kind: 'empty' };
}

/** Wert einer Schwelle mit Einheit: W, bpm, Tempo je km (Lauf) oder je 100 m (CSS). */
export function formatThresholdValue(kind: ThresholdKind, value: number, locale: Locale): string {
  switch (kind) {
    case 'ftp_w':
      return `${formatNumber(value, locale, 0)} W`;
    case 'lthr_bpm':
    case 'hr_max_bpm':
    case 'hr_rest_bpm':
      return `${formatNumber(value, locale, 0)} bpm`;
    case 'run_speed_mps':
      return `${formatPace(value)} /km`;
    case 'css_mps':
      return value > 0 ? `${formatDuration(100 / value)} /100 m` : formatPace(null);
  }
}

/** Gültiger Wert je Tag aus Stufen (sortiert nach Datum); vor der ersten Stufe `null`. */
export function stepValueOn(
  steps: { valid_from: string; value: number }[],
  iso: string,
): number | null {
  let v: number | null = null;
  for (const s of steps) if (s.valid_from <= iso) v = s.value;
  return v;
}
