import { describe, expect, it } from 'vitest';
import {
  ADHERENCE_CODES,
  DISTRIBUTION_CODES,
  EFFICIENCY_CODES,
  FORM_CODES,
  POWER_CURVE_CODES,
  RAMP_CODES,
  READINESS_CODES,
  DECISION_CODES,
  DECISION_REASONS,
  DECISION_MISSING,
  INSIGHT_CODES,
  VARIANT_KINDS,
  type DashPowerDuration,
  type DashReadiness,
} from './backend/types';
import {
  adherenceStatement,
  buildHeatmap,
  calendarStats,
  denseSeries,
  directionOf,
  distributionStatement,
  efficiencyStatement,
  formName,
  heatThresholds,
  insightStatement,
  lastPart,
  levelOf,
  powerCurveStatement,
  powerCurveSummary,
  rampStatement,
  readinessStatement,
  signed,
  signedPercent,
  sparkPath,
  splitPmc,
  statementKey,
  formatPowerDuration,
} from './dashboard';
import { catalogs, hasKey, translate } from './i18n';
import type { Locale } from './backend/types';

const tr = (locale: Locale) => (key: string, params?: Record<string, string | number>) =>
  translate(locale, key, params);

const readiness = (over: Partial<DashReadiness> = {}): DashReadiness => ({
  state: 'readiness.normal',
  last_date: '2026-10-15',
  date: '2026-10-15',
  hrv: null,
  resting_hr: null,
  sleep_h: null,
  body_battery_high: null,
  body_battery_low: null,
  plan_adjusted: false,
  series: [],
  ...over,
});

const duration = (trend: DashPowerDuration['trend']): DashPowerDuration => ({
  duration_s: 60,
  current_w: 300,
  previous_w: 290,
  change_percent: 3,
  trend,
});

describe('Aussagen aus Zustandscodes', () => {
  const all = [
    ...RAMP_CODES,
    ...DISTRIBUTION_CODES,
    ...ADHERENCE_CODES,
    ...EFFICIENCY_CODES,
    ...POWER_CURVE_CODES,
    ...READINESS_CODES,
  ];

  it('haben zu jedem Code einen Text in beiden Sprachen', () => {
    for (const code of all) {
      expect(hasKey('de', statementKey(code)), `de:${code}`).toBe(true);
      expect(hasKey('en', statementKey(code)), `en:${code}`).toBe(true);
    }
    for (const code of FORM_CODES) {
      expect(hasKey('de', `ui.form.${formName(code)}_hint`), code).toBe(true);
      expect(hasKey('en', `ui.form.${formName(code)}_hint`), code).toBe(true);
    }
  });

  it('beschreiben Trainingsdaten und sagen nichts über Gesundheit (SPEC 2.3)', () => {
    const banned =
      /(krank|gesund|diagnos|verletz|übertrain|erschöpf|medizin|illness|injur|overtrain|disease|health)/i;
    for (const lang of ['de', 'en'] as const) {
      for (const [key, text] of Object.entries(catalogs[lang])) {
        if (!/^ui\.(dash|decision|insight|week_story)\./.test(key)) continue;
        expect(text, `${lang}:${key}`).not.toMatch(banned);
      }
    }
  });

  it('füllen die Platzhalter aus den Werten', () => {
    const ramp = { current: 6.2, state: 'ramp.steep' as const, limit: 5, series: [] };
    expect(rampStatement(ramp, tr('de'), 'de')).toBe(
      'Deine Fitness wächst um 6,2 Punkte pro Woche, schneller als die Grenze von 5 im Plan.',
    );
    expect(rampStatement({ ...ramp, current: -1.5, state: 'ramp.falling' }, tr('en'), 'en')).toBe(
      'Your fitness is falling by 1.5 points per week.',
    );
    const dist = {
      window_easy_percent: 62.4,
      window_total_s: 7200,
      min_easy_percent: 70,
      state: 'distribution.intensity_heavy' as const,
      weeks: [],
    };
    expect(distributionStatement(dist, tr('de'), 'de')).toContain('Nur 62 %');
    expect(
      efficiencyStatement(
        {
          current: 1.6,
          change_percent: -3.46,
          trend: 'efficiency.falling',
          sessions: 4,
          series: [],
        },
        tr('de'),
        'de',
      ),
    ).toContain('3,5 %');
  });
});

describe('leere Abschnitte sagen, was fehlt', () => {
  it('Planerfüllung ohne Plan und ohne Soll', () => {
    const a = { ratio_percent: null, state: 'adherence.no_data' as const, weeks: [] };
    expect(adherenceStatement(a, false, tr('de'), 'de')).toBe(
      'Ohne aktiven Plan gibt es nichts zu vergleichen.',
    );
    expect(adherenceStatement(a, true, tr('de'), 'de')).toContain('noch keine Last geplant');
  });

  it('Bereitschaft: nie Daten, zu alt, zu kurze Basis', () => {
    const never = readiness({ state: 'readiness.no_data', last_date: null, date: null });
    expect(readinessStatement(never, tr('de'), 'de')).toBe(
      'Verbinde Garmin für Erholungswerte, dann erscheinen hier HRV, Ruhepuls und Schlaf im Vergleich zu deinem eigenen Schnitt.',
    );
    const stale = readiness({ state: 'readiness.no_data', date: null, last_date: '2026-09-01' });
    expect(readinessStatement(stale, tr('de'), 'de')).toContain('01.09.2026');
    const short = readiness({ state: 'readiness.no_data' });
    expect(readinessStatement(short, tr('en'), 'en')).toContain('more days');
    expect(readinessStatement(readiness({ state: 'readiness.low' }), tr('de'), 'de')).toContain(
      'deutlich unter',
    );
  });

  it('Leistungskurve ohne vergleichbare Dauer', () => {
    expect(powerCurveSummary([])).toBe('power_curve.no_data');
    expect(powerCurveStatement([duration('power_curve.no_data')], tr('de'))).toContain('fehlen');
  });

  it('Kalender ohne Last', () => {
    expect(tr('de')('ui.dash.calendar.empty').length).toBeGreaterThan(10);
  });
});

describe('Gesamturteil der Leistungskurve', () => {
  it('zählt gestiegene gegen gesunkene Dauern', () => {
    expect(
      powerCurveSummary([
        duration('power_curve.rising'),
        duration('power_curve.rising'),
        duration('power_curve.falling'),
      ]),
    ).toBe('power_curve.rising');
    expect(powerCurveSummary([duration('power_curve.falling'), duration('power_curve.flat')])).toBe(
      'power_curve.falling',
    );
    expect(
      powerCurveSummary([duration('power_curve.rising'), duration('power_curve.falling')]),
    ).toBe('power_curve.flat');
    expect(powerCurveSummary([duration('power_curve.flat'), duration('power_curve.no_data')])).toBe(
      'power_curve.flat',
    );
  });
});

describe('Zahlen mit Vorzeichen', () => {
  it('nutzt das echte Minuszeichen und nie -0', () => {
    expect(signed(3.24, 'de')).toBe('+3,2');
    expect(signed(-3.24, 'de')).toBe('−3,2');
    expect(signed(-0.04, 'de')).toBe('0,0');
    expect(signed(2.5, 'en')).toBe('+2.5');
    expect(signedPercent(-12, 'de')).toBe('−12 %');
  });

  it('wählt die Pfeilrichtung nach der angezeigten Zahl', () => {
    expect(directionOf(0.4, 0)).toBe('flat');
    expect(directionOf(0.6, 0)).toBe('up');
    expect(directionOf(-0.04, 1)).toBe('flat');
    expect(directionOf(-0.6, 1)).toBe('down');
  });

  it('kürzt Dauern', () => {
    expect([5, 60, 300, 1200, 3600].map(formatPowerDuration)).toEqual([
      '5 s',
      '1 min',
      '5 min',
      '20 min',
      '1 h',
    ]);
  });
});

describe('Verlauf mit Prognose', () => {
  const p = (date: string, v: number) => ({ date, load: 0, ctl: v, atl: v + 1, tsb: v - 1 });

  it('schließt die Prognose am letzten Ist-Tag an', () => {
    const s = splitPmc([p('2026-10-14', 10), p('2026-10-15', 11)], [p('2026-10-16', 12)]);
    expect(s.dates).toEqual(['2026-10-14', '2026-10-15', '2026-10-16']);
    expect(s.ctl).toEqual([10, 11, null]);
    expect(s.ctlProjected).toEqual([null, 11, 12]);
    expect(s.projectionStart).toBe(2);
  });

  it('hat ohne Prognose keine gestrichelten Werte', () => {
    const s = splitPmc([p('2026-10-14', 10), p('2026-10-15', 11)], []);
    expect(s.ctlProjected).toEqual([null, null]);
    expect(s.projectionStart).toBe(-1);
  });
});

describe('Mini-Verlauf', () => {
  it('unterbricht die Linie an Lücken', () => {
    const d = sparkPath([1, 2, null, 3, 4], 100, 20, 0);
    expect(d.match(/M/g)).toHaveLength(2);
  });

  it('zeichnet leere und konstante Reihen ohne Fehler', () => {
    expect(sparkPath([], 100, 20)).toBe('');
    expect(sparkPath([null, null, null], 100, 20)).toBe('');
    expect(sparkPath([5, 5, 5], 100, 20)).toMatch(/^M/);
    expect(sparkPath([7], 100, 20)).toBe('');
  });

  it('füllt fehlende Tage mit null', () => {
    const pts = [
      { date: '2026-10-13', v: 1 },
      { date: '2026-10-15', v: 3 },
    ];
    expect(denseSeries(pts, (x) => x.v, '2026-10-15', 4)).toEqual([null, 1, null, 3]);
  });
});

describe('Belastungskalender', () => {
  it('legt Tage in Wochenspalten, Montag oben', () => {
    // 15.10.2026 ist ein Donnerstag, der 12.10. der Montag der Woche.
    const map = buildHeatmap([{ date: '2026-10-12', load: 50 }], '2026-10-15', 10);
    const monday = map.cells.find((c) => c.date === '2026-10-12');
    const thursday = map.cells.find((c) => c.date === '2026-10-15');
    expect(monday?.row).toBe(0);
    expect(thursday?.row).toBe(3);
    expect(monday?.col).toBe(thursday?.col);
    expect(monday?.load).toBe(50);
    expect(map.cells.find((c) => c.date === '2026-10-14')?.load).toBeNull();
  });

  it('stuft nach Quartilen und lässt Ausreißer die Skala nicht plattdrücken', () => {
    const th = heatThresholds([10, 20, 30, 40, 5000]);
    expect(levelOf(null, th)).toBe(0);
    expect(levelOf(0, th)).toBe(0);
    expect(levelOf(10, th)).toBe(1);
    expect(levelOf(40, th)).toBe(3);
    expect(levelOf(5000, th)).toBe(4);
    expect(heatThresholds([])).toEqual([1, 2, 3]);
  });

  it('deckt 365 Tage und markiert Monatsanfänge ohne Überlappung', () => {
    const map = buildHeatmap([], '2026-10-15', 365);
    expect(map.cells).toHaveLength(365);
    expect(map.cols).toBeGreaterThanOrEqual(52);
    expect(map.cols).toBeLessThanOrEqual(54);
    for (let i = 1; i < map.months.length; i++) {
      expect(map.months[i].col - map.months[i - 1].col).toBeGreaterThanOrEqual(3);
    }
  });

  it('zählt Tage, Summe und stärkste Woche', () => {
    const s = calendarStats([
      { date: '2026-10-12', load: 50 },
      { date: '2026-10-13', load: 30 },
      { date: '2026-10-19', load: 70 },
      { date: '2026-10-20', load: 0 },
    ]);
    expect(s).toEqual({ trainingDays: 3, total: 150, bestWeek: 80 });
    expect(calendarStats([])).toEqual({ trainingDays: 0, total: 0, bestWeek: 0 });
  });
});

describe('Tagesentscheidung und Erkenntnisse (T44)', () => {
  it('hat Texte für jeden Code in beiden Sprachen', () => {
    for (const lang of ['de', 'en'] as const) {
      for (const c of DECISION_CODES) {
        expect(hasKey(lang, `ui.decision.state.${lastPart(c)}`), c).toBe(true);
        expect(hasKey(lang, `ui.decision.ask.${lastPart(c)}`), c).toBe(true);
      }
      for (const c of ['no_plan', 'rest', 'lighten_empty']) {
        expect(hasKey(lang, `ui.decision.hint.${c}`), c).toBe(true);
      }
      for (const r of DECISION_REASONS) {
        expect(hasKey(lang, `ui.decision.reason.${lastPart(r)}`), r).toBe(true);
      }
      for (const m of DECISION_MISSING) expect(hasKey(lang, `ui.decision.missing.${m}`)).toBe(true);
      for (const k of VARIANT_KINDS) expect(hasKey(lang, `ui.decision.kind.${k}`)).toBe(true);
      for (const c of INSIGHT_CODES) expect(hasKey(lang, `ui.insight.ask.${c}`), c).toBe(true);
    }
  });

  it('nimmt für Erkenntnisse die Aussage des Abschnitts', async () => {
    const { MockBackend } = await import('./backend/mock');
    const dash = await new MockBackend({ today: '2026-10-14', dashboard: 'extreme' }).getDashboard(
      '2026-10-14',
    );
    for (const lang of ['de', 'en'] as const) {
      for (const code of INSIGHT_CODES) {
        const text = insightStatement(code, dash, tr(lang), lang);
        expect(text.length, code).toBeGreaterThan(10);
        expect(text, code).not.toMatch(/ui\.|\{|NaN/);
      }
    }
    expect(insightStatement('form.overloaded', dash, tr('de'), 'de')).toBe(
      translate('de', 'ui.form.overloaded_hint'),
    );
  });
});
