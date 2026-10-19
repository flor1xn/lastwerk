import { cleanup, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { timeLabel, timeSplits } from '../charts';
import type { StoryDay } from '../week-story';
import BandScale from './BandScale.svelte';
import DeviationRow from './DeviationRow.svelte';
import WeekStrip from './WeekStrip.svelte';

// Bausteine der Instrumente (T62, SPEC 5.5).

afterEach(cleanup);

const markerShift = () =>
  (screen.getByTestId('scale-marker').parentElement as HTMLElement).style.transform;

describe('BandScale', () => {
  // Skala von -50 bis +25, also 75 Punkte breit. Lage der Marke = (TSB + 50) / 75.
  it.each([
    // TSB -30: (−30 + 50) / 75 = 26,666… %, Grenze überlastet zu belastet
    [-30, 'loaded', 'translateX(26.666666666666668%)'],
    // TSB -10: 40 / 75 = 53,333… %, Grenze belastet zu ausgewogen
    [-10, 'balanced', 'translateX(53.333333333333336%)'],
    // TSB +5: 55 / 75 = 73,333… %, Grenze ausgewogen zu frisch
    [5, 'fresh', 'translateX(73.33333333333333%)'],
    // TSB -80 stößt links an: 0 %
    [-80, 'overloaded', 'translateX(0%)'],
    // TSB +40 stößt rechts an: 100 %
    [40, 'fresh', 'translateX(100%)'],
  ] as const)('setzt die Marke für TSB %d in den richtigen Bereich', (tsb, state, transform) => {
    render(BandScale, { tsb, state, testid: 'scale' });
    expect(markerShift()).toBe(transform);
    const root = screen.getByTestId('scale');
    expect(root.dataset.state).toBe(state);
    expect(root.querySelector('.seg.on')?.classList.contains(state)).toBe(true);
    expect(root.getAttribute('role')).toBe('img');
  });

  it('nennt den Zustand als Text, nicht nur als Farbe', () => {
    render(BandScale, { tsb: -14, state: 'loaded', testid: 'scale' });
    const root = screen.getByTestId('scale');
    expect(root.getAttribute('aria-label')).toContain('Belastet');
    expect(root.querySelector('.names .on')?.textContent).toBe('Belastet');
  });
});

describe('DeviationRow', () => {
  it('zeigt den Balken nach links für Werte unter der Basis, begrenzt auf 25 %', () => {
    render(DeviationRow, {
      label: 'HRV',
      value: '51',
      unit: 'ms',
      deviation: -15,
      deviationText: '−15 %',
      tone: 'warn',
      testid: 'dev',
    });
    const bar = screen.getByTestId('dev').querySelector('.bar i') as HTMLElement;
    // 15 / 25 = 0,6
    expect(bar.style.transform).toBe('scaleX(0.6)');
    expect(bar.classList.contains('below')).toBe(true);
    expect(screen.getByTestId('dev').classList.contains('warn')).toBe(true);
    expect(screen.getByTestId('dev').textContent).toContain('−15 %');
  });

  it('stößt bei großen Abweichungen an und bleibt ohne Basis leer', () => {
    const { unmount } = render(DeviationRow, {
      label: 'Ruhepuls',
      value: '70',
      unit: 'bpm',
      deviation: 40,
      deviationText: '+40 %',
      testid: 'dev',
    });
    expect((screen.getByTestId('dev').querySelector('.bar i') as HTMLElement).style.transform).toBe(
      'scaleX(1)',
    );
    unmount();
    render(DeviationRow, {
      label: 'Schlaf',
      value: '7,0',
      unit: 'h',
      deviation: null,
      deviationText: 'ohne Basis',
      testid: 'dev',
    });
    expect((screen.getByTestId('dev').querySelector('.bar i') as HTMLElement).style.transform).toBe(
      'scaleX(0)',
    );
  });
});

describe('WeekStrip', () => {
  const day = (date: string, state: StoryDay['state'], load = 0, plannedLoad = 0): StoryDay => ({
    date,
    state,
    planned: plannedLoad > 0 ? 1 : 0,
    done: load > 0 ? 1 : 0,
    hard: false,
    load,
    plannedLoad,
  });

  it('zeigt den Zustand je Tag als Form und als Text', () => {
    render(WeekStrip, {
      today: '2026-10-18',
      testid: 'strip',
      days: [
        day('2026-10-12', 'rest'),
        day('2026-10-13', 'done', 89, 80),
        day('2026-10-14', 'missed', 0, 40),
        day('2026-10-15', 'empty'),
        day('2026-10-16', 'extra', 30),
        day('2026-10-17', 'done', 45, 45),
        day('2026-10-18', 'planned', 0, 60),
      ],
    });
    const items = screen.getAllByTestId('day');
    expect(items).toHaveLength(7);
    expect(items[1].classList.contains('done')).toBe(true);
    expect(items[2].classList.contains('missed')).toBe(true);
    expect(items[2].textContent).toContain('✕');
    expect(items[2].textContent).toContain('verpasst');
    expect(items[6].classList.contains('today')).toBe(true);
    expect(items[6].classList.contains('planned')).toBe(true);
    // Höchster Wert der Woche 89: Dienstag 100 %, Sonntag geplant 60 / 89.
    expect((items[1].querySelector('b') as HTMLElement).style.height).toBe('100%');
    expect(items[0].querySelector('b')).toBeNull();
  });
});

describe('Zeitachse', () => {
  const at = (iso: string) => Date.parse(`${iso}T00:00:00Z`) / 1000;

  it('setzt Marken an Monatsanfänge und dünnt sie aus', () => {
    const splits = timeSplits(at('2026-06-21'), at('2026-10-18'), 10);
    expect(splits.map((s) => new Date(s * 1000).toISOString().slice(0, 10))).toEqual([
      '2026-07-01',
      '2026-08-01',
      '2026-09-01',
      '2026-10-01',
    ]);
    expect(timeSplits(at('2026-06-21'), at('2026-10-18'), 2)).toHaveLength(2);
  });

  it('nimmt Montage, wenn weniger als zwei Monatswechsel im Bereich liegen', () => {
    const splits = timeSplits(at('2026-10-01'), at('2026-10-25'), 10);
    // Montage im Oktober 2026: 5., 12., 19.
    expect(splits.map((s) => new Date(s * 1000).toISOString().slice(0, 10))).toEqual([
      '2026-10-05',
      '2026-10-12',
      '2026-10-19',
    ]);
  });

  it('beschriftet Monatsanfänge mit dem Monat, den Januar mit dem Jahr', () => {
    expect(timeLabel(at('2026-10-01'), 'de')).toMatch(/^Okt/);
    expect(timeLabel(at('2026-01-01'), 'en')).toBe('2026');
    expect(timeLabel(at('2026-10-12'), 'de')).toBe('12.10.');
  });
});
