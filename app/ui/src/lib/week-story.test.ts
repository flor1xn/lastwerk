import { describe, expect, it } from 'vitest';
import type { ActivityItem, Plan, PlanSession } from './backend/types';
import { buildWeekStory } from './week-story';

function act(id: number, date: string, load: number | null, primary = true): ActivityItem {
  return {
    id,
    local_date: date,
    start_utc: `${date}T07:00:00Z`,
    sport: 'cycling',
    sub_sport: null,
    duration_s: 3600,
    distance_m: 30_000,
    avg_hr: null,
    avg_power_w: null,
    load: {
      status: load === null ? 'missing_threshold' : 'ok',
      value: load,
      method: load === null ? null : 'tss',
      intensity_factor: null,
      estimated: false,
      missing: load === null ? 'ftp_w' : null,
    },
    is_primary: primary,
    duplicate_of: primary ? null : 1,
  };
}

function session(load: number, hard = false, slot: 'am' | 'pm' = 'am'): PlanSession {
  return {
    slot,
    sport: 'cycling',
    template_id: hard ? 'bike.threshold.2x20' : 'bike.endurance.steady',
    duration_min: 60,
    planned_load: load,
    hard,
  };
}

// Zwei Planwochen ab Montag, 12. Oktober 2026. Woche 1: Mo 12. eine, Mi 14. (heute) eine
// harte, Sa 17. zwei Einheiten, sonst frei. Woche 2: Di 20. hart, Do 22. locker, So 25. Ruhe
// als Eintrag "rest".
const PLAN = {
  weeks: [
    {
      week_start: '2026-10-12',
      phase: 'base',
      target_load: 300,
      days: [
        { date: '2026-10-12', sessions: [session(50)] },
        { date: '2026-10-14', sessions: [session(90, true)] },
        { date: '2026-10-17', sessions: [session(70), session(30, false, 'pm')] },
      ],
    },
    {
      week_start: '2026-10-19',
      phase: 'base',
      target_load: 300,
      days: [
        { date: '2026-10-20', sessions: [session(95, true)] },
        { date: '2026-10-22', sessions: [session(55)] },
        {
          date: '2026-10-25',
          sessions: [{ ...session(0), sport: 'rest', template_id: 'rest.day', duration_min: 0 }],
        },
      ],
    },
  ],
} as unknown as Plan;

const TODAY = '2026-10-14';

describe('Wochen-Story (T44)', () => {
  it('zählt bis gestern erledigt gegen geplant, heute und Zukunft nie verpasst', () => {
    // Mo 12. erledigt (Last 48), Di 13. zusätzlich ohne Last, heute noch nichts.
    const s = buildWeekStory(TODAY, [act(1, '2026-10-12', 48), act(2, '2026-10-13', null)], PLAN);
    expect(s.weekStart).toBe('2026-10-12');
    expect(s.hasPlan).toBe(true);
    // Bis gestern geplant: nur Mo 12. (eine Einheit), erledigt eine.
    expect([s.plannedPast, s.donePast, s.missedDays]).toEqual([1, 1, 0]);
    // Ganze Woche: 1 + 1 + 2 = 4 Einheiten, Last 50 + 90 + 70 + 30 = 240.
    expect(s.plannedSessions).toBe(4);
    expect(s.plannedLoad).toBe(240);
    // Zwei Aktivitäten, Last nur die berechenbare: 48.
    expect([s.sessions, s.load]).toEqual([2, 48]);
    const state = Object.fromEntries(s.days.map((d) => [d.date, d.state]));
    expect(state['2026-10-12']).toBe('done');
    expect(state['2026-10-13']).toBe('extra');
    expect(state['2026-10-14']).toBe('planned');
    expect(state['2026-10-17']).toBe('planned');
    // Ein freier Tag in der Zukunft ist wie im Kalender leer, kein Ruhetag mit Zustand.
    expect(state['2026-10-15']).toBe('empty');
    expect(s.days.find((d) => d.date === TODAY)?.hard).toBe(true);
  });

  it('verpasst nur vergangene Tage, zwei Einheiten an einem Tag zählen einzeln', () => {
    // Sonntag 18.: Mo 12. ohne Aktivität (verpasst), Mi 14. erledigt, Sa 17. eine von zwei.
    const s = buildWeekStory(
      '2026-10-18',
      [act(1, '2026-10-14', 80), act(2, '2026-10-17', 60), act(3, '2026-10-17', 20, false)],
      PLAN,
    );
    // Geplant bis gestern 1 + 1 + 2 = 4, erledigt 0 + 1 + 1 = 2 (die Kopie zählt nicht).
    expect([s.plannedPast, s.donePast, s.missedDays]).toEqual([4, 2, 1]);
    expect(s.days.find((d) => d.date === '2026-10-17')?.state).toBe('partial');
    expect(s.load).toBe(140);
  });

  it('Aktivitäten nach heute zählen nicht mit', () => {
    const s = buildWeekStory(TODAY, [act(1, '2026-10-16', 50)], PLAN);
    expect([s.sessions, s.load]).toEqual([0, 0]);
  });

  it('blickt sieben Tage nach heute voraus: harte Tage, Ruhetage, nächster harter Tag', () => {
    const s = buildWeekStory(TODAY, [], PLAN);
    // 15. bis 21. Oktober: Sa 17. locker, Di 20. hart, alle anderen frei (15., 16., 18., 19.,
    // 21.) also fünf Ruhetage; der 14. selbst gehört nicht dazu.
    expect(s.outlook).toEqual({ covered: 7, hardDays: 1, restDays: 5, nextHard: '2026-10-20' });
    // Am 23. reichen nur der 24. und 25. in den Plan (Planende am 25.): 24. frei, 25. Ruhe als
    // Eintrag "rest", also zwei Ruhetage.
    const late = buildWeekStory('2026-10-23', [], PLAN);
    expect(late.outlook).toEqual({ covered: 2, hardDays: 0, restDays: 2, nextHard: null });
  });

  it('ohne Plan bleibt nur die Zählung der Aktivitäten', () => {
    const s = buildWeekStory(TODAY, [act(1, '2026-10-13', 40)], null);
    expect(s.hasPlan).toBe(false);
    expect([s.plannedSessions, s.plannedPast, s.sessions, s.load]).toEqual([0, 0, 1, 40]);
    expect(s.outlook.covered).toBe(0);
    expect(s.days.find((d) => d.date === '2026-10-13')?.state).toBe('trained');
  });
});
