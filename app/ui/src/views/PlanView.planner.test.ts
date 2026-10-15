import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackend } from '../lib/backend';
import { MockBackend } from '../lib/backend/mock';
import { nextMonday } from '../lib/plan';
import { setLocale } from '../lib/i18n';
import { selectedPlanId } from '../lib/state';
import PlanView from './PlanView.svelte';

// Kein Canvas in jsdom.
vi.mock(
  '../lib/components/UplotChart.svelte',
  async () => import('../lib/components/UplotChartStub.svelte'),
);

let backend: MockBackend;

beforeEach(() => {
  backend = new MockBackend({ today: '2026-10-13' });
  setBackend(backend);
  selectedPlanId.set(null);
});

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
});

const dayOn = (i: number) =>
  screen.getByTestId(`planner-day-${i}`).getAttribute('aria-pressed') === 'true';

describe('Planer mit drei Fragen', () => {
  it('zeigt die drei Fragen mit den Standardantworten und eine Primäraktion', async () => {
    render(PlanView);
    const planner = await screen.findByTestId('planner');
    expect(planner.querySelectorAll('button.primary')).toHaveLength(1);
    expect(screen.getByTestId('planner-submit').textContent?.trim()).toBe('Plan erstellen');
    // Fit bleiben, 6 Stunden, Di, Do, Sa, So.
    expect((screen.getByTestId('planner-kind-continuous') as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId('planner-hours-value').textContent).toBe('6h');
    expect([0, 1, 2, 3, 4, 5, 6].map(dayOn)).toEqual([false, true, false, true, false, true, true]);
    expect(screen.getByTestId('planner-days-hint').textContent).toBe(
      '4 Trainingstage, rund 90 Minuten je Tag',
    );
    expect(screen.getByTestId('planner-start').textContent).toContain(
      new Intl.DateTimeFormat('de-DE', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        timeZone: 'UTC',
      }).format(new Date(`${nextMonday()}T00:00:00Z`)),
    );
    // Der Schieber geht von 2 bis 20 in halben Stunden.
    const slider = screen.getByTestId('planner-hours') as HTMLInputElement;
    expect([slider.min, slider.max, slider.step]).toEqual(['2', '20', '0.5']);
  });

  it('zeigt den Wert des Schiebers live und passt die Minuten je Tag an', async () => {
    render(PlanView);
    await screen.findByTestId('planner');
    await fireEvent.input(screen.getByTestId('planner-hours'), { target: { value: '7.5' } });
    expect(screen.getByTestId('planner-hours-value').textContent).toBe('7,5h');
    // 450 Minuten auf 4 Tage = 112,5, auf 5 gerundet 115 (Anzeige "rund").
    expect(screen.getByTestId('planner-days-hint').textContent).toContain('rund 115 Minuten');
    await fireEvent.click(screen.getByTestId('planner-day-0'));
    expect(screen.getByTestId('planner-days-hint').textContent).toBe(
      '5 Trainingstage, rund 90 Minuten je Tag',
    );
  });

  it('sperrt die Primäraktion ohne Trainingstag und sagt warum', async () => {
    render(PlanView);
    await screen.findByTestId('planner');
    for (const i of [1, 3, 5, 6]) await fireEvent.click(screen.getByTestId(`planner-day-${i}`));
    expect((screen.getByTestId('planner-submit') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('planner-days-hint').textContent).toBe('Wähle mindestens einen Tag.');
  });

  it('erzeugt aus den Antworten einen aktiven Plan und schreibt Verfügbarkeit und Ziel', async () => {
    render(PlanView);
    await screen.findByTestId('planner');
    await fireEvent.click(screen.getByTestId('planner-kind-event'));
    const date = `${new Date().getFullYear() + 1}-06-20`;
    await fireEvent.input(screen.getByTestId('planner-date'), { target: { value: date } });
    await fireEvent.change(screen.getByTestId('planner-sport'), { target: { value: 'running' } });
    await fireEvent.input(screen.getByTestId('planner-hours'), { target: { value: '4' } });
    // Nur Mo, Mi, Fr: erst Di, Do, Sa, So abwählen.
    for (const i of [1, 3, 5, 6, 0, 2, 4])
      await fireEvent.click(screen.getByTestId(`planner-day-${i}`));
    await fireEvent.click(screen.getByTestId('planner-submit'));

    await screen.findByTestId('planner-done');
    const display = await screen.findByTestId('plan-display');
    await vi.waitFor(() => expect(display.dataset.status).toBe('active'));
    const inputs = await backend.getPlanningInputs();
    // Handrechnung: 4 Stunden = 240 Minuten auf 3 Tage = 80 Minuten je Tag.
    expect(inputs.availability.map((d) => d.max_minutes)).toEqual([80, 0, 80, 0, 80, 0, 0]);
    expect(inputs.availability.map((d) => d.fixed_rest)).toEqual([
      false,
      true,
      false,
      true,
      false,
      true,
      true,
    ]);
    const goal = inputs.goals.find((g) => g.active);
    expect(goal).toMatchObject({ kind: 'event', event_date: date, sport: 'running' });
    const list = await backend.listPlans();
    expect(list.filter((p) => p.status === 'active')).toHaveLength(1);
  });

  it('verlangt für einen Wettkampf ein Datum und ruft sonst nichts auf', async () => {
    render(PlanView);
    await screen.findByTestId('planner');
    await fireEvent.click(screen.getByTestId('planner-kind-event'));
    await fireEvent.click(screen.getByTestId('planner-submit'));
    expect((await screen.findByTestId('planner-error')).textContent).toBe('Ungültiges Datum.');
    expect(await backend.listPlans()).toEqual([]);
    // Ein Datum vor dem Planbeginn gilt auch nicht.
    await fireEvent.input(screen.getByTestId('planner-date'), { target: { value: '2020-01-01' } });
    await fireEvent.click(screen.getByTestId('planner-submit'));
    expect((await screen.findByTestId('planner-error')).textContent).toBe('Ungültiges Datum.');
    expect(await backend.listPlans()).toEqual([]);
  });

  it('zeigt Fehler des Rust-Teils als Text, hier zu wenig Zeit je Tag', async () => {
    render(PlanView);
    await screen.findByTestId('planner');
    await fireEvent.input(screen.getByTestId('planner-hours'), { target: { value: '2' } });
    for (const i of [0, 2, 4]) await fireEvent.click(screen.getByTestId(`planner-day-${i}`));
    await fireEvent.click(screen.getByTestId('planner-submit'));
    // 120 Minuten auf 7 Tage = 17 -> 15 Minuten, unter der kleinsten Einheit.
    expect((await screen.findByTestId('planner-error')).textContent).toContain('zu knapp');
  });

  it('belegt die Fragen aus dem aktiven Ziel und der Verfügbarkeit vor', async () => {
    const date = `${new Date().getFullYear() + 1}-09-01`;
    await backend.setGoal({ event_date: date, sport: 'swimming', priority: 'A', label: null });
    await backend.setAvailability(
      [0, 1, 2, 3, 4, 5, 6].map((weekday) => ({
        weekday,
        max_minutes: weekday === 2 || weekday === 5 ? 120 : 0,
        allow_hard: true,
        fixed_rest: weekday !== 2 && weekday !== 5,
      })),
    );
    render(PlanView);
    await screen.findByTestId('planner');
    expect((screen.getByTestId('planner-kind-event') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByTestId('planner-date') as HTMLInputElement).value).toBe(date);
    expect((screen.getByTestId('planner-sport') as HTMLSelectElement).value).toBe('swimming');
    expect(screen.getByTestId('planner-hours-value').textContent).toBe('4h');
    expect([0, 1, 2, 3, 4, 5, 6].map(dayOn)).toEqual([
      false,
      false,
      true,
      false,
      false,
      true,
      false,
    ]);
  });
});

describe('Aufbau der Planansicht', () => {
  it('trägt Titel und Aussage, die Panels liegen hinter einer eingeklappten Auswahl', async () => {
    render(PlanView);
    await screen.findByTestId('planner');
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Plan');
    expect(screen.getByTestId('lead').textContent).toContain('Drei Antworten');
    const fine = screen.getByTestId('fine-tuning') as HTMLDetailsElement;
    expect(fine.open).toBe(false);
    expect(fine.querySelector('summary')?.textContent).toContain('Feineinstellungen und Export');
    // Die bisherigen Panels sind weiter da und funktionieren.
    for (const id of [
      'goal-section',
      'availability-section',
      'generate-section',
      'plan-list',
      'export-ics',
    ]) {
      expect(within(fine).getByTestId(id)).toBeTruthy();
    }
    // Planer und Gespräch stehen außerhalb.
    expect(fine.contains(screen.getByTestId('planner'))).toBe(false);
    expect(fine.contains(screen.getByTestId('coach'))).toBe(false);
  });

  it('zeigt Texte auch auf Englisch', async () => {
    setLocale('en');
    render(PlanView);
    await screen.findByTestId('planner');
    expect(screen.getByTestId('planner-submit').textContent?.trim()).toBe('Create plan');
    expect(screen.getByTestId('planner-days-hint').textContent).toBe(
      '4 training days, about 90 minutes each',
    );
    expect(screen.getByTestId('planner-hours-value').textContent).toBe('6h');
    expect(screen.getByTestId('planner-day-0').textContent?.trim()).toBe('Mon');
  });
});
