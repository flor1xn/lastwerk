// Garmin in der Oberfläche (T23, SPEC 5.6): gemeinsamer Zustand für Erststart, Einstellungen
// und Seitenleiste. Anmeldung und Abgleich laufen im Rust-Teil; hier stehen nur die Abläufe
// der Oberfläche und die Zuordnung der Codes zu Texten. Kein Token, kein Name, kein Pfad.

import { get, writable } from 'svelte/store';
import { getBackend } from './backend';
import type { GarminStatus, GarminSyncProgress, GarminSyncSummary } from './backend/types';
import { errorKey } from './errors';
import { hasKey, type Params } from './i18n';
import { bumpData } from './state';

export type GarminSyncState =
  | { kind: 'idle' }
  | { kind: 'running'; progress: GarminSyncProgress | null }
  | { kind: 'done'; summary: GarminSyncSummary }
  | { kind: 'error'; key: string; summary: GarminSyncSummary | null };

export type GarminLoginState =
  { kind: 'idle' } | { kind: 'waiting' } | { kind: 'done'; ok: boolean; key: string };

/** Zustand des Abgleichs (Seitenleiste, Einstellungen, Erststart). */
export const garminSync = writable<GarminSyncState>({ kind: 'idle' });
/** Zustand der Anmeldung im Fenster von Garmin. */
export const garminLogin = writable<GarminLoginState>({ kind: 'idle' });
/** Zuletzt gelesener Zustand der Verbindung, `null` vor dem ersten Lesen. */
export const garminStatus = writable<GarminStatus | null>(null);
/** Schlüssel des Fehlertextes, wenn der Zustand nicht lesbar war (etwa ohne Schlüsselbund). */
export const garminStatusError = writable<string | null>(null);

/** Codes, die nicht von lastwerk-garmin kommen, sondern vom Anmeldefenster. */
const LOGIN_CODES = ['connected', 'cancelled', 'unexpected_redirect'];

/** Text zu einem Code aus Anmeldung oder Abbruch des Abgleichs. Unbekannte Codes zeigen den
 * allgemeinen Fehlertext, nie den Code selbst. */
export function garminCodeKey(code: string): string {
  if (LOGIN_CODES.includes(code)) return `ui.garmin.code.${code}`;
  const key = `ui.error.garmin_${code}`;
  return hasKey('de', key) ? key : 'ui.error.internal';
}

/** Text und Platzhalter zum Fortschritt eines Abgleichs. */
export function progressText(p: GarminSyncProgress): [string, Params] {
  if (p.phase === 'activities') {
    return p.total === null
      ? ['ui.garmin.sync.phase_activities', { done: p.done }]
      : ['ui.garmin.sync.phase_activities_total', { done: p.done, total: p.total }];
  }
  const total = p.total ?? p.done;
  return [`ui.garmin.sync.phase_${p.phase}`, { done: p.done, total }];
}

export async function refreshGarminStatus(): Promise<void> {
  try {
    garminStatus.set(await getBackend().garminStatus());
    garminStatusError.set(null);
  } catch (e) {
    garminStatusError.set(errorKey(e));
  }
}

/** Startet einen Abgleich. Läuft schon einer in dieser Oberfläche, passiert nichts. */
export async function runGarminSync(
  trigger: 'manual' | 'start' = 'manual',
): Promise<GarminSyncSummary | null> {
  const before = get(garminSync);
  if (before.kind === 'running') return null;
  garminSync.set({ kind: 'running', progress: null });
  try {
    const summary = await getBackend().garminSync(trigger, (progress) =>
      garminSync.set({ kind: 'running', progress }),
    );
    if (summary === null) {
      // Kein Abgleich (beim Start nicht verbunden, ausgeschaltet oder schon gelaufen): der
      // vorige Zustand bleibt stehen.
      garminSync.set(before);
      return null;
    }
    garminSync.set(
      summary.aborted
        ? { kind: 'error', key: garminCodeKey(summary.aborted.code), summary }
        : { kind: 'done', summary },
    );
    // Neue Aktivitäten und Erholungswerte: Ansichten laden neu.
    bumpData();
    return summary;
  } catch (e) {
    garminSync.set({ kind: 'error', key: errorKey(e), summary: null });
    return null;
  } finally {
    await refreshGarminStatus();
  }
}

/** Öffnet das Anmeldefenster. Nach Erfolg startet der erste Abgleich. */
export async function connectGarmin(): Promise<void> {
  if (get(garminLogin).kind === 'waiting') return;
  garminLogin.set({ kind: 'waiting' });
  try {
    const result = await getBackend().garminLogin();
    garminLogin.set({ kind: 'done', ok: result.ok, key: garminCodeKey(result.code) });
    await refreshGarminStatus();
    if (result.ok) void runGarminSync('manual');
  } catch (e) {
    garminLogin.set({ kind: 'done', ok: false, key: errorKey(e) });
  }
}

/** "Trennen": löscht die Anmeldedaten. Fehler wirft die Funktion weiter. */
export async function disconnectGarmin(): Promise<void> {
  await getBackend().garminLogout();
  garminLogin.set({ kind: 'idle' });
  garminSync.set({ kind: 'idle' });
  await refreshGarminStatus();
}

/** Einmal nach dem Laden der App: abgleichen, wenn verbunden und eingeschaltet. Der Rust-Teil
 * prüft beides noch einmal und lässt den Abgleich je Programmstart nur einmal zu. Fehler beim
 * Start bleiben still, der Zustand steht in den Einstellungen. */
export async function syncOnStart(): Promise<void> {
  try {
    const status = await getBackend().garminStatus();
    garminStatus.set(status);
    if (status.connected && status.sync_on_start) await runGarminSync('start');
  } catch {
    // Kein Schlüsselbund oder keine Datenbank: kein Abgleich beim Start.
  }
}

/** Für Tests: alles auf den Anfang. */
export function resetGarminState(): void {
  garminSync.set({ kind: 'idle' });
  garminLogin.set({ kind: 'idle' });
  garminStatus.set(null);
  garminStatusError.set(null);
}
