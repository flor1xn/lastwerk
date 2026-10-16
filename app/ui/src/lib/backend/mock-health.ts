// Seite "Gesundheit" der Attrappe (T39): erfundene, deterministische Tages- und Trainingswerte.
// Das Muster folgt dem, was Garmin liefert (SPEC 5.6): Tageswerte, Bereitschaft und akute Last
// nur für die letzten 28 Tage, VO2max und Fitnessalter nur an Tagen eines Abgleichs (hier
// wöchentlich), Ausdauer- und Bergwert wöchentlich. SpO2 fehlt in manchen Nächten. Fehlende
// Werte sind null, nie 0; Tage ganz ohne Wert fehlen.
//
// Varianten wie beim Dashboard: `empty` und `gaps` ohne Werte, `sparse` nur zwei Wochen
// Tageswerte ohne wöchentliche, `extreme` große Werte an den Grenzen dessen, was Garmin meldet.

import { addDays } from '../dates';
import type { DashboardVariant } from './mock-dashboard';
import type { HealthPage, HealthPoint, HealthRange } from './types';

/** Tage mit Tageswerten (Garmin liefert sie im Abgleich für 28 Tage). */
const DAILY_DAYS = 28;
const LEVELS = ['POOR', 'LOW', 'MODERATE', 'HIGH', 'PRIME'];

const r1 = (v: number) => Math.round(v * 10) / 10;

function emptyPoint(date: string): HealthPoint {
  return {
    date,
    steps: null,
    step_distance_km: null,
    hr_min_bpm: null,
    hr_max_bpm: null,
    respiration_waking_brpm: null,
    respiration_sleep_brpm: null,
    spo2_avg_pct: null,
    spo2_low_pct: null,
    intensity_moderate_min: null,
    intensity_vigorous_min: null,
    stress_avg: null,
    stress_max: null,
    readiness_score: null,
    readiness_level: null,
    garmin_acute_load: null,
    fitness_age: null,
    vo2max_running: null,
    vo2max_cycling: null,
    endurance_score: null,
    hill_score: null,
    hydration_ml: null,
  };
}

function hasValue(p: HealthPoint): boolean {
  return Object.entries(p).some(([k, v]) => k !== 'date' && v !== null);
}

export function buildHealth(
  today: string,
  days: HealthRange,
  variant: DashboardVariant,
  sensitiveReleased: boolean,
  hydrationStored: boolean,
  mulberry32: (seed: number) => () => number,
): HealthPage {
  const rnd = mulberry32(3901);
  const points: HealthPoint[] = [];
  const none = variant === 'empty' || variant === 'gaps';
  const extreme = variant === 'extreme';
  const dailyDays = variant === 'sparse' ? 14 : DAILY_DAYS;
  // Immer ein Jahr erzeugen, damit die Werte nicht vom Zeitraum abhängen.
  for (let i = 364; i >= 0; i--) {
    const date = addDays(today, -i);
    const p = emptyPoint(date);
    const wave = Math.sin(i / 9) * 0.15;
    const daily = !none && i < dailyDays && rnd() >= 0.05;
    if (daily) {
      const steps = extreme ? 64812 - i * 13 : Math.round(9000 * (1 + wave) + (rnd() - 0.5) * 4000);
      p.steps = steps;
      p.step_distance_km = r1(steps * 0.00078);
      p.hr_min_bpm = extreme ? 31 : Math.round(46 + rnd() * 5);
      p.hr_max_bpm = extreme ? 212 : Math.round(150 + rnd() * 30);
      p.respiration_waking_brpm = r1(14 + rnd() * 3);
      p.respiration_sleep_brpm = r1(12 + rnd() * 2);
      if (rnd() < 0.7) {
        p.spo2_avg_pct = extreme ? 100 : Math.round(94 + rnd() * 4);
        p.spo2_low_pct = extreme ? 61 : Math.round(86 + rnd() * 6);
      }
      p.intensity_moderate_min = extreme ? 1440 : Math.round(rnd() * 60);
      p.intensity_vigorous_min = extreme ? 980 : Math.round(rnd() * 45);
      p.stress_avg = extreme ? 99 : Math.round(25 + wave * 40 + rnd() * 10);
      p.stress_max = extreme ? 100 : Math.round(70 + rnd() * 29);
      const score = extreme ? (i % 2 === 0 ? 1 : 100) : Math.round(60 + wave * 120 + rnd() * 10);
      p.readiness_score = Math.max(1, Math.min(100, score));
      p.readiness_level =
        extreme && i === 1
          ? 'UNKNOWN_NEW_LEVEL'
          : LEVELS[Math.min(4, Math.floor(p.readiness_score / 20))];
      p.garmin_acute_load = extreme ? 2480 : Math.round(420 + wave * 300 + rnd() * 80);
      // Immer eine Zufallszahl ziehen, damit die Freigabe die übrigen Werte nicht verschiebt.
      const hydrationDraw = rnd();
      if (sensitiveReleased && hydrationStored && hydrationDraw < 0.8) {
        p.hydration_ml = extreme ? 9850 : Math.round(1800 + rnd() * 1400);
      }
    }
    // Wöchentliche Werte: Abgleich am selben Wochentag, Ausdauer und Berg ebenso.
    if (!none && variant !== 'sparse' && i % 7 === 3) {
      p.vo2max_running = extreme ? 88.4 : r1(52 + (364 - i) / 120 + (rnd() - 0.5));
      p.vo2max_cycling = rnd() < 0.85 ? (extreme ? 91.2 : r1(55 + (364 - i) / 150)) : null;
      p.fitness_age = extreme ? 20 : Math.round(34 - (364 - i) / 180);
    }
    if (!none && variant !== 'sparse' && i % 7 === 0) {
      p.endurance_score = extreme ? 13640 : Math.round(6200 + (364 - i) * 3 + rnd() * 120);
      p.hill_score = rnd() < 0.9 ? (extreme ? 100 : Math.round(55 + (364 - i) / 40)) : null;
    }
    if (i < days && hasValue(p)) points.push(p);
  }
  return { days, points, sensitive_released: sensitiveReleased };
}
