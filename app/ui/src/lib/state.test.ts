import { get } from 'svelte/store';
import { afterEach, describe, expect, it } from 'vitest';
import {
  closeActivity,
  historyTab,
  openActivity,
  openHistory,
  openSettings,
  selectedActivityId,
  settingsTab,
  view,
  VIEWS,
} from './state';

afterEach(() => {
  view.set('today');
  historyTab.set('calendar');
  settingsTab.set('data');
  selectedActivityId.set(null);
});

describe('Navigation (T42)', () => {
  it('hat genau drei Hauptpunkte, ohne Detail', () => {
    expect([...VIEWS]).toEqual(['today', 'plan', 'history']);
  });

  it('öffnet den Verlauf mit und ohne Reiter', () => {
    historyTab.set('progress');
    openHistory();
    expect(get(view)).toBe('history');
    expect(get(historyTab)).toBe('progress');
    openHistory('body');
    expect(get(historyTab)).toBe('body');
  });

  it('öffnet die Einstellungen mit Reiter', () => {
    openSettings('coach');
    expect(get(view)).toBe('settings');
    expect(get(settingsTab)).toBe('coach');
    openSettings();
    expect(get(settingsTab)).toBe('coach');
  });

  it('öffnet eine Aktivität im Reiter Aktivitäten und kehrt zur Liste zurück', () => {
    openActivity(42);
    expect(get(view)).toBe('history');
    expect(get(historyTab)).toBe('activities');
    expect(get(selectedActivityId)).toBe(42);
    closeActivity();
    expect(get(selectedActivityId)).toBeNull();
    expect(get(historyTab)).toBe('activities');
  });
});
