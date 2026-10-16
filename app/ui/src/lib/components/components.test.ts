import { cleanup, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import type { DayLoad, LoadInfo, ZoneSchemeTime } from '../backend/types';
import { setLocale } from '../i18n';
import LoadCell from './LoadCell.svelte';
import WeekBars from './WeekBars.svelte';
import ZoneTable from './ZoneTable.svelte';

afterEach(() => {
  cleanup();
  setLocale('de');
});

const ok = (over: Partial<LoadInfo> = {}): LoadInfo => ({
  status: 'ok',
  value: 88.94,
  method: 'tss',
  intensity_factor: 0.93,
  estimated: false,
  missing: null,
  ...over,
});

const failed = (status: LoadInfo['status']): LoadInfo => ({
  status,
  value: null,
  method: null,
  intensity_factor: null,
  estimated: false,
  missing: null,
});

describe('LoadCell', () => {
  it('zeigt bei fehlender Schwelle "nicht berechenbar" und keine Zahl', () => {
    render(LoadCell, { load: failed('missing_threshold') });
    const cell = screen.getByTestId('load');
    expect(cell.textContent).toBe('nicht berechenbar');
    expect(cell.dataset.status).toBe('missing_threshold');
    expect(cell.textContent).not.toMatch(/\d/);
    expect(cell.getAttribute('title')).toContain('Schwellenwert');
  });

  it('nennt im Tooltip den fehlenden Wert und wo er einzutragen ist (T39)', () => {
    render(LoadCell, { load: { ...failed('missing_threshold'), missing: 'ftp_w' } });
    const cell = screen.getByTestId('load');
    expect(cell.textContent).toBe('nicht berechenbar');
    expect(cell.getAttribute('title')).toBe(
      'FTP fehlt. Garmin hat diesen Wert nicht geliefert. Eintragen unter Einstellungen, Schwellenwerte.',
    );
  });

  it('zeigt jeden Fehlerstatus als Text, nie als 0', () => {
    for (const status of ['missing_channel', 'implausible', 'unknown'] as const) {
      const { unmount } = render(LoadCell, { load: failed(status) });
      expect(screen.getByTestId('load').textContent).toBe('nicht berechenbar');
      unmount();
    }
  });

  it('zeigt den Wert mit Dezimalkomma und der Methode', () => {
    render(LoadCell, { load: ok() });
    const cell = screen.getByTestId('load');
    expect(cell.textContent).toContain('88,9');
    expect(cell.textContent).toContain('TSS');
    expect(screen.queryByTestId('estimated')).toBeNull();
  });

  it('kennzeichnet geschätzte Lasten', () => {
    render(LoadCell, { load: ok({ method: 'r_tss', estimated: true }) });
    expect(screen.getByTestId('estimated').textContent).toBe('geschätzt');
    expect(screen.getByTestId('load').textContent).toContain('rTSS');
  });

  it('folgt der Sprache: Punkt und englische Texte', () => {
    setLocale('en');
    render(LoadCell, { load: ok({ method: 's_tss', estimated: true }) });
    expect(screen.getByTestId('load').textContent).toContain('88.9');
    expect(screen.getByTestId('estimated').textContent).toBe('estimated');
    cleanup();
    render(LoadCell, { load: failed('missing_threshold') });
    expect(screen.getByTestId('load').textContent).toBe('not computable');
  });
});

describe('ZoneTable', () => {
  const scheme = (over: Partial<ZoneSchemeTime> = {}): ZoneSchemeTime => ({
    id: 'hr_karvonen',
    basis: 'hr_reserve',
    seconds: [600, 1200, 1200, 600, 0],
    estimated: false,
    source: 'Q14',
    status: 'Faustregel',
    ...over,
  });

  it('zeigt Zeit und Anteil je Zone mit Quelle und Prüfstatus', () => {
    render(ZoneTable, { scheme: scheme() });
    expect(screen.getAllByRole('row')).toHaveLength(5);
    expect(screen.getByText('Herzfrequenzzonen nach Reserve (5 Zonen)')).toBeTruthy();
    expect(screen.getByText(/Quelle: Q14 · Prüfstatus: Faustregel/)).toBeTruthy();
    // 600 s von 3600 s sind 17 Prozent, 20:00 Minuten ein Drittel.
    expect(screen.getAllByText('17 %').length).toBe(2);
    expect(screen.getAllByText('33 %').length).toBe(2);
    expect(screen.queryByTestId('estimated')).toBeNull();
  });

  it('kennzeichnet Zonen auf geschätzter HR_max als geschätzt', () => {
    render(ZoneTable, { scheme: scheme({ estimated: true }) });
    expect(screen.getByTestId('estimated').textContent).toBe('geschätzt');
    expect(screen.getByText('Maximalpuls geschätzt')).toBeTruthy();
  });

  it('teilt nicht durch 0, wenn die Summe 0 ist', () => {
    render(ZoneTable, { scheme: scheme({ seconds: [0, 0, 0, 0, 0] }) });
    expect(screen.queryByText(/NaN/)).toBeNull();
  });
});

describe('WeekBars', () => {
  const days: DayLoad[] = [
    { date: '2026-10-12', by_sport: [], not_computable: 0 },
    { date: '2026-10-13', by_sport: [], not_computable: 2 },
    {
      date: '2026-10-14',
      by_sport: [
        { sport: 'cycling', load: 80 },
        { sport: 'running', load: 20 },
      ],
      not_computable: 0,
    },
    ...[15, 16, 17, 18].map((d) => ({ date: `2026-10-${d}`, by_sport: [], not_computable: 0 })),
  ];

  it('zeigt sieben Tage, Summen und "nicht berechenbar" statt einer 0', () => {
    render(WeekBars, { days });
    expect(screen.getAllByTestId('day')).toHaveLength(7);
    const na = screen.getByTestId('day-not-computable');
    expect(na.textContent?.replace(/\s+/g, ' ').trim()).toBe('2× nicht berechenbar');
    // Nur der Tag mit Last trägt eine Summe, die anderen zeigen keine Zahl.
    const values = [...document.querySelectorAll('.value')].map((e) => e.textContent?.trim());
    expect(values.filter((v) => v !== '')).toEqual(['100']);
  });
});
