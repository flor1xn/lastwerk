import { describe, expect, it } from 'vitest';
import templates from '../../../../catalog/templates.json';
import type { Plan, PlanWarning } from './backend/types';
import { MockBackend } from './backend/mock';
import { catalogs, hasKey, translate } from './i18n';
import {
  describeGoal,
  formatCounts,
  keyText,
  nextMonday,
  planTotals,
  ruleInfo,
  sortedWarnings,
  templateName,
} from './plan';

const t = (key: string, params?: Record<string, string | number>) => translate('de', key, params);

function plan(over: Partial<Plan> = {}): Plan {
  return {
    schema_version: 1,
    plan_id: '00000000-0000-4000-8000-000000000001',
    created_at: '2026-10-13T08:00:00Z',
    locale: 'de',
    source: 'rules',
    rules_version: '1.0.0',
    goal: { kind: 'continuous' },
    warnings: [],
    weeks: [],
    ...over,
  };
}

const session = (minutes: number, load: number) => ({
  slot: 'am' as const,
  sport: 'cycling' as const,
  template_id: 'bike.endurance.steady',
  duration_min: minutes,
  planned_load: load,
  hard: false,
});

describe('keyText und Namen', () => {
  it('liefert den Text der Sprache oder den Schlüssel selbst', () => {
    expect(keyText('de', 'reason.week.growth')).toContain('moderat');
    expect(keyText('en', 'reason.week.growth')).not.toBe(keyText('de', 'reason.week.growth'));
    expect(keyText('de', 'reason.week.gibt_es_nicht')).toBe('reason.week.gibt_es_nicht');
    // Ein fremder Text bleibt Text und wird nicht als Schlüssel gedeutet.
    expect(keyText('de', '<b>x</b>')).toBe('<b>x</b>');
  });

  it('schlägt den Vorlagennamen nach', () => {
    expect(templateName('de', 'bike.endurance.steady')).toBe('Grundlagenfahrt');
    expect(templateName('de', 'bike.unbekannt.x_y')).toBe('bike unbekannt x y');
  });
});

describe('ruleInfo (Warnungen mit Code, Text und Quelle)', () => {
  it('nennt Name und Erklärung mit der Quelle', () => {
    const r = ruleInfo('de', 'R03');
    expect(r.name).toBe('Harte Einheiten je Woche');
    expect(r.explanation).toContain('Quelle: Faustregel');
    // R2 verweist auf Quellen aus SPEC Anhang A.
    expect(ruleInfo('de', 'R02').explanation).toContain('Q12');
  });

  it('gibt für eine unbekannte Regel nur den Code zurück', () => {
    expect(ruleInfo('de', 'R99')).toEqual({ name: 'Regel R99', explanation: null });
    expect(ruleInfo('en', 'R99').name).toBe('Rule R99');
    expect(ruleInfo('de', '<script>').explanation).toBeNull();
  });

  it('hat für R01 bis R11 in beiden Sprachen Name und Erklärung mit Quelle', () => {
    for (const lang of ['de', 'en'] as const) {
      for (let n = 1; n <= 11; n++) {
        const code = `R${String(n).padStart(2, '0')}`;
        const r = ruleInfo(lang, code);
        expect(r.name, `${lang}:${code}`).not.toMatch(/^(Regel|Rule) R/);
        expect(r.explanation, `${lang}:${code}`).toMatch(/Quelle|Source/);
      }
    }
  });
});

describe('Summen und Reihenfolge', () => {
  // Handrechnung: Woche 1 meldet planned_load 80,5 (50,0 + 30,5), 60 + 45 = 105 Minuten.
  // Woche 2 hat kein planned_load, die Summe der Einheiten zählt: 70,0 und 90 Minuten.
  // Zusammen 3 Einheiten, 195 Minuten, Last 150,5.
  it('summiert Einheiten, Minuten und Last', () => {
    const p = plan({
      weeks: [
        {
          week_start: '2026-10-19',
          phase: 'build',
          target_load: 80,
          planned_load: 80.5,
          days: [
            { date: '2026-10-19', sessions: [session(60, 50)] },
            { date: '2026-10-20', sessions: [session(45, 30.5)] },
          ],
        },
        {
          week_start: '2026-10-26',
          phase: 'recovery',
          target_load: 70,
          days: [{ date: '2026-10-26', sessions: [session(90, 70)] }],
        },
      ],
    });
    expect(planTotals(p)).toEqual({ sessions: 3, minutes: 195, load: 150.5 });
  });

  it('sortiert Warnungen nach Datum und Code, ohne Datum zuerst', () => {
    const w: PlanWarning[] = [
      { code: 'R10', severity: 'warn', date: '2026-11-02' },
      { code: 'R02', severity: 'warn', date: '2026-11-02' },
      { code: 'R06', severity: 'info' },
      { code: 'R03', severity: 'warn', date: '2026-10-26' },
    ];
    expect(sortedWarnings(w).map((x) => x.code)).toEqual(['R06', 'R03', 'R02', 'R10']);
  });

  it('wählt als Planbeginn den Montag nach dem angegebenen Tag', () => {
    expect(nextMonday('2026-10-13')).toBe('2026-10-19'); // Dienstag
    expect(nextMonday('2026-10-12')).toBe('2026-10-19'); // Montag
    expect(nextMonday('2026-10-18')).toBe('2026-10-19'); // Sonntag
  });
});

describe('Formatierung', () => {
  const date = (iso: string) => iso.split('-').reverse().join('.');

  it('beschreibt Ziele', () => {
    expect(describeGoal(t, { kind: 'continuous' }, date)).toBe('Fortlaufend');
    expect(
      describeGoal(
        t,
        { kind: 'event', event_date: '2027-05-01', sport: 'cycling', priority: 'A' },
        date,
      ),
    ).toBe('Termin 01.05.2027, Rad, Priorität A');
    expect(describeGoal(t, { kind: 'event', event_date: '2027-05-01', sport: null }, date)).toBe(
      'Termin 01.05.2027',
    );
  });

  it('fasst die Zeilen eines Backups zusammen', () => {
    expect(
      formatCounts(t, {
        thresholds: 3,
        goals: 1,
        availability: 7,
        plans: 2,
        manual_sessions: 0,
        settings: 5,
        recovery_days: 9,
      }),
    ).toBe(
      '3 Schwellenwerte, 1 Ziele, 7 Verfügbarkeiten, 2 Pläne, 5 Einstellungen, 9 Erholungstage',
    );
  });
});

describe('Texte für Pläne', () => {
  it('hat zu jeder Vorlage des Katalogs einen Namen in beiden Sprachen', () => {
    for (const tpl of templates.templates) {
      for (const lang of ['de', 'en'] as const) {
        expect(hasKey(lang, `template.${tpl.id}.name`), `${lang}:${tpl.id}`).toBe(true);
      }
    }
  });

  it('deckt Phasen, Slots, Status und Herkunft ab', () => {
    const keys = [
      ...['base', 'build', 'peak', 'taper', 'recovery', 'race'].map((x) => `ui.plan.phase.${x}`),
      ...['am', 'pm'].map((x) => `ui.plan.slot.${x}`),
      ...['draft', 'active', 'archived'].map((x) => `ui.plan.status.${x}`),
      ...['rules', 'llm'].map((x) => `ui.plan.source.${x}`),
      ...['info', 'warn'].map((x) => `ui.plan.warnings.${x}`),
      ...['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'].map((x) => `ui.plan.avail.weekday.${x}`),
    ];
    for (const key of keys) {
      expect(key in catalogs.de, `de:${key}`).toBe(true);
      expect(key in catalogs.en, `en:${key}`).toBe(true);
    }
  });

  it('nutzt in der Attrappe nur Schlüssel, die es gibt', async () => {
    const backend = new MockBackend({ today: '2026-10-13' });
    const detail = await backend.generatePlan({
      week_start: '2026-10-19',
      goal_date: null,
      goal_sport: null,
      weeks: 3,
      locale: 'de',
      sports: ['cycling', 'running', 'swimming'],
      strength_per_week: 2,
    });
    const keys = new Set<string>();
    for (const w of detail.plan.weeks) {
      w.reason_keys?.forEach((k) => keys.add(k));
      for (const d of w.days) {
        for (const s of d.sessions) {
          keys.add(`template.${s.template_id}.name`);
          s.reason_keys?.forEach((k) => keys.add(k));
        }
      }
    }
    expect(keys.size).toBeGreaterThan(5);
    for (const key of keys) expect(hasKey('de', key), key).toBe(true);
  });
});
