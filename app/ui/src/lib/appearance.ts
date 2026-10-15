// Sprache und Thema wechseln und im Backend merken. Die Oberfläche wirkt sofort, ein Fehler
// beim Speichern ändert daran nichts (die Wahl gilt dann nur bis zum Neustart).

import { getBackend } from './backend';
import type { Locale, Theme } from './backend/types';
import { setLocale } from './i18n';
import { applyTheme } from './state';

export async function setLanguage(next: Locale): Promise<void> {
  setLocale(next);
  if (typeof document !== 'undefined') document.documentElement.lang = next;
  try {
    await getBackend().setSettings({ locale: next });
  } catch {
    // Nur nicht gemerkt, die Anzeige bleibt in der gewählten Sprache.
  }
}

export async function setThemeChoice(next: Theme): Promise<void> {
  applyTheme(next);
  try {
    await getBackend().setSettings({ theme: next });
  } catch {
    // Nur nicht gemerkt.
  }
}
