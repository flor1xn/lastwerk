import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setBackend } from '../../lib/backend';
import { MockBackend, type MockOptions } from '../../lib/backend/mock';
import { resetCoachSetupState } from '../../lib/coach-setup';
import { setLocale } from '../../lib/i18n';
import { get } from 'svelte/store';
import { settingsTab, view } from '../../lib/state';
import PlanView from '../PlanView.svelte';
import SettingsView from '../SettingsView.svelte';
import CoachSetup from './CoachSetup.svelte';

vi.mock(
  '../../lib/components/UplotChart.svelte',
  async () => import('../../lib/components/UplotChartStub.svelte'),
);

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
  view.set('today');
  settingsTab.set('data');
  resetCoachSetupState();
});

function mock(options: MockOptions = {}) {
  const backend = new MockBackend({ today: '2026-10-14', setupStepMs: 3, ...options });
  setBackend(backend);
  return backend;
}

const text = (id: string) => screen.getByTestId(id).textContent ?? '';

describe('Einrichtung des Coachs, Baustein', () => {
  it('Ollama fehlt: Zustand, Knopf "Jetzt einrichten" und die Liste mit Empfehlung', async () => {
    mock({ ollama: 'missing' });
    render(CoachSetup);
    expect((await screen.findByTestId('coach-setup-state')).textContent).toContain(
      'Ollama ist nicht installiert',
    );
    expect(text('coach-setup-install-button')).toContain('Jetzt einrichten');
    expect(text('coach-setup-hardware')).toContain('16 GB Arbeitsspeicher');
    // 16 GB ohne Grafikkarte: 4b ist empfohlen, 8b langsam, 14b zu groß (aber wählbar).
    expect(screen.getByTestId('coach-model-qwen3:4b').getAttribute('data-recommended')).toBe(
      'true',
    );
    expect(text('coach-model-qwen3:4b')).toContain('Empfohlen für deinen PC');
    expect(text('coach-model-qwen3:8b')).toContain('langsam auf diesem PC');
    expect(text('coach-model-qwen3:14b')).toContain('zu groß für diesen PC');
    expect(text('coach-model-qwen3:1.7b')).not.toContain('Empfohlen');
    // Größen stehen in der Zeile (Komma auf Deutsch).
    expect(text('coach-model-qwen3:4b')).toContain('2,5 GB');
    expect(text('coach-model-qwen3:14b')).toContain('9,3 GB');
    // Solange Ollama nicht läuft, lässt sich nichts laden.
    for (const name of ['qwen3:1.7b', 'qwen3:4b', 'qwen3:8b', 'qwen3:14b']) {
      expect((screen.getByTestId(`coach-load-${name}`) as HTMLButtonElement).disabled).toBe(true);
    }
  });

  it('richtet Ollama ein, zeigt den Fortschritt und schaltet dann das Laden frei', async () => {
    mock({ ollama: 'missing' });
    render(CoachSetup);
    await fireEvent.click(await screen.findByTestId('coach-setup-install-button'));
    await screen.findByTestId('coach-setup-installing');
    await waitFor(() =>
      expect(screen.getByTestId('coach-setup-state').textContent).toContain('läuft'),
    );
    expect(screen.queryByTestId('coach-setup-install')).toBeNull();
    expect(text('coach-setup-model')).toContain('Noch kein Modell geladen');
    expect((screen.getByTestId('coach-load-qwen3:4b') as HTMLButtonElement).disabled).toBe(false);
    // Das empfohlene Modell ist der Hauptknopf, solange kein Modell da ist.
    expect(screen.getByTestId('coach-load-qwen3:4b').className).toContain('primary');
    expect(screen.getByTestId('coach-load-qwen3:8b').className).not.toContain('primary');
  });

  it('lädt ein Modell mit Balken, Zahlen und Hinweis auf den Hintergrund', async () => {
    mock({ ollama: 'running', setupStepMs: 15 });
    render(CoachSetup);
    await fireEvent.click(await screen.findByTestId('coach-load-qwen3:8b'));
    const progress = await screen.findByTestId('coach-pull-progress');
    expect(progress.textContent).toContain('Du kannst weitermachen');
    const bar = progress.querySelector('[role="progressbar"]');
    expect(bar).toBeTruthy();
    await waitFor(() => {
      const now = Number(
        progress.querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow'),
      );
      expect(now).toBeGreaterThan(0);
    });
    // Während des Ladens sind die anderen Knöpfe gesperrt, "Abbrechen" ist da.
    expect((screen.getByTestId('coach-load-qwen3:14b') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('coach-pull-cancel')).toBeTruthy();
    expect(screen.getByTestId('coach-pull-numbers').textContent).toMatch(/\d+,\d von 5,2 GB/);
    await waitFor(() => expect(screen.queryByTestId('coach-pull-progress')).toBeNull(), {
      timeout: 5000,
    });
    expect(text('coach-setup-pulled')).toContain('qwen3:8b ist geladen');
    expect(text('coach-model-qwen3:8b')).toContain('im Einsatz');
  });

  it('Abbrechen zeigt "Abgebrochen" und lädt nichts', async () => {
    mock({ ollama: 'running', setupStepMs: 15 });
    render(CoachSetup);
    await fireEvent.click(await screen.findByTestId('coach-load-qwen3:8b'));
    await fireEvent.click(await screen.findByTestId('coach-pull-cancel'));
    await waitFor(() => expect(text('coach-setup-error')).toContain('Abgebrochen'));
    expect(screen.getByTestId('coach-setup-error').className).not.toContain('error');
    expect(screen.queryByTestId('coach-pull-progress')).toBeNull();
    expect(text('coach-model-qwen3:8b')).not.toContain('geladen');
  });

  it('ein vorhandenes Modell lässt sich wählen', async () => {
    const backend = mock({ ollama: 'running' });
    render(CoachSetup);
    await screen.findByTestId('coach-setup-ready');
    expect(text('coach-setup-model')).toContain('qwen3:4b');
    expect(text('coach-model-qwen3:4b')).toContain('im Einsatz');
    // Kein Knopf für das Modell im Einsatz.
    expect(screen.queryByTestId('coach-use-qwen3:4b')).toBeNull();
    expect(screen.queryByTestId('coach-load-qwen3:4b')).toBeNull();
    // Ein geladenes Modell außerhalb des Katalogs steht nicht in der Liste, ein Katalogmodell
    // wird nach dem Laden wählbar.
    await fireEvent.click(screen.getByTestId('coach-load-qwen3:1.7b'));
    await waitFor(() => expect(text('coach-model-qwen3:1.7b')).toContain('im Einsatz'));
    await fireEvent.click(screen.getByTestId('coach-use-qwen3:4b'));
    await waitFor(() => expect(text('coach-model-qwen3:4b')).toContain('im Einsatz'));
    expect((await backend.coachStatus()).model).toBe('qwen3:4b');
  });

  it('ohne Skript (kein Windows) zeigt die Anleitung statt des Knopfs', async () => {
    mock({ ollama: 'missing', setupCanInstall: false });
    render(CoachSetup);
    await screen.findByTestId('coach-setup-manual');
    expect(screen.queryByTestId('coach-setup-install-button')).toBeNull();
    expect(text('coach-setup-manual')).toContain('ollama.com');
    expect(screen.getByTestId('coach-setup-recheck')).toBeTruthy();
  });

  it('Variante "laedt" zeigt den laufenden Download, ohne dass der Nutzer ihn startete', async () => {
    mock({ ollama: 'loading', setupStepMs: 200 });
    render(CoachSetup);
    const progress = await screen.findByTestId('coach-pull-progress');
    expect(progress).toBeTruthy();
    expect(screen.getByTestId('coach-model-qwen3:4b').textContent).toContain('qwen3:4b');
    expect(screen.getByTestId('coach-pull-cancel')).toBeTruthy();
  });

  it('Empfehlung folgt der Hardware', async () => {
    mock({ ollama: 'running', setupHardware: { ram_gb: 32, vram_gb: 12 } });
    render(CoachSetup);
    await screen.findByTestId('coach-model-qwen3:14b');
    expect(text('coach-setup-hardware')).toContain('32 GB Arbeitsspeicher, 12 GB Grafikspeicher');
    expect(screen.getByTestId('coach-model-qwen3:14b').getAttribute('data-recommended')).toBe(
      'true',
    );
    expect(screen.getByTestId('coach-model-qwen3:4b').getAttribute('data-recommended')).toBe(
      'false',
    );
  });

  it('ein Rechner, auf dem nichts passt, empfiehlt nichts als "passend"', async () => {
    mock({ ollama: 'running', setupHardware: { ram_gb: 3, vram_gb: null } });
    render(CoachSetup);
    await screen.findByTestId('coach-model-qwen3:4b');
    expect(screen.queryByTestId('badge-recommended')).toBeNull();
    expect(text('coach-model-qwen3:4b')).toContain('zu groß für diesen PC');
    // Wählbar bleibt es trotzdem.
    expect((screen.getByTestId('coach-load-qwen3:8b') as HTMLButtonElement).disabled).toBe(false);
  });

  it('Texte erscheinen auch auf Englisch', async () => {
    setLocale('en');
    mock({ ollama: 'missing' });
    render(CoachSetup);
    await screen.findByTestId('coach-setup-install-button');
    expect(text('coach-setup-install-button')).toContain('Set up now');
    expect(text('coach-model-qwen3:4b')).toContain('Recommended for your PC');
    expect(text('coach-model-qwen3:8b')).toContain('slow on this PC');
    expect(text('coach-model-qwen3:14b')).toContain('too big for this PC');
    expect(text('coach-model-qwen3:4b')).toContain('2.5 GB');
  });

  it('der Baustein hat weder Eingabefeld noch Pfad noch Link ins Netz', async () => {
    mock({ ollama: 'missing' });
    render(CoachSetup);
    await screen.findByTestId('coach-setup-install-button');
    const root = screen.getByTestId('coach-setup-block');
    expect(root.querySelector('input')).toBeNull();
    expect(root.querySelector('a')).toBeNull();
  });
});

describe('Einrichtung des Coachs, Einstellungen und Plan', () => {
  it('die Einstellungen zeigen den Abschnitt "Lokaler Coach" im Reiter Coach', async () => {
    mock({ ollama: 'running' });
    render(SettingsView);
    expect(screen.queryByTestId('coach-setup-section')).toBeNull();
    await fireEvent.click(await screen.findByTestId('tab-settings-coach'));
    const section = await screen.findByTestId('coach-setup-section');
    expect(section.querySelector('h3')?.textContent).toBe('Lokaler Coach');
    await screen.findByTestId('coach-setup-ready');
  });

  it('der Hinweis ohne Modell in der Planansicht führt zum Reiter Coach', async () => {
    mock({ ollama: 'missing' });
    view.set('plan');
    render(PlanView);
    const link = await screen.findByTestId('coach-open-setup');
    expect(link.textContent).toContain('Zur Einrichtung in den Einstellungen');
    await fireEvent.click(link);
    expect(get(view)).toBe('settings');
    expect(get(settingsTab)).toBe('coach');
  });
});
