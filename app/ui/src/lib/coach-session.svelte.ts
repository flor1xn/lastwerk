// Gespräch mit dem Coach als Zustand der Sitzung (T43). Ein Modul, eine Instanz: Das Gespräch
// übersteht das Schließen des Seitenfensters und jeden Wechsel der Ansicht. Es lebt nur im
// Speicher der Oberfläche, wie Verlauf und Vorschläge im Rust-Teil (SPEC 9.7); "Neu beginnen"
// (`coach_reset`) leert beides.

import { getBackend } from './backend';
import {
  BackendError,
  type ChatFocus,
  type PlanDetail,
  type ProposalPreview,
} from './backend/types';
import type { ProposalState } from './coach';
import { errorKey } from './errors';

export interface CoachMsg {
  id: number;
  role: 'user' | 'coach';
  text: string;
  rejected: string[];
  proposal: ProposalPreview | null;
  pstate: ProposalState;
  perror: string | null;
}

/** Ausgang eines Sendeversuchs, für die Oberfläche (etwa erneut prüfen, ob ein Modell läuft). */
export type SendResult = 'ok' | 'failed' | 'unavailable' | 'ignored';

class CoachSession {
  messages = $state<CoachMsg[]>([]);
  /** Entwurf im Eingabefeld. `openCoach({ prefill })` setzt ihn, gesendet wird nur auf Wunsch. */
  draft = $state('');
  sending = $state(false);
  error = $state<string | null>(null);
  private nextId = 1;
  // Zählt Anfragen; eine abgebrochene Anfrage verliert ihre Nummer und wird ignoriert.
  private token = 0;

  /** Sendet den Entwurf mit dem Fokus der Oberfläche. Ohne Antwort kommt der Text zurück. */
  async send(focus: ChatFocus | null): Promise<SendResult> {
    const text = this.draft.trim();
    if (text === '' || this.sending) return 'ignored';
    this.error = null;
    const mine = ++this.token;
    const user: CoachMsg = {
      id: this.nextId++,
      role: 'user',
      text,
      rejected: [],
      proposal: null,
      pstate: 'open',
      perror: null,
    };
    this.messages.push(user);
    this.draft = '';
    this.sending = true;
    try {
      const reply = await getBackend().coachChat(text, focus);
      if (mine !== this.token) return 'ignored';
      this.messages.push({
        id: this.nextId++,
        role: 'coach',
        text: reply.reply,
        rejected: reply.rejected_codes,
        proposal: reply.proposal,
        pstate: 'open',
        perror: null,
      });
      return 'ok';
    } catch (e) {
      if (mine !== this.token) return 'ignored';
      // Das Modell hat nicht geantwortet: Die Nachricht war nicht Teil des Gesprächs. Sie
      // kommt zurück ins Eingabefeld, damit man sie nicht neu tippen muss.
      this.messages = this.messages.filter((m) => m.id !== user.id);
      this.draft = text;
      if (!(e instanceof BackendError && e.code === 'coach_cancelled')) {
        this.error = errorKey(e);
      }
      return e instanceof BackendError &&
        ['coach_unavailable', 'coach_unreachable'].includes(e.code)
        ? 'unavailable'
        : 'failed';
    } finally {
      if (mine === this.token) this.sending = false;
    }
  }

  async cancel(): Promise<void> {
    // Sofort freigeben; der Rust-Teil stellt weitere Aufrufe ein und verwirft die Antwort.
    this.token++;
    this.sending = false;
    const last = this.messages[this.messages.length - 1];
    if (last && last.role === 'user') {
      this.messages = this.messages.slice(0, -1);
      this.draft = last.text;
    }
    try {
      await getBackend().coachCancel();
    } catch {
      // Es läuft ohnehin nichts mehr, das die Oberfläche abwarten müsste.
    }
  }

  async reset(): Promise<void> {
    this.token++;
    this.sending = false;
    this.error = null;
    this.messages = [];
    try {
      await getBackend().coachReset();
    } catch (e) {
      this.error = errorKey(e);
    }
  }

  /** Übernimmt einen Vorschlag. Gibt den neuen Plan zurück oder `null`. */
  async accept(m: CoachMsg): Promise<PlanDetail | null> {
    if (!m.proposal) return null;
    m.pstate = 'applying';
    m.perror = null;
    try {
      const detail = await getBackend().coachApply(m.proposal.id);
      m.pstate = 'applied';
      // Alle übrigen offenen Vorschläge beziehen sich auf den alten Plan.
      for (const other of this.messages) {
        if (other !== m && other.pstate === 'open') other.pstate = 'stale';
      }
      return detail;
    } catch (e) {
      if (e instanceof BackendError && e.code === 'proposal_stale') {
        m.pstate = 'stale';
      } else {
        m.pstate = 'open';
        m.perror = errorKey(e);
      }
      return null;
    }
  }

  dismiss(m: CoachMsg): void {
    m.pstate = 'dismissed';
  }

  /** Nur für Tests: vergisst alles, ohne den Rust-Teil zu fragen. */
  clearForTests(): void {
    this.token++;
    this.messages = [];
    this.draft = '';
    this.sending = false;
    this.error = null;
  }
}

export const coachSession = new CoachSession();
