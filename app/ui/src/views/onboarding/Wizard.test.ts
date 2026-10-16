import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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
  vi.restoreAllMocks();
  cleanup();
  setBackend(null);
  setLocale('de');
  wizardOpen.set(false);
  view.set('today');
  resetGarminState();
  resetCoachSetupState();
});

async function start(
  options: ConstructorParameters<typeof MockBackend>[0] & { onboarded: boolean },
) {
  const backend = new MockBackend({ today: '2026-10-14', ...options });
  setBackend(backend);
  render(App);
  return backend;
}

/** Systemsprache des Browsers (jsdom meldet sonst en-US). */
function systemLanguage(lang: string) {
  vi.spyOn(navigator, 'language', 'get').mockReturnValue(lang);
}

beforeEach(() => systemLanguage('de-DE'));

async function connect() {
  await fireEvent.click(await screen.findByTestId('garmin-connect-button'));
  await screen.findByTestId('garmin-login-result');
}

describe('Einrichtung (T41)', () => {
  it('erscheint beim ersten Start und nicht nach der Einrichtung', async () => {
    await start({ onboarded: false });
    expect(await screen.findByTestId('wizard')).toBeTruthy();
    expect(screen.queryByTestId('nav-today')).toBeNull();
    cleanup();
    await start({ onboarded: true });
    await screen.findByTestId('nav-today');
    expect(screen.queryByTestId('wizard')).toBeNull();
  });

  it('beginnt mit Garmin, ohne Sprachschritt und ohne Fragen nach Schwellen', async () => {
    await start({ onboarded: false });
    expect((await screen.findByTestId('wizard-progress')).textContent).toContain('1 von 3');
    await screen.findByTestId('wizard-step-garmin');
    expect(screen.getByTestId('wizard-title').textContent).toContain('Mit Garmin verbinden');
    for (const id of ['language', 'folder', 'thresholds', 'goal']) {
      expect(screen.queryByTestId(`wizard-step-${id}`)).toBeNull();
    }
    expect(screen.queryByTestId('threshold-section')).toBeNull();
    // Ohne Verbindung heißt der Knopf "Ohne Garmin weiter" und sagt, was dann fehlt.
    expect(screen.getByTestId('wizard-skip').textContent).toContain('Ohne Garmin weiter');
    expect(screen.getByTestId('wizard-skip-hint').textContent).toContain(
      'Schwellenwerte trägst du in den Einstellungen selbst ein',
    );
    const takes = screen.getByTestId('garmin-takes').textContent ?? '';
    for (const word of ['FTP', 'Laktatschwelle', 'Ruhepuls', 'Geschlecht', 'Geburtsjahr']) {
      expect(takes).toContain(word);
    }
  });

  it('übernimmt beim ersten Start die Sprache des Systems', async () => {
    systemLanguage('en-GB');
    const backend = await start({ onboarded: false });
    await waitFor(() =>
      expect(screen.getByTestId('wizard-title').textContent).toContain('Connect with Garmin'),
    );
    expect((await backend.getSettings()).locale).toBe('en');
    // Wählbar bleibt sie oben im Assistenten.
    await fireEvent.change(screen.getByTestId('wizard-language'), { target: { value: 'de' } });
    await waitFor(() =>
      expect(screen.getByTestId('wizard-title').textContent).toContain('Mit Garmin verbinden'),
    );
  });

  it('der Garmin-Schritt hat kein Feld für Zugangsdaten und keinen Kopierbefehl', async () => {
    await start({ onboarded: false });
    const step = await screen.findByTestId('garmin-step');
    expect(step.querySelector('input')).toBeNull();
    expect(step.querySelector('form')).toBeNull();
    expect(screen.queryByTestId('garmin-command')).toBeNull();
    expect(step.textContent).not.toContain('lastwerk-bridge');
    expect(screen.getByTestId('garmin-risk').textContent).toContain('Inoffizieller Zugang');
    expect((await screen.findByTestId('garmin-state')).textContent).toContain('Nicht verbunden');
    expect(document.querySelector('input[type="password"]')).toBeNull();
  });

  it('meldet eine abgebrochene Anmeldung als Text', async () => {
    const backend = await start({ onboarded: false });
    backend.garmin.loginCode = 'cancelled';
    await connect();
    expect(screen.getByTestId('garmin-login-result').textContent).toContain('abgebrochen');
    expect(backend.garmin.syncs).toBe(0);
    expect(screen.getByTestId('wizard-skip')).toBeTruthy();
  });

  it('führt von Garmin über Freigaben und Coach zum fertigen Plan aus der Historie', async () => {
    const history = { sport: 'running' as const, hours: 7.5, days: [0, 2, 4, 6], weeks: 11 };
    const backend = await start({ onboarded: false, history, historyAfterSync: true });
    await connect();
    await waitFor(() => expect(backend.garmin.syncs).toBe(1));
    await fireEvent.click(await screen.findByTestId('wizard-next'));

    // Schritt 2: Abgleich sichtbar, S3 je Bereich (alle aus), Coach nur auf Klick.
    await screen.findByTestId('wizard-step-extras');
    expect(screen.getByTestId('wizard-progress').textContent).toContain('2 von 3');
    expect(screen.getByTestId('wizard-sync')).toBeTruthy();
    for (const c of ['body', 'blood_pressure', 'nutrition', 'hydration']) {
      expect((screen.getByTestId(`garmin-share-${c}`) as HTMLInputElement).checked, c).toBe(false);
    }
    expect(screen.queryByTestId('garmin-delete-body')).toBeNull();
    await fireEvent.click(screen.getByTestId('garmin-share-body'));
    await waitFor(async () => expect((await backend.getSettings()).garmin_share_body).toBe(true));
    expect(screen.getByTestId('wizard-coach').querySelector('details')?.open).toBe(false);

    // Schritt 3: die drei Fragen, vorbelegt aus der Historie.
    await fireEvent.click(screen.getByTestId('wizard-next'));
    await screen.findByTestId('wizard-step-plan');
    expect((await screen.findByTestId('wizard-plan-history')).textContent).toContain('11 Wochen');
    const planner = await screen.findByTestId('planner');
    expect(planner.textContent).toContain('7,5');
    await fireEvent.click(screen.getByTestId('planner-submit'));
    await screen.findByTestId('nav-today');
    expect((await backend.getSettings()).onboarded).toBe(true);
    const plans = await backend.listPlans();
    expect(plans.some((p) => p.status === 'active')).toBe(true);
  });

  it('lässt sich ohne Garmin und ohne Plan abschließen', async () => {
    const backend = await start({ onboarded: false });
    await fireEvent.click(await screen.findByTestId('wizard-skip'));
    await screen.findByTestId('wizard-step-extras');
    // Ohne Verbindung keine Freigaben, der Coach bleibt angeboten.
    expect(screen.queryByTestId('wizard-s3')).toBeNull();
    expect(screen.getByTestId('wizard-coach')).toBeTruthy();
    await fireEvent.click(screen.getByTestId('wizard-next'));
    await fireEvent.click(await screen.findByTestId('wizard-finish'));
    await screen.findByTestId('nav-today');
    expect((await backend.getSettings()).onboarded).toBe(true);
    expect(await backend.listPlans()).toEqual([]);
  });

  it('lässt sich in den Einstellungen wiederholen, ohne die Sprache zu ändern', async () => {
    systemLanguage('en-US');
    const backend = await start({ onboarded: true });
    await fireEvent.click(await screen.findByTestId('nav-settings'));
    await fireEvent.click(await screen.findByTestId('about-repeat'));
    await screen.findByTestId('wizard');
    expect((await backend.getSettings()).locale).toBe('de');
  });
});
