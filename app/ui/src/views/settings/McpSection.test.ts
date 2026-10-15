import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setBackend } from '../../lib/backend';
import { MockBackend } from '../../lib/backend/mock';
import type { SettingsPatch } from '../../lib/backend/types';
import { setLocale } from '../../lib/i18n';
import SettingsView from '../SettingsView.svelte';

let backend: MockBackend;

beforeEach(() => {
  backend = new MockBackend({ today: '2026-10-14' });
  setBackend(backend);
});

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
});

async function toggle(): Promise<HTMLInputElement> {
  return (await screen.findByTestId('mcp-recovery-toggle')) as HTMLInputElement;
}

describe('Freigabe der Erholungswerte für den MCP-Server', () => {
  it('ist standardmäßig aus und sagt, dass nichts geliefert wird', async () => {
    render(SettingsView);
    expect((await toggle()).checked).toBe(false);
    expect(screen.getByTestId('mcp-recovery-state').textContent).toContain('Gesperrt');
    expect((await backend.getSettings()).mcp_recovery_enabled).toBe(false);
  });

  it('nennt vor dem Einschalten die Folge für HRV, Ruhepuls und Schlafdauer', async () => {
    render(SettingsView);
    await toggle();
    const section = screen.getByTestId('mcp-section');
    expect(section.textContent).toContain('HRV, Ruhepuls, Schlafdauer');
    expect(section.textContent).toContain('nicht zurückholbar');
  });

  it('schaltet erst nach der Bestätigung ein', async () => {
    render(SettingsView);
    const box = await toggle();
    await fireEvent.click(box);
    expect(await screen.findByTestId('mcp-recovery-confirm')).toBeTruthy();
    // Noch nichts gespeichert, der Schalter zeigt den gespeicherten Zustand.
    expect((await backend.getSettings()).mcp_recovery_enabled).toBe(false);
    expect(box.checked).toBe(false);

    await fireEvent.click(screen.getByTestId('mcp-recovery-yes'));
    await screen.findByText(/Freigegeben: get_recovery_days/);
    expect((await backend.getSettings()).mcp_recovery_enabled).toBe(true);
    expect((await toggle()).checked).toBe(true);
    expect(screen.queryByTestId('mcp-recovery-confirm')).toBeNull();
  });

  it('bricht beim Abbrechen ab und ändert nichts', async () => {
    render(SettingsView);
    await fireEvent.click(await toggle());
    await fireEvent.click(await screen.findByTestId('mcp-recovery-no'));
    expect(screen.queryByTestId('mcp-recovery-confirm')).toBeNull();
    expect((await backend.getSettings()).mcp_recovery_enabled).toBe(false);
    expect((await toggle()).checked).toBe(false);
  });

  it('sperrt beim Ausschalten sofort und ohne Rückfrage', async () => {
    await backend.setSettings({ mcp_recovery_enabled: true });
    render(SettingsView);
    const box = await toggle();
    expect(box.checked).toBe(true);
    await fireEvent.click(box);
    await screen.findByText(/Gesperrt: get_recovery_days/);
    expect(screen.queryByTestId('mcp-recovery-confirm')).toBeNull();
    expect((await backend.getSettings()).mcp_recovery_enabled).toBe(false);
  });

  it('hat Texte in beiden Sprachen', async () => {
    setLocale('en');
    render(SettingsView);
    await toggle();
    expect(screen.getByTestId('mcp-section').textContent).toContain(
      'Allow recovery values for the MCP server',
    );
    expect(screen.getByTestId('mcp-recovery-state').textContent).toContain('Blocked');
  });

  it('die Attrappe nimmt nur Wahrheitswerte an, wie der Rust-Teil', async () => {
    for (const bad of ['true', 1, 0, 'TRUE']) {
      await expect(
        backend.setSettings({ mcp_recovery_enabled: bad } as unknown as SettingsPatch),
      ).rejects.toMatchObject({ code: 'invalid_setting' });
    }
    expect((await backend.getSettings()).mcp_recovery_enabled).toBe(false);
  });
});
