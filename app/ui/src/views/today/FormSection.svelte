<script lang="ts">
  import type { Dashboard } from '../../lib/backend/types';
  import Metric from '../../lib/components/Metric.svelte';
  import PmcChart from '../../lib/components/PmcChart.svelte';
  import Section from '../../lib/components/Section.svelte';
  import StatRow from '../../lib/components/StatRow.svelte';
  import { formName } from '../../lib/dashboard';
  import { formatNumber } from '../../lib/format';
  import { garminSync } from '../../lib/garmin';
  import { locale, t } from '../../lib/i18n';

  // Form-Verlauf: TSB groß mit Zustand als Text, Fitness und Ermüdung daneben, darunter der
  // Verlauf der letzten 120 Tage mit den vier Formzuständen als Bänder und der gestrichelten
  // Prognose aus dem aktiven Plan.
  let { dash }: { dash: Dashboard } = $props();

  const form = $derived(dash.form);
  const state = $derived(form ? formName(form.state) : null);
  // Form laut Plan in sieben Tagen (Prognose beginnt morgen, der siebte Tag ist Index 6).
  const ahead = $derived(dash.projection.length >= 7 ? dash.projection[6] : null);
</script>

<Section
  label={$t('ui.dash.form.title')}
  statement={state
    ? $t(`ui.form.${state}_hint`)
    : $garminSync.kind === 'running'
      ? $t('ui.dash.form.loading')
      : $t('ui.dash.form.empty')}
  testid="today-form"
>
  {#if form && state}
    <StatRow>
      <Metric
        size="xl"
        value={formatNumber(form.tsb, $locale, 0)}
        valueTestid="form-value"
        label={$t('ui.week.tsb')}
      >
        {#snippet extra()}
          <span class="state-mark {state}" data-testid="form-state">{$t(`ui.form.${state}`)}</span>
        {/snippet}
      </Metric>
      <Metric value={formatNumber(form.ctl, $locale, 0)} label={$t('ui.week.ctl')} />
      <Metric value={formatNumber(form.atl, $locale, 0)} label={$t('ui.week.atl')} />
    </StatRow>
    <div class="chart">
      <PmcChart
        history={dash.pmc}
        projection={dash.projection}
        bands
        height={260}
        label={$t('ui.week.pmc_title')}
        testid="today-pmc"
      />
    </div>
    {#if dash.projection.length > 0}
      <p class="note muted" data-testid="projection-note">
        {$t('ui.dash.form.projection_note')}
        {#if ahead}
          {$t('ui.dash.form.week_ahead', { tsb: formatNumber(ahead.tsb, $locale, 0) })}
        {/if}
      </p>
    {/if}
  {/if}
</Section>

<style>
  .chart {
    margin-top: var(--space-5);
  }

  .note {
    margin: var(--space-3) 0 0;
    font-size: var(--fs-small);
  }
</style>
