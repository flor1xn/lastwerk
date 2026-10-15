// Coach der Attrappe (T26 Teil B): antwortet aus einer festen Liste, ohne Modell und ohne
// Netzwerk. Die Antworten wechseln der Reihe nach: eine einfache Antwort, ein Vorschlag mit
// Änderungen am Plan (`plan_edit`), ein Vorschlag mit neuen Zeiten (`planning_change`) und ein
// Vorschlag, den die Regeln verwerfen. Die Texte sind frei erfunden und stammen von keinem
// Modell. Mit `available: false` verhält sich die Attrappe wie ein Rechner ohne Modell.
//
// Die Attrappe prüft nichts nach den Regeln des Rust-Teils; sie zeigt, wie die Oberfläche mit
// den vier Antwortarten umgeht (Vorher und Nachher, Übernehmen, Verwerfen, Abbruch).

import { daysBetween, mondayOf } from '../dates';
import type { MockPlanning } from './mock-plans';
import {
  BackendError,
  type AvailabilityDay,
  type ChangeLine,
  type CoachReply,
  type CoachStatus,
  type EditLine,
  type Locale,
  type Plan,
  type PlanDetail,
  type PlanSport,
  type PlanWeek,
  type ProposalPreview,
  type PromptPreview,
  type WeekComparison,
  type WeekFigures,
} from './types';

export interface MockCoachOptions {
  /** Ein Modell ist da. `false` ist der Zustand ohne Ollama. */
  available: boolean;
  /** Wartezeit je Antwort in Millisekunden, damit der Zustand "denkt nach" sichtbar wird. */
  delayMs: number;
  /** Bezugstag. */
  today: string;
  locale: () => Locale;
}

const MODELS = ['qwen3:4b', 'llama3.2:3b'];
const BASE_URL = 'http://127.0.0.1:11434';
const MAX_MESSAGE = 1000;
const MAX_HISTORY = 12;

const err = (code: string) => new BackendError(code, `ui.error.${code}`);
const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
const round1 = (v: number) => Math.round(v * 10) / 10;

/** Antworten der Attrappe in beiden Sprachen, in der Reihenfolge des Skripts. */
const TEXTS: Record<
  Locale,
  { plain: string; edit: string; editWhy: string; change: string; rejected: string }
> = {
  de: {
    plain:
      'Deine Form ist ausgewogen, die Belastung der letzten Wochen passt zum Plan. Wenn dir etwas nicht passt, sag mir einfach, was sich ändern soll.',
    edit: 'Ich habe die letzte Woche etwas entlastet: Eine Einheit wird kürzer, eine entfällt.',
    editWhy:
      'Die längste Einheit der letzten Woche wird um 15 Minuten kürzer, die letzte Einheit entfällt. So bleibt die Woche locker.',
    change: 'Dienstag ist jetzt frei. Ich lasse den Planer die Woche neu verteilen.',
    rejected:
      'Zwei harte Einheiten direkt hintereinander lässt die Regel R03 nicht zu. Deshalb mache ich keinen Vorschlag.',
  },
  en: {
    plain:
      'Your form is balanced and the load of the last weeks fits the plan. If something does not suit you, just tell me what should change.',
    edit: 'I eased off the last week a little: one session gets shorter, one is dropped.',
    editWhy:
      'The longest session of the last week is 15 minutes shorter and the last session is dropped. That keeps the week easy.',
    change: 'Tuesday is free now. I let the planner spread the week again.',
    rejected:
      'Rule R03 does not allow two hard sessions back to back, so I am not making a proposal.',
  },
};

interface Pending {
  id: string;
  basePlanId: string;
  kind: 'plan_edit' | 'planning_change';
  after: Plan;
  availability: AvailabilityDay[] | null;
  weekStart: string;
  weeks: number | null;
  sports: PlanSport[] | null;
}

function figures(w: PlanWeek): WeekFigures {
  const sessions = w.days.flatMap((d) => d.sessions);
  return {
    minutes: sessions.reduce((sum, s) => sum + s.duration_min, 0),
    hard: sessions.filter((s) => s.hard).length,
    load: w.planned_load ?? null,
  };
}

/** Vorher und Nachher je Woche, nach dem Beginn der Woche gepaart. */
export function compareWeeks(before: Plan, after: Plan): WeekComparison[] {
  const starts = [...new Set([...before.weeks, ...after.weeks].map((w) => w.week_start))].sort();
  return starts.map((week_start) => {
    const b = before.weeks.find((w) => w.week_start === week_start);
    const a = after.weeks.find((w) => w.week_start === week_start);
    return { week_start, before: b ? figures(b) : null, after: a ? figures(a) : null };
  });
}

function sumLoad(week: PlanWeek): number {
  return round1(
    week.days.reduce((s, d) => s + d.sessions.reduce((x, y) => x + y.planned_load, 0), 0),
  );
}

export class MockCoach {
  private model: string | null = null;
  /** Modelle, die der Server hat. Die Einrichtung der Attrappe (T29) ändert sie. */
  private models = [...MODELS];
  private history: { role: 'user' | 'assistant'; text: string }[] = [];
  private pending = new Map<string, Pending>();
  private busy = false;
  private epoch = 0;
  private step = 0;
  private nextId = 1;

  constructor(
    private readonly planning: MockPlanning,
    private readonly options: MockCoachOptions,
  ) {}

  status(): CoachStatus {
    if (!this.options.available) {
      return {
        available: false,
        provider: 'ollama',
        base_url: BASE_URL,
        model: null,
        models: [],
        reason: 'no_server',
      };
    }
    if (this.models.length === 0) {
      return {
        available: false,
        provider: 'ollama',
        base_url: BASE_URL,
        model: null,
        models: [],
        reason: 'no_model',
      };
    }
    return {
      available: true,
      provider: 'ollama',
      base_url: BASE_URL,
      model: this.model && this.models.includes(this.model) ? this.model : this.models[0],
      models: [...this.models],
      reason: null,
    };
  }

  /** Für die Einrichtung der Attrappe: Ollama läuft oder nicht (T29). */
  setAvailable(available: boolean): void {
    this.options.available = available;
  }

  /** Für die Einrichtung der Attrappe: die Modelle des Servers (T29). */
  setModels(models: string[]): void {
    this.models = [...models];
  }

  setModel(model: string): CoachStatus {
    const m = model.trim();
    if (m.length > 200 || [...m].some((c) => c < ' ')) throw err('invalid_value');
    this.model = m === '' ? null : m;
    return this.status();
  }

  reset(): void {
    this.cancel();
    this.history = [];
    this.pending.clear();
  }

  cancel(): void {
    this.epoch++;
  }

  /** Der Text, der bei der nächsten Nachricht zum Modell ginge (erfundene Fassung). */
  preview(message: string): PromptPreview {
    const base = this.planning.basePlan();
    if (!base) throw err('no_plan');
    const text = message.trim();
    if ([...text].length > MAX_MESSAGE) throw err('message_too_long');
    const status = this.status();
    const input = {
      input_version: 1,
      goal: { kind: base.goal.kind, ...(base.goal.sport ? { sport: base.goal.sport } : {}) },
      fitness: { ctl: 31, atl: 38, tsb: -7 },
      recovery_signal: false,
      base_plan: base.weeks.flatMap((w) =>
        w.days.flatMap((d) =>
          d.sessions.map((s) => ({
            date: d.date,
            slot: s.slot,
            template_id: s.template_id,
            duration_min: s.duration_min,
          })),
        ),
      ),
      metrics: [
        { code: 'ctl_now', value: 31.2 },
        { code: 'tsb_now', value: -6.8 },
        { code: 'ramp_ctl_7d', value: 2.1 },
      ],
    };
    const system =
      this.options.locale() === 'de'
        ? `Du bist der Trainingsberater in Lastwerk (Attrappe). Antworte als JSON nach dem Schema.\n\nEingang:\n${JSON.stringify(input)}`
        : `You are the training advisor in Lastwerk (mock). Answer as JSON following the schema.\n\nInput:\n${JSON.stringify(input)}`;
    const messages: PromptPreview['messages'] = [{ role: 'system', content: system }];
    for (const m of this.history) {
      messages.push({
        role: m.role,
        content:
          m.role === 'assistant' ? JSON.stringify({ schema_version: 1, reply: m.text }) : m.text,
      });
    }
    if (text !== '') messages.push({ role: 'user', content: text });
    return {
      provider: { name: status.provider, base_url: status.base_url, model: status.model },
      messages,
    };
  }

  async chat(message: string): Promise<CoachReply> {
    if (!this.options.available || this.models.length === 0) throw err('coach_unavailable');
    // Steuerzeichen außer Zeilenumbruch und Tabulator werden Leerzeichen, wie im Rust-Teil.
    const text = [...message]
      .map((c) => ((c < ' ' && c !== '\n' && c !== '\t') || c === '\u007f' ? ' ' : c))
      .join('')
      .trim();
    if (text === '') throw err('empty_message');
    if ([...text].length > MAX_MESSAGE) throw err('message_too_long');
    const base = this.planning.basePlan();
    if (!base) throw err('no_plan');
    if (this.busy) throw err('busy');
    this.busy = true;
    const epoch = this.epoch;
    try {
      if (this.options.delayMs > 0) await sleep(this.options.delayMs);
      if (this.epoch !== epoch) throw err('coach_cancelled');
      const reply = this.scripted(base);
      this.history.push({ role: 'user', text }, { role: 'assistant', text: reply.reply });
      this.history = this.history.slice(-MAX_HISTORY);
      return reply;
    } finally {
      this.busy = false;
    }
  }

  apply(proposalId: string): PlanDetail {
    const p = this.pending.get(proposalId);
    if (!p) throw err('not_found');
    const base = this.planning.basePlan();
    if (!base || base.plan_id !== p.basePlanId) {
      this.pending.delete(proposalId);
      throw err('proposal_stale');
    }
    let plan: Plan;
    if (p.kind === 'plan_edit') {
      plan = structuredClone(p.after);
    } else {
      if (p.availability) this.planning.setAvailability(p.availability);
      plan = this.planning.make({
        week_start: p.weekStart,
        goal_date: null,
        goal_sport: null,
        weeks: p.weeks,
        locale: base.locale,
        sports: (p.sports ?? []).filter(
          (s): s is 'cycling' | 'running' | 'swimming' => s !== 'strength' && s !== 'rest',
        ),
        strength_per_week: 1,
      }).plan;
    }
    plan.plan_id = this.planning.nextPlanId();
    plan.created_at = `${this.options.today}T09:00:00Z`;
    this.pending.clear();
    return this.planning.store(plan, 'active');
  }

  // --- Skript -------------------------------------------------------------------------

  private scripted(base: Plan): CoachReply {
    const texts = TEXTS[this.options.locale()];
    const n = this.step++ % 4;
    if (n === 0) return { reply: texts.plain, proposal: null, rejected_codes: [] };
    if (n === 1) return this.editProposal(base, texts);
    if (n === 2) return this.changeProposal(base, texts);
    return { reply: texts.rejected, proposal: null, rejected_codes: ['R03'] };
  }

  private newId(): string {
    return `mock-proposal-${this.nextId++}`;
  }

  private editProposal(base: Plan, texts: (typeof TEXTS)['de']): CoachReply {
    const after = structuredClone(base);
    const last = after.weeks[after.weeks.length - 1];
    const sessions = last.days.flatMap((d) => d.sessions.map((s) => ({ date: d.date, s })));
    const edits: EditLine[] = [];
    if (sessions.length > 0) {
      const longest = sessions.reduce((a, b) => (b.s.duration_min > a.s.duration_min ? b : a));
      const shorter = Math.max(20, longest.s.duration_min - 15);
      edits.push({
        op: 'replace',
        date: longest.date,
        slot: longest.s.slot,
        template_id: longest.s.template_id,
        duration_min: shorter,
        to_date: null,
        to_slot: null,
      });
      longest.s.planned_load = round1(
        (longest.s.planned_load * shorter) / Math.max(1, longest.s.duration_min),
      );
      longest.s.duration_min = shorter;
      const tail = sessions.filter((x) => x !== longest).pop();
      if (tail) {
        edits.push({
          op: 'remove',
          date: tail.date,
          slot: tail.s.slot,
          template_id: null,
          duration_min: null,
          to_date: null,
          to_slot: null,
        });
        const day = last.days.find((d) => d.date === tail.date);
        if (day) day.sessions = day.sessions.filter((x) => x !== tail.s);
      }
    }
    last.planned_load = sumLoad(last);
    after.source = 'llm';
    after.explanation = texts.editWhy;
    const id = this.newId();
    this.pending.set(id, {
      id,
      basePlanId: base.plan_id,
      kind: 'plan_edit',
      after,
      availability: null,
      weekStart: base.weeks[0].week_start,
      weeks: null,
      sports: null,
    });
    const proposal: ProposalPreview = {
      id,
      kind: 'plan_edit',
      explanation: texts.editWhy,
      summary: { weeks: compareWeeks(base, after), edits, change: null, warnings: 0 },
    };
    return { reply: texts.edit, proposal, rejected_codes: [] };
  }

  private changeProposal(base: Plan, texts: (typeof TEXTS)['de']): CoachReply {
    const days = this.planning.currentAvailability().map((d) => ({ ...d }));
    // Dienstag frei, sonst Sonntag (der Dienstag war schon frei).
    const free = days[1].fixed_rest ? 6 : 1;
    days[free] = { weekday: free, max_minutes: 0, allow_hard: false, fixed_rest: true };
    const thisMonday = mondayOf(this.options.today);
    const first = base.weeks[0].week_start;
    const weekStart = first > thisMonday ? first : thisMonday;
    const last = base.weeks[base.weeks.length - 1].week_start;
    const remaining = Math.max(1, Math.floor(daysBetween(weekStart, last) / 7) + 1);
    const sports = [
      ...new Set(base.weeks.flatMap((w) => w.days.flatMap((d) => d.sessions.map((s) => s.sport)))),
    ].filter((s): s is 'cycling' | 'running' | 'swimming' => s !== 'strength' && s !== 'rest');
    const after = this.planning.make(
      {
        week_start: weekStart,
        goal_date: null,
        goal_sport: null,
        weeks: remaining,
        locale: base.locale,
        sports,
        strength_per_week: 1,
      },
      days,
    ).plan;
    after.plan_id = this.planning.nextPlanId();
    const change: ChangeLine = {
      availability: days,
      sports: null,
      strength_per_week: null,
      weeks: null,
      week_start: weekStart,
    };
    const id = this.newId();
    this.pending.set(id, {
      id,
      basePlanId: base.plan_id,
      kind: 'planning_change',
      after,
      availability: days,
      weekStart,
      weeks: remaining,
      sports,
    });
    const proposal: ProposalPreview = {
      id,
      kind: 'planning_change',
      explanation: null,
      summary: {
        weeks: compareWeeks(base, after),
        edits: [],
        change,
        warnings: after.warnings.filter((w) => w.severity === 'warn').length,
      },
    };
    return { reply: texts.change, proposal, rejected_codes: [] };
  }
}
