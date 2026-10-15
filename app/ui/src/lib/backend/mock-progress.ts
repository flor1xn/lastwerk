// Fortschritt der Attrappe (T32): erfundene, deterministische Tageszeilen über gut zwei Jahre.
// Wie das Dashboard der Attrappe hängen sie nicht an den Aktivitäten der Attrappe (die reichen
// nur 140 Tage zurück), sondern bilden eine eigene Geschichte: das Vorjahr etwas ruhiger, eine
// Urlaubslücke im Sommer, Kraft ohne berechenbare Last. Der Zeitraum folgt dem Rust-Teil
// (`service::progress::progress_from`).
//
// Varianten wie beim Dashboard (`?dashboard=<name>`): `empty` ohne Zeilen, `sparse` nur die
// letzten drei Wochen, `extreme` sehr große Werte an jedem Tag, alle übrigen wie `rich`.

import { addDays, daysBetween } from '../dates';
import type { Progress, ProgressEntry, Sport } from './types';
import type { DashboardVariant } from './mock-dashboard';

/** Erster Tag: der frühere aus 1. Januar des Vorjahres und Monatserstem 23 Monate zurück. */
export function progressFrom(today: string): string {
  const year = Number(today.slice(0, 4));
  const month = Number(today.slice(5, 7)) - 1 - 23;
  const y = year + Math.floor(month / 12);
  const m = ((month % 12) + 12) % 12;
  const monthsBack = `${String(y).padStart(4, '0')}-${String(m + 1).padStart(2, '0')}-01`;
  const prevYear = `${String(year - 1).padStart(4, '0')}-01-01`;
  return monthsBack < prevYear ? monthsBack : prevYear;
}

type Rnd = () => number;

function entry(
  date: string,
  sport: Sport,
  duration_s: number,
  distance_m: number,
  load: number | null,
): ProgressEntry {
  return {
    date,
    sport,
    count: 1,
    duration_s: Math.round(duration_s),
    distance_m: Math.round(distance_m),
    load: load === null ? 0 : Math.round(load * 10) / 10,
    load_missing: load === null ? 1 : 0,
  };
}

export function buildProgress(
  today: string,
  variant: DashboardVariant,
  mulberry32: (seed: number) => Rnd,
): Progress {
  const from = progressFrom(today);
  const entries: ProgressEntry[] = [];
  if (variant === 'empty') return { today, from, entries };
  const start = variant === 'sparse' ? addDays(today, -20) : from;
  const thisYear = today.slice(0, 4);
  for (let d = start; d <= today; d = addDays(d, 1)) {
    const rnd = mulberry32(daysBetween('2020-01-01', d) * 104_729 + 11);
    const weekday = (new Date(`${d}T00:00:00Z`).getUTCDay() + 6) % 7;
    if (variant === 'extreme') {
      entries.push(entry(d, 'cycling', 6 * 3600, 220_000, 480 + rnd() * 40));
      entries.push(entry(d, 'running', 3 * 3600, 42_000, 260));
      continue;
    }
    // Urlaub: zwei Wochen ab Mitte August jedes Jahres ohne Training.
    const md = d.slice(5);
    if (md >= '08-12' && md <= '08-25') continue;
    // Das Vorjahr ist etwas ruhiger, das laufende Jahr baut auf.
    const scale = d.slice(0, 4) === thisYear ? 1 : 0.82;
    if (weekday === 1 || weekday === 4) {
      const t = (2400 + rnd() * 1500) * scale;
      const v = 3.1 + rnd() * 0.5;
      entries.push(entry(d, 'running', t, t * v, (t / 3600) * 62));
    } else if (weekday === 2) {
      const t = (3600 + rnd() * 1200) * scale;
      entries.push(entry(d, 'cycling', t, t * 8.6, (t / 3600) * 78));
    } else if (weekday === 3 && rnd() < 0.5) {
      const t = 2100 + rnd() * 600;
      entries.push(entry(d, 'swimming', t, t * 0.9, (t / 3600) * 50));
    } else if (weekday === 0 && rnd() < 0.6) {
      // Montag: Kraft ohne Messkanal, also ohne berechenbare Last.
      entries.push(entry(d, 'strength', 2700, 0, null));
    } else if (weekday === 5) {
      const t = (9000 + rnd() * 5400) * scale;
      entries.push(entry(d, 'cycling', t, t * 7.9, (t / 3600) * 66));
    } else if (weekday === 6 && rnd() < 0.7) {
      const t = (4200 + rnd() * 1800) * scale;
      const v = 3.0 + rnd() * 0.3;
      entries.push(entry(d, 'running', t, t * v, (t / 3600) * 58));
    }
  }
  return { today, from, entries };
}
