import { cleanup, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setBackend } from '../../lib/backend';
import { MockBackend } from '../../lib/backend/mock';
import { changesFor } from '../../lib/changelog';
import { setLocale } from '../../lib/i18n';
import changelog from '../../../../../catalog/changelog.json';
import { settingsTab } from '../../lib/state';
import SettingsView from '../SettingsView.svelte';

beforeEach(() => setBackend(new MockBackend({ today: '2026-10-14' })));
afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
  settingsTab.set('data');
});

describe('Version und Änderungen', () => {
  it('führt de und en mit denselben Versionen und gleich vielen Zeilen', () => {
    const de = changelog.de as Record<string, string[]>;
    const en = changelog.en as Record<string, string[]>;
    expect(Object.keys(en).sort()).toEqual(Object.keys(de).sort());
    for (const v of Object.keys(de)) expect(en[v].length).toBe(de[v].length);
  });

  it('liefert für eine unbekannte Version keine Zeilen', () => {
    expect(changesFor('9.9.9', 'de')).toEqual([]);
  });

  it('zeigt die Version und, falls vorhanden, die Änderungen', async () => {
    settingsTab.set('app');
    render(SettingsView);
    const version = await screen.findByTestId('about-version');
    expect(version.textContent).toContain('0.0.0');
    expect(screen.getByTestId('about-none')).toBeTruthy();
  });
});
