import { describe, expect, it } from 'vitest';
import { MockBackend, mulberry32 } from './mock';
import { buildHealth } from './mock-health';

describe('Gesundheit der Attrappe (T39)', () => {
  it('liefert nur Tage mit Wert, aufsteigend, im Zeitraum', () => {
    const page = buildHealth('2026-10-15', 90, 'rich', false, false, mulberry32);
    expect(page.days).toBe(90);
    expect(page.points.length).toBeGreaterThan(0);
    const dates = page.points.map((p) => p.date);
    expect([...dates].sort()).toEqual(dates);
    expect(dates[0] >= '2026-07-18').toBe(true);
    expect(dates[dates.length - 1] <= '2026-10-15').toBe(true);
    for (const p of page.points) {
      const values = Object.entries(p).filter(([k, v]) => k !== 'date' && v !== null);
      expect(values.length, p.date).toBeGreaterThan(0);
    }
  });

  it('Tageswerte nur in den letzten 28 Tagen, VO2max und Ausdauer spärlich', () => {
    const page = buildHealth('2026-10-15', 365, 'rich', false, false, mulberry32);
    const withSteps = page.points.filter((p) => p.steps !== null);
    expect(withSteps.every((p) => p.date >= '2026-09-18')).toBe(true);
    const vo2 = page.points.filter((p) => p.vo2max_running !== null);
    expect(vo2.length).toBeGreaterThan(40);
    expect(vo2.length).toBeLessThan(60);
    // Ohne Freigabe nie Hydration.
    expect(page.points.every((p) => p.hydration_ml === null)).toBe(true);
    expect(page.sensitive_released).toBe(false);
  });

  it('leer ohne Daten, Hydration nur nach Freigabe und Abgleich', async () => {
    expect(buildHealth('2026-10-15', 30, 'empty', true, true, mulberry32).points).toEqual([]);
    const backend = new MockBackend({ today: '2026-10-15', garminConnected: true });
    expect(await backend.deleteSensitiveData()).toBe(0);
    await backend.setSettings({ garmin_share_hydration: true });
    let page = await backend.getHealth('2026-10-15', 30);
    expect(page.sensitive_released).toBe(true);
    expect(page.points.some((p) => p.hydration_ml !== null)).toBe(false);
    await backend.garminSync('manual', () => {});
    page = await backend.getHealth('2026-10-15', 30);
    const hydrated = page.points.filter((p) => p.hydration_ml !== null).length;
    expect(hydrated).toBeGreaterThan(0);
    expect(await backend.deleteSensitiveData()).toBe(hydrated);
    page = await backend.getHealth('2026-10-15', 30);
    expect(page.points.some((p) => p.hydration_ml !== null)).toBe(false);
  });

  it('lehnt falschen Zeitraum ab', async () => {
    const backend = new MockBackend({ today: '2026-10-15' });
    await expect(backend.getHealth('2026-10-15', 7 as 30)).rejects.toMatchObject({
      code: 'invalid_value',
    });
  });
});
