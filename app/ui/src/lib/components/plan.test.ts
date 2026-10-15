import { cleanup, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { MockBackend } from '../backend/mock';
import type { PlanDetail } from '../backend/types';
import { setLocale } from '../i18n';
import PlanDisplay from './PlanDisplay.svelte';

// Das Diagramm braucht Canvas, das jsdom nicht hat. Die Daten des Verlaufs prüft der Test der
// Attrappe, hier geht es um die Anzeige von Text.
vi.mock('./UplotChart.svelte', async () => import('./UplotChartStub.svelte'));

afterEach(() => {
  cleanup();
  setLocale('de');
});

let base: PlanDetail;

beforeAll(async () => {
  const backend = new MockBackend({ today: '2026-10-13' });
  base = await backend.generatePlan({
    week_start: '2026-10-19',
    goal_date: null,
    goal_sport: null,
    weeks: 3,
    locale: 'de',
    sports: ['cycling', 'running'],
    strength_per_week: 1,
  });
});

function withPlan(edit: (d: PlanDetail) => void): PlanDetail {
  const d = structuredClone(base);
  edit(d);
  return d;
}

describe('PlanDisplay', () => {
  it('zeigt Wochen mit Phase, Ziel- und Planlast', () => {
    render(PlanDisplay, { detail: base });
    const weeks = screen.getAllByTestId('plan-week');
    expect(weeks).toHaveLength(3);
    expect(within(weeks[0]).getByTestId('plan-phase').textContent).toBe('Basis');
    expect(weeks[0].dataset.phase).toBe('base');
    expect(weeks[2].dataset.phase).toBe('recovery');
    for (const w of weeks) {
      expect(within(w).getAllByTestId('plan-day')).toHaveLength(7);
      expect(within(w).getByTestId('week-target').textContent).toMatch(/^\d+,\d$/);
      expect(within(w).getByTestId('week-planned').textContent).toMatch(/^\d+,\d$/);
    }
    // Begründung der Woche aus den Sprachdateien.
    expect(within(weeks[0]).getByTestId('week-reasons').textContent).toContain('Basisphase');
  });

  it('zeigt je Tag Einheiten mit Vorlagenname, Dauer, Last und Härte', () => {
    render(PlanDisplay, { detail: base });
    const sessions = screen.getAllByTestId('plan-session');
    expect(sessions.length).toBeGreaterThan(8);
    const hard = sessions.find((s) => s.dataset.hard === 'true');
    expect(hard).toBeDefined();
    expect(hard?.textContent).toContain('hart');
    expect(hard?.textContent).toMatch(/\d+ min/);
    expect(hard?.textContent).toMatch(/Last \d+,\d/);
    // Name der Vorlage statt ihrer Kennung.
    expect(screen.getAllByText('Grundlagenfahrt').length).toBeGreaterThan(0);
    expect(screen.queryByText('bike.endurance.steady')).toBeNull();
    // Tage ohne Einheit sind als Ruhetag gekennzeichnet.
    expect(screen.getAllByText('Ruhetag').length).toBeGreaterThan(0);
  });

  it('zeigt Warnungen mit Code, Text und Quelle', () => {
    render(PlanDisplay, {
      detail: withPlan((d) => {
        d.plan.warnings = [
          { code: 'R02', severity: 'warn', date: '2026-10-26' },
          { code: 'R06', severity: 'info' },
        ];
      }),
    });
    const items = screen.getAllByTestId('plan-warning');
    expect(items.map((i) => i.dataset.code)).toEqual(['R06', 'R02']);
    const r02 = items[1];
    expect(r02.textContent).toContain('R02');
    expect(r02.textContent).toContain('Anstieg des Wochenvolumens');
    expect(within(r02).getByTestId('plan-warning-text').textContent).toContain(
      'Quelle: Faustregel',
    );
    expect(r02.textContent).toContain('26.10.2026');
    expect(items[0].dataset.severity).toBe('info');
  });

  it('zeigt eine Regel ohne Text nur mit dem Code', () => {
    render(PlanDisplay, {
      detail: withPlan((d) => {
        d.plan.warnings = [{ code: 'R99', severity: 'warn' }];
      }),
    });
    const item = screen.getByTestId('plan-warning');
    expect(item.textContent).toContain('R99');
    expect(item.textContent).toContain('Regel R99');
    expect(within(item).queryByTestId('plan-warning-text')).toBeNull();
  });

  it('zeigt ohne Warnungen einen Hinweis und meldet eine harte Verletzung', () => {
    const { unmount } = render(PlanDisplay, {
      detail: withPlan((d) => {
        d.plan.warnings = [];
        d.validation = { schema_errors: [], violations: [] };
      }),
    });
    expect(screen.getByTestId('plan-no-warnings').textContent).toBe('Keine Warnungen.');
    expect(screen.getByTestId('plan-validation').textContent).toContain('Schema in Ordnung');
    unmount();

    render(PlanDisplay, {
      detail: withPlan((d) => {
        d.validation = {
          schema_errors: [],
          violations: [{ code: 'R05', hard: true, date: null, detail: {} }],
        };
      }),
    });
    const v = screen.getByTestId('plan-validation');
    expect(v.dataset.valid).toBe('false');
    expect(v.textContent).toContain('Harte Verletzungen: 1');
    expect(screen.getByTestId('plan-hard').textContent).toContain('Ruhetag');
  });

  it('zeigt Texte des Sprachmodells als Text, nie als HTML (SPEC 9.5)', () => {
    const evil =
      '<script>window.hacked = 1</script><img src=x onerror="window.hacked = 2"> **fett**';
    const { container } = render(PlanDisplay, {
      detail: withPlan((d) => {
        d.plan.source = 'llm';
        d.plan.explanation = evil;
        // Auch ein fremder Schlüssel und ein fremder Regelcode bleiben Text.
        d.plan.weeks[0].reason_keys = ['<b>fett</b>'];
        d.plan.warnings = [{ code: '<i>R1</i>', severity: 'warn' }];
      }),
    });
    const text = screen.getByTestId('plan-explanation');
    expect(text.textContent).toBe(evil);
    expect(text.children).toHaveLength(0);
    expect(container.querySelector('script')).toBeNull();
    expect(container.querySelector('img')).toBeNull();
    expect(container.querySelector('b')).toBeNull();
    expect(container.querySelector('i')).toBeNull();
    expect(screen.getAllByTestId('week-reasons')[0].textContent).toContain('<b>fett</b>');
    expect(screen.getByTestId('plan-warning').textContent).toContain('<i>R1</i>');
    expect((window as unknown as { hacked?: number }).hacked).toBeUndefined();
  });

  it('wechselt mit der Sprache', () => {
    setLocale('en');
    render(PlanDisplay, { detail: base });
    expect(screen.getAllByTestId('plan-phase')[0].textContent).toBe('Base');
    expect(screen.getAllByText('Steady endurance ride').length).toBeGreaterThanOrEqual(0);
    expect(screen.getByTestId('plan-validation').textContent).toContain('Schema valid');
  });
});
