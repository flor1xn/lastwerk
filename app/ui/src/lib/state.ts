// Zustand, den mehrere Ansichten teilen: aktive Ansicht und Reiter, gewählte Aktivität, Thema.

import { get, writable } from 'svelte/store';
import type { FitMigration, Theme } from './backend/types';

/**
 * Hauptpunkte der Seitenleiste (T42, E8): nur Heute, Plan und Verlauf. Einstellungen stehen
 * unten in der Seitenleiste. Alles Weitere ist ein Reiter innerhalb dieser Seiten.
 */
export const VIEWS = ['today', 'plan', 'history'] as const;
export type MainView = (typeof VIEWS)[number];
export type View = MainView | 'settings';

/** Reiter des Verlaufs. */
export const HISTORY_TABS = ['calendar', 'activities', 'progress', 'performance', 'body'] as const;
export type HistoryTab = (typeof HISTORY_TABS)[number];

/** Umschalter im Reiter Kalender: Monatsraster oder Woche. */
export const CALENDAR_MODES = ['month', 'week'] as const;
export type CalendarMode = (typeof CALENDAR_MODES)[number];

/** Umschalter im Reiter Körper: Erholung oder Gesundheit. */
export const BODY_TABS = ['recovery', 'health'] as const;
export type BodyTab = (typeof BODY_TABS)[number];

/** Reiter der Einstellungen. */
export const SETTINGS_TABS = ['data', 'coach', 'profile', 'app'] as const;
export type SettingsTab = (typeof SETTINGS_TABS)[number];

export const view = writable<View>('today');
export const historyTab = writable<HistoryTab>('calendar');
export const calendarMode = writable<CalendarMode>('month');
export const bodyTab = writable<BodyTab>('recovery');
export const settingsTab = writable<SettingsTab>('data');
/** Im Reiter Aktivitäten: `null` zeigt die Liste, sonst das Detail dieser Aktivität. */
export const selectedActivityId = writable<number | null>(null);
/** Der in der Planansicht gezeigte Plan (ID). Bleibt beim Wechsel der Ansicht erhalten. */
export const selectedPlanId = writable<string | null>(null);

/** Zählt hoch, wenn sich Daten geändert haben (Import, Schwelle). Ansichten laden dann neu. */
export const dataVersion = writable(0);

export function bumpData(): void {
  dataVersion.update((n) => n + 1);
}

/** Öffnet den Verlauf, auf Wunsch mit einem bestimmten Reiter. */
export function openHistory(tab?: HistoryTab): void {
  if (tab) historyTab.set(tab);
  view.set('history');
}

/** Öffnet die Einstellungen, auf Wunsch mit einem bestimmten Reiter. */
export function openSettings(tab?: SettingsTab): void {
  if (tab) settingsTab.set(tab);
  view.set('settings');
}

/** Öffnet das Detail einer Aktivität an Ort und Stelle: Verlauf, Reiter Aktivitäten. */
export function openActivity(id: number): void {
  selectedActivityId.set(id);
  openHistory('activities');
}

/** Zurück vom Detail zur Liste der Aktivitäten. */
export function closeActivity(): void {
  selectedActivityId.set(null);
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
