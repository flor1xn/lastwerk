// Texte zur nicht berechenbaren Last (T39): Fehlt ein bestimmter Wert, nennt die Oberfläche ihn
// und sagt, wo er einzutragen ist. Unbekannte Arten fallen auf den allgemeinen Text zurück.

import type { LoadInfo } from './backend/types';
import { hasKey } from './i18n';

export interface MissingText {
  /** Schlüssel für "FTP fehlt" und Ähnliches. */
  what: string;
  /** Schlüssel des Hinweises, wo der Wert einzutragen ist. */
  hint: string;
}

export function missingText(load: LoadInfo): MissingText | null {
  if (load.status !== 'missing_threshold' || !load.missing) return null;
  const what = `ui.load.missing.${load.missing}`;
  if (!hasKey('de', what)) return null;
  return {
    what,
    hint: load.missing === 'sex' ? 'ui.load.missing_hint_athlete' : 'ui.load.missing_hint',
  };
}

/** Garmins Trainingseffekte mit eigenem Text. Andere Schlüssel erscheinen lesbar gemacht. */
export const TRAINING_EFFECTS = [
  'AEROBIC_BASE',
  'TEMPO',
  'LACTATE_THRESHOLD',
  'VO2MAX',
  'ANAEROBIC_CAPACITY',
  'SPEED',
  'RECOVERY',
  'NO_BENEFIT',
] as const;

/** Text zu Garmins Trainingseffekt: bekannter Schlüssel aus der Sprachdatei, sonst der
 * Schlüssel mit Leerzeichen statt Unterstrichen (nie als HTML). */
export function trainingEffectText(effect: string, tr: (key: string) => string): string {
  if ((TRAINING_EFFECTS as readonly string[]).includes(effect)) {
    return tr(`ui.detail.garmin.effect.${effect}`);
  }
  return effect.replace(/_/g, ' ').trim();
}
