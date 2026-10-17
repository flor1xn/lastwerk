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
    expect(card.querySelector('.primary')?.textContent?.trim()).toBe('Mit Coach besprechen');
    // Die einzige Karte der Seite.
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
    expect(screen.getByTestId('lead').textContent).toContain('leichteren Einheit als geplant');
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
    backend.dashboardVariant = 'low';
    render(TodayView);
    await screen.findByTestId('today-insights');
    const row = screen.getAllByTestId('insight').find((r) => r.dataset.code === 'readiness.low');
    expect(row).toBeTruthy();
    expect(row?.textContent).toContain('deutlich unter deinem Schnitt');
    await fireEvent.click(row!.querySelector('button')!);
    expect(get(coachOpen)).toBe(true);
    expect(get(coachFocus).topic).toBe('readiness.low');
    expect(coachSession.draft).toContain('Erholungswerte liegen unter meinem Schnitt');
    expect(coachSession.messages).toHaveLength(0);
  });

  it('sagt ohne Erkenntnis, dass nichts auffällt', async () => {
    backend.dashboardVariant = 'empty';
    render(TodayView);
    const section = await screen.findByTestId('today-insights');
    expect(screen.queryAllByTestId('insight')).toHaveLength(0);
    expect(section.textContent).toContain('nichts auf');
  });
});

describe('Dashboard auf der Startseite', () => {
  it('zeigt die Abschnitte in der neuen Reihenfolge, jeder mit Aussage', async () => {
    await activatePlan();
    render(TodayView);
    await screen.findByTestId('today-form');
    const ids = [
      'today-decision',
      'today-insights',
      'today-readiness',
      'today-form',
      'today-week-story',
      'today-ramp',
      'today-distribution',
    ];
    const nodes = ids.map((id) => screen.getByTestId(id));
    for (let i = 1; i < nodes.length; i++) {
      expect(
        nodes[i - 1].compareDocumentPosition(nodes[i]) & Node.DOCUMENT_POSITION_FOLLOWING,
        ids[i],
      ).toBeTruthy();
    }
    for (const id of ids.slice(2)) {
      const section = screen.getByTestId(id);
      expect(
        section.querySelector('.section-statement')?.textContent?.trim().length,
        id,
      ).toBeGreaterThan(0);
    }
    // Nach Verlauf umgezogen.
    for (const id of [
      'today-adherence',
      'today-efficiency',
      'today-power',
      'today-calendar',
      'today-week',
    ]) {
      expect(screen.queryByTestId(id), id).toBeNull();
      expect(screen.queryByTestId(id.replace('today-', 'dash-')), id).toBeNull();
    }
    expect(screen.getByTestId('vital-hrv')).toBeTruthy();
    expect(screen.getAllByTestId('share-week')).toHaveLength(8);
    expect(screen.queryByTestId('vital-body-battery')).toBeNull();
  });

  it('erzählt die Woche mit Ausblick auf die nächsten sieben Tage', async () => {
    await activatePlan();
    render(TodayView);
    const story = await screen.findByTestId('today-week-story');
    expect(story.querySelector('.section-statement')?.textContent).toMatch(
      /^Bis gestern \d+ von \d+ geplanten Einheiten erledigt/,
    );
    expect(screen.getByTestId('week-story-outlook').textContent).toMatch(
      /Die nächsten sieben Tage im Plan: harte Tage \d+, Ruhetage \d+\./,
    );
    expect(story.querySelectorAll('[data-testid="day"]')).toHaveLength(7);
  });

  it('verbindet Form, Bereitschaft und Plan in einem Satz', async () => {
    await activatePlan();
    render(TodayView);
    const lead = (await screen.findByTestId('lead')).textContent ?? '';
    expect(lead).toMatch(/^Deine Form ist /);
    expect(lead).toContain('Erholungswerte');
    expect(lead).toContain('lockere Einheit');
    expect(lead.endsWith('.')).toBe(true);
  });

  it('zeigt die Prognose des aktiven Plans gestrichelt erst mit Plan', async () => {
    render(TodayView);
    await screen.findByTestId('today-form');
    expect(screen.queryByTestId('legend-projection')).toBeNull();
    cleanup();
    await activatePlan();
    render(TodayView);
    expect(await screen.findByTestId('legend-projection')).toBeTruthy();
    expect(screen.getByTestId('projection-note')).toBeTruthy();
  });

  it('sagt ohne Daten je Abschnitt, was fehlt, und zeigt keine Zahlen', async () => {
    backend.dashboardVariant = 'empty';
    render(TodayView);
    await screen.findByTestId('today-readiness');
    expect(screen.getByTestId('today-readiness').textContent).toContain(
      'Verbinde Garmin für Erholungswerte',
    );
    expect(screen.getByTestId('today-form').textContent).toContain('Noch keine Last berechnet');
    expect(screen.getByTestId('today-ramp').textContent).toContain('fehlt noch');
    expect(screen.getByTestId('today-distribution').textContent).toContain('Zeit in Zonen');
    expect(screen.getByTestId('today-week-story').textContent).toContain(
      'Ohne aktiven Plan gibt es keinen Vergleich',
    );
    for (const id of ['ramp-metric', 'distribution-metric', 'form-value', 'vital-hrv']) {
      expect(screen.queryByTestId(id), id).toBeNull();
    }
    expect(screen.getByTestId('lead').textContent).toBe(
      'Deine Einheit für heute und wie erholt du dafür bist.',
    );
  });

  it('zeigt den Hinweis zu R12 nur, wenn die Erholung niedrig ist und der Plan angepasst hat', async () => {
    backend.dashboardVariant = 'low';
    render(TodayView);
    await screen.findByTestId('today-readiness');
    expect(screen.getByTestId('today-readiness').textContent).toContain('deutlich unter');
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
        /NaN|undefined|Infinity|ui\.(decision|insight|week_story)\./,
      );
      unmount();
    }
  });

  it('folgt der Sprache', async () => {
    setLocale('en');
    await activatePlan();
    render(TodayView);
    await screen.findByTestId('today-form');
    expect(screen.getByTestId('today-readiness').querySelector('.section-label')?.textContent).toBe(
      'Readiness',
    );
    expect(screen.getByTestId('lead').textContent).toMatch(/^Your form is /);
    expect(screen.getByTestId('decision-discuss').textContent?.trim()).toBe('Discuss with coach');
    expect(screen.getByTestId('today-insights').querySelector('.section-label')?.textContent).toBe(
      'Coach insights',
    );
    expect(screen.getByTestId('form-state').textContent).toMatch(
      /Fresh|Balanced|Loaded|Overloaded/,
    );
  });
});
