import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../../App.svelte';
import { setBackend } from '../../lib/backend';
import { MockBackend, type MockOptions } from '../../lib/backend/mock';
import { BackendError } from '../../lib/backend/types';
import { resetGarminState } from '../../lib/garmin';
import { setLocale } from '../../lib/i18n';
import { dataVersion, fitMigration, view } from '../../lib/state';
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
  fitMigration.set(null);
  resetGarminState();
});

function mock(options: MockOptions = {}) {
  const backend = new MockBackend({ today: '2026-10-14', ...options });
  setBackend(backend);
  return backend;
}

describe('Einstellungen, Abschnitt Daten (T28)', () => {
  it('zeigt nur, wo Lastwerk ablegt: Text und Pfad ohne Eingabefeld, ohne Ordnerwahl', async () => {
    mock();
    render(SettingsView);
    const section = await screen.findByTestId('data-section');
    expect(section.textContent).toContain('Du musst keinen Ordner wählen');
    expect(screen.getByTestId('fit-root').textContent).toBe('attrappe/Lastwerk/fit');
    expect(section.querySelector('input')).toBeNull();
    expect(screen.queryByText('Ordner wählen')).toBeNull();
    expect(screen.getByTestId('rescan').textContent).toContain('Neu einlesen');
    expect(screen.getByTestId('add-fit-files').textContent).toContain('FIT-Dateien hinzufügen');
  });

  it('liest auf Klick neu ein, zeigt das Ergebnis und meldet geänderte Daten', async () => {
    mock();
    render(SettingsView);
    const before = get(dataVersion);
    await fireEvent.click(await screen.findByTestId('rescan'));
    const summary = await screen.findByTestId('import-summary');
    expect(summary.textContent).toContain('Prüfsumme stimmt nicht');
    expect(get(dataVersion)).toBe(before + 1);
    expect(screen.queryByTestId('add-result')).toBeNull();
  });

  it('fügt Dateien hinzu und meldet nur Zahlen; dieselbe Auswahl ist danach schon vorhanden', async () => {
    const backend = mock();
    render(SettingsView);
    await fireEvent.click(await screen.findByTestId('add-fit-files'));
    const result = await screen.findByTestId('add-result');
    expect(result.textContent).toContain('2 gewählt: 2 neu abgelegt, 0 schon vorhanden');
    expect(screen.getByTestId('import-summary')).toBeTruthy();
    expect(result.textContent).not.toContain('.fit');
    await fireEvent.click(screen.getByTestId('add-fit-files'));
    await waitFor(() =>
      expect(screen.getByTestId('add-result').textContent).toContain(
        '2 gewählt: 0 neu abgelegt, 2 schon vorhanden',
      ),
    );
    expect(backend.calls.filter((c) => c.command === 'add_fit_files')).toHaveLength(2);
  });

  it('weist Abgelehntes aus und nennt, was nicht übernommen wird', async () => {
    const backend = mock();
    backend.pickedFiles = 3;
    render(SettingsView);
    await fireEvent.click(await screen.findByTestId('add-fit-files'));
    const result = await screen.findByTestId('add-result');
    expect(result.textContent).toContain('1 nicht übernommen');
    expect(result.textContent).toContain('über 64 MiB');
  });

  it('meldet nichts, wenn der Dateidialog abgebrochen wird', async () => {
    const backend = mock();
    backend.pickedFiles = 0;
    render(SettingsView);
    await fireEvent.click(await screen.findByTestId('add-fit-files'));
    await waitFor(() =>
      expect(backend.calls.some((c) => c.command === 'add_fit_files')).toBe(true),
    );
    expect(screen.queryByTestId('add-result')).toBeNull();
    expect(screen.queryByTestId('import-summary')).toBeNull();
    expect(screen.queryByTestId('import-error')).toBeNull();
    expect((screen.getByTestId('add-fit-files') as HTMLButtonElement).disabled).toBe(false);
  });

  it('zeigt einen Fehler als Text aus der Sprachdatei', async () => {
    const backend = mock();
    vi.spyOn(backend, 'addFitFiles').mockRejectedValue(
      new BackendError('fit_root_io', 'ui.error.fit_root_io'),
    );
    render(SettingsView);
    await fireEvent.click(await screen.findByTestId('add-fit-files'));
    const error = await screen.findByTestId('import-error');
    expect(error.textContent).toContain('Datenordner');
  });
});

describe('Meldung zur Übernahme des früheren Ordners', () => {
  it('erscheint einmal mit Zahlen, nie mit einem Pfad, und lässt sich ausblenden', async () => {
    mock({ fitMigration: { copied: 12, identical: 3, conflicts: 1, failed: 0, missing: false } });
    render(App);
    const notice = await screen.findByTestId('migration-notice');
    expect(notice.textContent).toContain('12 neu, 3 waren schon da');
    expect(notice.textContent).toContain('Unverändert geblieben');
    expect(notice.textContent).not.toContain('Nicht kopiert');
    expect(notice.textContent).not.toContain('/');
    await fireEvent.click(screen.getByTestId('migration-dismiss'));
    expect(screen.queryByTestId('migration-notice')).toBeNull();
  });

  it('nennt einen fehlenden alten Ordner und verweist auf "FIT-Dateien hinzufügen"', async () => {
    mock({ fitMigration: { copied: 0, identical: 0, conflicts: 0, failed: 0, missing: true } });
    render(App);
    const notice = await screen.findByTestId('migration-notice');
    expect(notice.textContent).toContain('nicht gefunden');
    expect(notice.textContent).toContain('FIT-Dateien hinzufügen');
  });

  it('weist fehlgeschlagene Kopien aus', async () => {
    mock({ fitMigration: { copied: 4, identical: 0, conflicts: 0, failed: 2, missing: false } });
    render(App);
    expect((await screen.findByTestId('migration-notice')).textContent).toContain(
      'Nicht kopiert: 2',
    );
  });

  it('fehlt, wenn es nichts zu melden gibt', async () => {
    mock();
    render(App);
    await screen.findByTestId('nav-today');
    expect(screen.queryByTestId('migration-notice')).toBeNull();
  });
});
