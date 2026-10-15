// Änderungen je Version aus catalog/changelog.json. Dieselbe Datei erzeugt die Release-Notizen
// (scripts/release-notes.mjs), damit App und Release nie auseinanderlaufen. Reiner Text, kein
// HTML (SPEC 9.5).

import changelog from '../../../../catalog/changelog.json';
import type { Locale } from './backend/types';

type Catalog = Record<string, Record<string, string[]>>;

/** Zeilen "Neu in dieser Version" für die Version und Sprache, leer ohne Eintrag. */
export function changesFor(version: string, locale: Locale): string[] {
  const lines = (changelog as Catalog)[locale]?.[version];
  return Array.isArray(lines) ? lines : [];
}
