import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackend } from '../lib/backend';
import { MockBackend } from '../lib/backend/mock';
import { setLocale } from '../lib/i18n';
import { bodyTab, calendarMode, historyTab, selectedActivityId, view } from '../lib/state';
import HistoryView from './HistoryView.svelte';

vi.mock(
  '../lib/components/UplotChart.svelte',
  async () => import('../lib/components/UplotChartStub.svelte'),
);

beforeEach(() => {
  setBackend(new MockBackend({ today: '2026-10-14', garminConnected: true }));
  view.set('history');
});

afterEach(() => {
  cleanup();
  setBackend(null);
  setLocale('de');
  view.set('today');
  historyTab.set('calendar');
  calendarMode.set('month');
  bodyTab.set('recovery');
  selectedActivityId.set(null);
});

const tab = (value: string) => screen.getByTestId(`tab-history-${value}`);

describe('Reiter (T42)', () => {
  it('zeigt die fünf Reiter, nur der aktive ist per Tab-Taste erreichbar', async () => {
    render(HistoryView);
    expect(screen.getByRole('tablist').getAttribute('aria-label')).toBe('Bereiche des Verlaufs');
    const tabs = screen.getAllByRole('tab');
    expect(tabs.map((b) => b.textContent)).toEqual([
      'Kalender',
      'Aktivitäten',
      'Fortschritt',
      'Leistung',
      'Körper',
    ]);
    expect(tabs.map((b) => b.getAttribute('tabindex'))).toEqual(['0', '-1', '-1', '-1', '-1']);
    expect(tab('calendar').getAttribute('aria-selected')).toBe('true');
    const panel = screen.getByRole('tabpanel');
    expect(panel.getAttribute('aria-labelledby')).toBe('tab-history-calendar');
    await screen.findByTestId('calendar-view');
  });

  it('wechselt mit Pfeiltasten, Pos1 und Ende und nimmt den Fokus mit', async () => {
    render(HistoryView);
    tab('calendar').focus();
    await fireEvent.keyDown(tab('calendar'), { key: 'ArrowRight' });
    expect(get(historyTab)).toBe('activities');
    await waitFor(() => expect(document.activeElement).toBe(tab('activities')));
    expect(tab('activities').getAttribute('tabindex')).toBe('0');
    expect(tab('calendar').getAttribute('tabindex')).toBe('-1');

    await fireEvent.keyDown(tab('activities'), { key: 'End' });
    expect(get(historyTab)).toBe('body');
    // Am Ende geht es wieder vorn weiter.
    await fireEvent.keyDown(tab('body'), { key: 'ArrowRight' });
    expect(get(historyTab)).toBe('calendar');
    await fireEvent.keyDown(tab('calendar'), { key: 'ArrowLeft' });
    expect(get(historyTab)).toBe('body');
    await fireEvent.keyDown(tab('body'), { key: 'Home' });
    expect(get(historyTab)).toBe('calendar');
    // Andere Tasten lassen die Auswahl in Ruhe.
    await fireEvent.keyDown(tab('calendar'), { key: 'a' });
    expect(get(historyTab)).toBe('calendar');
  });

  it('schaltet im Kalender zwischen Monat und Woche', async () => {
    render(HistoryView);
    await screen.findByTestId('calendar-view');
    await fireEvent.click(
      screen.getByTestId('calendar-mode').querySelector('button[data-value="week"]')!,
    );
    await screen.findByTestId('week-range');
    expect(screen.queryByTestId('calendar-view')).toBeNull();
    // Kein zweites PMC-Diagramm in der Woche.
    expect(screen.queryByTestId('pmc-values')).toBeNull();
  });

  it('schaltet im Körper zwischen Erholung und Gesundheit', async () => {
    historyTab.set('body');
    render(HistoryView);
    await screen.findByTestId('recovery');
    await fireEvent.click(
      screen.getByTestId('body-tab').querySelector('button[data-value="health"]')!,
    );
    await screen.findByTestId('health');
    expect(screen.queryByTestId('recovery')).toBeNull();
  });

  it('öffnet das Detail einer Aktivität an Ort und Stelle und kehrt zur Liste zurück', async () => {
    historyTab.set('activities');
    render(HistoryView);
    const rows = await screen.findAllByTestId('activity-row');
    await fireEvent.click(rows[0]);
    await screen.findByTestId('detail-title');
    expect(get(view)).toBe('history');
    expect(get(historyTab)).toBe('activities');
    expect(screen.queryByTestId('activity-table')).toBeNull();
    await fireEvent.click(screen.getByTestId('detail-back'));
    await screen.findByTestId('activity-table');
    expect(get(selectedActivityId)).toBeNull();
    // Auch ein Klick auf den aktiven Reiter führt zurück zur Liste.
    await fireEvent.click((await screen.findAllByTestId('activity-row'))[0]);
    await screen.findByTestId('detail-title');
    await fireEvent.click(tab('activities'));
    await screen.findByTestId('activity-table');
  });
});
