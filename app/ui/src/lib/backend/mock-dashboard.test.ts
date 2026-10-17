import { describe, expect, it } from 'vitest';
import { formState } from '../today';
import { MockBackend } from './mock';
import {
  DASHBOARD_VARIANTS,
  DECISION_OVERRIDES,
  isDashboardVariant,
  isDecisionOverride,
  rankInsights,
} from './mock-dashboard';
import { DECISION_CODES, INSIGHT_CODES } from './types';

// Die Attrappe des Dashboards: deterministisch, in sich stimmig, und die leere Variante erfindet
// nichts.

const TODAY = '2026-10-14';

describe('Dashboard der Attrappe', () => {
  it('ist deterministisch', async () => {
    const a = await new MockBackend({ today: TODAY }).getDashboard(TODAY);
    const b = await new MockBackend({ today: TODAY }).getDashboard(TODAY);
    expect(a).toEqual(b);
  });

  it('füllt in der Standardvariante alle Abschnitte stimmig', async () => {
    const d = await new MockBackend({ today: TODAY }).getDashboard(TODAY);
    expect(d.pmc).toHaveLength(120);
    expect(d.pmc.at(-1)?.date).toBe(TODAY);
    expect(d.projection).toEqual([]);
    expect(d.plan).toBeNull();
    expect(d.form?.state).toBe(`form.${formState(d.form?.tsb ?? 0)}`);
    expect(d.ramp.series).toHaveLength(84);
    expect(d.distribution.weeks).toHaveLength(8);
    expect(d.power_curve.durations.map((p) => p.duration_s)).toEqual([5, 60, 300, 1200, 3600]);
    expect(d.readiness.series.length).toBeGreaterThan(30);
    expect(d.readiness.body_battery_high).toBeNull();
    expect(d.load_days.length).toBeGreaterThan(200);
    const dates = d.load_days.map((l) => l.date);
    expect(dates).toEqual([...dates].sort());
    expect(d.adherence.weeks).toEqual([]);
  });

  it('rundet auf eine Nachkommastelle', async () => {
    const d = await new MockBackend({ today: TODAY }).getDashboard(TODAY);
    const one = (v: number) => Math.abs(Math.round(v * 10) - v * 10) < 1e-6;
    for (const p of d.pmc) for (const v of [p.ctl, p.atl, p.tsb]) expect(one(v)).toBe(true);
    for (const r of d.ramp.series) expect(one(r.ramp)).toBe(true);
  });

  it('liefert mit aktivem Plan Prognose und Planerfüllung', async () => {
    const backend = new MockBackend({ today: TODAY });
    const detail = await backend.generatePlan({
      week_start: '2026-10-12',
      goal_date: null,
      goal_sport: null,
      weeks: 3,
      locale: 'de',
      sports: ['cycling'],
      strength_per_week: 0,
    });
    await backend.setPlanStatus(detail.plan.plan_id, 'active');
    const d = await backend.getDashboard(TODAY);
    expect(d.plan?.id).toBe(detail.plan.plan_id);
    expect(d.projection.length).toBeGreaterThan(7);
    expect(d.projection[0].date > TODAY).toBe(true);
    expect(d.adherence.weeks).toHaveLength(3);
  });

  it('erfindet in der leeren Variante nichts', async () => {
    const d = await new MockBackend({ today: TODAY, dashboard: 'empty' }).getDashboard(TODAY);
    expect(d.form).toBeNull();
    expect(d.pmc).toEqual([]);
    expect(d.load_days).toEqual([]);
    expect(d.ramp.current).toBeNull();
    expect(d.readiness.state).toBe('readiness.no_data');
    expect(d.readiness.series).toEqual([]);
    expect(d.efficiency.current).toBeNull();
    expect(d.power_curve.durations.every((p) => p.current_w === null)).toBe(true);
    expect(d.distribution.weeks.every((w) => w.easy_percent === null)).toBe(true);
  });

  it('kennt alle Varianten und lehnt ein ungültiges Datum ab', async () => {
    for (const variant of DASHBOARD_VARIANTS) {
      const d = await new MockBackend({ today: TODAY, dashboard: variant }).getDashboard(TODAY);
      expect(d.today).toBe(TODAY);
    }
    expect(isDashboardVariant('rich')).toBe(true);
    expect(isDashboardVariant('x')).toBe(false);
    await expect(new MockBackend().getDashboard('gestern')).rejects.toMatchObject({
      code: 'invalid_date',
    });
  });

  it('löst in der Variante low die Erholung aus', async () => {
    const d = await new MockBackend({ today: TODAY, dashboard: 'low' }).getDashboard(TODAY);
    expect(d.readiness.state).toBe('readiness.low');
    expect(d.readiness.hrv?.deviation_percent ?? 0).toBeLessThan(-15);
  });
});

describe('Tagesentscheidung und Erkenntnisse der Attrappe (T44)', () => {
  async function withPlan(variant: (typeof DASHBOARD_VARIANTS)[number]) {
    const b = new MockBackend({ today: TODAY, dashboard: variant });
    const detail = await b.generatePlan({
      week_start: '2026-10-12',
      goal_date: null,
      goal_sport: null,
      weeks: 2,
      locale: 'de',
      sports: ['cycling'],
      strength_per_week: 0,
    });
    await b.setPlanStatus(detail.plan.plan_id, 'active');
    return b.getDashboard(TODAY);
  }

  it('liefert ohne Plan no_plan und mit Plan je Variante eine feste Lage', async () => {
    const none = await new MockBackend({ today: TODAY }).getDashboard(TODAY);
    expect(none.decision.state).toBe('decision.no_plan');
    expect(none.decision.main).toBeNull();

    const rich = await withPlan('rich');
    expect(rich.decision.state).toBe('decision.as_planned');
    expect(rich.decision.alternatives.map((a) => a.kind)).toEqual(['harder']);
    const low = await withPlan('low');
    expect(low.decision.state).toBe('decision.lighten');
    expect(low.decision.main?.kind).toBe('lighter');
    expect(low.decision.rules).toEqual(['R12']);
    const sparse = await withPlan('sparse');
    expect(sparse.decision.missing).toEqual(['readiness']);
    expect(sparse.decision.alternatives).toEqual([]);
    const extreme = await withPlan('extreme');
    expect(extreme.decision.state).toBe('decision.lighten');
    expect(extreme.decision.main).toBeNull();
    for (const d of [rich, low, sparse, extreme]) {
      expect(DECISION_CODES).toContain(d.decision.state);
    }
  });

  it('erzwingt mit ?decision= jeden Zustand, auch ohne Plan', async () => {
    for (const o of DECISION_OVERRIDES) {
      expect(isDecisionOverride(o)).toBe(true);
      const d = await new MockBackend({ today: TODAY, decision: o }).getDashboard(TODAY);
      expect(d.decision.state).toBe(
        o === 'lighten_empty' || o === 'worst' ? 'decision.lighten' : `decision.${o}`,
      );
    }
    expect(isDecisionOverride('foo')).toBe(false);
  });

  it('wählt höchstens drei Erkenntnisse in fester Reihenfolge', async () => {
    expect(
      rankInsights(['ramp.building', 'efficiency.rising', 'form.overloaded', 'ramp.steep', 'x']),
    ).toEqual([{ code: 'form.overloaded' }, { code: 'ramp.steep' }, { code: 'efficiency.rising' }]);
    for (const v of DASHBOARD_VARIANTS) {
      const d = await new MockBackend({ today: TODAY, dashboard: v }).getDashboard(TODAY);
      expect(d.insights.length, v).toBeLessThanOrEqual(3);
      for (const i of d.insights) expect(INSIGHT_CODES).toContain(i.code);
    }
    const empty = await new MockBackend({ today: TODAY, dashboard: 'empty' }).getDashboard(TODAY);
    expect(empty.insights).toEqual([]);
  });
});
