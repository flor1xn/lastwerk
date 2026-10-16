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
import type { HealthPage, HealthPoint, HealthRange, SensitiveCategory } from './types';

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
    floors_up: null,
    floors_down: null,
    training_status: null,
    garmin_chronic_load: null,
    garmin_acwr: null,
    load_aerobic_low: null,
    load_aerobic_low_min: null,
    load_aerobic_low_max: null,
    load_aerobic_high: null,
    load_aerobic_high_min: null,
    load_aerobic_high_max: null,
    load_anaerobic: null,
    load_anaerobic_min: null,
    load_anaerobic_max: null,
    load_balance: null,
    race_5k_s: null,
    race_10k_s: null,
    race_half_s: null,
    race_marathon_s: null,
    run_acute_distance_km: null,
    run_impact_load: null,
    run_tolerance_km: null,
    run_tolerance_level: null,
    weight_kg: null,
    bmi: null,
    body_fat_pct: null,
    body_water_pct: null,
    bone_kg: null,
    muscle_kg: null,
    systolic_mmhg: null,
    diastolic_mmhg: null,
    pulse_bpm: null,
    calories_kcal: null,
    carbs_g: null,
    fat_g: null,
    protein_g: null,
    calories_goal_kcal: null,
  };
}

const STATUSES = ['productive', 'maintaining', 'recovery', 'peaking'];

function hasValue(p: HealthPoint): boolean {
  return Object.entries(p).some(([k, v]) => k !== 'date' && v !== null);
}

export function buildHealth(
  today: string,
  days: HealthRange,
  variant: DashboardVariant,
  released: readonly SensitiveCategory[],
  stored: ReadonlySet<SensitiveCategory>,
  mulberry32: (seed: number) => () => number,
): HealthPage {
  const rnd = mulberry32(3901);
  // Eigene Folge für die Werte aus T40, damit die älteren Werte gleich bleiben.
  const more = mulberry32(4001);
  const s3 = (c: SensitiveCategory) => released.includes(c) && stored.has(c);
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
      if (s3('hydration') && hydrationDraw < 0.8) {
        p.hydration_ml = extreme ? 9850 : Math.round(1800 + rnd() * 1400);
      }
      p.floors_up = extreme ? 1840 : Math.round(more() * 18);
      p.floors_down = extreme ? 1790 : Math.round(more() * 16);
      p.training_status =
        extreme && i === 2 ? 'unknown_new_status' : STATUSES[Math.floor(i / 9) % STATUSES.length];
      p.garmin_chronic_load = extreme ? 2310 : Math.round(380 + wave * 150);
      p.garmin_acwr =
        Math.round(((p.garmin_acute_load ?? 0) / (p.garmin_chronic_load || 1)) * 100) / 100;
      p.run_acute_distance_km = extreme ? 412.6 : r1(28 + more() * 12);
      p.run_impact_load = extreme ? 980000 : Math.round(31000 + more() * 9000);
      p.run_tolerance_km = extreme ? 999.9 : r1(58 + more() * 6);
      p.run_tolerance_level = (p.run_acute_distance_km ?? 0) > 36 ? 'medium_load' : 'low_load';
      if (s3('nutrition') && more() < 0.85) {
        p.calories_kcal = extreme ? 19800 : Math.round(2200 + more() * 600);
        p.carbs_g = extreme ? 2950 : Math.round(260 + more() * 80);
        p.fat_g = extreme ? 990 : Math.round(70 + more() * 25);
        p.protein_g = extreme ? 880 : Math.round(110 + more() * 30);
        p.calories_goal_kcal = 2600;
      }
      if (i === 0) {
        p.load_aerobic_low = extreme ? 9800 : 310;
        p.load_aerobic_low_min = 186;
        p.load_aerobic_low_max = 420;
        p.load_aerobic_high = extreme ? 9900 : 420;
        p.load_aerobic_high_min = 240;
        p.load_aerobic_high_max = 520;
        p.load_anaerobic = extreme ? 0 : 60;
        p.load_anaerobic_min = 80;
        p.load_anaerobic_max = 230;
        p.load_balance = extreme ? 'unknown_new_balance' : 'anaerobic_shortage';
      }
      if (i % 7 === 1) {
        p.race_5k_s = extreme ? 86399 : Math.round(1320 - (28 - i) * 2);
        p.race_10k_s = extreme ? 301 : Math.round(2760 - (28 - i) * 4);
        p.race_half_s = Math.round(6120 - (28 - i) * 9);
        p.race_marathon_s = Math.round(12900 - (28 - i) * 20);
      }
    }
    // Körper und Blutdruck: Messungen an manchen Tagen über das ganze Jahr (S3).
    if (!none && s3('body') && i % 4 === 0) {
      p.weight_kg = extreme ? 399.9 : r1(74 - (364 - i) / 200 + (more() - 0.5));
      p.bmi = extreme ? 99.9 : r1(23 - (364 - i) / 600);
      p.body_fat_pct = extreme ? 99.9 : r1(16 - (364 - i) / 400);
      p.body_water_pct = r1(58 + more());
      p.bone_kg = r1(3.4 + more() * 0.2);
      p.muscle_kg = r1(33 + (364 - i) / 500);
    }
    if (!none && s3('blood_pressure') && i % 6 === 0) {
      p.systolic_mmhg = extreme ? 299 : Math.round(118 + more() * 10);
      p.diastolic_mmhg = extreme ? 199 : Math.round(76 + more() * 8);
      p.pulse_bpm = Math.round(52 + more() * 10);
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
  return { days, points, sensitive_released: released.length > 0, released: [...released] };
}
