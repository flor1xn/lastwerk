import { describe, expect, it } from 'vitest';
import { mulberry32 } from './mock';
import { buildRecovery } from './mock-recovery';

describe('Erholung der Attrappe (T34)', () => {
  it('liefert je Tag des Zeitraums einen Punkt mit Basis und Untergrenze nach R12', () => {
    const page = buildRecovery('2026-10-15', 90, 'rich', mulberry32);
    expect(page.points).toHaveLength(90);
    expect(page.points[89].date).toBe('2026-10-15');
    for (const p of page.points) {
      if (p.hrv_base !== null) expect(p.hrv_floor).toBeCloseTo(p.hrv_base * 0.85, 0);
      if (p.sleep_base_h !== null) expect(p.sleep_floor_h).toBeCloseTo(p.sleep_base_h * 0.8, 1);
      if (p.low) expect(p.hrv_ms !== null || p.sleep_h !== null).toBe(true);
    }
    expect(page.low_days).toBe(page.points.filter((p) => p.low).length);
  });

  it('kennt leere und niedrige Varianten', () => {
    const empty = buildRecovery('2026-10-15', 30, 'empty', mulberry32);
    expect(empty.last_date).toBeNull();
    expect(empty.state).toBe('readiness.no_data');
    expect(empty.points.every((p) => p.hrv_ms === null)).toBe(true);
    expect(buildRecovery('2026-10-15', 30, 'low', mulberry32).state).toBe('readiness.low');
  });
});
