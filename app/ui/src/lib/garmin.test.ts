import { get } from 'svelte/store';
import { afterEach, describe, expect, it } from 'vitest';
import { setBackend } from './backend';
import { MockBackend } from './backend/mock';
import {
  garminCodeKey,
  garminSync,
  progressText,
  resetGarminState,
  runGarminSync,
  syncOnStart,
} from './garmin';
import { hasKey, translate } from './i18n';

afterEach(() => {
  setBackend(null);
  resetGarminState();
});

/** Codes von lastwerk-garmin (T21) und die des Anmeldefensters (T23). */
const CODES = [
  'not_connected',
  'auth_failed',
  'throttled',
  'locked',
  'not_available',
  'network',
  'invalid_response',
  'store_failed',
  'invalid_input',
  'connected',
  'cancelled',
  'unexpected_redirect',
];

describe('Texte zu Garmin', () => {
  it('jeder Code hat einen Text in beiden Sprachen', () => {
    for (const code of CODES) {
      const key = garminCodeKey(code);
      expect(key, code).not.toBe('ui.error.internal');
      expect(hasKey('de', key) && hasKey('en', key), key).toBe(true);
    }
  });

  it('ein unbekannter Code zeigt nie sich selbst', () => {
    expect(garminCodeKey('etwas_neues')).toBe('ui.error.internal');
    expect(garminCodeKey('../x')).toBe('ui.error.internal');
  });

  it('Fortschritt je Phase', () => {
    const text = (p: Parameters<typeof progressText>[0]) => translate('de', ...progressText(p));
    expect(text({ phase: 'activities', done: 4, total: null })).toBe('Aktivitäten 4');
    expect(text({ phase: 'activities', done: 1, total: 3 })).toBe('Aktivitäten 1 von 3');
    expect(text({ phase: 'recovery', done: 2, total: 5 })).toBe('Erholung 2 von 5');
    expect(text({ phase: 'import', done: 7, total: 9 })).toBe('Import 7 von 9');
  });
});

describe('Abläufe', () => {
  it('ein zweiter Abgleich während des ersten startet nicht', async () => {
    const backend = new MockBackend({ today: '2026-10-14', garminConnected: true });
    setBackend(backend);
    const first = runGarminSync('manual');
    expect(get(garminSync).kind).toBe('running');
    expect(await runGarminSync('manual')).toBeNull();
    expect((await first)?.new_activities).toBe(3);
    expect(backend.garmin.syncs).toBe(1);
  });

  it('die Attrappe meldet busy wie der Rust-Teil', async () => {
    const backend = new MockBackend({
      today: '2026-10-14',
      garminConnected: true,
      importStepMs: 1,
    });
    const first = backend.garminSync('manual', () => {});
    await expect(backend.garminSync('manual', () => {})).rejects.toMatchObject({ code: 'busy' });
    await first;
  });

  it('der Start gleicht höchstens einmal ab', async () => {
    const backend = new MockBackend({ today: '2026-10-14', garminConnected: true });
    setBackend(backend);
    await syncOnStart();
    await syncOnStart();
    expect(backend.garmin.syncs).toBe(1);
    expect(get(garminSync).kind).toBe('done');
  });

  it('ohne Verbindung ist "Jetzt abgleichen" ein Fehler mit Text', async () => {
    setBackend(new MockBackend({ today: '2026-10-14' }));
    await runGarminSync('manual');
    const state = get(garminSync);
    expect(state.kind).toBe('error');
    if (state.kind === 'error') expect(state.key).toBe('ui.error.garmin_not_connected');
  });
});
