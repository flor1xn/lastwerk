// Auswahl des Backends: In Tauri gibt es `window.__TAURI_INTERNALS__`, dann spricht die
// Oberfläche mit dem Rust-Teil. Sonst (Browser, Vitest, Playwright) läuft die Attrappe mit
// synthetischen Daten.

import { MockBackend } from './mock';
import { isOllamaVariant } from './mock-coach-setup';
import { isDashboardVariant } from './mock-dashboard';
import { TauriBackend } from './tauri';
import type { Backend } from './types';

export function insideTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export function createBackend(): Backend {
  if (insideTauri()) return new TauriBackend();
  // `?erststart` startet die Attrappe als frische Installation (Rauchtest des Assistenten).
  const fresh = new URLSearchParams(window.location.search).has('erststart');
  // `?dashboard=empty|low|sparse|extreme|gaps` wählt erfundene Dashboard-Daten für die Sichtprüfung.
  const variant = new URLSearchParams(window.location.search).get('dashboard');
  // `?coach=off` ist ein Rechner ohne lokales Modell (Hinweis zur Einrichtung statt Gespräch).
  const coachOff = new URLSearchParams(window.location.search).get('coach') === 'off';
  // `?ollama=fehlt|laeuft|laedt` wählt den Zustand von Ollama in der Einrichtung (T29): nicht
  // installiert, läuft mit Modellen, läuft und lädt gerade das Startmodell.
  const ollamaParam = new URLSearchParams(window.location.search).get('ollama');
  const ollamaName: Record<string, string> = {
    fehlt: 'missing',
    laeuft: 'running',
    laedt: 'loading',
  };
  const ollama = ollamaName[ollamaParam ?? ''] ?? null;
  // `?uebernahme=fertig|fehlt` täuscht die einmalige Übernahme des früheren FIT-Ordners vor
  // (Meldung nach dem Start, T28).
  const migration = new URLSearchParams(window.location.search).get('uebernahme');
  // `?cache=alt` täuscht veraltete abgeleitete Werte vor: Die Oberfläche liest beim Start ein (T33).
  const cacheOld = new URLSearchParams(window.location.search).get('cache') === 'alt';
  // `?plan=laufend` legt einen aktiven Plan an, der vor drei Wochen begann (Kalender, T35).
  const runningPlan = new URLSearchParams(window.location.search).get('plan') === 'laufend';
  const mock = new MockBackend({
    importStepMs: 40,
    onboarded: !fresh,
    // Frische Installation: Nach dem ersten Abgleich schlägt die Historie die drei Fragen vor.
    history: fresh ? { sport: 'running', hours: 7.5, days: [1, 3, 5, 6], weeks: 12 } : null,
    historyAfterSync: fresh,
    dashboard: isDashboardVariant(variant) ? variant : undefined,
    coach: !coachOff,
    cacheOutdated: cacheOld,
    runningPlan,
    // Kurze Wartezeit, damit der Zustand "denkt nach" im Browser zu sehen ist.
    coachDelayMs: 700,
    ollama: isOllamaVariant(ollama) ? ollama : undefined,
    // Einrichtung und Laden in sichtbaren Schritten (rund 1,3 s für die Einrichtung, 6 s für ein
    // Modell, das der Nutzer wählt, 24 s für das Startmodell der Variante `laedt`).
    setupStepMs: 250,
    fitMigration:
      migration === 'fertig'
        ? { copied: 12, identical: 3, conflicts: 1, failed: 0, missing: false }
        : migration === 'fehlt'
          ? { copied: 0, identical: 0, conflicts: 0, failed: 0, missing: true }
          : undefined,
  });
  // Nur die Attrappe im Browser: Der Rauchtest liest die protokollierten Aufrufe der Exporte
  // aus (kein Dateidialog im Browser). Im Tauri-Fenster gibt es dieses Objekt nicht.
  (window as unknown as { __lastwerkMockCalls?: unknown }).__lastwerkMockCalls = mock.calls;
  return mock;
}

let current: Backend | null = null;

/** Das Backend der laufenden Oberfläche. Wird beim ersten Zugriff angelegt. */
export function getBackend(): Backend {
  current ??= createBackend();
  return current;
}

/** Für Tests: eigenes Backend einsetzen. */
export function setBackend(backend: Backend | null): void {
  current = backend;
}
