import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../../App.svelte';
import { setBackend } from '../../lib/backend';
import { MockBackend } from '../../lib/backend/mock';
import { resetGarminState } from '../../lib/garmin';
import { setLocale } from '../../lib/i18n';
import { view } from '../../lib/state';
import SettingsView from '../SettingsView.svelte';

vi.mock(
  '../../lib/components/UplotChart.svelte',
  async () => import('../../lib/components/UplotChartStub.svelte'),
);

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
  view.set('today');
  resetGarminState();
});

function mock(options: { garminConnected?: boolean } = {}) {
  const backend = new MockBackend({ today: '2026-10-14', ...options });
  setBackend(backend);
  return backend;
}

describe('Einstellungen, Abschnitt Garmin', () => {
  it('zeigt "nicht verbunden" mit Knopf und Hinweis auf den inoffiziellen Zugang', async () => {
    mock();
    render(SettingsView);
    expect((await screen.findByTestId('garmin-state')).textContent).toContain('Nicht verbunden');
    expect(screen.getByTestId('garmin-connect-button')).toBeTruthy();
    expect(screen.getByTestId('garmin-risk').textContent).toContain('Inoffizieller Zugang');
    expect(screen.getByTestId('garmin-last-sync').textContent).toContain('Noch kein Abgleich');
    expect(screen.queryByTestId('garmin-sync-now')).toBeNull();
    // Kein Feld für Zugangsdaten im ganzen Abschnitt.
    const section = screen.getByTestId('garmin-section');
    expect(section.querySelector('input[type="password"]')).toBeNull();
    expect(section.querySelector('input[type="text"]')).toBeNull();
  });

  it('gleicht auf Klick ab und zeigt das Datum des letzten Abgleichs', async () => {
    const backend = mock({ garminConnected: true });
    render(SettingsView);
    expect((await screen.findByTestId('garmin-state')).textContent).toContain('Verbunden');
    await fireEvent.click(screen.getByTestId('garmin-sync-now'));
    await waitFor(() =>
      expect(screen.getByTestId('garmin-sync-state').textContent).toContain(
        'Neue Aktivitäten: 3, Erholungstage: 30',
      ),
    );
    await waitFor(() =>
      expect(screen.getByTestId('garmin-last-sync').textContent).toContain('14.10.2026'),
    );
    expect(backend.garmin.syncs).toBe(1);
  });

  it('trennt erst nach der Bestätigung', async () => {
    const backend = mock({ garminConnected: true });
    render(SettingsView);
    await fireEvent.click(await screen.findByTestId('garmin-disconnect'));
    expect(screen.getByTestId('garmin-disconnect-confirm').textContent).toContain('Garmin');
    await fireEvent.click(screen.getByTestId('garmin-disconnect-no'));
    expect(backend.garmin.connected).toBe(true);
    await fireEvent.click(screen.getByTestId('garmin-disconnect'));
    await fireEvent.click(screen.getByTestId('garmin-disconnect-yes'));
    await screen.findByTestId('garmin-disconnected');
    expect(backend.garmin.connected).toBe(false);
    expect(screen.getByTestId('garmin-state').textContent).toContain('Nicht verbunden');
  });

  it('schaltet den Abgleich beim Start als Einstellung', async () => {
    const backend = mock();
    render(SettingsView);
    const toggle = (await screen.findByTestId('garmin-sync-on-start')) as HTMLInputElement;
    expect(toggle.checked).toBe(true);
    await fireEvent.click(toggle);
    await waitFor(async () =>
      expect((await backend.getSettings()).garmin_sync_on_start).toBe(false),
    );
    expect((await backend.garminStatus()).sync_on_start).toBe(false);
  });

  it('zeigt eine Sperre mit Uhrzeit', async () => {
    const backend = mock({ garminConnected: true });
    backend.garmin.lockedUntil = '2026-10-14T18:30:00Z';
    render(SettingsView);
    const locked = await screen.findByTestId('garmin-locked');
    expect(locked.textContent).toContain('gesperrt bis');
    expect(locked.textContent).toContain('14.10.26');
  });

  it('nennt einen Abbruch durch Garmin mit Text statt Code', async () => {
    const backend = mock({ garminConnected: true });
    backend.garmin.abortCode = 'throttled';
    render(SettingsView);
    await fireEvent.click(await screen.findByTestId('garmin-sync-now'));
    await waitFor(() =>
      expect(screen.getByTestId('garmin-sync-state').textContent).toContain('drosselt'),
    );
  });

  it('zeigt den Status des Zusatzprogramms nur unter "Erweitert"', async () => {
    const backend = mock();
    backend.bridgeStatus = {
      has_data: true,
      recovery_days: 3,
      activity_files: 2,
      last_day: '2026-10-13',
    };
    const spy = vi.spyOn(backend, 'getBridgeStatus');
    render(SettingsView);
    const advanced = (await screen.findByTestId('garmin-advanced')) as HTMLDetailsElement;
    expect(advanced.open).toBe(false);
    expect(spy).not.toHaveBeenCalled();
    advanced.open = true;
    await fireEvent(advanced, new Event('toggle'));
    await waitFor(() =>
      expect(screen.getByTestId('bridge-status').textContent).toContain('3 Tage Erholung'),
    );
  });
});

describe('Seitenleiste und Start', () => {
  it('gleicht beim Start einmal ab, wenn verbunden, und zeigt den Zustand in der Seitenleiste', async () => {
    const backend = mock({ garminConnected: true });
    render(App);
    await waitFor(() =>
      expect(screen.getByTestId('sidebar-sync').textContent).toContain('Neue Aktivitäten: 3'),
    );
    expect(backend.garmin.syncs).toBe(1);
    expect(backend.garmin.startDone).toBe(true);
  });

  it('gleicht beim Start nicht ab, wenn nicht verbunden oder ausgeschaltet', async () => {
    const off = mock();
    render(App);
    await screen.findByTestId('nav-today');
    expect(off.garmin.syncs).toBe(0);
    expect(screen.queryByTestId('sidebar-sync')).toBeNull();
    cleanup();
    const disabled = mock({ garminConnected: true });
    await disabled.setSettings({ garmin_sync_on_start: false });
    render(App);
    await screen.findByTestId('nav-today');
    expect(disabled.garmin.syncs).toBe(0);
  });
});
