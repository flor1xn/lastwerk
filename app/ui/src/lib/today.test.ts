import { describe, expect, it } from 'vitest';
import type { Plan, PlanListItem, PlanSession } from './backend/types';
import { activePlanId, formState, planCovers, sessionsOn } from './today';

const session = (slot: 'am' | 'pm', template_id: string): PlanSession => ({
  slot,
  sport: 'cycling',
  template_id,
  duration_min: 60,
  planned_load: 50,
  hard: false,
});

const plan: Plan = {
  schema_version: 1,
  plan_id: 'p1',
  created_at: '2026-10-12T08:00:00Z',
  locale: 'de',
  source: 'rules',
  rules_version: 'test',
  goal: { kind: 'continuous' },
  warnings: [],
  weeks: [
    {
      week_start: '2026-10-12',
      phase: 'base',
      target_load: 300,
      days: [
        { date: '2026-10-13', sessions: [] },
        // Absichtlich in falscher Reihenfolge: Nachmittag zuerst.
        { date: '2026-10-14', sessions: [session('pm', 'b'), session('am', 'a')] },
      ],
    },
  ],
};

describe('formState', () => {
  // Grenzen aus SPEC 5.5: ab +5 frisch, -10 bis unter +5 ausgewogen, -30 bis unter -10
  // belastet, darunter überlastet. Die Grenzwerte gehören jeweils zum helleren Zustand.
  it.each([
    [20, 'fresh'],
    [5, 'fresh'],
    [4.9, 'balanced'],
    [-10, 'balanced'],
    [-10.1, 'loaded'],
    [-30, 'loaded'],
    [-30.1, 'overloaded'],
  ] as const)('TSB %s ist %s', (tsb, expected) => {
    expect(formState(tsb)).toBe(expected);
  });
});

describe('sessionsOn', () => {
  it('liefert Vormittag vor Nachmittag', () => {
    expect(sessionsOn(plan, '2026-10-14').map((s) => s.template_id)).toEqual(['a', 'b']);
  });

  it('liefert für einen freien Tag des Plans eine leere Liste', () => {
    expect(sessionsOn(plan, '2026-10-13')).toEqual([]);
    expect(planCovers(plan, '2026-10-13')).toBe(true);
  });

  it('erkennt Tage außerhalb des Plans', () => {
    expect(sessionsOn(plan, '2026-11-01')).toEqual([]);
    expect(planCovers(plan, '2026-11-01')).toBe(false);
  });
});

describe('activePlanId', () => {
  const item = (id: string, status: PlanListItem['status']): PlanListItem => ({
    id,
    created_at: '2026-10-12T08:00:00Z',
    status,
    source: 'rules',
    locale: 'de',
    rules_version: 'test',
    goal_id: null,
    goal: { kind: 'continuous' },
    first_week: null,
    last_week: null,
    weeks: 1,
  });

  it('nimmt den aktiven Plan', () => {
    expect(activePlanId([item('a', 'draft'), item('b', 'active')])).toBe('b');
  });

  it('liefert null ohne aktiven Plan', () => {
    expect(activePlanId([item('a', 'draft'), item('c', 'archived')])).toBeNull();
    expect(activePlanId([])).toBeNull();
  });
});
