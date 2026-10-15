import { describe, expect, it } from 'vitest';
import { MockBackend } from './mock';
import { BackendError, type GeneratePlanInput } from './types';

const TODAY = '2026-10-13';
const MONDAY = '2026-10-19';

function input(over: Partial<GeneratePlanInput> = {}): GeneratePlanInput {
  return {
    week_start: MONDAY,
    goal_date: null,
    goal_sport: null,
    weeks: 3,
    locale: 'de',
    sports: ['cycling'],
    strength_per_week: 1,
    ...over,
  };
}

async function code(p: Promise<unknown>): Promise<string> {
  try {
    await p;
    return 'ok';
  } catch (e) {
    return e instanceof BackendError ? e.code : 'andere';
  }
}

describe('Plan der Attrappe', () => {
  it('erzeugt einen Entwurf mit sieben Tagen je Woche und Verlauf', async () => {
    const b = new MockBackend({ today: TODAY });
    const d = await b.generatePlan(input());
    expect(d.status).toBe('draft');
    expect(d.plan.weeks).toHaveLength(3);
    expect(d.plan.weeks.every((w) => w.days.length === 7)).toBe(true);
    expect(d.plan.weeks[0].week_start).toBe(MONDAY);
    expect(d.projection).toHaveLength(21);
    expect(d.validation.schema_errors).toEqual([]);
    expect(d.validation.violations.every((v) => !v.hard)).toBe(true);
    // Handrechnung (SPEC 8.2): CTL des ersten Tages = ctl0 + (L - ctl0) * (1 - exp(-1/42)).
    const load = d.plan.weeks[0].days[0].sessions.reduce((s, x) => s + x.planned_load, 0);
    expect(d.projection[0].load).toBeCloseTo(load, 6);
    expect(d.projection[0].ctl).toBeCloseTo(d.ctl0 + (load - d.ctl0) * (1 - Math.exp(-1 / 42)), 6);
    expect(d.ctl0).toBeGreaterThan(0);
  });

  it('lehnt ungültige Eingaben mit Fehlercodes ab und speichert nichts', async () => {
    const b = new MockBackend({ today: TODAY });
    expect(await code(b.generatePlan(input({ week_start: '2026-10-20' })))).toBe('not_monday');
    expect(await code(b.generatePlan(input({ week_start: 'x' })))).toBe('invalid_date');
    expect(await code(b.generatePlan(input({ weeks: 27 })))).toBe('invalid_value');
    expect(await code(b.generatePlan(input({ strength_per_week: 3 })))).toBe('invalid_value');
    expect(await code(b.generatePlan(input({ sports: ['strength' as 'cycling'] })))).toBe(
      'invalid_sport',
    );
    expect(await b.listPlans()).toEqual([]);
  });

  it('hält höchstens einen Plan aktiv', async () => {
    const b = new MockBackend({ today: TODAY });
    const a = await b.generatePlan(input());
    const c = await b.generatePlan(input({ weeks: 2 }));
    await b.setPlanStatus(a.plan.plan_id, 'active');
    const list = await b.setPlanStatus(c.plan.plan_id, 'active');
    const status = (id: string) => list.find((p) => p.id === id)?.status;
    expect(status(a.plan.plan_id)).toBe('archived');
    expect(status(c.plan.plan_id)).toBe('active');
    expect((await b.getPlan(c.plan.plan_id)).status).toBe('active');
    expect(await code(b.setPlanStatus('unbekannt', 'draft'))).toBe('not_found');
  });

  it('nutzt die Verfügbarkeit: fester Ruhetag und Höchstzeit', async () => {
    const b = new MockBackend({ today: TODAY });
    await b.setAvailability([
      { weekday: 1, max_minutes: 30, allow_hard: false, fixed_rest: false },
      { weekday: 5, max_minutes: 0, allow_hard: false, fixed_rest: true },
    ]);
    const d = await b.generatePlan(input());
    for (const w of d.plan.weeks) {
      expect(w.days[5].sessions).toEqual([]);
      const minutes = w.days[1].sessions.reduce((s, x) => s + x.duration_min, 0);
      expect(minutes).toBeLessThanOrEqual(30);
      expect(w.days[1].sessions.every((s) => !s.hard)).toBe(true);
    }
  });

  it('prüft Ziele und Verfügbarkeit wie der Rust-Teil', async () => {
    const b = new MockBackend({ today: TODAY });
    const empty = await b.getPlanningInputs();
    expect(empty.goals).toEqual([]);
    expect(empty.availability).toHaveLength(7);
    expect(empty.availability_set).toBe(0);
    expect(
      await code(b.setGoal({ event_date: '2026-13-40', sport: null, priority: null, label: null })),
    ).toBe('invalid_date');
    expect(
      await code(
        b.setGoal({ event_date: null, sport: null, priority: null, label: 'x'.repeat(201) }),
      ),
    ).toBe('invalid_value');
    const one = { weekday: 0, max_minutes: 60, allow_hard: true, fixed_rest: false };
    expect(await code(b.setAvailability([one, { ...one, max_minutes: 1441, weekday: 1 }]))).toBe(
      'invalid_value',
    );
    expect(await code(b.setAvailability([one, one]))).toBe('invalid_value');
    expect((await b.getPlanningInputs()).availability_set).toBe(0);

    const first = await b.setGoal({
      event_date: '2027-05-01',
      sport: 'cycling',
      priority: 'A',
      label: '  Rennen ',
    });
    expect(first.goals[0]).toMatchObject({ kind: 'event', label: 'Rennen', active: true });
    const second = await b.setGoal({
      event_date: null,
      sport: 'cycling',
      priority: 'B',
      label: null,
    });
    expect(second.goals).toHaveLength(2);
    expect(second.goals.filter((g) => g.active)).toHaveLength(1);
    expect(second.goals[0]).toMatchObject({ kind: 'continuous', priority: null });
  });

  it('protokolliert Exporte und verlangt die Bestätigung der Wiederherstellung', async () => {
    const b = new MockBackend({ today: TODAY });
    const d = await b.generatePlan(input());
    const events = d.plan.weeks.flatMap((w) => w.days).reduce((s, x) => s + x.sessions.length, 0);
    expect(await b.exportPlanIcs(d.plan.plan_id)).toEqual({ saved: true, events, counts: null });
    expect((await b.exportPlanJson(d.plan.plan_id)).saved).toBe(true);
    const backup = await b.exportBackup();
    expect(backup.counts).toMatchObject({ plans: 1, goals: 0 });
    expect(await code(b.restoreBackup(false))).toBe('unconfirmed');
    expect((await b.restoreBackup(true)).restored).toBe(true);
    expect(b.calls.map((c) => c.command)).toEqual([
      'export_plan_ics',
      'export_plan_json',
      'export_backup',
      'restore_backup',
      'restore_backup',
    ]);
    expect(await code(b.exportPlanIcs('unbekannt'))).toBe('not_found');
  });
});
