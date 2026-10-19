import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../../../App.svelte';
import { setBackend } from '../../backend';
import { MockBackend } from '../../backend/mock';
import { coachSession } from '../../coach-session.svelte';
import { setLocale } from '../../i18n';
import {
  closeActivity,
  closeCoach,
  coachFocus,
  coachOpen,
  coachReleased,
  historyTab,
  openActivity,
  openCoach,
  openHistory,
  view,
  wizardOpen,
} from '../../state';

// Seitenfenster des Coachs in der ganzen App (T43): Knopf und Kürzel, Esc, Prefill, Gespräch
// über Ansichtswechsel hinweg, Fokus als Codes.

vi.mock('../UplotChart.svelte', async () => import('../UplotChartStub.svelte'));

let backend: MockBackend;

beforeEach(async () => {
  backend = new MockBackend({ today: '2026-10-13' });
  await backend.quickPlan({
    goal: { kind: 'continuous', date: null, sport: null },
    hours_per_week: 6,
    days: [0, 2, 4, 6],
    week_start: '2026-10-19',
    locale: 'de',
  });
  setBackend(backend);
  coachSession.clearForTests();
  coachReleased.set(false);
  closeCoach();
});

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
  view.set('today');
  historyTab.set('calendar');
  closeActivity();
  closeCoach();
  wizardOpen.set(false);
});

const drawer = () => screen.getByTestId('coach-drawer');
const input = () => screen.getByTestId('coach-input') as HTMLTextAreaElement;
const messages = () => screen.queryAllByTestId('coach-message');

async function app() {
  render(App);
  return screen.findByTestId('nav-coach');
}

async function release() {
  await fireEvent.click(await screen.findByTestId('coach-consent-accept'));
  await vi.waitFor(() => expect(input().disabled).toBe(false));
}

describe('Seitenfenster des Coachs', () => {
  it('öffnet und schließt über den Knopf in der Seitenleiste, nicht modal', async () => {
    const toggle = await app();
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(drawer().dataset.open).toBe('false');
    expect(drawer().getAttribute('aria-hidden')).toBe('true');
    expect(screen.queryByTestId('coach')).toBeNull();

    await fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    expect(drawer().dataset.open).toBe('true');
    expect(drawer().tagName).toBe('ASIDE');
    expect(drawer().getAttribute('aria-modal')).toBeNull();
    expect(await screen.findByTestId('coach')).toBeTruthy();
    // Der Inhalt bleibt bedienbar: Die Navigation wechselt die Ansicht bei offenem Fenster.
    await fireEvent.click(screen.getByTestId('nav-history'));
    expect(get(view)).toBe('history');
    expect(get(coachOpen)).toBe(true);

    await fireEvent.click(toggle);
    expect(get(coachOpen)).toBe(false);
  });

  it('öffnet mit Strg+J und Cmd+J, Esc schließt und gibt den Fokus zurück', async () => {
    const toggle = await app();
    toggle.focus();
    await fireEvent.keyDown(window, { key: 'j', ctrlKey: true });
    expect(get(coachOpen)).toBe(true);
    await release();
    await vi.waitFor(() => expect(document.activeElement).toBe(input()));

    await fireEvent.keyDown(input(), { key: 'Escape' });
    expect(get(coachOpen)).toBe(false);
    await vi.waitFor(() => expect(document.activeElement).toBe(toggle));

    await fireEvent.keyDown(window, { key: 'J', metaKey: true });
    expect(get(coachOpen)).toBe(true);
    await fireEvent.keyDown(window, { key: 'j', ctrlKey: true });
    expect(get(coachOpen)).toBe(false);
    // Ohne Strg oder Cmd passiert nichts.
    await fireEvent.keyDown(window, { key: 'j' });
    expect(get(coachOpen)).toBe(false);
  });

  it('reagiert im Assistenten nicht auf das Kürzel', async () => {
    await app();
    wizardOpen.set(true);
    await screen.findByTestId('wizard');
    await fireEvent.keyDown(window, { key: 'j', ctrlKey: true });
    expect(get(coachOpen)).toBe(false);
  });

  it('füllt mit einem Prefill nur das Eingabefeld und sendet nie', async () => {
    await app();
    const chat = vi.spyOn(backend, 'coachChat');
    openCoach({ prefill: 'Warum ist die Rampe so steil?', topic: 'ramp.steep' });
    await release();
    expect(input().value).toBe('Warum ist die Rampe so steil?');
    expect(messages()).toHaveLength(0);
    expect(chat).not.toHaveBeenCalled();
    expect(screen.getByTestId('coach-context').textContent).toBe(
      'Ich sehe: Heute · Thema Fitnessanstieg',
    );
    // Auch ohne Freigabe sendet ein Prefill nichts: Die Freigabe bleibt das Tor.
    closeCoach();
    coachReleased.set(false);
    openCoach({ prefill: 'Noch eine Frage' });
    await screen.findByTestId('coach-consent');
    expect(chat).not.toHaveBeenCalled();
  });

  it('behält das Gespräch beim Schließen und beim Wechsel der Ansicht', async () => {
    const toggle = await app();
    await fireEvent.click(toggle);
    await release();
    await fireEvent.input(input(), { target: { value: 'Hallo Coach' } });
    await fireEvent.click(screen.getByTestId('coach-send'));
    await vi.waitFor(() => expect(messages()).toHaveLength(2));

    await fireEvent.click(toggle);
    await fireEvent.click(screen.getByTestId('nav-plan'));
    await fireEvent.click(screen.getByTestId('nav-history'));
    await fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    expect(messages()).toHaveLength(2);
    expect(messages()[0].textContent).toContain('Hallo Coach');

    // Mit dem Fokus der Ansicht, in der gesendet wurde.
    expect(backend.coachFocusLog()).toEqual([{ view: 'today', topic: null }]);

    // "Neu beginnen" leert das Gespräch.
    await fireEvent.click(screen.getByTestId('coach-reset'));
    await vi.waitFor(() => expect(messages()).toHaveLength(0));
  });

  it('zeigt die Kontextzeile je Ansicht, ohne ID der Aktivität im Fokus', async () => {
    await app();
    openCoach();
    const line = () => screen.getByTestId('coach-context').textContent;
    expect(line()).toBe('Ich sehe: Heute');
    openHistory('performance');
    await vi.waitFor(() => expect(line()).toBe('Ich sehe: Verlauf · Leistung'));
    openActivity(4711);
    await vi.waitFor(() => expect(line()).toBe('Ich sehe: Verlauf · Aktivität'));
    expect(get(coachFocus)).toEqual({ view: 'history_activity', topic: null });
    expect(JSON.stringify(get(coachFocus))).not.toContain('4711');
    openHistory('body');
    await vi.waitFor(() => expect(line()).toBe('Ich sehe: Verlauf · Körper · Erholung'));
  });

  it('vergisst das Thema, sobald der Nutzer die Ansicht wechselt', () => {
    openCoach({ topic: 'decision.lighten' });
    expect(get(coachFocus)).toEqual({ view: 'today', topic: 'decision.lighten' });
    view.set('plan');
    expect(get(coachFocus)).toEqual({ view: 'plan', topic: null });
    view.set('today');
    expect(get(coachFocus).topic).toBeNull();
  });

  it('der Knopf in der Planansicht öffnet das Fenster', async () => {
    await app();
    await fireEvent.click(screen.getByTestId('nav-plan'));
    await fireEvent.click(await screen.findByTestId('plan-talk-coach'));
    expect(get(coachOpen)).toBe(true);
    expect(screen.getByTestId('coach-context').textContent).toBe('Ich sehe: Plan');
  });
});
