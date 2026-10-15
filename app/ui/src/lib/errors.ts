import { BackendError } from './backend/types';

/** Schlüssel des Fehlertextes. Unbekannte Fehler zeigen den allgemeinen Text, nie Rohmeldungen
 * (Meldungen von Fremdcode können Pfade enthalten). */
export function errorKey(e: unknown): string {
  return e instanceof BackendError ? e.messageKey : 'ui.error.internal';
}
