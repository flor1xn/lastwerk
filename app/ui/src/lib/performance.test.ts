import { describe, expect, it } from 'vitest';
import type { PerfPower, PowerCurveCode } from './backend/types';
import {
  curveCounts,
  curveSummary,
  formatCurveDuration,
  formatThresholdValue,
  formatTimeDiff,
  runCounts,
  stepValueOn,
  thresholdView,
} from './performance';

function p(duration_s: number, value: number | null, trend: PowerCurveCode): PerfPower {
  return {
    duration_s,
    current: value === null ? null : { value, date: '2026-10-01', activity_id: 1 },
    previous_w: null,
    change_percent: null,
    trend,
  };
}

describe('Leistungskurve (T33)', () => {
  it('fasst die Deutung zusammen wie das Dashboard', () => {
    expect(curveSummary([])).toBe('power_curve.no_data');
    expect(
      curveSummary([
        p(5, 900, 'power_curve.rising'),
        p(60, 400, 'power_curve.rising'),
        p(300, 300, 'power_curve.falling'),
      ]),
    ).toBe('power_curve.rising');
    expect(curveSummary([p(5, 900, 'power_curve.rising'), p(60, 400, 'power_curve.falling')])).toBe(
      'power_curve.flat',
    );
    expect(
      curveCounts([p(5, 900, 'power_curve.rising'), p(60, null, 'power_curve.no_data')]),
    ).toEqual({ known: 1, rising: 1 });
  });

  it('beschriftet Dauern ohne Rundungsfehler', () => {
    expect(formatCurveDuration(1)).toBe('1 s');
    expect(formatCurveDuration(45)).toBe('45 s');
    expect(formatCurveDuration(90)).toBe('1:30 min');
    expect(formatCurveDuration(420)).toBe('7 min');
    expect(formatCurveDuration(3600)).toBe('1 h');
    expect(formatCurveDuration(5400)).toBe('1:30 h');
  });
});

describe('Laufbestzeiten (T37)', () => {
  it('zählt schnellere Strecken und formatiert den Unterschied', () => {
    const best = (d: number, cur: number | null, prev: number | null) => ({
      distance_m: d,
      current: cur === null ? null : { value: cur, date: '2026-10-01', activity_id: 1 },
      previous_s: prev,
      change_percent: null,
    });
    expect(
      runCounts([
        best(1000, 230, 240),
        best(5000, 1250, 1240),
        best(10_000, 2600, null),
        best(21_097, null, 5800),
      ]),
    ).toEqual({ known: 3, compared: 2, faster: 1 });
    expect(formatTimeDiff(-12)).toBe('\u22120:12');
    expect(formatTimeDiff(65)).toBe('+1:05');
    expect(formatTimeDiff(0)).toBe('±0:00');
  });
});

describe('Schwellen (T36)', () => {
  const ftp = (value: number, valid_from: string) => ({
    kind: 'ftp_w' as const,
    sport: 'cycling' as const,
    value,
    valid_from,
    origin: 'manual' as const,
  });
  const sugg = (month: string, value_w: number) => ({
    month,
    value_w,
    best_20min_w: value_w / 0.95,
    date: `${month}-10`,
    activity_id: 1,
  });

  it('wählt den Zustand aus FTP und jüngstem Vorschlag', () => {
    expect(thresholdView([], []).kind).toBe('empty');
    expect(
      thresholdView([ftp(250, '2026-01-01'), ftp(260, '2026-08-01')], [sugg('2026-09', 268)]),
    ).toMatchObject({ kind: 'above', ftp: { value: 260 } });
    expect(thresholdView([ftp(270, '2026-08-01')], [sugg('2026-09', 268)]).kind).toBe('current');
    expect(thresholdView([], [sugg('2026-09', 268)]).kind).toBe('no_ftp');
    expect(
      thresholdView(
        [{ kind: 'lthr_bpm', sport: null, value: 168, valid_from: '2026-01-01', origin: 'manual' }],
        [],
      ).kind,
    ).toBe('others');
  });

  it('schlägt den gültigen Wert einer Stufe nach', () => {
    const steps = [ftp(250, '2026-03-01'), ftp(260, '2026-08-01')];
    expect(stepValueOn(steps, '2026-02-28')).toBeNull();
    expect(stepValueOn(steps, '2026-03-01')).toBe(250);
    expect(stepValueOn(steps, '2026-10-15')).toBe(260);
  });

  it('formatiert Schwellen je Art', () => {
    expect(formatThresholdValue('ftp_w', 262.4, 'de')).toBe('262 W');
    expect(formatThresholdValue('lthr_bpm', 168, 'en')).toBe('168 bpm');
    // 1000 / 4 m/s = 250 s = 4:10 je km; 100 / 1,25 m/s = 80 s = 1:20 je 100 m.
    expect(formatThresholdValue('run_speed_mps', 4, 'de')).toBe('4:10 /km');
    expect(formatThresholdValue('css_mps', 1.25, 'de')).toBe('1:20 /100 m');
  });
});
