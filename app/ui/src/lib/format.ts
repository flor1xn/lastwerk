// Zahlen, Dauern und Daten für die Anzeige. Dezimalkomma in de, Punkt in en (Intl).
// Fehlende Werte sind null und erscheinen als Gedankenstrich-Ersatz "·", nie als 0.

import type { Locale } from './backend/types';
import { fromIso } from './dates';
import { intlTag } from './i18n';

/** Platzhalter für einen fehlenden Wert (kein Gedankenstrich, kein 0). */
export const MISSING = '·';

const numberFormats = new Map<string, Intl.NumberFormat>();

function numberFormat(locale: Locale, digits: number): Intl.NumberFormat {
  const key = `${locale}:${digits}`;
  let f = numberFormats.get(key);
  if (!f) {
    f = new Intl.NumberFormat(intlTag(locale), {
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    });
    numberFormats.set(key, f);
  }
  return f;
}

/** Zahl mit fester Zahl Nachkommastellen. null und nicht endliche Werte geben MISSING. */
export function formatNumber(value: number | null | undefined, locale: Locale, digits = 1): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return MISSING;
  return numberFormat(locale, digits).format(value);
}

/** Dauer als h:mm:ss, unter einer Stunde als m:ss. */
export function formatDuration(totalSeconds: number | null | undefined): string {
  if (totalSeconds === null || totalSeconds === undefined || !Number.isFinite(totalSeconds)) {
    return MISSING;
  }
  const s = Math.max(0, Math.round(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const mm = String(m).padStart(2, '0');
  const ss = String(sec).padStart(2, '0');
  return h > 0 ? `${h}:${mm}:${ss}` : `${m}:${ss}`;
}

/** Strecke in Kilometern mit einer Nachkommastelle. */
export function formatDistance(meters: number | null | undefined, locale: Locale): string {
  if (meters === null || meters === undefined) return MISSING;
  return `${formatNumber(meters / 1000, locale, 1)} km`;
}

/** Tempo in Minuten je Kilometer aus m/s, zum Beispiel 4:35. Unter 0,5 m/s (über 33 min/km)
 * ist es Gehen oder Stehen und erscheint als fehlend (Konvention der Anzeige). */
export function formatPace(speedMps: number | null | undefined): string {
  if (speedMps === null || speedMps === undefined || !Number.isFinite(speedMps)) return MISSING;
  if (speedMps < MIN_PACE_SPEED_MPS) return MISSING;
  return formatDuration(1000 / speedMps);
}

/** Untergrenze der Geschwindigkeit für eine Tempoangabe (Konvention der Anzeige). */
export const MIN_PACE_SPEED_MPS = 0.5;

/** Geschwindigkeit in km/h mit einer Nachkommastelle. */
export function formatSpeed(speedMps: number | null | undefined, locale: Locale): string {
  if (speedMps === null || speedMps === undefined || !Number.isFinite(speedMps)) return MISSING;
  return `${formatNumber(speedMps * 3.6, locale, 1)} km/h`;
}

const dateFormats = new Map<string, Intl.DateTimeFormat>();

function dateFormat(
  locale: Locale,
  kind: 'date' | 'weekday' | 'day-month' | 'long',
): Intl.DateTimeFormat {
  const key = `${locale}:${kind}`;
  let f = dateFormats.get(key);
  if (!f) {
    const options: Intl.DateTimeFormatOptions =
      kind === 'date'
        ? { year: 'numeric', month: '2-digit', day: '2-digit' }
        : kind === 'weekday'
          ? { weekday: 'short' }
          : kind === 'long'
            ? { weekday: 'long', day: 'numeric', month: 'long' }
            : { day: '2-digit', month: '2-digit' };
    f = new Intl.DateTimeFormat(intlTag(locale), { ...options, timeZone: 'UTC' });
    dateFormats.set(key, f);
  }
  return f;
}

/** Kalenderdatum, de 13.10.2026, en 10/13/2026. */
export function formatDate(iso: string, locale: Locale): string {
  return dateFormat(locale, 'date').format(fromIso(iso));
}

/** Kurzer Wochentag, de Mo, en Mon. */
export function formatWeekday(iso: string, locale: Locale): string {
  return dateFormat(locale, 'weekday').format(fromIso(iso));
}

/** Datum als Titel, de "Dienstag, 13. Oktober", en "Tuesday, October 13". */
export function formatLongDate(iso: string, locale: Locale): string {
  return dateFormat(locale, 'long').format(fromIso(iso));
}

/** Tag und Monat, de 13.10., en 10/13. */
export function formatDayMonth(iso: string, locale: Locale): string {
  return dateFormat(locale, 'day-month').format(fromIso(iso));
}

/** Liest eine Dezimalzahl aus einem Eingabefeld, mit Komma oder Punkt. Ungültig ist null. */
export function parseDecimal(text: string): number | null {
  const cleaned = text.trim().replace(',', '.');
  if (cleaned === '') return null;
  const value = Number(cleaned);
  return Number.isFinite(value) ? value : null;
}
