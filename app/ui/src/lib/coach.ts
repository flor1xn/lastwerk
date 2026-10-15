// Hilfen für das Gespräch mit dem Coach (T26 Teil B). Reine Funktionen ohne DOM.

import type { Locale } from './backend/types';
import { MISSING } from './format';
import { hasKey, translate } from './i18n';

/** Längste Nachricht in Zeichen (SPEC 9.7, wie im Rust-Teil). */
export const MAX_MESSAGE_CHARS = 1000;
/** Ab so vielen Zeichen zeigt das Eingabefeld den Zähler. */
export const COUNTER_FROM = 800;

/** Stand eines Vorschlags in der Oberfläche. `stale`: Der Plan hat sich seither geändert. */
export type ProposalState = 'open' | 'applying' | 'applied' | 'dismissed' | 'stale';

export interface Cell {
  /** Anzeigetext: "480", "480 → 420" oder "· → 60". */
  text: string;
  /** Vorher und nachher unterscheiden sich. */
  changed: boolean;
}

/** Vorher und nachher einer Zahl in einer Zelle. Fehlt ein Wert, steht der Platzhalter. */
export function compareCell(
  before: number | null | undefined,
  after: number | null | undefined,
  format: (n: number) => string,
): Cell {
  const has = (v: number | null | undefined): v is number => v !== null && v !== undefined;
  const b = has(before) ? format(before) : MISSING;
  const a = has(after) ? format(after) : MISSING;
  if (b === a) return { text: b, changed: false };
  return { text: `${b} → ${a}`, changed: true };
}

const RULE_CODE = /^R\d{2}$/;

/** Regelcode mit Namen der Regel ("R03 Keine harten Tage hintereinander"), sonst der Code. */
export function ruleLabel(locale: Locale, code: string): string {
  if (RULE_CODE.test(code) && hasKey(locale, `rule.${code}.name`)) {
    return `${code} ${translate(locale, `rule.${code}.name`)}`;
  }
  return code;
}
