import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackend } from '../lib/backend';
import { MockBackend, type MockOptions } from '../lib/backend/mock';
import { setLocale } from '../lib/i18n';
import { coachReleased, selectedPlanId } from '../lib/state';
import PlanView from './PlanView.svelte';

// Kein Canvas in jsdom.
vi.mock(
  '../lib/components/UplotChart.svelte',
  async () => import('../lib/components/UplotChartStub.svelte'),
);

let backend: MockBackend;

function setup(options: MockOptions = {}) {
  backend = new MockBackend({ today: '2026-10-13', ...options });
  setBackend(backend);
}

beforeEach(() => {
  selectedPlanId.set(null);
  coachReleased.set(false);
  setup();
});

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
});

/** Plan über die drei Fragen mit den Standardantworten erzeugen. */
async function withPlan() {
  render(PlanView);
  await screen.findByTestId('planner');
  await fireEvent.click(screen.getByTestId('planner-submit'));
  await screen.findByTestId('plan-display');
  await fireEvent.click(await screen.findByTestId('coach-consent-accept'));
  await vi.waitFor(() =>
    expect((screen.getByTestId('coach-input') as HTMLTextAreaElement).disabled).toBe(false),
  );
}

async function withPlanWithoutConsent() {
  render(PlanView);
  await screen.findByTestId('planner');
  await fireEvent.click(screen.getByTestId('planner-submit'));
  await screen.findByTestId('plan-display');
  await screen.findByTestId('coach-consent');
}

async function say(text: string) {
  await fireEvent.input(screen.getByTestId('coach-input'), { target: { value: text } });
  await fireEvent.click(screen.getByTestId('coach-send'));
}

const messages = () => screen.queryAllByTestId('coach-message');

describe('Gespräch mit dem Coach', () => {
  it('zeigt die Antwort als Text, nie als HTML', async () => {
    await withPlan();
    await say('Hallo <b>fett</b> <img src=x onerror=alert(1)>');
    await vi.waitFor(() => expect(messages()).toHaveLength(2));
    const [user, coach] = messages();
    // Die Nachricht des Nutzers steht wörtlich da, ohne Elemente daraus.
    expect(within(user).getByTestId('coach-text').textContent).toBe(
      'Hallo <b>fett</b> <img src=x onerror=alert(1)>',
    );
    expect(user.querySelector('b, img')).toBeNull();
    expect(within(coach).getByTestId('coach-text').textContent).toContain('Deine Form ist');
    expect(coach.querySelector('b, img, a, script')).toBeNull();
  });

  it('setzt auch eine Antwort des Modells mit HTML und Markdown als Text', async () => {
    await withPlan();
    const evil = '<script>boom()</script> **fett** [Link](http://example.com) <img src=x>';
    vi.spyOn(backend, 'coachChat').mockResolvedValue({
      reply: evil,
      proposal: null,
      rejected_codes: [],
    });
    await say('Frage');
    await vi.waitFor(() => expect(messages()).toHaveLength(2));
    const coach = messages()[1];
    expect(within(coach).getByTestId('coach-text').textContent).toBe(evil);
    expect(coach.querySelector('script, img, a, strong, b')).toBeNull();
  });

  it('zeigt Vorher und Nachher und übernimmt einen Vorschlag erst auf Klick', async () => {
    await withPlan();
    const before = (await backend.listPlans()).map((p) => p.id);
    await say('Hallo');
    await vi.waitFor(() => expect(messages()).toHaveLength(2));
    await say('Weniger');
    const card = await screen.findByTestId('proposal');
    expect(card.dataset.kind).toBe('plan_edit');
    expect(within(card).getByTestId('proposal-explanation').textContent).toContain('15 Minuten');
    expect(within(card).getByTestId('proposal-changes').querySelectorAll('li')).toHaveLength(2);
    const rows = within(card).getAllByTestId('proposal-week');
    expect(rows.length).toBeGreaterThan(0);
    // Mindestens eine Woche hat Vorher und Nachher in einer Zelle.
    expect(card.textContent).toMatch(/\d+ → \d+/);
    // Noch nichts übernommen.
    expect((await backend.listPlans()).map((p) => p.id)).toEqual(before);

    await fireEvent.click(within(card).getByTestId('proposal-accept'));
    await within(card).findByTestId('proposal-applied');
    const list = await backend.listPlans();
    expect(list).toHaveLength(before.length + 1);
    const active = list.filter((p) => p.status === 'active');
    expect(active).toHaveLength(1);
    expect(active[0].source).toBe('llm');
    // Die Ansicht zeigt den neuen Plan.
    await vi.waitFor(() =>
      expect(screen.getByTestId('plan-display').dataset.planId).toBe(active[0].id),
    );
  });

  it('verwirft einen Vorschlag ohne etwas zu ändern', async () => {
    await withPlan();
    await say('a');
    await vi.waitFor(() => expect(messages()).toHaveLength(2));
    await say('b');
    await screen.findByTestId('proposal');
    const plans = await backend.listPlans();
    await fireEvent.click(screen.getByTestId('proposal-dismiss'));
    await screen.findByTestId('proposal-dismissed');
    expect(await backend.listPlans()).toEqual(plans);
    // Verworfen ist verworfen: kein Knopf zum Übernehmen mehr.
    expect(screen.queryByTestId('proposal-accept')).toBeNull();
  });

  it('zeigt neue Zeiten mit den Tagen und übernimmt sie', async () => {
    await withPlan();
    for (const t of ['a', 'b']) {
      await say(t);
      await vi.waitFor(() => expect(messages().length).toBeGreaterThanOrEqual(2));
      await vi.waitFor(() => expect(screen.getByTestId('coach-send')).toBeTruthy());
    }
    await vi.waitFor(() => expect(messages()).toHaveLength(4));
    await say('c');
    const card = await vi.waitFor(() => {
      const c = screen
        .getAllByTestId('proposal')
        .find((x) => (x as HTMLElement).dataset.kind === 'planning_change');
      if (!c) throw new Error('noch nicht da');
      return c as HTMLElement;
    });
    expect(within(card).getByTestId('proposal-days').querySelectorAll('li')).toHaveLength(7);
    expect(card.textContent).toContain('Beginn:');
    await fireEvent.click(within(card).getByTestId('proposal-accept'));
    await within(card).findByTestId('proposal-applied');
    // Der ältere, offene Vorschlag bezieht sich auf den alten Plan und ist veraltet.
    const stale = screen.getAllByTestId('proposal').filter((x) => x.dataset.state === 'stale');
    expect(stale.length).toBeGreaterThanOrEqual(1);
  });

  it('nennt die Regelcodes eines verworfenen Vorschlags', async () => {
    await withPlan();
    for (let i = 0; i < 3; i++) {
      await say(`n${i}`);
      await vi.waitFor(() => expect(messages()).toHaveLength((i + 1) * 2));
    }
    await say('Zwei harte Tage');
    const note = await screen.findByTestId('coach-rejected');
    expect(note.textContent).toContain('R03');
    expect(screen.getAllByTestId('proposal')).toHaveLength(2);
  });

  it('zeigt einen Fehler, behält die Nachricht im Feld und lässt sie nicht im Verlauf', async () => {
    await withPlan();
    vi.spyOn(backend, 'coachChat').mockRejectedValue(
      new (await import('../lib/backend/types')).BackendError(
        'coach_timeout',
        'ui.error.coach_timeout',
      ),
    );
    await say('Meine Frage');
    const error = await screen.findByTestId('coach-error');
    expect(error.textContent).toBe('Das Modell hat nicht rechtzeitig geantwortet.');
    expect(messages()).toHaveLength(0);
    expect((screen.getByTestId('coach-input') as HTMLTextAreaElement).value).toBe('Meine Frage');
  });

  it('bricht ab, gibt das Feld frei und ignoriert die späte Antwort', async () => {
    setup({ coachDelayMs: 60 });
    await withPlan();
    await say('Langsame Frage');
    await screen.findByTestId('coach-thinking');
    await fireEvent.click(screen.getByTestId('coach-cancel'));
    expect(screen.queryByTestId('coach-thinking')).toBeNull();
    expect((screen.getByTestId('coach-input') as HTMLTextAreaElement).value).toBe('Langsame Frage');
    await new Promise((r) => setTimeout(r, 120));
    expect(messages()).toHaveLength(0);
    expect(screen.queryByTestId('coach-error')).toBeNull();
  });

  it('beginnt auf Wunsch von vorn', async () => {
    await withPlan();
    await say('Hallo');
    await vi.waitFor(() => expect(messages()).toHaveLength(2));
    await fireEvent.click(screen.getByTestId('coach-reset'));
    await vi.waitFor(() => expect(messages()).toHaveLength(0));
    expect(screen.getByTestId('coach-empty')).toBeTruthy();
    expect((await backend.coachPreview('')).messages).toHaveLength(1);
  });

  it('füllt einen Vorschlag für die erste Frage ins Feld, ohne zu senden', async () => {
    await withPlan();
    await fireEvent.click(screen.getByTestId('coach-suggestion-busy_day'));
    expect((screen.getByTestId('coach-input') as HTMLTextAreaElement).value).toBe(
      'Dienstag habe ich keine Zeit',
    );
    expect(messages()).toHaveLength(0);
  });

  it('zeigt den exakten Eingang auf Klick und folgt dem Eingabefeld', async () => {
    await withPlan();
    await fireEvent.click(screen.getByTestId('coach-seen'));
    await vi.waitFor(() => expect(screen.getAllByTestId('coach-preview-message')).toHaveLength(1));
    const system = screen.getAllByTestId('coach-preview-message')[0];
    expect(system.dataset.role).toBe('system');
    expect(system.textContent).toContain('"base_plan"');
    expect(screen.getByTestId('coach-preview-target').textContent).toContain('127.0.0.1');
    await fireEvent.input(screen.getByTestId('coach-input'), { target: { value: 'Mein Wunsch' } });
    await vi.waitFor(() => {
      const all = screen.getAllByTestId('coach-preview-message');
      expect(all).toHaveLength(2);
      expect(all[1].textContent).toContain('Mein Wunsch');
    });
    // Die Vorschau sendet nichts.
    expect(messages()).toHaveLength(0);
  });

  it('zählt Zeichen erst nahe der Grenze und sendet mit Enter', async () => {
    await withPlan();
    const input = screen.getByTestId('coach-input') as HTMLTextAreaElement;
    expect(input.maxLength).toBe(1000);
    await fireEvent.input(input, { target: { value: 'x'.repeat(850) } });
    expect(screen.getByTestId('coach-counter').textContent?.trim()).toBe('850 von 1000');
    await fireEvent.input(input, { target: { value: 'Hallo' } });
    expect(screen.getByTestId('coach-counter').textContent?.trim()).toBe('');
    // Umschalt-Enter ist ein Zeilenumbruch und sendet nicht, Enter sendet.
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    expect(messages()).toHaveLength(0);
    await fireEvent.keyDown(input, { key: 'Enter' });
    await vi.waitFor(() => expect(messages()).toHaveLength(2));
  });

  it('verlangt eine Freigabe je Sitzung und lässt vorher die Vorschau zu', async () => {
    await withPlanWithoutConsent();
    const consent = screen.getByTestId('coach-consent');
    expect(consent.textContent).toContain('qwen3:4b');
    expect(consent.textContent).toContain('auf diesem Rechner');
    expect(screen.queryByTestId('coach-input')).toBeNull();
    // Die Vorschau geht schon vor der Freigabe.
    await fireEvent.click(screen.getByTestId('coach-seen'));
    await vi.waitFor(() => expect(screen.getAllByTestId('coach-preview-message')).toHaveLength(1));
    // Es wird nichts gesendet, solange nicht freigegeben ist.
    expect(messages()).toHaveLength(0);
    await fireEvent.click(screen.getByTestId('coach-consent-accept'));
    expect(screen.queryByTestId('coach-consent')).toBeNull();
    expect(screen.getByTestId('coach-input')).toBeTruthy();
    // Die Freigabe gilt für die Sitzung: Eine neu aufgebaute Ansicht fragt nicht noch einmal.
    cleanup();
    render(PlanView);
    await screen.findByTestId('coach-input');
    expect(screen.queryByTestId('coach-consent')).toBeNull();
  });

  it('erlaubt ohne Plan kein Senden und sagt, was fehlt', async () => {
    render(PlanView);
    await screen.findByTestId('planner');
    await fireEvent.click(await screen.findByTestId('coach-consent-accept'));
    await screen.findByTestId('coach-input');
    expect((screen.getByTestId('coach-input') as HTMLTextAreaElement).disabled).toBe(true);
    expect(screen.getByTestId('coach-no-plan').textContent).toContain('Erstelle zuerst einen');
  });
});

describe('ohne lokales Modell', () => {
  beforeEach(() => setup({ coach: false }));

  it('erklärt die Einrichtung, der Planer bleibt voll nutzbar', async () => {
    render(PlanView);
    const setupBox = await screen.findByTestId('coach-setup');
    expect(setupBox.dataset.reason).toBe('no_server');
    expect(setupBox.textContent).toContain('ollama pull');
    expect(screen.getByTestId('coach-setup-reason').textContent).toContain('127.0.0.1:11434');
    expect(screen.queryByTestId('coach-input')).toBeNull();

    await screen.findByTestId('planner');
    await fireEvent.click(screen.getByTestId('planner-submit'));
    await screen.findByTestId('plan-display');
    expect((await backend.listPlans()).filter((p) => p.status === 'active')).toHaveLength(1);
  });

  it('prüft auf Klick erneut', async () => {
    render(PlanView);
    await screen.findByTestId('coach-setup');
    const spy = vi.spyOn(backend, 'coachStatus');
    await fireEvent.click(screen.getByTestId('coach-recheck'));
    await vi.waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    await screen.findByTestId('coach-setup');
  });
});

describe('Modellwahl', () => {
  it('zeigt bei mehreren Modellen eine Auswahl und merkt die Wahl', async () => {
    render(PlanView);
    const select = (await screen.findByTestId('coach-model')) as HTMLSelectElement;
    expect([...select.options].map((o) => o.value)).toEqual(['qwen3:4b', 'llama3.2:3b']);
    await fireEvent.change(select, { target: { value: 'llama3.2:3b' } });
    await vi.waitFor(() =>
      expect((screen.getByTestId('coach-model') as HTMLSelectElement).value).toBe('llama3.2:3b'),
    );
    expect((await backend.coachStatus()).model).toBe('llama3.2:3b');
  });
});
