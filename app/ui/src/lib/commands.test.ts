import { get } from 'svelte/store';
import { beforeEach, describe, expect, it } from 'vitest';
import type { ActivityItem } from './backend/types';
import { activityCommands, askCoachCommand, filterCommands, staticCommands } from './commands';
import { coachSession } from './coach-session.svelte';
import { translate } from './i18n';
import {
  bodyTab,
  closeCoach,
  coachOpen,
  historyTab,
  selectedActivityId,
  settingsTab,
  view,
} from './state';

const t = (key: string, params?: Record<string, string | number>) => translate('de', key, params);

function activity(id: number, date: string, duplicate: number | null = null): ActivityItem {
  return {
    id,
    local_date: date,
    start_utc: `${date}T07:00:00Z`,
    sport: 'cycling',
    sub_sport: null,
    duration_s: 3600,
    distance_m: 30_000,
    avg_hr: null,
    avg_power_w: null,
    load: {
      status: 'missing_threshold',
      value: null,
      method: null,
      intensity_factor: null,
      estimated: false,
      missing: null,
    } as ActivityItem['load'],
    is_primary: duplicate === null,
    duplicate_of: duplicate,
  };
}

beforeEach(() => {
  view.set('today');
  closeCoach();
  coachSession.draft = '';
});

describe('Befehle der Palette', () => {
  it('führen zu Ansichten, Reitern und Einstellungen', () => {
    const all = staticCommands(t, 'Strg J');
    const run = (id: string) => all.find((c) => c.id === id)!.run();
    run('go.plan');
    expect(get(view)).toBe('plan');
    run('history.performance');
    expect(get(view)).toBe('history');
    expect(get(historyTab)).toBe('performance');
    run('history.body.health');
    expect(get(historyTab)).toBe('body');
    expect(get(bodyTab)).toBe('health');
    run('settings.app');
    expect(get(view)).toBe('settings');
    expect(get(settingsTab)).toBe('app');
    expect(all.find((c) => c.id === 'go.today')!.shortcut).toBe('G H');
  });

  it('ohne Eingabe zeigen sie alle festen Befehle, aber keine Aktivitäten', () => {
    const all = [
      ...staticCommands(t, 'Strg J'),
      ...activityCommands([activity(1, '2026-10-17')], t, 'de'),
    ];
    const shown = filterCommands(all, '');
    expect(shown.some((c) => c.group === 'activities')).toBe(false);
    expect(shown.length).toBe(all.length - 1);
  });

  it('filtern ohne Rücksicht auf Groß- und Kleinschreibung und Umlaute, Treffer am Anfang zuerst', () => {
    const all = staticCommands(t, 'Strg J');
    const hits = filterCommands(all, 'KORPER');
    expect(hits.map((c) => c.id)).toEqual(['history.body.recovery', 'history.body.health']);
    // "sprache" steht nur in den Suchwörtern des Reiters App.
    expect(filterCommands(all, 'sprache').map((c) => c.id)).toEqual(['settings.app']);
    // Alle Wörter müssen passen.
    expect(filterCommands(all, 'verlauf kalender').map((c) => c.id)).toEqual(['history.calendar']);
  });

  it('zeigen Aktivitäten mit Datum und Sportart, ohne Duplikate, höchstens acht', () => {
    const items = Array.from({ length: 12 }, (_, i) =>
      activity(i + 1, `2026-10-${String(i + 1).padStart(2, '0')}`),
    );
    items.push(activity(99, '2026-10-05', 5));
    const cmds = activityCommands(items, t, 'de');
    expect(cmds).toHaveLength(12);
    expect(cmds[0].label).toBe('01.10.2026 · Rad · 1:00:00 · 30,0 km');
    const hits = filterCommands(cmds, 'rad');
    expect(hits).toHaveLength(8);
    cmds[2].run();
    expect(get(selectedActivityId)).toBe(3);
    expect(get(historyTab)).toBe('activities');
  });

  it('machen Text ohne Treffer zur Frage an den Coach, ohne zu senden', () => {
    const ask = askCoachCommand('  warum so müde?  ', t);
    expect(ask.label).toBe('Coach fragen: „warum so müde?“');
    ask.run();
    expect(get(coachOpen)).toBe(true);
    expect(coachSession.draft).toBe('warum so müde?');
  });
});
