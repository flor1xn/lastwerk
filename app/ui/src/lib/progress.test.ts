import { describe, expect, it } from 'vitest';
import type { ProgressEntry, Sport } from './backend/types';
import {
  changePercent,
  cumulative,
  daysInYear,
  filterSport,
  metricValue,
  monthly,
  sameDayPreviousYear,
  sportsPresent,
  weekly,
  yearToDate,
} from './progress';

function e(
  date: string,
  sport: Sport,
  duration_s: number,
  distance_m = 0,
  load = 0,
  load_missing = 0,
): ProgressEntry {
  return { date, sport, count: 1, duration_s, distance_m, load, load_missing };
}

// Referenzdaten (Handrechnung in den Tests):
//   2025-03-10 Rad 3600 s, 30 km, Last 60
//   2025-10-15 Lauf 1800 s, 6 km, Last 30
//   2025-12-31 Rad 7200 s, 60 km, Last 120
//   2026-01-05 Rad 5400 s, 45 km, Last 90 (Montag)
//   2026-10-12 Lauf 3600 s, 12 km, ohne Last (Montag)
//   2026-10-15 Rad 3600 s, 30 km, Last 70 (Donnerstag, "heute")
const ENTRIES: ProgressEntry[] = [
  e('2025-03-10', 'cycling', 3600, 30_000, 60),
  e('2025-10-15', 'running', 1800, 6_000, 30),
  e('2025-12-31', 'cycling', 7200, 60_000, 120),
  e('2026-01-05', 'cycling', 5400, 45_000, 90),
  e('2026-10-12', 'running', 3600, 12_000, 0, 1),
  e('2026-10-15', 'cycling', 3600, 30_000, 70),
];
const TODAY = '2026-10-15';

describe('yearToDate', () => {
  it('summiert dieses Jahr und das Vorjahr bis zum gleichen Kalendertag', () => {
    const y = yearToDate(ENTRIES, TODAY);
    expect(y.year).toBe(2026);
    // 2026: 5400 + 3600 + 3600 = 12600 s = 3,5 h; Last 90 + 70 = 160; eine Einheit ohne Last.
    expect(y.current.duration_s).toBe(12_600);
    expect(metricValue(y.current, 'duration')).toBe(3.5);
    expect(y.current.load).toBe(160);
    expect(y.current.load_missing).toBe(1);
    expect(y.current.count).toBe(3);
    // 2025 bis 15.10.: 3600 + 1800 = 5400 s, 36 km, Last 90 (31.12. zählt nicht).
    expect(y.previous.duration_s).toBe(5400);
    expect(metricValue(y.previous, 'distance')).toBe(36);
    expect(y.previous.load).toBe(90);
  });

  it('ersetzt den 29. Februar im Nicht-Schaltjahr durch den 28.', () => {
    expect(sameDayPreviousYear('2028-02-29')).toBe('2027-02-28');
    expect(sameDayPreviousYear('2029-03-01')).toBe('2028-03-01');
    expect(sameDayPreviousYear('2025-02-28')).toBe('2024-02-28');
  });
});

describe('changePercent', () => {
  it('rechnet die Veränderung und meidet Division durch null', () => {
    // (160 - 90) / 90 * 100 = 77,78 Prozent.
    expect(changePercent(160, 90)).toBeCloseTo(77.78, 2);
    expect(changePercent(5, 0)).toBeNull();
    expect(changePercent(0, 10)).toBe(-100);
  });
});

describe('weekly', () => {
  it('bildet Wochen ab Montag, älteste zuerst, mit Summen je Sportart', () => {
    const w = weekly(ENTRIES, TODAY, 2);
    expect(w.map((b) => b.start)).toEqual(['2026-10-05', '2026-10-12']);
    expect(w[0].total.count).toBe(0);
    // Woche ab 12.10.: Lauf 3600 s und Rad 3600 s.
    expect(w[1].total.duration_s).toBe(7200);
    expect(w[1].bySport.running?.duration_s).toBe(3600);
    expect(w[1].bySport.cycling?.load).toBe(70);
    expect(w[1].bySport.swimming).toBeUndefined();
  });

  it('liefert 52 Wochen und lässt ältere Tage weg', () => {
    const w = weekly(ENTRIES, TODAY);
    expect(w).toHaveLength(52);
    // Erste Woche: 52 Wochen inklusive der laufenden, also Montag 51 Wochen vor dem 12.10.2026.
    expect(w[0].start).toBe('2025-10-20');
    const sum = w.reduce((s, b) => s + b.total.duration_s, 0);
    // Ab 20.10.2025: 7200 + 5400 + 3600 + 3600 = 19800 s.
    expect(sum).toBe(19_800);
  });
});

describe('monthly', () => {
  it('liefert die Monate neueste zuerst', () => {
    const m = monthly(ENTRIES, TODAY, 13);
    expect(m[0].month).toBe('2026-10');
    expect(m[12].month).toBe('2025-10');
    expect(m[0].total.duration_s).toBe(7200);
    expect(m[3].month).toBe('2026-07');
    expect(m[3].total.count).toBe(0);
    expect(m[9].month).toBe('2026-01');
    expect(m[9].total.load).toBe(90);
    expect(m[10].month).toBe('2025-12');
    expect(m[12].total.distance_m).toBe(6000);
  });

  it('geht über den Jahreswechsel zurück', () => {
    expect(monthly([], '2026-02-10', 4).map((b) => b.month)).toEqual([
      '2026-02',
      '2026-01',
      '2025-12',
      '2025-11',
    ]);
  });
});

describe('cumulative', () => {
  it('summiert ab dem 1. Januar bis zum letzten Tag', () => {
    const c = cumulative(ENTRIES, 2026, TODAY, 'load');
    // 1.1. bis 15.10.2026 sind 288 Tage.
    expect(c).toHaveLength(288);
    expect(c[3]).toBe(0);
    expect(c[4]).toBe(90); // 5. Januar
    expect(c[287]).toBe(160);
  });

  it('endet spätestens am 31. Dezember und zählt das ganze Vorjahr', () => {
    const c = cumulative(ENTRIES, 2025, '2026-10-15', 'distance');
    expect(c).toHaveLength(daysInYear(2025));
    // 30 + 6 + 60 = 96 km am Jahresende.
    expect(c[c.length - 1]).toBe(96);
  });

  it('ist leer vor Jahresbeginn', () => {
    expect(cumulative(ENTRIES, 2027, TODAY, 'load')).toEqual([]);
  });
});

describe('Sportarten', () => {
  it('filtert und listet vorhandene Sportarten in fester Reihenfolge', () => {
    expect(filterSport(ENTRIES, 'running')).toHaveLength(2);
    expect(filterSport(ENTRIES, null)).toHaveLength(6);
    expect(sportsPresent(ENTRIES)).toEqual(['cycling', 'running']);
    expect(sportsPresent([])).toEqual([]);
  });
});
