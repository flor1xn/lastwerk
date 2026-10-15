import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackend } from '../lib/backend';
import { MockBackend } from '../lib/backend/mock';
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

async function generate() {
  await fireEvent.input(screen.getByTestId('gen-week-start'), { target: { value: '2026-10-19' } });
  await fireEvent.input(screen.getByTestId('gen-weeks'), { target: { value: '2' } });
  await fireEvent.click(screen.getByTestId('gen-run'));
  return screen.findByTestId('plan-display');
}

describe('PlanView', () => {
  it('erzeugt einen Plan, zeigt ihn und listet ihn als Entwurf', async () => {
    render(PlanView);
    await screen.findByTestId('generate-section');
    expect(await screen.findByTestId('plan-list-empty')).toBeTruthy();

    const display = await generate();
    expect(within(display).getAllByTestId('plan-week')).toHaveLength(2);
    expect(screen.getByTestId('gen-done').textContent).toContain('Entwurf');
    const row = await screen.findByTestId('plan-row');
    expect(row.dataset.status).toBe('draft');
  });

  it('weist einen Beginn, der kein Montag ist, ab, ohne den Rust-Teil zu fragen', async () => {
    render(PlanView);
    await screen.findByTestId('generate-section');
    await fireEvent.input(screen.getByTestId('gen-week-start'), {
      target: { value: '2026-10-20' },
    });
    await fireEvent.click(screen.getByTestId('gen-run'));
    expect((await screen.findByTestId('gen-error')).textContent).toBe(
      'Der Planbeginn muss ein Montag sein.',
    );
    expect(await backend.listPlans()).toEqual([]);
  });

  it('aktiviert und archiviert Pläne', async () => {
    render(PlanView);
    await screen.findByTestId('generate-section');
    await generate();
    await fireEvent.click(await screen.findByTestId('plan-activate'));
    const row = await screen.findByTestId('plan-row');
    await vi.waitFor(() => expect(row.dataset.status).toBe('active'));
    await vi.waitFor(() =>
      expect(screen.getByTestId('plan-display').dataset.status).toBe('active'),
    );
    await fireEvent.click(screen.getByTestId('plan-archive'));
    await vi.waitFor(() => expect(row.dataset.status).toBe('archived'));
  });

  it('speichert Ziel und Verfügbarkeit', async () => {
    render(PlanView);
    await screen.findByTestId('goal-section');
    await fireEvent.input(screen.getByTestId('goal-date'), { target: { value: '2027-05-01' } });
    await fireEvent.input(screen.getByTestId('goal-label'), { target: { value: 'Mein Rennen' } });
    await fireEvent.click(screen.getByTestId('goal-save'));
    await screen.findByTestId('goal-saved');
    expect(screen.getByTestId('goal-active').textContent).toContain('Termin 01.05.2027');
    expect(screen.getByTestId('goal-active').textContent).toContain('Mein Rennen');

    await fireEvent.input(screen.getByTestId('avail-minutes-2'), { target: { value: '30' } });
    await fireEvent.click(screen.getByTestId('avail-rest-6'));
    await fireEvent.click(screen.getByTestId('avail-save'));
    await screen.findByTestId('avail-saved');
    const saved = (await backend.getPlanningInputs()).availability;
    expect(saved[2].max_minutes).toBe(30);
    expect(saved[6].fixed_rest).toBe(true);
  });

  it('ruft die Exporte über das Backend auf und zeigt das Ergebnis', async () => {
    render(PlanView);
    await screen.findByTestId('generate-section');
    // Ohne Plan sind ICS und JSON gesperrt, das Backup nicht.
    expect((screen.getByTestId('export-ics') as HTMLButtonElement).disabled).toBe(true);
    await generate();
    await vi.waitFor(() =>
      expect((screen.getByTestId('export-ics') as HTMLButtonElement).disabled).toBe(false),
    );

    await fireEvent.click(screen.getByTestId('export-ics'));
    await vi.waitFor(() =>
      expect(screen.getByTestId('export-result').textContent).toMatch(
        /^ICS gespeichert: \d+ Veranstaltungen\.$/,
      ),
    );
    await fireEvent.click(screen.getByTestId('export-json'));
    await vi.waitFor(() =>
      expect(screen.getByTestId('export-result').textContent).toBe('Plan gespeichert.'),
    );
    await fireEvent.click(screen.getByTestId('export-backup'));
    await vi.waitFor(() =>
      expect(screen.getByTestId('export-result').textContent).toContain('Backup gespeichert: '),
    );
    expect(backend.calls.map((c) => c.command)).toEqual([
      'export_plan_ics',
      'export_plan_json',
      'export_backup',
    ]);
  });

  it('verlangt vor der Wiederherstellung eine Bestätigung und weist auf den Import hin', async () => {
    render(PlanView);
    await screen.findByTestId('generate-section');
    await fireEvent.click(screen.getByTestId('export-restore'));
    expect(screen.getByTestId('restore-confirm').textContent).toContain('ersetzt alle Nutzerdaten');
    expect(backend.calls).toEqual([]);

    // Abbrechen ruft nichts auf.
    await fireEvent.click(screen.getByTestId('restore-no'));
    expect(screen.queryByTestId('restore-confirm')).toBeNull();
    expect(backend.calls).toEqual([]);

    await fireEvent.click(screen.getByTestId('export-restore'));
    await fireEvent.click(screen.getByTestId('restore-yes'));
    await vi.waitFor(() =>
      expect(screen.getByTestId('export-result').textContent).toContain('Wiederhergestellt'),
    );
    expect(screen.getByTestId('restore-hint').textContent).toContain('lastwerk rebuild');
    expect(backend.calls).toEqual([{ command: 'restore_backup', args: { confirmed: true } }]);
  });
});
