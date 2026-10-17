// Trainingskalender je Monat (T35): Plan gegen Ist. Reine Zählung ohne Schwelle: Ein Tag ist
// erledigt, wenn mindestens so viele zählende Aktivitäten wie geplante Einheiten vorliegen,
// teilweise bei weniger, verpasst ohne Aktivität. Heute und Zukunft sind nie verpasst.
// Die Daten kommen aus vorhandenen Befehlen (`list_activities`, aktiver Plan), nichts Neues im
// Rust-Teil.

import { addDays, daysBetween, mondayOf } from './dates';
import type { ActivityItem, Plan, PlanSession } from './backend/types';

export type DayState =
  'done' | 'partial' | 'missed' | 'extra' | 'trained' | 'planned' | 'rest' | 'empty';

export interface CalDay {
  date: string;
  inMonth: boolean;
  isToday: boolean;
  /** Nur zählende Aktivitäten (SPEC 6.4), in der Reihenfolge der Liste. */
  activities: ActivityItem[];
  /** Geplante Einheiten ohne Ruhetage, Vormittag vor Nachmittag. */
  planned: PlanSession[];
  /** Der Tag gehört zu einer Planwoche. */
  covered: boolean;
  state: DayState;
}

export interface CalWeek {
  start: string;
  days: CalDay[];
  /** Summe der berechenbaren Lasten; Einheiten ohne Last fehlen darin. */
  load: number;
  durationS: number;
  plannedLoad: number;
  plannedMin: number;
}

/** Erster und letzter Tag des Rasters: ganze Wochen von Montag bis Sonntag um den Monat. */
export function monthRange(month: string): { first: string; last: string } {
  const start = `${month}-01`;
  const y = Number(month.slice(0, 4));
  const m = Number(month.slice(5, 7));
  const next =
    m === 12 ? `${y + 1}-01-01` : `${month.slice(0, 5)}${String(m + 1).padStart(2, '0')}-01`;
  const end = addDays(next, -1);
  return { first: mondayOf(start), last: addDays(mondayOf(end), 6) };
}

/** Monat davor oder danach, `YYYY-MM`. */
export function shiftMonth(month: string, delta: number): string {
  const index = Number(month.slice(0, 4)) * 12 + Number(month.slice(5, 7)) - 1 + delta;
  return `${String(Math.floor(index / 12)).padStart(4, '0')}-${String((index % 12) + 1).padStart(2, '0')}`;
}

export function dayState(
  date: string,
  today: string,
  planned: number,
  done: number,
  covered: boolean,
): DayState {
  if (date > today) return planned > 0 ? 'planned' : 'empty';
  if (planned > 0) {
    if (done >= planned) return 'done';
    if (done > 0) return 'partial';
    return date === today ? 'planned' : 'missed';
  }
  if (done > 0) return covered ? 'extra' : 'trained';
  return covered ? 'rest' : 'empty';
}

/** Geplante Einheiten eines Tages ohne Ruhetage und ob der Tag zu einer Planwoche gehört. */
export function plannedOn(
  plan: Plan | null,
  date: string,
): { sessions: PlanSession[]; covered: boolean } {
  if (!plan) return { sessions: [], covered: false };
  for (const w of plan.weeks) {
    for (const d of w.days) {
      if (d.date === date) {
        const sessions = d.sessions
          .filter((s) => s.sport !== 'rest')
          .sort((a, b) => (a.slot === b.slot ? 0 : a.slot === 'am' ? -1 : 1));
        return { sessions, covered: true };
      }
    }
    // Planwochen decken alle sieben Tage, auch ohne Eintrag für einen Ruhetag.
    if (date >= w.week_start && daysBetween(w.week_start, date) < 7) {
      return { sessions: [], covered: true };
    }
  }
  return { sessions: [], covered: false };
}

/** Wochen des Monats mit Tagen, Zuständen und Summen. */
export function buildMonth(
  month: string,
  today: string,
  activities: ActivityItem[],
  plan: Plan | null,
): CalWeek[] {
  const { first, last } = monthRange(month);
  const byDate = new Map<string, ActivityItem[]>();
  for (const a of activities) {
    if (!a.is_primary) continue;
    const list = byDate.get(a.local_date) ?? [];
    list.push(a);
    byDate.set(a.local_date, list);
  }
  const weeks: CalWeek[] = [];
  for (let start = first; start <= last; start = addDays(start, 7)) {
    const week: CalWeek = {
      start,
      days: [],
      load: 0,
      durationS: 0,
      plannedLoad: 0,
      plannedMin: 0,
    };
    for (let i = 0; i < 7; i++) {
      const date = addDays(start, i);
      const acts = byDate.get(date) ?? [];
      const { sessions, covered } = plannedOn(plan, date);
      for (const a of acts) {
        if (a.load.status === 'ok' && a.load.value !== null) week.load += a.load.value;
        week.durationS += a.duration_s;
      }
      for (const s of sessions) {
        week.plannedLoad += s.planned_load;
        week.plannedMin += s.duration_min;
      }
      week.days.push({
        date,
        inMonth: date.slice(0, 7) === month,
        isToday: date === today,
        activities: acts,
        planned: sessions,
        covered,
        state: dayState(date, today, sessions.length, acts.length, covered),
      });
    }
    weeks.push(week);
  }
  return weeks;
}

export interface MonthCounts {
  /** Geplante Einheiten an Tagen bis gestern (heute zählt noch nicht als offen). */
  plannedPast: number;
  donePast: number;
  missedDays: number;
  sessions: number;
}

/** Zählung für die Aussage, nur Tage des Monats. */
export function monthCounts(weeks: CalWeek[], today: string): MonthCounts {
  const out: MonthCounts = { plannedPast: 0, donePast: 0, missedDays: 0, sessions: 0 };
  for (const w of weeks) {
    for (const d of w.days) {
      if (!d.inMonth) continue;
      out.sessions += d.activities.length;
      if (d.date < today && d.planned.length > 0) {
        out.plannedPast += d.planned.length;
        out.donePast += Math.min(d.activities.length, d.planned.length);
      }
      if (d.state === 'missed') out.missedDays += 1;
    }
  }
  return out;
}
