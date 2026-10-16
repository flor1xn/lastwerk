// Planer mit drei Fragen (T26, SPEC 7.2, O31): Verteilung der Wochenzeit auf Tage und die
// Vorbelegung der Fragen aus Ziel und Verfügbarkeit. Reine Funktionen ohne DOM.
//
// Die Verteilung steht maßgeblich in app/src-tauri/src/service/quick.rs
// (`distribute_minutes`). Diese Fassung gibt dieselben Minuten, damit die Oberfläche sie anzeigen
// kann und die Attrappe wie der Rust-Teil verteilt; die Tests halten dieselben Handrechnungen.

import type { AvailabilityDay, EnduranceSport, PlanningInputs } from './backend/types';

/** Grenzen des Schiebers (Stunden je Woche, Schritt eine halbe Stunde). */
export const SLIDER_MIN = 2;
export const SLIDER_MAX = 20;
export const SLIDER_STEP = 0.5;

/** Grenzen, die der Rust-Teil prüft. */
export const MIN_HOURS = 1;
export const MAX_HOURS = 30;

/** Kleinste Einheit des Planers in Minuten (`structure.min_session_minutes`). */
export const MIN_SESSION_MINUTES = 20;
/** Längster Tag in Minuten. */
export const MAX_DAY_MINUTES = 1440;
/** Dauerstufe des Planers in Minuten (`structure.duration_step_min`). */
const STEP_MIN = 5;

/** Standardantworten ohne gespeicherte Verfügbarkeit: Di, Do, Sa, So und 6 Stunden. */
export const DEFAULT_DAYS = [1, 3, 5, 6];
export const DEFAULT_HOURS = 6;

export type DistributionError = 'invalid_value' | 'too_little_time';

/**
 * Verteilt die Wochenzeit gleichmäßig auf die gewählten Tage (O31, Faustregel), auf 5 Minuten
 * gerundet. Ein Rest von höchstens 5 Minuten je Tag geht an die späteren Tage, die Summe bleibt
 * die auf 5 Minuten gerundete Wochenzeit. Gewählte Tage erlauben harte Einheiten, die übrigen
 * sind feste Ruhetage mit 0 Minuten. Fehler als Code: `invalid_value` (Stunden außerhalb 1 bis
 * 30, kein Tag, doppelter oder unbekannter Tag, mehr als ein Tag hat) oder `too_little_time`
 * (weniger als die kleinste Einheit je Tag).
 */
export function distributeMinutes(
  hours: number,
  days: readonly number[],
): AvailabilityDay[] | DistributionError {
  if (!Number.isFinite(hours) || hours < MIN_HOURS || hours > MAX_HOURS) return 'invalid_value';
  const chosen = [...days].sort((a, b) => a - b);
  if (chosen.length === 0) return 'invalid_value';
  for (let i = 0; i < chosen.length; i++) {
    const d = chosen[i];
    if (!Number.isInteger(d) || d < 0 || d > 6 || chosen[i - 1] === d) return 'invalid_value';
  }
  const n = chosen.length;
  const total = Math.round((hours * 60) / STEP_MIN) * STEP_MIN;
  const base = Math.floor(total / n / STEP_MIN) * STEP_MIN;
  const extra = (total - base * n) / STEP_MIN;
  const firstExtra = n - extra;
  const out: AvailabilityDay[] = [];
  for (let weekday = 0; weekday < 7; weekday++) {
    const i = chosen.indexOf(weekday);
    if (i < 0) {
      out.push({ weekday, max_minutes: 0, allow_hard: false, fixed_rest: true });
      continue;
    }
    const minutes = base + (i >= firstExtra ? STEP_MIN : 0);
    if (minutes < MIN_SESSION_MINUTES) return 'too_little_time';
    if (minutes > MAX_DAY_MINUTES) return 'invalid_value';
    out.push({ weekday, max_minutes: minutes, allow_hard: true, fixed_rest: false });
  }
  return out;
}

/** Minuten je Trainingstag für die Anzeige ("rund 90 Minuten"). 0, wenn kein Tag gewählt ist. */
export function minutesPerDay(hours: number, dayCount: number): number {
  if (dayCount <= 0) return 0;
  return Math.round((hours * 60) / dayCount / STEP_MIN) * STEP_MIN;
}

/** Stunden auf den Schieber legen: halbe Stunden, zwischen 2 und 20. */
export function clampHours(hours: number): number {
  const stepped = Math.round(hours / SLIDER_STEP) * SLIDER_STEP;
  return Math.min(SLIDER_MAX, Math.max(SLIDER_MIN, stepped));
}

export interface Answers {
  kind: 'event' | 'continuous';
  date: string;
  sport: EnduranceSport;
  hours: number;
  days: number[];
}

/**
 * Vorbelegung der drei Fragen aus dem aktiven Ziel und der gespeicherten Verfügbarkeit. Ist die
 * Verfügbarkeit nicht gesetzt (`availability_set` 0), gilt der Vorschlag aus der Historie (T41),
 * ohne ihn die Standardantworten. Sonst sind die Tage mit Zeit und ohne festen Ruhetag die
 * Trainingstage, und die Stunden sind ihre Summe, auf halbe Stunden gelegt.
 */
export function defaultAnswers(inputs: PlanningInputs | null): Answers {
  const goal = inputs?.goals.find((g) => g.active) ?? null;
  const history = inputs?.history ?? null;
  const sport: EnduranceSport =
    goal?.sport === 'running' || goal?.sport === 'swimming' || goal?.sport === 'cycling'
      ? goal.sport
      : (history?.sport ?? 'cycling');
  const answers: Answers = {
    kind: goal?.kind === 'event' && goal.event_date ? 'event' : 'continuous',
    date: goal?.kind === 'event' && goal.event_date ? goal.event_date : '',
    sport,
    hours: DEFAULT_HOURS,
    days: [...DEFAULT_DAYS],
  };
  if (history && (!inputs || inputs.availability_set === 0) && history.days.length > 0) {
    answers.days = [...history.days];
    answers.hours = clampHours(history.hours);
  }
  if (inputs && inputs.availability_set > 0) {
    const training = inputs.availability.filter((d) => !d.fixed_rest && d.max_minutes > 0);
    if (training.length > 0) {
      answers.days = training.map((d) => d.weekday);
      answers.hours = clampHours(training.reduce((sum, d) => sum + d.max_minutes, 0) / 60);
    }
  }
  return answers;
}
