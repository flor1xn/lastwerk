import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackend } from '../lib/backend';
import { MockBackend } from '../lib/backend/mock';
import { coachSession } from '../lib/coach-session.svelte';
import { setLocale } from '../lib/i18n';
import { closeCoach, coachFocus, coachOpen, view } from '../lib/state';
import TodayView from './TodayView.svelte';

// Kein Canvas in jsdom.
vi.mock(
  '../lib/components/UplotChart.svelte',
  async () => import('../lib/components/UplotChartStub.svelte'),
);

let backend: MockBackend;

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date('2026-10-14T12:00:00'));
  backend = new MockBackend({ today: '2026-10-14' });
  setBackend(backend);
  view.set('today');
});

afterEach(() => {
  cleanup();
  closeCoach();
  coachSession.clearForTests();
  setBackend(null);
  setLocale('de');
  vi.useRealTimers();
});

describe('TodayView', () => {
  it('sagt ohne aktiven Plan, was fehlt, und führt zum Plan', async () => {
    render(TodayView);
    const card = await screen.findByTestId('today-decision');
    expect(card.dataset.state).toBe('no_plan');
    expect(screen.getByTestId('today-title').textContent).toBe('Noch kein aktiver Plan');
    await fireEvent.click(screen.getByRole('button', { name: 'Zum Plan' }));
    let current = '';
    view.subscribe((v) => (current = v))();
    expect(current).toBe('plan');
  });

  it('zeigt den Formzustand als Text', async () => {
    render(TodayView);
    const state = await screen.findByTestId('form-state');
    expect(['Frisch', 'Ausgewogen', 'Belastet', 'Überlastet']).toContain(state.textContent);
  });
});

async function activatePlan() {
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
}

describe('Tagesentscheidung', () => {
  it('rich: wie geplant mit härterer Alternative, genau eine Primäraktion', async () => {
    await activatePlan();
    render(TodayView);
    const card = await screen.findByTestId('today-decision');
    expect(card.dataset.state).toBe('as_planned');
    expect(screen.getByTestId('decision-state').textContent).toBe('Heute wie geplant');
    expect(screen.getByTestId('today-title').textContent).toBe('Grundlagenfahrt');
    expect(screen.getByTestId('decision-meta').textContent).toContain('90 Minuten');
    expect(card.querySelectorAll('.primary')).toHaveLength(1);
    expect(card.querySelector('.primary')?.textContent?.trim()).toMatch(/^Mit Coach besprechen/);
    // Die einzige erhobene Fläche der Seite.
    expect(document.querySelectorAll('.card')).toHaveLength(1);
    expect(screen.getByTestId('decision-alt-harder').textContent?.trim()).toBe('Härter');
  });

  it('Alternativen zeigen nur Einzelheiten und ändern den Plan nicht', async () => {
    await activatePlan();
    const before = JSON.stringify(await backend.getPlan((await backend.listPlans())[0].id));
    render(TodayView);
    const alt = await screen.findByTestId('decision-alt-harder');
    expect(alt.getAttribute('aria-expanded')).toBe('false');
    expect(alt.classList.contains('primary')).toBe(false);
    await fireEvent.click(alt);
    expect(alt.getAttribute('aria-expanded')).toBe('true');
    const detail = screen.getByTestId('decision-alt-detail');
    expect(detail.textContent).toContain('Schwelle 2 mal 20');
    expect(detail.textContent).toContain('ändern den Plan nicht');
    await fireEvent.click(alt);
    expect(screen.queryByTestId('decision-alt-detail')).toBeNull();
    const after = JSON.stringify(await backend.getPlan((await backend.listPlans())[0].id));
    expect(after).toBe(before);
    expect(get(coachOpen)).toBe(false);
  });

  it('low: leichter mit Grund, Regel R12 und "wie geplant" als Alternative', async () => {
    backend.dashboardVariant = 'low';
    await activatePlan();
    render(TodayView);
    const card = await screen.findByTestId('today-decision');
    expect(card.dataset.state).toBe('lighten');
    expect(screen.getByTestId('decision-state').textContent).toBe('Heute leichter als geplant');
    expect(screen.getByTestId('decision-reasons').textContent).toContain('HRV oder Schlafdauer');
    expect(screen.getByTestId('decision-rules').textContent).toContain('R12');
    expect(screen.getByTestId('decision-rules').textContent).toContain('Erholungsdaten');
    expect(screen.getByTestId('decision-alt-planned')).toBeTruthy();
    // Die geplante Einheit steht durchgestrichen neben der Empfehlung.
    expect(screen.getByTestId('decision-replaced').textContent).toContain('Schwelle 2 mal 20');
  });

  it('leichter ohne passende Vorlage: keine Empfehlung, Hinweis und geplante Einheit', async () => {
    backend.decisionOverride = 'lighten_empty';
    render(TodayView);
    const card = await screen.findByTestId('today-decision');
    expect(card.dataset.state).toBe('lighten');
    expect(screen.queryByTestId('decision-meta')).toBeNull();
    expect(screen.getByTestId('today-title').textContent).toBe('Heute leichter als geplant');
    expect(screen.getByTestId('decision-hint').textContent).toContain('keine lockere Vorlage');
    expect(screen.getByTestId('decision-alt-planned')).toBeTruthy();
    expect(card.querySelectorAll('.primary')).toHaveLength(1);
    expect(card.textContent).not.toMatch(/NaN|undefined|null/);
  });

  it('zeigt eine zweite Einheit des Tages unter der Empfehlung', async () => {
    await activatePlan();
    const id = (await backend.listPlans())[0].id;
    const detail = await backend.getPlan(id);
    // Zwei Einheiten heute: die Empfehlung der Attrappe (Grundlagenfahrt, 90 Minuten) am
    // Vormittag und Kraft am Nachmittag, wie es der Planer an einem Tag erlaubt.
    const day = detail.plan.weeks.flatMap((w) => w.days).find((d) => d.date === '2026-10-14');
    expect(day).toBeTruthy();
    day!.sessions = [
      {
        slot: 'am',
        sport: 'cycling',
        template_id: 'bike.endurance.steady',
        duration_min: 90,
        planned_load: 62,
        hard: false,
      },
      {
        slot: 'pm',
        sport: 'strength',
        template_id: 'strength.general.a',
        duration_min: 45,
        planned_load: 20,
        hard: false,
      },
    ];
    vi.spyOn(backend, 'getPlan').mockResolvedValue(detail);
    render(TodayView);
    const card = await screen.findByTestId('today-decision');
    expect(card.textContent).toContain('Weitere Einheiten heute');
    expect(card.textContent).toContain('45 Minuten');
    expect(card.querySelectorAll('.more li')).toHaveLength(1);
  });

  it('sparse: nennt fehlende Eingänge und bietet nichts Härteres an', async () => {
    backend.dashboardVariant = 'sparse';
    await activatePlan();
    render(TodayView);
    await screen.findByTestId('today-decision');
    expect(screen.getByTestId('decision-missing').textContent).toContain('Erholungswerte');
    expect(screen.queryByTestId('decision-alternatives')).toBeNull();
  });

  it('empty ohne Plan: no_plan, ein Ruhetag sagt das', async () => {
    backend.dashboardVariant = 'empty';
    render(TodayView);
    expect((await screen.findByTestId('today-decision')).dataset.state).toBe('no_plan');
    cleanup();
    backend.decisionOverride = 'rest';
    render(TodayView);
    expect((await screen.findByTestId('today-title')).textContent).toBe('Heute frei');
    expect(screen.getByTestId('decision-rules').textContent).toContain('R05');
  });

  it('die Primäraktion öffnet den Coach mit Thema und Frage, ohne zu senden', async () => {
    backend.dashboardVariant = 'low';
    await activatePlan();
    render(TodayView);
    await fireEvent.click(await screen.findByTestId('decision-discuss'));
    expect(get(coachOpen)).toBe(true);
    expect(get(coachFocus).topic).toBe('decision.lighten');
    expect(coachSession.draft).toBe('Warum soll ich heute leichter trainieren als geplant?');
    expect(coachSession.messages).toHaveLength(0);
  });

  it('das Kürzel B öffnet den Coach wie die Primäraktion, nicht in Eingabefeldern', async () => {
    backend.dashboardVariant = 'low';
    await activatePlan();
    render(TodayView);
    await screen.findByTestId('decision-discuss');
    const input = document.createElement('input');
    document.body.append(input);
    await fireEvent.keyDown(input, { key: 'b' });
    expect(get(coachOpen)).toBe(false);
    input.remove();
    await fireEvent.keyDown(window, { key: 'b' });
    expect(get(coachOpen)).toBe(true);
    expect(get(coachFocus).topic).toBe('decision.lighten');
  });
});

describe('Coach-Erkenntnisse', () => {
  it('zeigt höchstens drei Zeilen mit den Aussagen der Abschnitte', async () => {
    backend.dashboardVariant = 'extreme';
    render(TodayView);
    const section = await screen.findByTestId('today-insights');
    const rows = screen.getAllByTestId('insight');
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThanOrEqual(3);
    expect(section.textContent).not.toMatch(/NaN|undefined|ui\.dash/);
  });

  it('ein Knopf öffnet den Coach mit Thema und Prefill und sendet nicht', async () => {
    backend.dashboardVariant = 'extreme';
    render(TodayView);
    await screen.findByTestId('today-insights');
    const row = screen.getAllByTestId('insight')[0];
    const code = row.dataset.code!;
    await fireEvent.click(row.querySelector('button')!);
    expect(get(coachOpen)).toBe(true);
    expect(get(coachFocus).topic).toBe(code);
    expect(coachSession.draft.length).toBeGreaterThan(0);
    expect(coachSession.messages).toHaveLength(0);
  });

  it('wiederholt nicht, was ein Instrument schon zeigt', async () => {
    backend.dashboardVariant = 'low';
    render(TodayView);
    await screen.findByTestId('today-readiness');
    const codes = screen.queryAllByTestId('insight').map((r) => r.dataset.code);
    expect(codes).not.toContain('readiness.low');
    expect(codes).not.toContain('form.overloaded');
    expect(screen.getByTestId('readiness-state').textContent).toContain(
      'Deutlich unter deinem Schnitt',
    );
  });

  it('ohne Plan, Last und Erholungswerte steht ein Weg in drei Schritten statt der Instrumente', async () => {
    backend.dashboardVariant = 'empty';
    render(TodayView);
    const start = await screen.findByTestId('today-start');
    expect(screen.queryByTestId('today-insights')).toBeNull();
    expect(screen.queryByTestId('today-form')).toBeNull();
    const steps = screen.getAllByTestId('start-step');
    expect(steps.map((s) => s.dataset.step)).toEqual(['data', 'plan', 'coach']);
    expect(start.querySelectorAll('.primary')).toHaveLength(1);
    await fireEvent.click(steps[1].querySelector('button')!);
    expect(get(view)).toBe('plan');
  });
});

describe('Heute als Tafel', () => {
  it('zeigt Entscheidung, drei Instrumente und Notizen in dieser Reihenfolge', async () => {
    await activatePlan();
    render(TodayView);
    await screen.findByTestId('today-form');
    const ids = [
      'today-head',
      'today-decision',
      'today-form',
      'today-readiness',
      'today-week-story',
      'today-insights',
    ];
    const nodes = ids.map((id) => screen.getByTestId(id));
    for (let i = 1; i < nodes.length; i++) {
      expect(
        nodes[i - 1].compareDocumentPosition(nodes[i]) & Node.DOCUMENT_POSITION_FOLLOWING,
        ids[i],
      ).toBeTruthy();
    }
    // Nach Verlauf umgezogen: Form-Verlauf, Rampe, Verteilung und alles aus T44.
    for (const id of [
      'dash-form',
      'dash-ramp',
      'dash-distribution',
      'dash-efficiency',
      'dash-power',
      'dash-calendar',
      'today-ramp',
      'today-distribution',
    ]) {
      expect(screen.queryByTestId(id), id).toBeNull();
    }
    expect(screen.queryByTestId('lead')).toBeNull();
    expect(screen.getByTestId('form-scale').dataset.state).toBe(
      screen.getByTestId('form-state').classList[1],
    );
    expect(screen.getByTestId('vital-hrv')).toBeTruthy();
    expect(screen.queryByTestId('vital-body-battery')).toBeNull();
  });

  it('nennt in der Kopfzeile die Woche des Plans und die Phase', async () => {
    await activatePlan();
    render(TodayView);
    expect((await screen.findByTestId('today-plan-week')).textContent).toMatch(
      /Woche 1 von 3 · Basis/,
    );
  });

  it('zählt die Woche und blickt auf die nächsten sieben Tage', async () => {
    await activatePlan();
    render(TodayView);
    const story = await screen.findByTestId('today-week-story');
    expect(screen.getByTestId('week-done').textContent).toMatch(/^\s*\d+\/\d+/);
    expect(screen.getByTestId('week-story-outlook').textContent).toMatch(
      /Die nächsten sieben Tage im Plan: harte Tage \d+, Ruhetage \d+\./,
    );
    expect(story.querySelectorAll('[data-testid="day"]')).toHaveLength(7);
  });

  it('sagt mit Plan, aber ohne Messwerte je Instrument, was fehlt, und zeigt keine Zahlen', async () => {
    backend.dashboardVariant = 'empty';
    await activatePlan();
    render(TodayView);
    await screen.findByTestId('today-readiness');
    expect(screen.queryByTestId('today-start')).toBeNull();
    expect(screen.getByTestId('today-readiness').textContent).toContain(
      'Verbinde Garmin für Erholungswerte',
    );
    expect(screen.getByTestId('today-form').textContent).toContain('Noch keine Last berechnet');
    for (const id of ['form-value', 'vital-hrv', 'form-scale']) {
      expect(screen.queryByTestId(id), id).toBeNull();
    }
  });

  it('zeigt den Hinweis zu R12 nur, wenn die Erholung niedrig ist und der Plan angepasst hat', async () => {
    backend.dashboardVariant = 'low';
    render(TodayView);
    await screen.findByTestId('today-readiness');
    expect(screen.getByTestId('today-readiness').textContent).toMatch(/deutlich unter/i);
    expect(screen.queryByTestId('plan-adjusted')).toBeNull();
    cleanup();

    await activatePlan();
    render(TodayView);
    expect((await screen.findByTestId('plan-adjusted')).textContent).toContain(
      'Lastwerk hat angepasst',
    );
    cleanup();

    backend.dashboardVariant = 'rich';
    render(TodayView);
    await screen.findByTestId('today-readiness');
    expect(screen.queryByTestId('plan-adjusted')).toBeNull();
  });

  it('bleibt bei großen Werten und wenigen Daten lesbar', async () => {
    await activatePlan();
    for (const variant of ['extreme', 'sparse', 'gaps'] as const) {
      backend.dashboardVariant = variant;
      const { unmount } = render(TodayView);
      await screen.findByTestId('today-form');
      expect(screen.getByTestId('today').textContent, variant).not.toMatch(
        /NaN|undefined|Infinity|ui\.(decision|insight|week_story|instrument)\./,
      );
      unmount();
    }
  });

  it('folgt der Sprache', async () => {
    setLocale('en');
    await activatePlan();
    render(TodayView);
    await screen.findByTestId('today-form');
    expect(screen.getByTestId('today-readiness').querySelector('.inst-label')?.textContent).toBe(
      'Recovery against your average',
    );
    expect(screen.getByTestId('decision-discuss').textContent?.trim()).toMatch(
      /^Discuss with coach/,
    );
    expect(screen.getByTestId('today-insights').getAttribute('aria-label')).toBe('Coach insights');
    expect(screen.getByTestId('form-state').textContent).toMatch(
      /Fresh|Balanced|Loaded|Overloaded/,
    );
  });
});
