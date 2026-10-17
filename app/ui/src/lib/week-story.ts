// Wochen-Story auf "Heute" (T44): diese Woche erledigt gegen geplant und der Blick auf die
// nächsten sieben Tage des Plans. Reine Zählung wie der Kalender (T35, `lib/calendar.ts`):
// keine Schwelle, heute und Zukunft sind nie verpasst, eine Aktivität erledigt höchstens eine
// geplante Einheit desselben Tages. Lasten sind Summen der berechenbaren Werte.

import { dayState, plannedOn, type DayState } from './calendar';
import { addDays, mondayOf } from './dates';
import type { ActivityItem, Plan } from './backend/types';

export interface StoryDay {
  date: string;
  state: DayState;
  /** Geplante Einheiten ohne Ruhetage. */
  planned: number;
  /** Zählende Aktivitäten (SPEC 6.4). */
  done: number;
  /** Mindestens eine geplante Einheit ist hart. */
  hard: boolean;
}

export interface WeekOutlook {
  /** Tage der nächsten sieben, die zu einer Planwoche gehören. */
  covered: number;
  hardDays: number;
  /** Tage im Plan ohne Einheit. */
  restDays: number;
  /** Erster Tag mit harter Einheit, falls vorhanden. */
  nextHard: string | null;
}

export interface WeekStory {
  weekStart: string;
  days: StoryDay[];
  /** Mindestens ein Tag dieser Woche gehört zum Plan. */
  hasPlan: boolean;
  /** Geplante Einheiten an Tagen bis gestern und davon erledigte. */
  plannedPast: number;
  donePast: number;
  missedDays: number;
  /** Alle geplanten Einheiten der Woche. */
  plannedSessions: number;
  /** Zählende Aktivitäten bis heute. */
  sessions: number;
  /** Last der Aktivitäten bis heute, nur berechenbare Werte. */
  load: number;
  /** Geplante Last der ganzen Woche. */
  plannedLoad: number;
  outlook: WeekOutlook;
}

function countable(activities: ActivityItem[]): Map<string, ActivityItem[]> {
  const byDate = new Map<string, ActivityItem[]>();
  for (const a of activities) {
    if (!a.is_primary) continue;
    const list = byDate.get(a.local_date) ?? [];
    list.push(a);
    byDate.set(a.local_date, list);
  }
  return byDate;
}

/** Die Woche von `today` (Montag bis Sonntag) und die sieben Tage nach heute. */
export function buildWeekStory(
  today: string,
  activities: ActivityItem[],
  plan: Plan | null,
): WeekStory {
  const weekStart = mondayOf(today);
  const byDate = countable(activities);
  const story: WeekStory = {
    weekStart,
    days: [],
    hasPlan: false,
    plannedPast: 0,
    donePast: 0,
    missedDays: 0,
    plannedSessions: 0,
    sessions: 0,
    load: 0,
    plannedLoad: 0,
    outlook: { covered: 0, hardDays: 0, restDays: 0, nextHard: null },
  };
  for (let i = 0; i < 7; i++) {
    const date = addDays(weekStart, i);
    const acts = date <= today ? (byDate.get(date) ?? []) : [];
    const { sessions, covered } = plannedOn(plan, date);
    const state = dayState(date, today, sessions.length, acts.length, covered);
    story.hasPlan ||= covered;
    story.plannedSessions += sessions.length;
    story.plannedLoad += sessions.reduce((s, x) => s + x.planned_load, 0);
    story.sessions += acts.length;
    for (const a of acts) {
      if (a.load.status === 'ok' && a.load.value !== null) story.load += a.load.value;
    }
    if (date < today && sessions.length > 0) {
      story.plannedPast += sessions.length;
      story.donePast += Math.min(acts.length, sessions.length);
    }
    if (state === 'missed') story.missedDays += 1;
    story.days.push({
      date,
      state,
      planned: sessions.length,
      done: acts.length,
      hard: sessions.some((s) => s.hard),
    });
  }
  for (let i = 1; i <= 7; i++) {
    const date = addDays(today, i);
    const { sessions, covered } = plannedOn(plan, date);
    if (!covered) continue;
    story.outlook.covered += 1;
    if (sessions.length === 0) story.outlook.restDays += 1;
    if (sessions.some((s) => s.hard)) {
      story.outlook.hardDays += 1;
      story.outlook.nextHard ??= date;
    }
  }
  return story;
}
