// Minimale Mehrsprachigkeit ohne Bibliothek (Entscheidung O12, SPEC 14).
//
// Es gibt zwei Sprachen und Schlüssel mit Punkten. Die Kataloge catalog/locales/de.json und
// en.json werden zur Build-Zeit eingebunden, nichts wird nachgeladen. Ein Schlüssel, der in
// der Sprache fehlt, erscheint als Schlüssel selbst (sichtbar, nie leer). Dass beide Dateien
// dieselben Schlüssel haben, prüft scripts/check-i18n.sh, dass der Code keinen unbekannten
// Schlüssel nutzt, prüft i18n.test.ts.

import { derived, get, writable, type Readable } from 'svelte/store';
import de from '../../../../catalog/locales/de.json';
import en from '../../../../catalog/locales/en.json';
import type { Locale } from './backend/types';

export type Params = Record<string, string | number>;
type Catalog = Record<string, string>;

/** Macht aus verschachteltem JSON eine flache Abbildung `a.b.c` zu Text. */
export function flatten(obj: unknown, prefix = '', out: Catalog = {}): Catalog {
  if (obj && typeof obj === 'object') {
    for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
      const key = prefix ? `${prefix}.${k}` : k;
      if (typeof v === 'string') out[key] = v;
      else flatten(v, key, out);
    }
  }
  return out;
}

export const catalogs: Record<Locale, Catalog> = { de: flatten(de), en: flatten(en) };

export const LOCALES: readonly Locale[] = ['de', 'en'];

/** Ersetzt `{name}` durch den Parameter. Unbekannte Platzhalter bleiben stehen. */
export function interpolate(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  );
}

export function translate(locale: Locale, key: string, params?: Params): string {
  const text = catalogs[locale][key];
  return text === undefined ? key : interpolate(text, params);
}

export function hasKey(locale: Locale, key: string): boolean {
  return key in catalogs[locale];
}

export const locale = writable<Locale>('de');

export type Translate = (key: string, params?: Params) => string;

/** In Komponenten: `$t('ui.nav.week')`. */
export const t: Readable<Translate> = derived(
  locale,
  ($locale): Translate =>
    (key, params) =>
      translate($locale, key, params),
);

/** Sprache setzen. Unbekannte Werte werden ignoriert. */
export function setLocale(next: string): void {
  if (next === 'de' || next === 'en') locale.set(next);
}

export function currentLocale(): Locale {
  return get(locale);
}

/** BCP-47-Kennung für Intl (Dezimalkomma in de, Punkt in en). */
export function intlTag(l: Locale): string {
  return l === 'de' ? 'de-DE' : 'en-US';
}
