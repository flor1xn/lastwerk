import { describe, expect, it } from 'vitest';
import { MockBackend } from './mock';
import { BackendError, type QuickPlanInput } from './types';

const TODAY = '2026-10-13';
const MONDAY = '2026-10-19';

function quick(over: Partial<QuickPlanInput> = {}): QuickPlanInput {
  return {
    goal: { kind: 'continuous', date: null, sport: null },
    hours_per_week: 6,
    days: [0, 2, 4, 6],
    week_start: MONDAY,
    locale: 'de',
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

describe('drei Fragen in der Attrappe', () => {
  it('setzt Verfügbarkeit und Ziel, erzeugt einen aktiven Plan und lässt Ruhetage frei', async () => {
    const b = new MockBackend({ today: TODAY });
    const d = await b.quickPlan(quick());
    expect(d.status).toBe('active');
    const inputs = await b.getPlanningInputs();
    expect(inputs.availability.map((x) => x.max_minutes)).toEqual([90, 0, 90, 0, 90, 0, 90]);
    expect(inputs.availability.map((x) => x.fixed_rest)).toEqual([
      false,
      true,
      false,
      true,
      false,
      true,
      false,
    ]);
    for (const w of d.plan.weeks) {
      for (const i of [1, 3, 5]) expect(w.days[i].sessions).toEqual([]);
    }
    expect((await b.listPlans()).filter((p) => p.status === 'active')).toHaveLength(1);
  });

  it('setzt ein Ziel nur einmal und ersetzt den aktiven Plan', async () => {
    const b = new MockBackend({ today: TODAY });
    const event = quick({ goal: { kind: 'event', date: '2026-11-04', sport: 'cycling' } });
    const first = await b.quickPlan(event);
    expect(first.plan.goal.event_date).toBe('2026-11-04');
    const second = await b.quickPlan(event);
    expect((await b.getPlanningInputs()).goals).toHaveLength(1);
    const list = await b.listPlans();
    expect(list.find((p) => p.id === first.plan.plan_id)?.status).toBe('archived');
    expect(list.find((p) => p.id === second.plan.plan_id)?.status).toBe('active');
  });

  it('lehnt falsche Antworten ab und ändert nichts', async () => {
    const b = new MockBackend({ today: TODAY });
    expect(await code(b.quickPlan(quick({ hours_per_week: 0.5 })))).toBe('invalid_value');
    expect(await code(b.quickPlan(quick({ days: [] })))).toBe('invalid_value');
    expect(await code(b.quickPlan(quick({ hours_per_week: 1, days: [0, 1, 2, 3, 4, 5, 6] })))).toBe(
      'too_little_time',
    );
    expect(await code(b.quickPlan(quick({ week_start: '2026-10-20' })))).toBe('not_monday');
    expect(
      await code(b.quickPlan(quick({ goal: { kind: 'event', date: null, sport: null } }))),
    ).toBe('invalid_date');
    expect(
      await code(b.quickPlan(quick({ goal: { kind: 'event', date: '2026-10-01', sport: null } }))),
    ).toBe('invalid_date');
    expect(await b.listPlans()).toEqual([]);
    expect((await b.getPlanningInputs()).availability_set).toBe(0);
  });
});

describe('Coach der Attrappe', () => {
  it('antwortet der Reihe nach: Text, Änderung am Plan, neue Zeiten, verworfen', async () => {
    const b = new MockBackend({ today: TODAY });
    await b.quickPlan(quick());
    const first = await b.coachChat('Hallo');
    expect(first.proposal).toBeNull();
    expect(first.rejected_codes).toEqual([]);

    const edit = await b.coachChat('Weniger');
    expect(edit.proposal?.kind).toBe('plan_edit');
    expect(edit.proposal?.explanation).toBeTruthy();
    expect(edit.proposal?.summary.edits.length).toBeGreaterThan(0);

    const change = await b.coachChat('Dienstag nicht');
    expect(change.proposal?.kind).toBe('planning_change');
    expect(change.proposal?.summary.change?.availability).toHaveLength(7);

    const rejected = await b.coachChat('Zwei harte Tage');
    expect(rejected.proposal).toBeNull();
    expect(rejected.rejected_codes).toEqual(['R03']);

    // Danach beginnt das Skript von vorn.
    expect((await b.coachChat('Noch mal')).proposal).toBeNull();
  });

  it('zeigt Vorher und Nachher und übernimmt einen plan_edit als neuen aktiven Plan', async () => {
    const b = new MockBackend({ today: TODAY });
    const base = await b.quickPlan(quick());
    await b.coachChat('Hallo');
    const r = await b.coachChat('Weniger');
    const weeks = r.proposal?.summary.weeks ?? [];
    expect(weeks).toHaveLength(base.plan.weeks.length);
    const changed = weeks.filter((w) => w.before?.minutes !== w.after?.minutes);
    expect(changed).toHaveLength(1);
    expect(changed[0].after!.minutes).toBeLessThan(changed[0].before!.minutes);

    const applied = await b.coachApply(r.proposal!.id);
    expect(applied.status).toBe('active');
    expect(applied.plan.source).toBe('llm');
    expect(applied.plan.plan_id).not.toBe(base.plan.plan_id);
    const list = await b.listPlans();
    expect(list.find((p) => p.id === base.plan.plan_id)?.status).toBe('archived');
    expect(await code(b.coachApply(r.proposal!.id))).toBe('not_found');
  });

  it('übernimmt neue Zeiten: Verfügbarkeit geschrieben, Dienstag im neuen Plan frei', async () => {
    const b = new MockBackend({ today: TODAY });
    await b.quickPlan(quick({ days: [1, 3, 5, 6] }));
    await b.coachChat('a');
    await b.coachChat('b');
    const r = await b.coachChat('c');
    const applied = await b.coachApply(r.proposal!.id);
    const avail = (await b.getPlanningInputs()).availability;
    expect(avail[1]).toMatchObject({ fixed_rest: true, max_minutes: 0 });
    for (const w of applied.plan.weeks) expect(w.days[1].sessions).toEqual([]);
  });

  it('weist leere und zu lange Nachrichten ab, verlangt einen Plan und eine Sperre', async () => {
    const b = new MockBackend({ today: TODAY, coachDelayMs: 20 });
    expect(await code(b.coachChat('Hallo'))).toBe('no_plan');
    await b.quickPlan(quick());
    expect(await code(b.coachChat('   '))).toBe('empty_message');
    expect(await code(b.coachChat('ä'.repeat(1001)))).toBe('message_too_long');
    const running = b.coachChat('erste');
    expect(await code(b.coachChat('zweite'))).toBe('busy');
    expect((await running).reply).toBeTruthy();
  });

  it('bricht ab: die Antwort kommt nie an, der Verlauf bleibt, Zurücksetzen leert ihn', async () => {
    const b = new MockBackend({ today: TODAY, coachDelayMs: 20 });
    await b.quickPlan(quick());
    await b.coachChat('Hallo');
    const slow = b.coachChat('Weniger');
    await b.coachCancel();
    expect(await code(slow)).toBe('coach_cancelled');
    // Der Verlauf hat nur die erste Runde: Anweisung, Nutzer, Coach.
    expect((await b.coachPreview('')).messages).toHaveLength(3);
    await b.coachReset();
    expect((await b.coachPreview('')).messages).toHaveLength(1);
  });

  it('ohne Modell meldet der Status den Grund und das Gespräch scheitert mit Code', async () => {
    const b = new MockBackend({ today: TODAY, coach: false });
    const s = await b.coachStatus();
    expect(s).toMatchObject({ available: false, reason: 'no_server', model: null, models: [] });
    expect(await code(b.coachChat('Hallo'))).toBe('coach_unavailable');
    // Der Planer bleibt nutzbar.
    expect((await b.quickPlan(quick())).status).toBe('active');
  });

  it('wählt ein Modell und fällt bei leerem Namen auf das erste zurück', async () => {
    const b = new MockBackend({ today: TODAY });
    expect((await b.coachStatus()).model).toBe('qwen3:4b');
    expect((await b.coachSetModel('llama3.2:3b')).model).toBe('llama3.2:3b');
    expect((await b.coachSetModel('')).model).toBe('qwen3:4b');
    expect(await code(b.coachSetModel('x\u0007y'))).toBe('invalid_value');
  });

  it('zeigt in der Vorschau Anweisung, Verlauf und den Text der nächsten Nachricht', async () => {
    const b = new MockBackend({ today: TODAY });
    await b.quickPlan(quick());
    await b.coachChat('Erste');
    const p = await b.coachPreview('Zweite');
    expect(p.messages.map((m) => m.role)).toEqual(['system', 'user', 'assistant', 'user']);
    expect(p.messages[3].content).toBe('Zweite');
    expect(p.provider.base_url).toMatch(/^http:\/\/127\.0\.0\.1/);
  });
});
