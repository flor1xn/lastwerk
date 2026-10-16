import { cleanup, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackend } from '../lib/backend';
import { MockBackend } from '../lib/backend/mock';
import { setLocale } from '../lib/i18n';
import { selectedActivityId } from '../lib/state';
import ActivityDetailView from './ActivityDetailView.svelte';

vi.mock(
  '../lib/components/UplotChart.svelte',
  async () => import('../lib/components/UplotChartStub.svelte'),
);

let backend: MockBackend;

beforeEach(() => {
  backend = new MockBackend({ today: '2026-10-14' });
  setBackend(backend);
});

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
  selectedActivityId.set(null);
});

async function firstOf(sport: 'running' | 'cycling') {
  const list = await backend.listActivities({ from: '2026-06-01', to: '2026-10-14', sport });
  return list.filter((a) => a.is_primary);
}

describe('Detail einer Aktivität (T39)', () => {
  it('nennt den fehlenden Wert und Garmins Last getrennt von der eigenen', async () => {
    const run = (await firstOf('running'))[0];
    selectedActivityId.set(run.id);
    render(ActivityDetailView);
    const reason = await screen.findByTestId('load-reason');
    expect(reason.textContent).toContain('Laufschwellengeschwindigkeit fehlt');
    expect(reason.textContent).toContain('Eintragen unter Einstellungen, Schwellenwerte');
    expect(screen.getByTestId('load-reason-settings')).toBeTruthy();
    // Die Attrappe meldet für jeden Lauf Garmins Last.
    expect(screen.getByTestId('garmin-load').textContent).toContain('von Garmin');
    expect(screen.getByTestId('garmin-load-note').textContent).toContain('eigenen Last');
    expect(screen.getByText('Garmins Trainingslast')).toBeTruthy();
    expect(screen.getByText('Trainingseffekt (Garmin)')).toBeTruthy();
    expect(screen.getByTestId('garmin-effect').textContent).not.toContain('_');
  });

  it('zeigt ohne Garmins Last keine Zeilen dafür', async () => {
    const list = await backend.listActivities({
      from: '2026-06-01',
      to: '2026-10-14',
      sport: null,
    });
    const swim = list.find((a) => a.sport === 'swimming')!;
    selectedActivityId.set(swim.id);
    render(ActivityDetailView);
    await screen.findByTestId('metrics');
    expect(screen.queryByTestId('garmin-load')).toBeNull();
    expect(screen.queryByTestId('garmin-load-note')).toBeNull();
  });
});
