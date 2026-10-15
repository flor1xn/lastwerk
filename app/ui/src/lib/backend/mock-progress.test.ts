import { describe, expect, it } from 'vitest';
import { mulberry32 } from './mock';
import { buildProgress, progressFrom } from './mock-progress';

describe('Fortschritt der Attrappe (T32)', () => {
  it('beginnt wie der Rust-Teil am früheren der beiden Tage', () => {
    expect(progressFrom('2026-10-15')).toBe('2024-11-01');
    expect(progressFrom('2026-12-31')).toBe('2025-01-01');
    expect(progressFrom('2026-01-01')).toBe('2024-02-01');
  });

  it('liefert sortierte Tageszeilen im Zeitraum, je Variante', () => {
    const rich = buildProgress('2026-10-15', 'rich', mulberry32);
    expect(rich.entries.length).toBeGreaterThan(300);
    expect(rich.entries.every((e) => e.date >= rich.from && e.date <= '2026-10-15')).toBe(true);
    expect(rich.entries.every((e, i, a) => i === 0 || a[i - 1].date <= e.date)).toBe(true);
    expect(rich.entries.some((e) => e.load_missing > 0)).toBe(true);
    expect(buildProgress('2026-10-15', 'empty', mulberry32).entries).toEqual([]);
    const sparse = buildProgress('2026-10-15', 'sparse', mulberry32);
    expect(sparse.entries.every((e) => e.date >= '2026-09-25')).toBe(true);
    // Deterministisch.
    expect(buildProgress('2026-10-15', 'rich', mulberry32)).toEqual(rich);
  });
});
