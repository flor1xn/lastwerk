import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackend } from '../lib/backend';
import { MockBackend } from '../lib/backend/mock';
import { setLocale } from '../lib/i18n';
import { get } from 'svelte/store';
import { openHistory, settingsTab, view } from '../lib/state';
import HealthView from './HealthView.svelte';

let backend: MockBackend;

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['Date'] });
  vi.setSystemTime(new Date('2026-10-14T12:00:00'));
  backend = new MockBackend({ today: '2026-10-14', garminConnected: true });
  setBackend(backend);
  openHistory('body');
});

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
  vi.useRealTimers();
});

describe('Gesundheit (T39)', () => {
  it('zeigt Tages- und Trainingswerte mit Stand und Verlauf, Garmins Last gekennzeichnet', async () => {
    render(HealthView);
    expect((await screen.findByTestId('lead')).textContent).toContain('Lastwerk rechnet nicht');
    const daily = await screen.findByTestId('health-daily');
    expect(daily.querySelector('.section-label')?.textContent).toBe('Tageswerte');
    const steps = screen.getByTestId('health-steps');
    expect(steps.textContent).toContain('Schritte');
    expect(steps.textContent).toContain('Stand 14.10.2026');
    expect(steps.querySelector('svg.spark')).not.toBeNull();
    const load = screen.getByTestId('health-garmin_acute_load');
    expect(load.textContent).toContain('Garmins akute Last');
    expect(load.textContent).toContain('von Garmin');
    expect(screen.getByTestId('health-readiness-level').textContent).toMatch(
      /Schwach|Niedrig|Mäßig|Hoch|Optimal/,
    );
    // Ohne Freigabe kein Abschnitt Hydration.
    expect(screen.queryByTestId('health-hydration')).toBeNull();
    expect(screen.getByTestId('health').textContent).not.toContain('NaN');
  });

  it('wechselt den Zeitraum, spärliche Werte erscheinen mit ihrem Tag', async () => {
    render(HealthView);
    await screen.findByTestId('health-daily');
    await fireEvent.click(
      screen.getByTestId('health-range').querySelector('button[data-value="365"]')!,
    );
    const vo2 = await screen.findByTestId('health-vo2max_running');
    expect(vo2.textContent).toContain('ml/kg/min');
    expect(vo2.textContent).toMatch(/Stand \d\d\.\d\d\.2026/);
  });

  it('zeigt Hydration nur nach Freigabe', async () => {
    await backend.setSettings({ garmin_share_hydration: true });
    await backend.garminSync('manual', () => {});
    render(HealthView);
    const section = await screen.findByTestId('health-hydration');
    expect(section.textContent).toContain('Freigabe');
    expect(screen.getByTestId('health-hydration_ml').textContent).toContain('ml');
  });

  it('zeigt Trainingsstatus, Rennprognosen und Lauftoleranz, S3-Bereiche nur nach Freigabe (T40)', async () => {
    await backend.setSettings({ garmin_share_body: true });
    await backend.garminSync('manual', () => {});
    render(HealthView);
    const status = await screen.findByTestId('health-status');
    expect(status.textContent).toContain('Trainingsstatus');
    expect(screen.getByTestId('health-training-status').textContent).toMatch(
      /Produktiv|Erhaltend|Erholung|Höchstform/,
    );
    expect(screen.getByTestId('health-load_anaerobic-target').textContent).toContain(
      'Ziel 80 bis 230',
    );
    expect(screen.getByTestId('health-balance').textContent).toContain('Zu wenig anaerob');
    // Rennzeiten als Dauer, nicht als Sekundenzahl.
    expect(screen.getByTestId('health-race_5k_s').textContent).toMatch(/\d+:\d\d/);
    expect(screen.getByTestId('health-tolerance')).not.toBeNull();
    expect(screen.getByTestId('health-floors_up').textContent).toContain('Etagen');
    expect(screen.getByTestId('health-body').textContent).toContain('Körper');
    expect(screen.getByTestId('health-weight_kg').textContent).toContain('kg');
    expect(screen.queryByTestId('health-blood-pressure')).toBeNull();
    expect(screen.queryByTestId('health-nutrition')).toBeNull();
  });

  it('sagt ohne Werte, woher sie kommen, und führt in die Einstellungen', async () => {
    backend.dashboardVariant = 'empty';
    render(HealthView);
    const empty = await screen.findByTestId('health-empty');
    expect(empty.textContent).toContain('Abgleich aus Garmin');
    await fireEvent.click(screen.getByRole('button', { name: 'Zu den Einstellungen' }));
    // Garmin liefert die Werte: Reiter "Daten und Garmin".
    expect(get(view)).toBe('settings');
    expect(get(settingsTab)).toBe('data');
  });

  it('übersteht extreme Werte ohne Platzhalterfehler', async () => {
    backend.dashboardVariant = 'extreme';
    render(HealthView);
    await waitFor(() => expect(screen.getByTestId('health-steps').textContent).toContain('64.812'));
    // Unbekannte Stufe von Garmin erscheint lesbar, nie als Schlüssel.
    expect(screen.getByTestId('health').textContent).not.toContain('ui.health');
  });
});
