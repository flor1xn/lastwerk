import { describe, expect, it } from 'vitest';
import type { PlanningInputs } from './backend/types';
import { clampHours, defaultAnswers, distributeMinutes, minutesPerDay } from './planner';

const minutes = (r: ReturnType<typeof distributeMinutes>) =>
  typeof r === 'string' ? r : r.map((d) => d.max_minutes);

// Dieselben Handrechnungen wie in app/src-tauri/src/service/quick.rs (O31).
describe('distributeMinutes', () => {
  it('teilt 6 Stunden auf 4 Tage: 360 / 4 = 90 je Tag, übrige Tage Ruhetag', () => {
    const d = distributeMinutes(6, [0, 2, 4, 6]);
    expect(minutes(d)).toEqual([90, 0, 90, 0, 90, 0, 90]);
    if (typeof d === 'string') throw new Error(d);
    expect(d.map((x) => x.fixed_rest)).toEqual([false, true, false, true, false, true, false]);
    expect(d.filter((x) => !x.fixed_rest).every((x) => x.allow_hard)).toBe(true);
    expect(d.filter((x) => x.fixed_rest).every((x) => !x.allow_hard)).toBe(true);
  });

  it('legt den Rest auf die späteren Tage: 330 / 4 = 82,5 -> 80, Rest 10 Minuten', () => {
    // Sortiert Mo, Di, Do, Sa: 80, 80, 85, 85. Summe 330.
    const d = distributeMinutes(5.5, [3, 0, 5, 1]);
    expect(minutes(d)).toEqual([80, 80, 0, 85, 0, 85, 0]);
    expect((minutes(d) as number[]).reduce((a, b) => a + b, 0)).toBe(330);
  });

  it('rundet ungerade Stunden zuerst auf 5 Minuten: 3,33 h = 199,8 -> 200, 100 je Tag', () => {
    expect(minutes(distributeMinutes(3.33, [1, 4]))).toEqual([0, 100, 0, 0, 100, 0, 0]);
  });

  it('lehnt Grenzen, Doppel und zu wenig Zeit ab', () => {
    expect(distributeMinutes(0.99, [0])).toBe('invalid_value');
    expect(distributeMinutes(30.1, [0, 1])).toBe('invalid_value');
    expect(distributeMinutes(Number.NaN, [0])).toBe('invalid_value');
    expect(distributeMinutes(5, [])).toBe('invalid_value');
    expect(distributeMinutes(5, [7])).toBe('invalid_value');
    expect(distributeMinutes(5, [1, 1])).toBe('invalid_value');
    // 60 / 7 = 8,57 -> 5 Minuten, unter der kleinsten Einheit von 20.
    expect(distributeMinutes(1, [0, 1, 2, 3, 4, 5, 6])).toBe('too_little_time');
    // 1800 Minuten auf einen Tag sind mehr, als der Tag hat.
    expect(distributeMinutes(30, [0])).toBe('invalid_value');
    expect(typeof distributeMinutes(1, [0])).toBe('object');
    expect(typeof distributeMinutes(30, [0, 1])).toBe('object');
  });
});

describe('Anzeige der Verteilung', () => {
  it('rechnet Minuten je Tag auf 5 gerundet und 0 ohne Tag', () => {
    expect(minutesPerDay(6, 4)).toBe(90);
    expect(minutesPerDay(5.5, 4)).toBe(85);
    expect(minutesPerDay(6, 0)).toBe(0);
  });

  it('legt Stunden auf halbe Stunden zwischen 2 und 20', () => {
    expect(clampHours(1)).toBe(2);
    expect(clampHours(25)).toBe(20);
    expect(clampHours(6.3)).toBe(6.5);
    expect(clampHours(6.2)).toBe(6);
  });
});

function inputs(over: Partial<PlanningInputs> = {}): PlanningInputs {
  return {
    goals: [],
    availability: Array.from({ length: 7 }, (_, weekday) => ({
      weekday,
      max_minutes: 90,
      allow_hard: true,
      fixed_rest: false,
    })),
    availability_set: 0,
    default_minutes: 90,
    history: null,
    ...over,
  };
}

describe('defaultAnswers', () => {
  it('nimmt ohne Verfügbarkeit den Vorschlag aus der Historie (T41)', () => {
    const history = { sport: 'running' as const, hours: 7.5, days: [0, 2, 4, 6], weeks: 11 };
    expect(defaultAnswers(inputs({ history }))).toMatchObject({
      kind: 'continuous',
      sport: 'running',
      hours: 7.5,
      days: [0, 2, 4, 6],
    });
    // Eine gespeicherte Verfügbarkeit geht vor, die Sportart des Ziels ebenso.
    const set = defaultAnswers(
      inputs({
        history,
        availability_set: 7,
        goals: [
          {
            id: 1,
            kind: 'event',
            sport: 'swimming',
            event_date: '2027-05-01',
            label: null,
            priority: 'A',
            active: true,
          },
        ],
      }),
    );
    expect(set.sport).toBe('swimming');
    expect(set.hours).toBe(10.5);
  });

  it('nimmt ohne gespeicherte Daten Di, Do, Sa, So und 6 Stunden, fortlaufend', () => {
    expect(defaultAnswers(null)).toMatchObject({
      kind: 'continuous',
      hours: 6,
      days: [1, 3, 5, 6],
    });
    expect(defaultAnswers(inputs())).toMatchObject({
      kind: 'continuous',
      hours: 6,
      days: [1, 3, 5, 6],
    });
  });

  it('übernimmt ein aktives Ziel mit Termin und Sportart', () => {
    const a = defaultAnswers(
      inputs({
        goals: [
          {
            id: 1,
            kind: 'event',
            event_date: '2027-05-01',
            sport: 'running',
            priority: 'A',
            label: 'geheim',
            active: true,
          },
        ],
      }),
    );
    expect(a).toMatchObject({ kind: 'event', date: '2027-05-01', sport: 'running' });
    // Die Bezeichnung des Ziels gehört nicht in die Fragen.
    expect(JSON.stringify(a)).not.toContain('geheim');
  });

  it('leitet Tage und Stunden aus der Verfügbarkeit ab', () => {
    // Drei Tage mit 90, 120 und 60 Minuten = 270 Minuten = 4,5 Stunden, übrige Ruhetage.
    const availability = Array.from({ length: 7 }, (_, weekday) => ({
      weekday,
      max_minutes: [90, 0, 120, 0, 60, 0, 0][weekday],
      allow_hard: true,
      fixed_rest: [1, 3, 5, 6].includes(weekday),
    }));
    const a = defaultAnswers(inputs({ availability, availability_set: 7 }));
    expect(a.days).toEqual([0, 2, 4]);
    expect(a.hours).toBe(4.5);
  });
});
