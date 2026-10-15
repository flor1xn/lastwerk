// Kalenderrechnung mit Daten als YYYY-MM-DD. Intern UTC-Mitternacht, damit Sommerzeit nie
// einen Tag verschiebt. Die Oberfläche kennt nur Kalendertage (SPEC 6.1: lokales Datum).

const ISO = /^(\d{4})-(\d{2})-(\d{2})$/;

export function isIsoDate(s: string): boolean {
  const m = ISO.exec(s);
  if (!m) return false;
  const d = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
  return toIso(d) === s;
}

export function fromIso(s: string): Date {
  const m = ISO.exec(s);
  if (!m) throw new RangeError(`kein Datum: ${s}`);
  return new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
}

export function toIso(d: Date): string {
  const y = String(d.getUTCFullYear()).padStart(4, '0');
  const m = String(d.getUTCMonth() + 1).padStart(2, '0');
  const day = String(d.getUTCDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

export function addDays(iso: string, days: number): string {
  const d = fromIso(iso);
  d.setUTCDate(d.getUTCDate() + days);
  return toIso(d);
}

/** Montag der Woche, in der `iso` liegt (Woche beginnt am Montag, SPEC 6.2: 0 = Montag). */
export function mondayOf(iso: string): string {
  const d = fromIso(iso);
  const weekday = (d.getUTCDay() + 6) % 7; // 0 = Montag
  return addDays(iso, -weekday);
}

/** Das lokale Kalenderdatum von `now` (nicht das UTC-Datum). */
export function localIso(now: Date = new Date()): string {
  const y = String(now.getFullYear()).padStart(4, '0');
  const m = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

export function daysBetween(fromIsoDate: string, toIsoDate: string): number {
  return Math.round((fromIso(toIsoDate).getTime() - fromIso(fromIsoDate).getTime()) / 86_400_000);
}
