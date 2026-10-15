<script lang="ts">
  import type { Dashboard } from '../../lib/backend/types';
  import { readChartColors } from '../../lib/charts';
  import { rampData, rampOptions } from '../../lib/charts-dashboard';
  import Metric from '../../lib/components/Metric.svelte';
  import Section from '../../lib/components/Section.svelte';
  import UplotChart from '../../lib/components/UplotChart.svelte';
  import { rampStatement, signed } from '../../lib/dashboard';
  import { formatNumber } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { resolvedTheme } from '../../lib/state';

  // Rampe: wie schnell die Fitness (CTL) in sieben Tagen wächst, gegen die Grenze des Planers.
  let { dash }: { dash: Dashboard } = $props();

  const ramp = $derived(dash.ramp);
  const options = $derived.by(() => {
    void $resolvedTheme;
    return rampOptions(
      readChartColors(),
      $locale,
      {
        date: $t('ui.dash.ramp.date'),
        ramp: $t('ui.dash.ramp.label'),
        limit: $t('ui.dash.ramp.limit', { value: formatNumber(ramp.limit, $locale, 0) }),
      },
      ramp.limit,
    );
  });
</script>

<Section
  label={$t('ui.dash.ramp.title')}
  statement={rampStatement(ramp, $t, $locale)}
  testid="today-ramp"
>
  {#if ramp.current !== null}
    <Metric
      value={signed(ramp.current, $locale, 1)}
      unit={$t('ui.dash.ramp.unit')}
      label={$t('ui.dash.ramp.label')}
      testid="ramp-metric"
    />
  {/if}
  {#if ramp.series.length > 1}
    <div class="chart">
      <UplotChart
        {options}
        data={rampData(ramp.series)}
        height={150}
        label={$t('ui.dash.ramp.chart_label')}
      />
    </div>
  {/if}
</Section>

<style>
  .chart {
    margin-top: var(--space-4);
  }
</style>
