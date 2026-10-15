import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackend } from '../lib/backend';
import { MockBackend } from '../lib/backend/mock';
import { setLocale } from '../lib/i18n';
import { view } from '../lib/state';
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
  setBackend(null);
  setLocale('de');
  vi.useRealTimers();
});

describe('TodayView', () => {
  it('sagt ohne aktiven Plan, was fehlt, und führt zum Plan', async () => {
    render(TodayView);
    expect((await screen.findByTestId('today-title')).textContent).toBe('Noch kein aktiver Plan');
    await fireEvent.click(screen.getByRole('button', { name: 'Zum Plan' }));
    let current = '';
    view.subscribe((v) => (current = v))();
    expect(current).toBe('plan');
  });

  it('zeigt mit aktivem Plan die Einheit von heute oder einen freien Tag, nie den Hinweis auf fehlenden Plan', async () => {
    const detail = await backend.generatePlan({
      week_start: '2026-10-12',
      goal_date: null,
      goal_sport: null,
      weeks: 2,
      locale: 'de',
      sports: ['cycling'],
      strength_per_week: 0,
    });
    await backend.setPlanStatus(detail.plan.plan_id, 'active');
    render(TodayView);
    const title = (await screen.findByTestId('today-title')).textContent ?? '';
    expect(title).not.toBe('Noch kein aktiver Plan');
    expect(title).not.toBe('Der aktive Plan deckt den heutigen Tag nicht ab');
    expect(title.length).toBeGreaterThan(0);
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

describe('Dashboard auf der Startseite', () => {
  it('zeigt alle Abschnitte mit einer Aussage und den Kennzahlen', async () => {
    render(TodayView);
    await screen.findByTestId('today-form');
    for (const id of [
      'today-readiness',
      'today-form',
      'today-ramp',
      'today-distribution',
      'today-week',
      'today-adherence',
      'today-efficiency',
      'today-power',
      'today-calendar',
    ]) {
      const section = screen.getByTestId(id);
      expect(
        section.querySelector('.section-label')?.textContent?.trim().length,
        id,
      ).toBeGreaterThan(0);
      expect(
        section.querySelector('.section-statement')?.textContent?.trim().length,
        id,
      ).toBeGreaterThan(0);
    }
    expect(screen.getByTestId('vital-hrv')).toBeTruthy();
    expect(screen.getByTestId('vital-resting_hr')).toBeTruthy();
    expect(screen.getByTestId('vital-sleep')).toBeTruthy();
    expect(screen.getAllByTestId('share-week')).toHaveLength(8);
    expect(screen.getAllByTestId('power-row')).toHaveLength(5);
    // Jede Zelle des Kalenders trägt den Wert im title.
    const cells = document.querySelectorAll('[data-testid="calendar"] rect.cell');
    expect(cells.length).toBe(365);
    expect(cells[cells.length - 1].querySelector('title')?.textContent).toMatch(/\d/);
    // Body Battery kommt mit T22: ohne Wert keine Spalte.
    expect(screen.queryByTestId('vital-body-battery')).toBeNull();
  });

  it('verbindet Form, Bereitschaft und Plan in einem Satz', async () => {
    await activatePlan();
    render(TodayView);
    const lead = (await screen.findByTestId('lead')).textContent ?? '';
    expect(lead).toMatch(/^Deine Form ist /);
    expect(lead).toContain('Erholungswerte');
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
    expect(screen.getByTestId('today-adherence').textContent).toContain('aktiven Plan');
    expect(screen.getByTestId('today-efficiency').textContent).toContain('fehlen');
    expect(screen.getByTestId('today-power').textContent).toContain('fehlen');
    expect(screen.getByTestId('today-calendar').textContent).toContain(
      'Noch keine berechnete Last',
    );
    // Keine erfundenen Kennzahlen: weder Metriken noch Diagramme in leeren Abschnitten.
    for (const id of [
      'ramp-metric',
      'distribution-metric',
      'efficiency-metric',
      'form-value',
      'vital-hrv',
      'calendar',
    ]) {
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
    for (const variant of ['extreme', 'sparse', 'gaps'] as const) {
      backend.dashboardVariant = variant;
      const { unmount } = render(TodayView);
      await screen.findByTestId('today-form');
      expect(screen.getByTestId('today').textContent, variant).not.toMatch(
        /NaN|undefined|Infinity/,
      );
      unmount();
    }
  });

  it('folgt der Sprache', async () => {
    setLocale('en');
    render(TodayView);
    await screen.findByTestId('today-form');
    expect(screen.getByTestId('today-readiness').querySelector('.section-label')?.textContent).toBe(
      'Readiness',
    );
    expect(screen.getByTestId('lead').textContent).toMatch(/^Your form is /);
    expect(screen.getByTestId('form-state').textContent).toMatch(
      /Fresh|Balanced|Loaded|Overloaded/,
    );
  });
});
