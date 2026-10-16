import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../../App.svelte';
import { setBackend } from '../../lib/backend';
import { resetCoachSetupState } from '../../lib/coach-setup';
import { resetGarminState } from '../../lib/garmin';
import { MockBackend } from '../../lib/backend/mock';
import { setLocale } from '../../lib/i18n';
import { view, wizardOpen } from '../../lib/state';

// Kein Canvas in jsdom.
vi.mock(
  '../../lib/components/UplotChart.svelte',
  async () => import('../../lib/components/UplotChartStub.svelte'),
);

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
  wizardOpen.set(false);
  view.set('today');
  resetGarminState();
  resetCoachSetupState();
});

async function start(options: { onboarded: boolean; setupStepMs?: number }) {
  const backend = new MockBackend({ today: '2026-10-14', ...options });
  setBackend(backend);
  render(App);
  return backend;
}

async function toGarmin() {
  await fireEvent.click(await screen.findByTestId('wizard-next'));
  await screen.findByTestId('garmin-step');
}

describe('Erststart-Assistent', () => {
  it('erscheint beim ersten Start und nicht nach der Einrichtung', async () => {
    await start({ onboarded: false });
    expect(await screen.findByTestId('wizard')).toBeTruthy();
    expect(screen.queryByTestId('nav-today')).toBeNull();
    cleanup();
    await start({ onboarded: true });
    await screen.findByTestId('nav-today');
    expect(screen.queryByTestId('wizard')).toBeNull();
  });

  it('führt durch zwei Schritte ohne Ordnerwahl, Schwellen, Ziel oder Coach (T39)', async () => {
    await start({ onboarded: false });
    expect((await screen.findByTestId('wizard-progress')).textContent).toContain('1 von 2');
    await screen.findByTestId('wizard-step-language');
    expect(screen.queryByTestId('wizard-skip')).toBeNull();
    await fireEvent.click(screen.getByTestId('wizard-next'));
    await screen.findByTestId('wizard-step-garmin');
    expect(screen.getByTestId('wizard-progress').textContent).toContain('2 von 2');
    expect(screen.getByTestId('wizard-skip-hint').textContent).toContain(
      'Schwellenwerte trägst du in den Einstellungen selbst ein',
    );
    // Kein Schritt für Ordner, Schwellen, Ziel oder Coach: Schwellen kommen aus Garmin, der Rest
    // steht in den Einstellungen und im Planer.
    for (const id of ['folder', 'thresholds', 'goal', 'coach']) {
      expect(screen.queryByTestId(`wizard-step-${id}`)).toBeNull();
    }
    expect(screen.queryByTestId('threshold-section')).toBeNull();
    expect(screen.queryByTestId('data-section')).toBeNull();
    await fireEvent.click(screen.getByTestId('wizard-back'));
    await screen.findByTestId('wizard-step-language');
  });

  it('der Garmin-Schritt sagt, was Lastwerk aus Garmin übernimmt', async () => {
    await start({ onboarded: false });
    await screen.findByTestId('wizard');
    await toGarmin();
    const takes = screen.getByTestId('garmin-takes').textContent ?? '';
    for (const word of [
      'FTP',
      'Laktatschwelle',
      'Ruhepuls',
      'Geschlecht',
      'Geburtsjahr',
      'Vorrang',
    ]) {
      expect(takes).toContain(word);
    }
  });

  it('speichert den Abschluss als Einstellung, auch wenn alles übersprungen wird', async () => {
    const backend = await start({ onboarded: false });
    await fireEvent.click(await screen.findByTestId('wizard-next'));
    await fireEvent.click(await screen.findByTestId('wizard-skip'));
    await screen.findByTestId('nav-today');
    expect((await backend.getSettings()).onboarded).toBe(true);
    expect(screen.queryByTestId('wizard')).toBeNull();
  });

  it('der Garmin-Schritt hat kein Feld für Zugangsdaten und keinen Kopierbefehl', async () => {
    await start({ onboarded: false });
    await screen.findByTestId('wizard');
    await toGarmin();
    const step = screen.getByTestId('garmin-step');
    expect(step.querySelector('input')).toBeNull();
    expect(step.querySelector('input[type="password"]')).toBeNull();
    expect(step.querySelector('form')).toBeNull();
    expect(screen.queryByTestId('garmin-command')).toBeNull();
    expect(step.textContent).not.toContain('lastwerk-bridge');
    expect(screen.getByTestId('garmin-risk').textContent).toContain('Inoffizieller Zugang');
    expect((await screen.findByTestId('garmin-state')).textContent).toContain('Nicht verbunden');
    expect(screen.getByTestId('garmin-connect-button').textContent).toContain(
      'Mit Garmin verbinden',
    );
    // Die Seite des Assistenten enthält an keiner Stelle ein Passwortfeld.
    expect(document.querySelector('input[type="password"]')).toBeNull();
  });

  it('verbindet über das Fenster von Garmin und startet den Abgleich', async () => {
    const backend = await start({ onboarded: false });
    await screen.findByTestId('wizard');
    await toGarmin();
    await fireEvent.click(await screen.findByTestId('garmin-connect-button'));
    expect((await screen.findByTestId('garmin-login-result')).textContent).toContain('erfolgreich');
    await waitFor(() =>
      expect(screen.getByTestId('garmin-sync-state').textContent).toContain('Neue Aktivitäten: 3'),
    );
    expect(screen.getByTestId('garmin-state').textContent).toContain('Verbunden');
    expect(screen.queryByTestId('garmin-connect-button')).toBeNull();
    expect(backend.garmin.connected).toBe(true);
    expect(backend.garmin.syncs).toBe(1);
    // Überspringen bleibt möglich, der Abschluss ist derselbe.
    expect(screen.getByTestId('wizard-skip')).toBeTruthy();
  });

  it('meldet eine abgebrochene Anmeldung als Text', async () => {
    const backend = await start({ onboarded: false });
    backend.garmin.loginCode = 'cancelled';
    await screen.findByTestId('wizard');
    await toGarmin();
    await fireEvent.click(await screen.findByTestId('garmin-connect-button'));
    expect((await screen.findByTestId('garmin-login-result')).textContent).toContain('abgebrochen');
    expect(backend.garmin.syncs).toBe(0);
    expect(screen.getByTestId('garmin-connect-button')).toBeTruthy();
  });

  it('lässt sich in den Einstellungen wiederholen', async () => {
    const backend = await start({ onboarded: true });
    await fireEvent.click(await screen.findByTestId('nav-settings'));
    await fireEvent.click(await screen.findByTestId('about-repeat'));
    await screen.findByTestId('wizard');
    expect((await backend.getSettings()).onboarded).toBe(true);
  });
});
