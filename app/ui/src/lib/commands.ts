// Befehle der Befehlspalette (T61, E14, SPEC 5.5). Reine Daten und Filter ohne DOM: Die Palette
// zeigt, was hier steht, und führt `run` aus. Kein Befehl hält eine eigene Einstellung; er führt
// zu der einen Stelle, an der sie steht, oder löst eine Handlung aus.

import type { ActivityItem, Locale } from './backend/types';
import { formatDate, formatDistance, formatDuration } from './format';
import type { Translate } from './i18n';
import {
  BODY_TABS,
  bodyTab,
  HISTORY_TABS,
  openActivity,
  openCoach,
  openHistory,
  openSettings,
  SETTINGS_TABS,
  view,
  type MainView,
} from './state';

export type CommandGroup = 'go' | 'history' | 'settings' | 'actions' | 'activities';

export interface Command {
  id: string;
  group: CommandGroup;
  /** Sichtbarer Text, fertig übersetzt. */
  label: string;
  /** Weitere Suchwörter, fertig übersetzt oder sprachneutral. */
  keywords: string;
  /** Kürzel als Anzeige, zum Beispiel "G H". */
  shortcut?: string;
  run: () => void;
}

/** Zweistufige Kürzel "G dann Taste" für die Hauptansichten (SPEC 5.5). */
export const GO_KEYS: Record<string, MainView | 'settings'> = {
  h: 'today',
  p: 'plan',
  v: 'history',
  e: 'settings',
};

const goShortcut = (v: MainView | 'settings') =>
  `G ${Object.keys(GO_KEYS)
    .find((k) => GO_KEYS[k] === v)!
    .toUpperCase()}`;

/** Feste Befehle in der Reihenfolge der Anzeige. */
export function staticCommands(t: Translate, coachKbd: string): Command[] {
  const go = (['today', 'plan', 'history', 'settings'] as const).map((v): Command => ({
    id: `go.${v}`,
    group: 'go',
    label: t(`ui.nav.${v}`),
    keywords: '',
    shortcut: goShortcut(v),
    run: () => view.set(v),
  }));
  const history: Command[] = HISTORY_TABS.flatMap((tab): Command[] => {
    if (tab === 'body') {
      return BODY_TABS.map((b) => ({
        id: `history.body.${b}`,
        group: 'history',
        label: `${t('ui.history.tab.body')} · ${t(`ui.history.body.${b}`)}`,
        keywords: t('ui.nav.history'),
        run: () => {
          bodyTab.set(b);
          openHistory('body');
        },
      }));
    }
    return [
      {
        id: `history.${tab}`,
        group: 'history',
        label: t(`ui.history.tab.${tab}`),
        keywords: t('ui.nav.history'),
        run: () => openHistory(tab),
      },
    ];
  });
  const settings: Command[] = SETTINGS_TABS.map((tab) => ({
    id: `settings.${tab}`,
    group: 'settings',
    label: t(`ui.settings.tab.${tab}`),
    keywords: `${t('ui.nav.settings')} ${t(`ui.palette.keywords.settings_${tab}`)}`,
    run: () => openSettings(tab),
  }));
  const actions: Command[] = [
    {
      id: 'action.coach',
      group: 'actions',
      label: t('ui.palette.cmd.coach'),
      keywords: t('ui.nav.coach'),
      shortcut: coachKbd,
      run: () => openCoach(),
    },
    {
      id: 'action.plan',
      group: 'actions',
      label: t('ui.palette.cmd.new_plan'),
      keywords: t('ui.nav.plan'),
      run: () => view.set('plan'),
    },
  ];
  return [...go, ...actions, ...history, ...settings];
}

/** Eine Aktivität als Befehl: Datum, Sportart, Dauer und Strecke, weil Aktivitäten keinen
 *  Namen haben. Duplikate erscheinen nicht. */
export function activityCommands(items: ActivityItem[], t: Translate, locale: Locale): Command[] {
  return items
    .filter((a) => a.duplicate_of === null)
    .map((a) => {
      const parts = [formatDate(a.local_date, locale), t(`ui.sport.${a.sport}`)];
      parts.push(formatDuration(a.duration_s));
      if (a.distance_m !== null) parts.push(formatDistance(a.distance_m, locale));
      return {
        id: `activity.${a.id}`,
        group: 'activities' as const,
        label: parts.join(' · '),
        keywords: `${t('ui.history.tab.activities')} ${a.local_date}`,
        run: () => openActivity(a.id),
      };
    });
}

function fold(text: string): string {
  return text.toLocaleLowerCase().normalize('NFD').replace(/\p{M}/gu, '');
}

/**
 * Filter: Jedes Wort der Eingabe muss in Text oder Suchwörtern vorkommen, Groß- und
 * Kleinschreibung und Akzente zählen nicht. Treffer am Anfang des Texts stehen vorn, sonst
 * bleibt die Reihenfolge. Aktivitäten erscheinen erst mit Eingabe und höchstens acht, damit die
 * Liste kurz bleibt (Faustregel).
 */
export function filterCommands(commands: Command[], query: string, maxActivities = 8): Command[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) return commands.filter((c) => c.group !== 'activities');
  const hits = commands.filter((c) => {
    const hay = fold(`${c.label} ${c.keywords}`);
    return words.every((w) => hay.includes(w));
  });
  const front = hits.filter((c) => fold(c.label).startsWith(words[0]));
  const rest = hits.filter((c) => !fold(c.label).startsWith(words[0]));
  let activities = 0;
  return [...front, ...rest].filter(
    (c) => c.group !== 'activities' || activities++ < maxActivities,
  );
}

/** Text ohne Treffer wird zur Frage an den Coach (E14). Gesendet wird nichts. */
export function askCoachCommand(query: string, t: Translate): Command {
  const text = query.trim();
  return {
    id: 'action.ask',
    group: 'actions',
    label: t('ui.palette.ask', { text }),
    keywords: '',
    run: () => openCoach({ prefill: text }),
  };
}
