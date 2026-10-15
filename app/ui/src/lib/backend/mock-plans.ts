// Planung der Attrappe: Ziele, Verfügbarkeit, Pläne und Export mit synthetischen Daten.
//
// Die Attrappe erzeugt einen kleinen, frei erfundenen Plan (2 bis 3 Wochen) aus den Angaben der
// Oberfläche. Sie hält die Form von schema/plan.schema.json ein und nutzt nur Schlüssel, die
// in den Sprachdateien stehen. Die Regeln des echten Planers (lastwerk-core) bildet sie nicht
// nach: Maßgeblich sind R1 bis R11 dort. Gespeichert wird nur im Speicher der Seite.

import { addDays, daysBetween, isIsoDate, mondayOf } from '../dates';
import { distributeMinutes } from '../planner';
import {
  BackendError,
  ENDURANCE_SPORTS,
  type AvailabilityDay,
  type BackupCounts,
  type EnduranceSport,
  type ExportResult,
  type GeneratePlanInput,
  type GoalInput,
  type GoalItem,
  type Phase,
  type Plan,
  type PlanDay,
  type PlanDetail,
  type PlanListItem,
  type PlanSession,
  type PlanSport,
  type PlanStatus,
  type PlanWarning,
  type PlanWeek,
  type PlanningInputs,
  type PmcPoint,
  type PmcSettings,
  type QuickPlanInput,
  type RestoreResult,
  type Violation,
} from './types';

/** Standard des Planers für nicht gesetzte Tage (Konvention, wie `DEFAULT_DAY` in der CLI). */
export const DEFAULT_MINUTES = 90;

/** Ein protokollierter Aufruf eines Befehls. Tests und der Rauchtest lesen ihn aus. */
export interface MockCall {
  command: string;
  args: Record<string, unknown>;
}

/** Was die Planung von der übrigen Attrappe braucht. */
export interface PlanningHost {
  today: string;
  /** Fitness und Ermüdung am Ende des Tages, aus den erfundenen Aktivitäten. */
  pmcAt(date: string): { ctl: number; atl: number };
  /** Zeitkonstanten des Verlaufs aus den Einstellungen (die Startwerte setzt die Planung selbst). */
  pmcSettings(): PmcSettings;
  /** Zahl der Schwellenwerte, für die Zeilenzahlen des Backups. */
  thresholdCount(): number;
  /** Zahl der Einstellungen, für die Zeilenzahlen des Backups. */
  settingCount(): number;
  computePmc(
    daily: ReadonlyMap<string, number>,
    from: string,
    to: string,
    p: PmcSettings,
  ): PmcPoint[];
  log(call: MockCall): void;
}

const err = (code: string) => new BackendError(code, `ui.error.${code}`);

const EASY: Record<EnduranceSport, string> = {
  cycling: 'bike.endurance.steady',
  running: 'run.easy.steady',
  swimming: 'swim.technique.easy',
};
const HARD: Record<EnduranceSport, string> = {
  cycling: 'bike.threshold.2x20',
  running: 'run.intervals.6x3',
  swimming: 'swim.threshold.sets',
};
const LONG: Record<EnduranceSport, string> = {
  cycling: 'bike.endurance.steady',
  running: 'run.long.steady',
  swimming: 'swim.technique.easy',
};

const round1 = (v: number) => Math.round(v * 10) / 10;

/** Plan-ID aus der laufenden Nummer: eine gültige UUID mit festem Anteil. */
const planId = (n: number) => `00000000-0000-4000-8000-${n.toString(16).padStart(12, '0')}`;

interface StoredPlan {
  plan: Plan;
  status: PlanStatus;
  goalId: number | null;
}

export class MockPlanning {
  private plans: StoredPlan[] = [];
  private goals: GoalItem[] = [];
  private nextGoalId = 1;
  private nextPlan = 1;
  private availability = new Map<number, AvailabilityDay>();

  constructor(private readonly host: PlanningHost) {}

  // --- Ziele und Verfügbarkeit ---------------------------------------------------------

  inputs(): PlanningInputs {
    const days: AvailabilityDay[] = [];
    for (let weekday = 0; weekday < 7; weekday++) {
      days.push(
        this.availability.get(weekday) ?? {
          weekday,
          max_minutes: DEFAULT_MINUTES,
          allow_hard: true,
          fixed_rest: false,
        },
      );
    }
    return {
      goals: [...this.goals].reverse().map((g) => ({ ...g })),
      availability: days,
      availability_set: this.availability.size,
      default_minutes: DEFAULT_MINUTES,
    };
  }

  setGoal(input: GoalInput): PlanningInputs {
    if (input.event_date !== null && !isIsoDate(input.event_date)) throw err('invalid_date');
    if (input.sport !== null && !(ENDURANCE_SPORTS as readonly string[]).includes(input.sport)) {
      throw err('invalid_sport');
    }
    const label = input.label?.trim() || null;
    if (label && [...label].length > 200) throw err('invalid_value');
    const event = input.event_date !== null;
    this.goals.forEach((g) => (g.active = false));
    this.goals.push({
      id: this.nextGoalId++,
      kind: event ? 'event' : 'continuous',
      event_date: input.event_date,
      sport: event ? input.sport : null,
      priority: event ? input.priority : null,
      label,
      active: true,
    });
    return this.inputs();
  }

  setAvailability(days: AvailabilityDay[]): PlanningInputs {
    const seen = new Set<number>();
    for (const d of days) {
      const ok =
        Number.isInteger(d.weekday) &&
        d.weekday >= 0 &&
        d.weekday <= 6 &&
        !seen.has(d.weekday) &&
        Number.isInteger(d.max_minutes) &&
        d.max_minutes >= 0 &&
        d.max_minutes <= 1440;
      if (!ok) throw err('invalid_value');
      seen.add(d.weekday);
    }
    for (const d of days) this.availability.set(d.weekday, { ...d });
    return this.inputs();
  }

  // --- Pläne ---------------------------------------------------------------------------

  private item(s: StoredPlan): PlanListItem {
    const p = s.plan;
    return {
      id: p.plan_id,
      created_at: p.created_at,
      status: s.status,
      source: p.source,
      locale: p.locale,
      rules_version: p.rules_version,
      goal_id: s.goalId,
      goal: structuredClone(p.goal),
      first_week: p.weeks[0]?.week_start ?? null,
      last_week: p.weeks[p.weeks.length - 1]?.week_start ?? null,
      weeks: p.weeks.length,
    };
  }

  list(): PlanListItem[] {
    return [...this.plans]
      .sort(
        (a, b) =>
          b.plan.created_at.localeCompare(a.plan.created_at) ||
          (a.plan.plan_id < b.plan.plan_id ? -1 : 1),
      )
      .map((s) => this.item(s));
  }

  setStatus(id: string, status: PlanStatus): PlanListItem[] {
    if (!['draft', 'active', 'archived'].includes(status)) throw err('invalid_value');
    const target = this.plans.find((s) => s.plan.plan_id === id);
    if (!target) throw err('not_found');
    if (status === 'active') {
      for (const s of this.plans) if (s.status === 'active') s.status = 'archived';
    }
    target.status = status;
    return this.list();
  }

  get(id: string): PlanDetail {
    const stored = this.plans.find((s) => s.plan.plan_id === id);
    if (!stored) throw err('not_found');
    return this.detail(stored);
  }

  private detail(s: StoredPlan): PlanDetail {
    const plan = structuredClone(s.plan);
    const first = plan.weeks[0].week_start;
    const last = plan.weeks[plan.weeks.length - 1].week_start;
    const state = this.host.pmcAt(addDays(first, -1));
    const daily = new Map<string, number>();
    for (const w of plan.weeks) {
      for (const d of w.days) {
        const load = d.sessions.reduce((sum, x) => sum + x.planned_load, 0);
        if (load > 0) daily.set(d.date, load);
      }
    }
    const projection = this.host.computePmc(daily, first, addDays(last, 6), {
      ...this.host.pmcSettings(),
      ctl0: state.ctl,
      atl0: state.atl,
    });
    const violations: Violation[] = plan.warnings.map((w) => ({
      code: w.code,
      hard: false,
      date: w.date ?? null,
      detail: {},
    }));
    return {
      status: s.status,
      plan,
      projection,
      ctl0: state.ctl,
      atl0: state.atl,
      validation: { schema_errors: [], violations },
    };
  }

  generate(input: GeneratePlanInput): PlanDetail {
    const { plan, goalId } = this.make(input);
    this.plans.push({ plan, status: 'draft', goalId });
    this.nextPlan++;
    return this.get(plan.plan_id);
  }

  /** Die drei Fragen (T26): Ziel und Verfügbarkeit setzen, Plan erzeugen und aktivieren. */
  quick(input: QuickPlanInput): PlanDetail {
    if (!isIsoDate(input.week_start)) throw err('invalid_date');
    if (mondayOf(input.week_start) !== input.week_start) throw err('not_monday');
    const availability = distributeMinutes(input.hours_per_week, input.days);
    if (typeof availability === 'string') throw err(availability);
    let date: string | null = null;
    let sport: EnduranceSport | null = null;
    if (input.goal.kind === 'event') {
      const wanted = input.goal.date;
      if (wanted === null || !isIsoDate(wanted) || wanted < input.week_start) {
        throw err('invalid_date');
      }
      if (
        input.goal.sport !== null &&
        !(ENDURANCE_SPORTS as readonly string[]).includes(input.goal.sport)
      ) {
        throw err('invalid_sport');
      }
      date = wanted;
      sport = input.goal.sport;
    }
    const active = this.goals.find((g) => g.active);
    const same =
      active !== undefined &&
      (input.goal.kind === 'continuous'
        ? active.kind === 'continuous'
        : active.kind === 'event' && active.event_date === date && active.sport === sport);
    if (!same) {
      this.setGoal({ event_date: date, sport, priority: date ? 'A' : null, label: null });
    }
    this.setAvailability(availability);
    const { plan } = this.make({
      week_start: input.week_start,
      goal_date: null,
      goal_sport: null,
      weeks: null,
      locale: input.locale,
      sports: [],
      strength_per_week: 1,
    });
    return this.store(plan, 'active');
  }

  /** Der Plan, auf den sich das Gespräch bezieht: der aktive, sonst der neueste Entwurf. */
  basePlan(): Plan | null {
    const active = this.plans.find((s) => s.status === 'active');
    if (active) return structuredClone(active.plan);
    const drafts = this.list().filter((p) => p.status === 'draft');
    return drafts.length > 0 ? structuredClone(this.get(drafts[0].id).plan) : null;
  }

  /** Nummer, die der nächste gespeicherte Plan bekommt. */
  nextPlanId(): string {
    return planId(this.nextPlan);
  }

  /** Speichert einen fertigen Plan (Gespräch mit dem Coach) und macht ihn auf Wunsch aktiv. */
  store(plan: Plan, status: PlanStatus): PlanDetail {
    if (status === 'active') {
      for (const s of this.plans) if (s.status === 'active') s.status = 'archived';
    }
    this.plans.push({ plan, status, goalId: this.goals.find((g) => g.active)?.id ?? null });
    this.nextPlan++;
    return this.get(plan.plan_id);
  }

  /** Zeit der Woche, wie `inputs()` sie liefert. */
  currentAvailability(): AvailabilityDay[] {
    return this.inputs().availability;
  }

  /**
   * Erzeugt einen Plan, ohne ihn zu speichern. `availability` ersetzt die gespeicherte
   * Verfügbarkeit (Vorschau eines Vorschlags des Coachs).
   */
  make(
    input: GeneratePlanInput,
    availability?: AvailabilityDay[],
  ): { plan: Plan; goalId: number | null } {
    if (!isIsoDate(input.week_start)) throw err('invalid_date');
    if (mondayOf(input.week_start) !== input.week_start) throw err('not_monday');
    if (input.goal_date !== null && !isIsoDate(input.goal_date)) throw err('invalid_date');
    if (
      input.weeks !== null &&
      !(Number.isInteger(input.weeks) && input.weeks >= 1 && input.weeks <= 26)
    ) {
      throw err('invalid_value');
    }
    if (!(
      Number.isInteger(input.strength_per_week) &&
      input.strength_per_week >= 0 &&
      input.strength_per_week <= 2
    )) {
      throw err('invalid_value');
    }
    for (const s of [...input.sports, ...(input.goal_sport ? [input.goal_sport] : [])]) {
      if (!(ENDURANCE_SPORTS as readonly string[]).includes(s)) throw err('invalid_sport');
    }

    const active = this.goals.find((g) => g.active) ?? null;
    const eventDate =
      input.goal_date ??
      (active?.kind === 'event' && active.event_date && active.event_date >= input.week_start
        ? active.event_date
        : null);
    const eventSport = input.goal_sport ?? active?.sport ?? null;
    const weeks =
      input.weeks ??
      (eventDate
        ? Math.min(
            6,
            Math.max(1, Math.floor(daysBetween(input.week_start, mondayOf(eventDate)) / 7) + 1),
          )
        : 3);
    const sports: EnduranceSport[] =
      input.sports.length > 0 ? [...new Set(input.sports)] : ['cycling', 'running'];

    const planWeeks: PlanWeek[] = [];
    for (let i = 0; i < weeks; i++) {
      const start = addDays(input.week_start, i * 7);
      const phase: Phase =
        eventDate && mondayOf(eventDate) === start
          ? 'race'
          : i === weeks - 1 && weeks >= 3
            ? 'recovery'
            : i === 0
              ? 'base'
              : 'build';
      planWeeks.push(
        this.buildWeek(
          start,
          i,
          phase,
          sports,
          input.strength_per_week,
          availability ?? this.inputs().availability,
        ),
      );
    }
    const warnings = this.warningsOf(planWeeks, input.week_start);

    const n = this.nextPlan;
    const plan: Plan = {
      schema_version: 1,
      plan_id: planId(n),
      created_at: `${this.host.today}T08:00:${String(n % 60).padStart(2, '0')}Z`,
      locale: input.locale,
      source: 'rules',
      rules_version: '1.0.0',
      goal: eventDate
        ? {
            kind: 'event',
            event_date: eventDate,
            ...(eventSport ? { sport: eventSport } : { sport: sports[0] }),
            priority: active?.priority ?? 'A',
          }
        : { kind: 'continuous' },
      weeks: planWeeks,
      warnings,
    };
    return { plan, goalId: active?.id ?? null };
  }

  /** Eine Woche aus den Vorgaben der Verfügbarkeit. */
  private buildWeek(
    start: string,
    index: number,
    phase: Phase,
    sports: EnduranceSport[],
    strength: number,
    inputs: AvailabilityDay[],
  ): PlanWeek {
    const factor = phase === 'recovery' ? 0.65 : phase === 'race' ? 0.5 : 1 + index * 0.05;
    const days: PlanDay[] = [];
    for (let wd = 0; wd < 7; wd++) {
      const date = addDays(start, wd);
      const avail = inputs[wd];
      const sessions: PlanSession[] = [];
      const cap = avail.fixed_rest ? 0 : avail.max_minutes;
      const sport = sports[wd % sports.length];
      const add = (
        sp: PlanSport,
        template: string,
        minutes: number,
        hard: boolean,
        reason: string,
      ) => {
        const duration = Math.min(cap, Math.round((minutes * factor) / 5) * 5);
        if (duration < 10) return;
        const intensity = hard ? 0.95 : sp === 'strength' ? 0.6 : 0.65;
        sessions.push({
          slot: 'am',
          sport: sp,
          template_id: template,
          duration_min: duration,
          planned_load: round1((duration / 60) * intensity * intensity * 100),
          hard,
          reason_keys: [reason],
        });
      };
      if (phase === 'race' && wd >= 4) {
        // Tag vor dem Termin, Termin und danach: keine Einheit (wie im echten Planer).
      } else if (wd === 1) {
        add(
          sport,
          avail.allow_hard ? HARD[sport] : EASY[sport],
          60,
          avail.allow_hard,
          avail.allow_hard ? 'reason.session.hard' : 'reason.session.easy',
        );
      } else if (wd === 2) {
        add(sport, EASY[sport], 50, false, 'reason.session.easy');
      } else if (wd === 3 && strength > 0) {
        add('strength', 'strength.general.a', 40, false, 'reason.session.strength');
      } else if (wd === 5) {
        add(sport, LONG[sport], 100, false, 'reason.session.long');
      } else if (wd === 6) {
        add(sport, EASY[sport], 45, false, 'reason.session.recovery_after_hard');
      }
      days.push({ date, sessions });
    }
    const planned = round1(
      days.reduce((s, d) => s + d.sessions.reduce((x, y) => x + y.planned_load, 0), 0),
    );
    return {
      week_start: start,
      phase,
      target_load: round1(planned * 1.04),
      planned_load: planned,
      reason_keys: [
        `reason.week.phase_${phase}`,
        ...(phase === 'recovery' ? ['reason.week.recovery_block'] : ['reason.week.growth']),
      ],
      days,
    };
  }

  /** Warnungen wie die Leitplanken R2 und R10: Zeitanstieg ab 10 Prozent, Form unter -30. */
  private warningsOf(weeks: PlanWeek[], first: string): PlanWarning[] {
    const out: PlanWarning[] = [];
    const minutes = weeks.map((w) =>
      w.days.reduce((s, d) => s + d.sessions.reduce((x, y) => x + y.duration_min, 0), 0),
    );
    for (let i = 1; i < weeks.length; i++) {
      if (minutes[i - 1] >= 60 && minutes[i] >= minutes[i - 1] * 1.1) {
        out.push({ code: 'R02', severity: 'warn', date: weeks[i].week_start });
      }
    }
    const state = this.host.pmcAt(addDays(first, -1));
    const daily = new Map<string, number>();
    for (const w of weeks)
      for (const d of w.days)
        daily.set(
          d.date,
          d.sessions.reduce((s, x) => s + x.planned_load, 0),
        );
    const points = this.host.computePmc(
      daily,
      first,
      addDays(weeks[weeks.length - 1].week_start, 6),
      {
        ...this.host.pmcSettings(),
        ctl0: state.ctl,
        atl0: state.atl,
      },
    );
    const low = points.reduce((a, b) => (b.tsb < a.tsb ? b : a), points[0]);
    if (low.tsb < -30) out.push({ code: 'R10', severity: 'warn', date: low.date });
    out.push({ code: 'R06', severity: 'info' });
    return out;
  }

  // --- Export und Sicherung ------------------------------------------------------------

  private counts(): BackupCounts {
    return {
      thresholds: this.host.thresholdCount(),
      goals: this.goals.length,
      availability: this.availability.size,
      plans: this.plans.length,
      manual_sessions: 0,
      settings: this.host.settingCount(),
      recovery_days: 0,
    };
  }

  exportIcs(id: string): ExportResult {
    const detail = this.get(id);
    const events = detail.plan.weeks
      .flatMap((w) => w.days)
      .reduce((s, d) => s + d.sessions.length, 0);
    this.host.log({ command: 'export_plan_ics', args: { id } });
    return { saved: true, events, counts: null };
  }

  exportJson(id: string): ExportResult {
    this.get(id);
    this.host.log({ command: 'export_plan_json', args: { id } });
    return { saved: true, events: null, counts: null };
  }

  exportBackup(): ExportResult {
    this.host.log({ command: 'export_backup', args: {} });
    return { saved: true, events: null, counts: this.counts() };
  }

  restore(confirmed: boolean): RestoreResult {
    this.host.log({ command: 'restore_backup', args: { confirmed } });
    if (!confirmed) throw err('unconfirmed');
    return { restored: true, counts: this.counts() };
  }
}
