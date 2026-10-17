<script lang="ts">
  import type { Dashboard } from '../../lib/backend/types';
  import { readChartColors } from '../../lib/charts';
  import { efData, efOptions } from '../../lib/charts-dashboard';
  import Metric from '../../lib/components/Metric.svelte';
  import Section from '../../lib/components/Section.svelte';
  import UplotChart from '../../lib/components/UplotChart.svelte';
  import { directionOf, efficiencyStatement, signedPercent } from '../../lib/dashboard';
  import { formatNumber } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { resolvedTheme } from '../../lib/state';

  // Effizienz: Leistung je Herzschlag (NP durch mittlere HF) langer Radeinheiten, 28-Tage-Mittel.
  let { dash }: { dash: Dashboard } = $props();

  const e = $derived(dash.efficiency);
  const options = $derived.by(() => {
    void $resolvedTheme;
    return efOptions(readChartColors(), $locale, {
      date: $t('ui.dash.efficiency.date'),
      ef: $t('ui.dash.efficiency.series'),
    });
  });
  const delta = $derived(
    e.change_percent === null
      ? undefined
      : {
          text: $t('ui.dash.efficiency.vs_before', {
            value: signedPercent(e.change_percent, $locale, 1),
          }),
          direction: directionOf(e.change_percent, 1),
          tone: e.trend === 'efficiency.rising' ? ('good' as const) : ('neutral' as const),
        },
  );
</script>

<Section
  label={$t('ui.dash.efficiency.title')}
  statement={efficiencyStatement(e, $t, $locale)}
  testid="dash-efficiency"
>
  {#if e.current !== null}
    <Metric
      value={formatNumber(e.current, $locale, 2)}
      unit={$t('ui.dash.efficiency.unit')}
      label={$t('ui.dash.efficiency.label')}
      {delta}
      testid="efficiency-metric"
    />
  {/if}
  {#if e.series.length > 1}
    <div class="chart">
      <UplotChart
        {options}
        data={efData(e.series)}
        height={150}
        label={$t('ui.dash.efficiency.chart_label')}
      />
    </div>
  {/if}
  {#if e.sessions > 0}
    <p class="note muted">{$t('ui.dash.efficiency.sessions', { n: e.sessions })}</p>
  {/if}
</Section>

<style>
  .chart {
    margin-top: var(--space-4);
  }

  .note {
    margin: var(--space-2) 0 0;
    font-size: var(--fs-small);
  }
</style>
