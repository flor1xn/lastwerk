import { describe, expect, it } from 'vitest';
import { MockBackend, mulberry32 } from './mock';
import { buildPerformance, MOCK_CURVE_DURATIONS } from './mock-performance';

describe('Leistung der Attrappe (T33)', () => {
  it('liefert das Raster mit Zeitraum und Vorzeitraum wie der Rust-Teil', () => {
    const p = buildPerformance('2026-10-15', '90', 'rich', [1, 2], [3], [], mulberry32);
    expect(p.power.map((x) => x.duration_s)).toEqual(MOCK_CURVE_DURATIONS);
    expect(p.from).toBe('2026-07-18');
    expect(p.previous_from).toBe('2026-04-19');
    for (const x of p.power) {
      expect(x.change_percent === null).toBe(x.trend === 'power_curve.no_data');
      if (x.current)
        expect(x.current.date >= '2026-07-18' && x.current.date <= '2026-10-15').toBe(true);
    }
    const all = buildPerformance('2026-10-15', 'all', 'rich', [1], [3], [], mulberry32);
    expect(all.from).toBeNull();
    expect(all.power.every((x) => x.previous_w === null)).toBe(true);
    const empty = buildPerformance('2026-10-15', '90', 'empty', [1], [3], [], mulberry32);
    expect(empty.power.every((x) => x.current === null)).toBe(true);
    expect(empty.run_bests.every((x) => x.current === null)).toBe(true);
    // T37: Strecken wie im Rust-Teil, Veränderung nur mit beiden Zeiten.
    expect(p.run_bests.map((r) => r.distance_m)).toEqual([1000, 5000, 10_000, 21_097, 42_195]);
    for (const r of p.run_bests) {
      expect(r.change_percent === null).toBe(r.current === null || r.previous_s === null);
    }
  });

  it('lehnt einen unbekannten Zeitraum ab und meldet den veralteten Cache nur bis zum Einlesen', async () => {
    const b = new MockBackend({ today: '2026-10-15', cacheOutdated: true });
    await expect(b.getPerformance('2026-10-15', '7' as never)).rejects.toMatchObject({
      code: 'invalid_value',
    });
    expect((await b.getSettings()).cache_outdated).toBe(true);
    await b.importFolder(() => {});
    expect((await b.getSettings()).cache_outdated).toBe(false);
  });
});
