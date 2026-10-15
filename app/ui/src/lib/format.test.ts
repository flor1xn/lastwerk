import { describe, expect, it } from 'vitest';
import { addDays, daysBetween, isIsoDate, localIso, mondayOf } from './dates';
import {
  formatDate,
  formatDistance,
  formatDuration,
  formatNumber,
  formatPace,
  formatSpeed,
  formatWeekday,
  MISSING,
  parseDecimal,
} from './format';

describe('formatNumber', () => {
  it('nutzt in de das Dezimalkomma und in en den Punkt', () => {
    expect(formatNumber(1234.5, 'de', 1)).toBe('1.234,5');
    expect(formatNumber(1234.5, 'en', 1)).toBe('1,234.5');
    expect(formatNumber(0.8, 'de', 2)).toBe('0,80');
    expect(formatNumber(0.8, 'en', 2)).toBe('0.80');
  });

  it('zeigt fehlende Werte als Platzhalter und nie als 0', () => {
    for (const v of [null, undefined, NaN, Infinity]) {
      expect(formatNumber(v, 'de')).toBe(MISSING);
    }
    expect(formatNumber(0, 'de', 0)).toBe('0');
  });
});

describe('formatDuration', () => {
  it('schreibt h:mm:ss und unter einer Stunde m:ss', () => {
    expect(formatDuration(3725)).toBe('1:02:05');
    expect(formatDuration(65)).toBe('1:05');
    expect(formatDuration(0)).toBe('0:00');
    expect(formatDuration(null)).toBe(MISSING);
  });
});

describe('formatDistance, formatDate', () => {
  it('formatiert Kilometer und Datum je Sprache', () => {
    expect(formatDistance(12_345, 'de')).toBe('12,3 km');
    expect(formatDistance(12_345, 'en')).toBe('12.3 km');
    expect(formatDistance(null, 'de')).toBe(MISSING);
    expect(formatDate('2026-10-13', 'de')).toBe('13.10.2026');
    expect(formatDate('2026-10-13', 'en')).toBe('10/13/2026');
    expect(formatWeekday('2026-10-13', 'de')).toMatch(/^Di/);
    expect(formatWeekday('2026-10-13', 'en')).toMatch(/^Tue/);
  });
});

describe('parseDecimal', () => {
  it('liest Komma und Punkt, lehnt Unsinn ab', () => {
    expect(parseDecimal('3,3')).toBe(3.3);
    expect(parseDecimal(' 250 ')).toBe(250);
    expect(parseDecimal('')).toBeNull();
    expect(parseDecimal('abc')).toBeNull();
    expect(parseDecimal('Infinity')).toBeNull();
  });
});

describe('dates', () => {
  it('rechnet Wochen mit Montag als Anfang', () => {
    // 13. Oktober 2026 ist ein Dienstag.
    expect(mondayOf('2026-10-13')).toBe('2026-10-12');
    expect(mondayOf('2026-10-12')).toBe('2026-10-12');
    expect(mondayOf('2026-10-18')).toBe('2026-10-12');
    expect(mondayOf('2026-10-19')).toBe('2026-10-19');
  });

  it('addiert Tage über Monats- und Jahresgrenzen und Sommerzeit', () => {
    expect(addDays('2026-12-31', 1)).toBe('2027-01-01');
    expect(addDays('2026-03-01', -1)).toBe('2026-02-28');
    expect(addDays('2026-03-28', 2)).toBe('2026-03-30');
    expect(daysBetween('2026-03-28', '2026-03-30')).toBe(2);
  });

  it('prüft Kalendertage streng', () => {
    expect(isIsoDate('2026-02-29')).toBe(false);
    expect(isIsoDate('2026-02-28')).toBe(true);
    expect(isIsoDate('13.10.2026')).toBe(false);
  });

  it('nimmt das lokale Datum', () => {
    expect(localIso(new Date(2026, 9, 13, 23, 30))).toBe('2026-10-13');
  });
});

describe('formatPace und formatSpeed (T31)', () => {
  it('rechnet m/s in min/km', () => {
    // 1000 m / 4 m/s = 250 s = 4:10 je Kilometer.
    expect(formatPace(4)).toBe('4:10');
    // 1000 / 2,5 = 400 s = 6:40.
    expect(formatPace(2.5)).toBe('6:40');
  });

  it('zeigt Gehen, Stehen und fehlende Werte als fehlend', () => {
    expect(formatPace(0)).toBe(MISSING);
    expect(formatPace(0.4)).toBe(MISSING);
    expect(formatPace(null)).toBe(MISSING);
    expect(formatPace(Number.NaN)).toBe(MISSING);
  });

  it('rechnet m/s in km/h', () => {
    // 10 m/s * 3,6 = 36 km/h.
    expect(formatSpeed(10, 'de')).toBe('36,0 km/h');
    expect(formatSpeed(10, 'en')).toBe('36.0 km/h');
    expect(formatSpeed(null, 'de')).toBe(MISSING);
  });
});
