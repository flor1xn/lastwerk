import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setBackend } from '../../lib/backend';
import { MockBackend } from '../../lib/backend/mock';
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

describe('Update auf Knopfdruck', () => {
  it('fragt nicht von selbst, sondern erst nach dem Klick', async () => {
    render(SettingsView);
    await screen.findByTestId('update-check');
    expect(screen.queryByTestId('update-none')).toBeNull();
    expect(screen.queryByTestId('update-available')).toBeNull();
  });

  it('meldet, dass die App aktuell ist', async () => {
    render(SettingsView);
    await fireEvent.click(await screen.findByTestId('update-check'));
    expect((await screen.findByTestId('update-none')).textContent).toContain('aktuell');
  });

  it('installiert eine neue Version erst nach der Bestätigung', async () => {
    backend.availableUpdate = '0.2.0';
    render(SettingsView);
    await fireEvent.click(await screen.findByTestId('update-check'));
    expect((await screen.findByTestId('update-available')).textContent).toContain('0.2.0');
    await fireEvent.click(screen.getByTestId('update-install'));
    expect(backend.availableUpdate).toBe('0.2.0');
    await fireEvent.click(await screen.findByTestId('update-yes'));
    await screen.findByTestId('update-check');
    expect(backend.availableUpdate).toBeNull();
  });

  it('zeigt Fehler als Text aus den Sprachdateien', async () => {
    backend.availableUpdate = null;
    await expect(backend.installUpdate(true)).rejects.toMatchObject({ code: 'no_update' });
    await expect(backend.installUpdate(false)).rejects.toMatchObject({ code: 'unconfirmed' });
  });
});
