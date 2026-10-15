// Einrichtung des lokalen Coachs in der Oberfläche (T29, SPEC 9.2): gemeinsamer Zustand für
// Erststart und Einstellungen, damit ein Download weiterläuft, wenn der Nutzer den Schritt
// verlässt. Installation und Laden laufen im Rust-Teil; hier stehen nur die Abläufe der
// Oberfläche, die Zuordnung der Codes zu Texten und reine Hilfen mit Tests. Kein Pfad, kein
// Fremdtext, kein Netzwerkzugriff aus dem Frontend.

import { get, writable } from 'svelte/store';
import { getBackend } from './backend';
import type {
  CoachCatalogEntry,
  CoachPullProgress,
  CoachSetupProgress,
  CoachSetupStatus,
  Locale,
} from './backend/types';
import { errorKey } from './errors';
import { formatNumber } from './format';
import { hasKey, type Params } from './i18n';

export type SetupOp =
  | { kind: 'idle' }
  | { kind: 'install'; progress: CoachSetupProgress | null }
  | { kind: 'pull'; name: string; progress: CoachPullProgress | null };

/** Zuletzt gelesener Zustand, `null` vor dem ersten Lesen. */
export const coachSetupStatus = writable<CoachSetupStatus | null>(null);
/** Was gerade läuft. Bleibt beim Wechsel der Ansicht erhalten. */
export const coachSetupOp = writable<SetupOp>({ kind: 'idle' });
/** Schlüssel des Fehlertextes der letzten Aktion, `null` ohne Fehler. */
export const coachSetupError = writable<string | null>(null);
/** Name des zuletzt vollständig geladenen Modells (Meldung "geladen"), bis der Nutzer weitermacht. */
export const coachSetupPulled = writable<string | null>(null);
/** Der Nutzer kam vom Hinweis im Plan: Die Einstellungen springen zum Abschnitt "Coach". */
export const coachSetupFocus = writable(false);

/** Wie oft der Zustand gelesen wird, solange ein Auftrag läuft, den diese Oberfläche nicht selbst
 * gestartet hat (etwa nach dem Neuladen des Fensters), in Millisekunden (Konvention). */
const POLL_MS = 1000;

let pollTimer: ReturnType<typeof setTimeout> | null = null;
/** Die Oberfläche folgt einem Auftrag, den sie nicht selbst gestartet hat. */
let adopted = false;

function stopPolling(): void {
  if (pollTimer !== null) clearTimeout(pollTimer);
  pollTimer = null;
  adopted = false;
}

function schedulePoll(): void {
  if (pollTimer !== null) return;
  pollTimer = setTimeout(() => {
    pollTimer = null;
    void refreshCoachSetup();
  }, POLL_MS);
}

/** Liest den Zustand. Findet er einen laufenden Auftrag ohne eigenen Aufruf, zeigt die
 * Oberfläche ihn und liest weiter, bis er endet. */
export async function refreshCoachSetup(): Promise<void> {
  try {
    const status = await getBackend().coachSetupStatus();
    coachSetupStatus.set(status);
    const op = get(coachSetupOp);
    if (status.busy && op.kind === 'idle') {
      adopted = true;
      const b = status.busy;
      coachSetupOp.set(
        b.kind === 'install'
          ? { kind: 'install', progress: null }
          : {
              kind: 'pull',
              name: b.name ?? '',
              progress: {
                name: b.name ?? '',
                phase: b.phase ?? 'other',
                completed: b.completed,
                total: b.total,
              },
            },
      );
      schedulePoll();
    } else if (adopted) {
      if (status.busy) {
        if (op.kind === 'pull') {
          coachSetupOp.set({
            kind: 'pull',
            name: op.name,
            progress: {
              name: op.name,
              phase: status.busy.phase ?? 'other',
              completed: status.busy.completed,
              total: status.busy.total,
            },
          });
        }
        schedulePoll();
      } else {
        stopPolling();
        coachSetupOp.set({ kind: 'idle' });
      }
    }
  } catch (e) {
    coachSetupError.set(errorKey(e));
  }
}

/** Richtet Ollama über das Skript der App ein (nur Windows). */
export async function installOllama(): Promise<void> {
  if (get(coachSetupOp).kind !== 'idle') return;
  coachSetupError.set(null);
  coachSetupOp.set({ kind: 'install', progress: null });
  try {
    const status = await getBackend().coachInstallOllama((progress) =>
      coachSetupOp.set({ kind: 'install', progress }),
    );
    coachSetupStatus.set(status);
  } catch (e) {
    coachSetupError.set(errorKey(e));
    await refreshCoachSetup();
  } finally {
    coachSetupOp.set({ kind: 'idle' });
  }
}

/** Lädt ein Modell. Läuft schon ein Auftrag in dieser Oberfläche, passiert nichts. */
export async function pullModel(name: string): Promise<void> {
  if (get(coachSetupOp).kind !== 'idle') return;
  coachSetupError.set(null);
  coachSetupPulled.set(null);
  coachSetupOp.set({ kind: 'pull', name, progress: null });
  try {
    const status = await getBackend().coachPullModel(name, (progress) =>
      coachSetupOp.set({ kind: 'pull', name, progress }),
    );
    coachSetupStatus.set(status);
    coachSetupPulled.set(name);
  } catch (e) {
    coachSetupError.set(errorKey(e));
    await refreshCoachSetup();
  } finally {
    coachSetupOp.set({ kind: 'idle' });
  }
}

/** Nimmt ein schon geladenes Modell für den Coach. */
export async function useModel(name: string): Promise<void> {
  coachSetupError.set(null);
  try {
    await getBackend().coachSetModel(name);
    await refreshCoachSetup();
  } catch (e) {
    coachSetupError.set(errorKey(e));
  }
}

export async function cancelPull(): Promise<void> {
  try {
    await getBackend().coachPullCancel();
  } catch (e) {
    coachSetupError.set(errorKey(e));
  }
}

/** Für Tests: alles auf den Anfang. */
export function resetCoachSetupState(): void {
  stopPolling();
  coachSetupStatus.set(null);
  coachSetupOp.set({ kind: 'idle' });
  coachSetupError.set(null);
  coachSetupPulled.set(null);
  coachSetupFocus.set(false);
}

// --- Reine Hilfen ---------------------------------------------------------------------

/** Zustand des Schritts, aus dem sich Anzeige und Knöpfe ergeben. */
export type StepState =
  | 'loading'
  /** Ollama fehlt oder läuft nicht, die App kann es einrichten. */
  | 'installable'
  /** Ollama fehlt oder läuft nicht, die App kann es nicht einrichten (Anleitung). */
  | 'manual'
  | 'installing'
  /** Ollama läuft, es ist noch kein Modell da. */
  | 'no_model'
  | 'pulling'
  | 'ready';

export function stepState(status: CoachSetupStatus | null, op: SetupOp): StepState {
  if (op.kind === 'install') return 'installing';
  if (op.kind === 'pull') return 'pulling';
  if (!status) return 'loading';
  if (status.ollama !== 'running') return status.can_install ? 'installable' : 'manual';
  return status.model ? 'ready' : 'no_model';
}

export type Badge = 'recommended' | 'slow' | 'too_big' | 'installed' | 'active';

/** Marken einer Zeile der Modellliste. "Empfohlen" trägt nur ein Modell, das auf diesem Rechner
 * läuft (`fits`): Fällt die Empfehlung auf das Startmodell zurück, obwohl es zu groß ist, wäre
 * "Empfohlen für deinen PC" falsch. Ein zu großes oder langsames Modell bleibt wählbar. */
export function badgesFor(entry: CoachCatalogEntry, activeModel: string | null): Badge[] {
  const out: Badge[] = [];
  if (entry.recommended && entry.fit === 'fits') out.push('recommended');
  if (entry.fit === 'slow') out.push('slow');
  if (entry.fit === 'too_big') out.push('too_big');
  if (entry.name === activeModel) out.push('active');
  else if (entry.installed) out.push('installed');
  return out;
}

/** Prozent von 0 bis 100, abgerundet. `null`, solange die Gesamtgröße unbekannt ist. */
export function percentOf(completed: number, total: number): number | null {
  if (!(total > 0) || !Number.isFinite(completed)) return null;
  return Math.max(0, Math.min(100, Math.floor((completed / total) * 100)));
}

/** Gigabyte (Dezimal, wie die Ollama-Bibliothek) mit einer Nachkommastelle. */
export function gbText(bytes: number, locale: Locale): string {
  return formatNumber(bytes / 1e9, locale, 1);
}

/** Schlüssel und Platzhalter für den Text zu einer Zeile des Einrichtungsskripts. Unbekannte
 * Werte zeigen den allgemeinen Text, nie den Wert selbst. */
export function installPhaseText(p: CoachSetupProgress): [string, Params] {
  if (p.phase === 'pull' && p.value === 'progress') {
    return ['ui.coach_setup.install_phase.pull_progress', { percent: p.percent ?? 0 }];
  }
  const key = `ui.coach_setup.install_phase.${p.phase}_${p.value}`;
  return [hasKey('de', key) ? key : 'ui.coach_setup.installing', {}];
}

/** Schlüssel zum Fehlercode der Zustandsdatei, `unknown` und Unbekanntes sind der allgemeine
 * Fehler. */
export function stateErrorKey(code: string): string {
  const key = `ui.error.coach_setup_${code}`;
  return hasKey('de', key) ? key : 'ui.error.coach_setup_failed';
}
