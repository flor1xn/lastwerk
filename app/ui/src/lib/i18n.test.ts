import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { get } from 'svelte/store';
import { afterEach, describe, expect, it } from 'vitest';
import { LOAD_METHODS, SPORTS, THRESHOLD_KINDS } from './backend/types';
import { catalogs, hasKey, interpolate, locale, setLocale, t, translate } from './i18n';
import { BODY_TABS, CALENDAR_MODES, HISTORY_TABS, SETTINGS_TABS, VIEWS } from './state';

afterEach(() => setLocale('de'));

describe('translate', () => {
  it('liefert den Text der Sprache', () => {
    expect(translate('de', 'common.not_computable')).toBe('nicht berechenbar');
    expect(translate('en', 'common.not_computable')).toBe('not computable');
  });

  it('ersetzt Platzhalter und lässt unbekannte stehen', () => {
    expect(interpolate('{a} von {b}', { a: 3, b: 'x' })).toBe('3 von x');
    expect(interpolate('{a} und {c}', { a: 1 })).toBe('1 und {c}');
    expect(translate('de', 'ui.import.progress', { done: 2, total: 9 })).toBe('Datei 2 von 9');
  });

  it('zeigt bei fehlendem Schlüssel den Schlüssel selbst', () => {
    expect(translate('de', 'gibt.es.nicht')).toBe('gibt.es.nicht');
    expect(hasKey('de', 'gibt.es.nicht')).toBe(false);
  });
});

describe('Store t', () => {
  it('folgt der gewählten Sprache', () => {
    expect(get(t)('ui.nav.history')).toBe('Verlauf');
    setLocale('en');
    expect(get(locale)).toBe('en');
    expect(get(t)('ui.nav.history')).toBe('History');
  });

  it('ignoriert unbekannte Sprachen', () => {
    setLocale('fr');
    expect(get(locale)).toBe('de');
  });
});

describe('Kataloge', () => {
  it('haben dieselben Schlüssel in beiden Sprachen', () => {
    expect(Object.keys(catalogs.de).sort()).toEqual(Object.keys(catalogs.en).sort());
  });

  it('haben keine leeren Texte und dieselben Platzhalter in beiden Sprachen', () => {
    const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    for (const [key, de] of Object.entries(catalogs.de)) {
      const en = catalogs.en[key];
      expect(de.trim(), key).not.toBe('');
      expect(en.trim(), key).not.toBe('');
      expect(placeholders(en), key).toEqual(placeholders(de));
    }
  });

  it('enthalten keine Gedankenstriche', () => {
    for (const lang of ['de', 'en'] as const) {
      for (const [key, text] of Object.entries(catalogs[lang])) {
        expect(text, `${lang}:${key}`).not.toMatch(/[–—]/);
      }
    }
  });

  it('decken die Aufzählungen ab, aus denen Schlüssel zusammengesetzt werden', () => {
    const keys = [
      ...SPORTS.map((s) => `ui.sport.${s}`),
      ...LOAD_METHODS.map((m) => `ui.load.method.${m}`),
      ...THRESHOLD_KINDS.flatMap((k) => [`ui.threshold.kind.${k}`, `ui.threshold.unit.${k}`]),
      ...['missing_threshold', 'missing_channel', 'implausible', 'unknown'].map(
        (s) => `ui.load.status.${s}`,
      ),
      ...['file_missing', 'file_changed', 'unreadable', 'no_channels'].map(
        (s) => `ui.detail.series_${s}`,
      ),
      ...['dark', 'light', 'system'].map((x) => `ui.theme.${x}`),
      ...['de', 'en'].map((x) => `ui.language.${x}`),
      ...[...VIEWS, 'settings'].map((v) => `ui.nav.${v}`),
      ...HISTORY_TABS.map((v) => `ui.history.tab.${v}`),
      ...CALENDAR_MODES.map((v) => `ui.history.mode.${v}`),
      ...BODY_TABS.map((v) => `ui.history.body.${v}`),
      ...SETTINGS_TABS.flatMap((v) => [`ui.settings.tab.${v}`, `ui.lead.settings_${v}`]),
      ...['power7', 'hr_lthr5', 'hr_karvonen', 'run_speed5', 'swim_css5'].flatMap((id) => [
        `zones.${id}.name`,
        ...[1, 2, 3, 4, 5].map((n) => `zones.${id}.z${n}`),
      ]),
    ];
    for (const key of keys) {
      expect(hasKey('de', key), `de:${key}`).toBe(true);
      expect(hasKey('en', key), `en:${key}`).toBe(true);
    }
  });
});

/** Alle Quelltextdateien unter src (ohne Tests). */
function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return /\.(svelte|ts)$/.test(name) && !name.endsWith('.test.ts') ? [path] : [];
  });
}

describe('Schlüssel im Code', () => {
  it('nutzt nur Schlüssel, die in beiden Katalogen stehen (SPEC 7.5)', () => {
    const root = join(__dirname, '..');
    const used = new Set<string>();
    for (const file of sources(root)) {
      const text = readFileSync(file, 'utf8');
      // Feste Schlüssel: t('a.b') und $t('a.b'). Zusammengesetzte (mit ${...}) prüft der
      // Test der Aufzählungen oben.
      for (const m of text.matchAll(/\bt\(\s*'([a-z0-9_.]+)'/g)) used.add(m[1]);
    }
    expect(used.size).toBeGreaterThan(40);
    const missing = [...used].filter((k) => !hasKey('de', k) || !hasKey('en', k));
    expect(missing).toEqual([]);
  });
});
