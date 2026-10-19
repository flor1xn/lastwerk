import { cleanup, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MockBackend } from '../../lib/backend/mock';
import FormHistorySection from './FormHistorySection.svelte';

// Kein Canvas in jsdom.
vi.mock(
  '../../lib/components/UplotChart.svelte',
  async () => import('../../lib/components/UplotChartStub.svelte'),
);

afterEach(cleanup);

describe('Form-Verlauf im Fortschritt (seit T63)', () => {
  it('zeigt die Prognose des aktiven Plans gestrichelt erst mit Plan', async () => {
    const backend = new MockBackend({ today: '2026-10-14' });
    render(FormHistorySection, { dash: await backend.getDashboard('2026-10-14') });
    expect(screen.getByTestId('dash-form')).toBeTruthy();
    expect(screen.queryByTestId('legend-projection')).toBeNull();
    cleanup();

    const detail = await backend.generatePlan({
      week_start: '2026-10-12',
      goal_date: null,
      goal_sport: null,
      weeks: 3,
      locale: 'de',
      sports: ['cycling'],
      strength_per_week: 0,
    });
    await backend.setPlanStatus(detail.plan.plan_id, 'active');
    render(FormHistorySection, { dash: await backend.getDashboard('2026-10-14') });
    expect(screen.getByTestId('legend-projection')).toBeTruthy();
    expect(screen.getByTestId('projection-note')).toBeTruthy();
  });
});
