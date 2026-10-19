// Hilfen für die Planansicht: Texte zu Schlüsseln aus dem Plan, Summen, Zusammenfassungen.
// Reine Funktionen ohne DOM. Texte aus Plänen sind Daten: Die Oberfläche zeigt sie als Text
// (Svelte maskiert sie), nie als HTML (SPEC 9.5). Fehlt ein Schlüssel in der Sprachdatei,
// erscheint der Schlüssel selbst, sichtbar und nie leer.

import { addDays, localIso, mondayOf } from './dates';
import type {
  BackupCounts,
  Locale,
  Plan,
  PlanGoal,
  PlanSession,
  PlanWarning,
  PlanWeek,
  Violation,
} from './backend/types';
import { hasKey, translate, type Translate } from './i18n';

/** Wochentage, Montag zuerst, wie `availability.weekday` (0 = Montag). */
export const WEEKDAYS = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'] as const;

/** Text zu einem Schlüssel der Sprachdateien oder der Schlüssel selbst. */
export function keyText(locale: Locale, key: string): string {
  return hasKey(locale, key) ? translate(locale, key) : key;
}

/** Name der Vorlage einer Einheit (`template.<id>.name`). Fehlt er (Vorlage aus einem neueren
 *  Katalog), steht die ID mit Leerzeichen statt Punkten und Unterstrichen, damit sie umbricht
 *  und nicht als roher Schlüssel erscheint. */
export function templateName(locale: Locale, templateId: string): string {
  const key = `template.${templateId}.name`;
  return hasKey(locale, key) ? translate(locale, key) : templateId.replace(/[._]+/g, ' ');
}

/** Beschreibung der Vorlage, falls es eine gibt. */
export function templateDescription(locale: Locale, templateId: string): string | null {
  const key = `template.${templateId}.description`;
  return hasKey(locale, key) ? translate(locale, key) : null;
}

const RULE_CODE = /^R\d{2}$/;

/**
 * Name und Erklärung einer Regel. Die Erklärung nennt die Quelle ("Quelle: Faustregel" oder
 * eine Quellennummer aus SPEC Anhang A). Eine unbekannte Regel liefert nur den Code.
 */
export function ruleInfo(
  locale: Locale,
  code: string,
): { name: string; explanation: string | null } {
  if (RULE_CODE.test(code) && hasKey(locale, `rule.${code}.name`)) {
    const explKey = `rule.${code}.explanation`;
    return {
      name: translate(locale, `rule.${code}.name`),
      explanation: hasKey(locale, explKey) ? translate(locale, explKey) : null,
    };
  }
  return { name: translate(locale, 'ui.plan.warnings.unknown_rule', { code }), explanation: null };
}

export function sessionsOf(week: PlanWeek): PlanSession[] {
  return week.days.flatMap((d) => d.sessions);
}

export function weekMinutes(week: PlanWeek): number {
  return sessionsOf(week).reduce((s, x) => s + x.duration_min, 0);
}

/** Geplante Last der Woche: das Feld `planned_load`, sonst die Summe der Einheiten. */
export function weekPlannedLoad(week: PlanWeek): number {
  return week.planned_load ?? sessionsOf(week).reduce((s, x) => s + x.planned_load, 0);
}

export interface PlanTotals {
  sessions: number;
  minutes: number;
  load: number;
}

export function planTotals(plan: Plan): PlanTotals {
  let sessions = 0;
  let minutes = 0;
  let load = 0;
  for (const w of plan.weeks) {
    const list = sessionsOf(w);
    sessions += list.length;
    minutes += weekMinutes(w);
    load += weekPlannedLoad(w);
  }
  return { sessions, minutes, load };
}

/** Warnungen nach Datum (ohne Datum zuerst) und Code. */
export function sortedWarnings(warnings: readonly PlanWarning[]): PlanWarning[] {
  return [...warnings].sort(
    (a, b) => (a.date ?? '').localeCompare(b.date ?? '') || a.code.localeCompare(b.code),
  );
}

export function hardViolations(violations: readonly Violation[]): Violation[] {
  return violations.filter((v) => v.hard);
}

export function softViolations(violations: readonly Violation[]): Violation[] {
  return violations.filter((v) => !v.hard);
}

/** Der erste Montag nach heute: sinnvoller Beginn für einen neuen Plan. */
export function nextMonday(today: string = localIso()): string {
  return addDays(mondayOf(today), 7);
}

/** Beschreibung eines Ziels, zum Beispiel "Termin 01.05.2027, Rad, Priorität A". */
export function describeGoal(
  t: Translate,
  goal: {
    kind: PlanGoal['kind'];
    event_date?: string | null;
    sport?: PlanGoal['sport'] | null;
    priority?: PlanGoal['priority'] | null;
  },
  formatDate: (iso: string) => string,
): string {
  if (goal.kind !== 'event' || !goal.event_date) return t('ui.plan.goal.continuous');
  const date = formatDate(goal.event_date);
  if (goal.sport && goal.priority) {
    return t('ui.plan.goal.event', {
      date,
      sport: t(`sport.${goal.sport}`),
      priority: goal.priority,
    });
  }
  return t('ui.plan.goal.event_plain', { date });
}

/** Zeilenzahlen eines Backups als Satzteil. */
export function formatCounts(t: Translate, c: BackupCounts): string {
  return t('ui.export.counts', {
    thresholds: c.thresholds,
    goals: c.goals,
    availability: c.availability,
    plans: c.plans,
    settings: c.settings,
    recovery_days: c.recovery_days,
  });
}
