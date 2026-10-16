// Hilfen für die Startseite "Heute": Formzustand aus dem TSB und die Einheiten des Tages.
// Reine Funktionen ohne DOM (SPEC 5.5).

import type { Plan, PlanListItem, PlanSession } from './backend/types';

export const FORM_STATES = ['fresh', 'balanced', 'loaded', 'overloaded'] as const;
export type FormState = (typeof FORM_STATES)[number];

/** Grenzen der Formzustände nach TSB (SPEC 5.5), auch für die Bänder im Verlauf. */
export const FORM_FRESH_FROM = 5;
export const FORM_BALANCED_FROM = -10;
export const FORM_LOADED_FROM = -30;

/**
 * Formzustand aus dem TSB des letzten Tages (SPEC 5.5). Die Grenze -30 ist die von R10, die
 * Grenzen -10 und +5 sind Faustregel ohne begutachtete Quelle.
 */
export function formState(tsb: number): FormState {
  if (tsb >= FORM_FRESH_FROM) return 'fresh';
  if (tsb >= FORM_BALANCED_FROM) return 'balanced';
  if (tsb >= FORM_LOADED_FROM) return 'loaded';
  return 'overloaded';
}

/** ID des aktiven Plans, falls es einen gibt. Höchstens einer ist aktiv. */
export function activePlanId(plans: PlanListItem[]): string | null {
  return plans.find((p) => p.status === 'active')?.id ?? null;
}

/** Einheiten des Plans an einem Tag, Vormittag vor Nachmittag. Leer ohne Einheit oder Tag. */
export function sessionsOn(plan: Plan, date: string): PlanSession[] {
  for (const week of plan.weeks) {
    for (const day of week.days) {
      if (day.date === date) {
        return [...day.sessions].sort((a, b) => (a.slot === b.slot ? 0 : a.slot === 'am' ? -1 : 1));
      }
    }
  }
  return [];
}

/** Liegt der Tag innerhalb der Wochen des Plans? Sonst ist "frei" die falsche Aussage. */
/** Erster Tag des Plans, wenn er nach `date` beginnt (T41: der Plan aus der Einrichtung beginnt
 * am nächsten Montag), sonst null. */
export function planStartsAfter(plan: Plan, date: string): string | null {
  const first = plan.weeks[0]?.days[0]?.date ?? null;
  return first !== null && first > date ? first : null;
}

export function planCovers(plan: Plan, date: string): boolean {
  return plan.weeks.some((w) => w.days.some((d) => d.date === date));
}
