// Zustand, den mehrere Ansichten teilen: aktive Ansicht, gewählte Aktivität, Thema.

import { get, writable } from 'svelte/store';
import type { FitMigration, Theme } from './backend/types';

export const VIEWS = [
  'today',
  'calendar',
  'week',
  'progress',
  'performance',
  'recovery',
  'health',
  'plan',
  'activities',
  'detail',
  'settings',
] as const;
export type View = (typeof VIEWS)[number];

export const view = writable<View>('today');
export const selectedActivityId = writable<number | null>(null);
/** Der in der Planansicht gezeigte Plan (ID). Bleibt beim Wechsel der Ansicht erhalten. */
export const selectedPlanId = writable<string | null>(null);

/** Zählt hoch, wenn sich Daten geändert haben (Import, Schwelle). Ansichten laden dann neu. */
export const dataVersion = writable(0);

export function bumpData(): void {
  dataVersion.update((n) => n + 1);
}

export function openActivity(id: number): void {
  selectedActivityId.set(id);
  view.set('detail');
}

export const theme = writable<Theme>('dark');

/** Das Thema, das gerade wirkt (löst "system" über das Betriebssystem auf). */
export const resolvedTheme = writable<'dark' | 'light'>('dark');

function systemPrefersLight(): boolean {
  return (
    typeof window !== 'undefined' && !!window.matchMedia?.('(prefers-color-scheme: light)').matches
  );
}

/** Setzt data-theme am Wurzelelement und aktualisiert `resolvedTheme`. */
export function applyTheme(next: Theme): void {
  theme.set(next);
  if (typeof document !== 'undefined') document.documentElement.dataset.theme = next;
  resolvedTheme.set(
    next === 'light' || (next === 'system' && systemPrefersLight()) ? 'light' : 'dark',
  );
}

/** Folgt Änderungen der Systemeinstellung, solange das Thema "system" ist. */
export function watchSystemTheme(): () => void {
  if (typeof window === 'undefined' || !window.matchMedia) return () => {};
  const query = window.matchMedia('(prefers-color-scheme: light)');
  const onChange = () => {
    if (get(theme) === 'system') applyTheme('system');
  };
  query.addEventListener('change', onChange);
  return () => query.removeEventListener('change', onChange);
}

/** Der Erststart-Assistent ist offen, etwa über "Einrichtung wiederholen" in den Einstellungen. */
export const wizardOpen = writable(false);

/**
 * Der Nutzer hat das Gespräch mit dem Coach in dieser Sitzung freigegeben (SPEC 9.7, 4: eine
 * Freigabe je Sitzung statt einer Bestätigung je Nachricht). Nur im Speicher: Ein Neustart der App
 * fragt neu.
 */
export const coachReleased = writable(false);
/** Ergebnis der Übernahme des früheren FIT-Ordners (T28), bis der Nutzer die Meldung ausblendet. */
export const fitMigration = writable<FitMigration | null>(null);
