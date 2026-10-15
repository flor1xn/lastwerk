import { describe, expect, it } from 'vitest';
import type { ActivityItem, Plan, PlanSession } from './backend/types';
import { buildMonth, dayState, monthCounts, monthRange, shiftMonth } from './calendar';

function act(id: number, date: string, load: number | null, primary = true): ActivityItem {
  return {
    id,
    local_date: date,
    start_utc: `${date}T07:00:00Z`,
    sport: 'running',
    sub_sport: null,
    duration_s: 3600,
    distance_m: 10_000,
    avg_hr: null,
    avg_power_w: null,
    load: {
      status: load === null ? 'missing_threshold' : 'ok',
      value: load,
      method: load === null ? null : 'r_tss',
      intensity_factor: null,
      estimated: false,
    },
    is_primary: primary,
    duplicate_of: primary ? null : 1,
  };
}

function session(
  minutes: number,
  load: number,
  sport: PlanSession['sport'] = 'running',
): PlanSession {
  return {
    slot: 'am',
    sport,
    template_id: 'run.easy',
    duration_min: minutes,
    planned_load: load,
    hard: false,
  };
}

// Plan: eine Woche ab Montag, 5. Oktober 2026. Di 6. und Do 8. je eine Einheit, Sa 10. zwei,
// So 11. Ruhetag als Eintrag mit Sportart "rest".
const PLAN = {
  weeks: [
    {
      week_start: '2026-10-05',
      phase: 'base',
      target_load: 200,
      days: [
        { date: '2026-10-06', sessions: [session(45, 40)] },
        { date: '2026-10-08', sessions: [session(60, 55)] },
        { date: '2026-10-10', sessions: [session(90, 80), { ...session(30, 20), slot: 'pm' }] },
        { date: '2026-10-11', sessions: [session(0, 0, 'rest')] },
      ],
    },
  ],
} as unknown as Plan;

describe('Monatsraster (T35)', () => {
  it('deckt ganze Wochen von Montag bis Sonntag ab', () => {
    // Oktober 2026: 1. ist ein Donnerstag, 31. ein Samstag.
    expect(monthRange('2026-10')).toEqual({ first: '2026-09-28', last: '2026-11-01' });
    // Februar 2027 beginnt an einem Montag und endet an einem Sonntag.
    expect(monthRange('2027-02')).toEqual({ first: '2027-02-01', last: '2027-02-28' });
    expect(monthRange('2026-12').last).toBe('2027-01-03');
  });

  it('blättert über den Jahreswechsel', () => {
    expect(shiftMonth('2026-12', 1)).toBe('2027-01');
    expect(shiftMonth('2026-01', -1)).toBe('2025-12');
    expect(shiftMonth('2026-10', -12)).toBe('2025-10');
  });
});

describe('Zustand eines Tages', () => {
  it('folgt nur der Zählung', () => {
    const today = '2026-10-15';
    expect(dayState('2026-10-14', today, 1, 1, true)).toBe('done');
    expect(dayState('2026-10-14', today, 2, 1, true)).toBe('partial');
    expect(dayState('2026-10-14', today, 1, 0, true)).toBe('missed');
    expect(dayState(today, today, 1, 0, true)).toBe('planned');
    expect(dayState('2026-10-16', today, 1, 0, true)).toBe('planned');
    expect(dayState('2026-10-14', today, 0, 1, true)).toBe('extra');
    expect(dayState('2026-10-14', today, 0, 1, false)).toBe('trained');
    expect(dayState('2026-10-14', today, 0, 0, true)).toBe('rest');
    expect(dayState('2026-10-14', today, 0, 0, false)).toBe('empty');
  });
});

describe('buildMonth und monthCounts', () => {
  const today = '2026-10-09';
  const acts = [
    act(1, '2026-10-06', 42),
    act(2, '2026-10-07', null),
    act(3, '2026-10-07', 30, false), // Duplikat zählt nicht
  ];
  const weeks = buildMonth('2026-10', today, acts, PLAN);
  const day = (d: string) => weeks.flatMap((w) => w.days).find((x) => x.date === d)!;

  it('setzt Zustände aus Plan und Aktivitäten', () => {
    expect(day('2026-10-06').state).toBe('done');
    expect(day('2026-10-07').state).toBe('extra');
    expect(day('2026-10-07').activities).toHaveLength(1);
    expect(day('2026-10-08').state).toBe('missed');
    expect(day('2026-10-10').state).toBe('planned');
    expect(day('2026-10-11').planned).toHaveLength(0); // Ruhetag zählt nicht als Einheit
    expect(day('2026-10-11').covered).toBe(true);
    expect(day('2026-10-05').state).toBe('rest');
    expect(day('2026-10-12').covered).toBe(false);
    expect(day('2026-09-28').inMonth).toBe(false);
  });

  it('summiert je Woche Ist und Soll', () => {
    const w = weeks.find((x) => x.start === '2026-10-05')!;
    // Ist: nur berechenbare Last 42, Dauer 2 * 3600 s. Soll: 40 + 55 + 80 + 20 = 195, 225 min.
    expect(w.load).toBe(42);
    expect(w.durationS).toBe(7200);
    expect(w.plannedLoad).toBe(195);
    expect(w.plannedMin).toBe(225);
  });

  it('zählt erledigte und verpasste Einheiten bis gestern', () => {
    // Bis 8. Oktober geplant: 6. (1) und 8. (1); erledigt nur der 6.; verpasst ein Tag.
    expect(monthCounts(weeks, today)).toEqual({
      plannedPast: 2,
      donePast: 1,
      missedDays: 1,
      sessions: 2,
    });
  });
});
