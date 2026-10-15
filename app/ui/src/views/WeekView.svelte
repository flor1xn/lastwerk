<script lang="ts">
  import { getBackend } from '../lib/backend';
  import type { WeekOverview } from '../lib/backend/types';
  import { viewDefaults } from '../lib/defaults';
  import { addDays, localIso, mondayOf } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import Metric from '../lib/components/Metric.svelte';
  import PmcChart from '../lib/components/PmcChart.svelte';
  import Section from '../lib/components/Section.svelte';
  import StatRow from '../lib/components/StatRow.svelte';
  import WeekBars from '../lib/components/WeekBars.svelte';
  import { formatDate, formatNumber } from '../lib/format';
  import { locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { dataVersion } from '../lib/state';

  const loader = new Loader<WeekOverview>();
  let weekStart = $state(mondayOf(localIso()));

  $effect(() => {
    const start = weekStart;
    void $dataVersion;
    void loader.run(() => getBackend().getWeekOverview(start, viewDefaults.pmcWeeks));
  });

  const data = $derived(loader.data);
  const last = $derived(data && data.pmc.length > 0 ? data.pmc[data.pmc.length - 1] : null);
  const total = $derived(data ? data.totals_by_sport.reduce((s, x) => s + x.load, 0) : 0);

  // Der Startwert-Hinweis gilt, wenn der erste Tag mit Last im sichtbaren Verlauf liegt.
  const startNote = $derived(
    data?.pmc_first_load_date &&
      data.pmc.length > 0 &&
      data.pmc[0].date <= data.pmc_first_load_date &&
      data.pmc_first_load_date <= data.pmc[data.pmc.length - 1].date
      ? data.pmc_first_load_date
      : null,
  );
</script>

<div class="page">
  <header class="page-head">
    <h1>{$t('ui.nav.week')}</h1>
    <p class="page-lead" data-testid="lead">{$t('ui.lead.week')}</p>
  </header>

  <div class="nav">
    <button onclick={() => (weekStart = addDays(weekStart, -7))} aria-label={$t('ui.week.previous')}
      >&lt;</button
    >
    <h2 data-testid="week-range">
      {#if data}
        {$t('ui.week.range', {
          from: formatDate(data.week_start, $locale),
          to: formatDate(data.week_end, $locale),
        })}
      {:else}
        {$t('common.loading')}
      {/if}
    </h2>
    <button onclick={() => (weekStart = addDays(weekStart, 7))} aria-label={$t('ui.week.next')}
      >&gt;</button
    >
    <button onclick={() => (weekStart = mondayOf(localIso()))}>{$t('ui.week.current')}</button>
  </div>

  {#if loader.error}
    <ErrorNote error={loader.error} />
  {:else if data}
    <Section label={$t('ui.week.load_title')}>
      {#if data.totals_by_sport.length > 0}
        <div data-testid="week-summary">
          <StatRow>
            <Metric size="xl" value={formatNumber(total, $locale, 1)} label={$t('ui.week.total')} />
            {#each data.totals_by_sport as part (part.sport)}
              <Metric
                value={formatNumber(part.load, $locale, 1)}
                label={$t(`ui.sport.${part.sport}`)}
                swatch={`var(--sport-${part.sport})`}
              />
            {/each}
          </StatRow>
        </div>
      {:else}
        <p class="muted" data-testid="week-summary">{$t('ui.week.no_load')}</p>
      {/if}
      <div class="bars">
        <WeekBars days={data.days} />
      </div>
      {#if data.not_computable > 0}
        <p class="warn" data-testid="week-not-computable">
          {$t('ui.week.not_computable', { n: data.not_computable })}
        </p>
      {/if}
    </Section>

    <Section label={$t('ui.week.pmc_title')}>
      {#snippet aside()}
        {#if last}
          <p class="stand muted">{formatDate(last.date, $locale)}</p>
        {/if}
      {/snippet}
      {#if data.pmc.length === 0}
        <p class="muted">{$t('ui.week.no_pmc')}</p>
      {:else}
        {#if last}
          <div data-testid="pmc-values">
            <StatRow>
              <Metric value={formatNumber(last.ctl, $locale, 1)} label={$t('ui.week.ctl')} />
              <Metric value={formatNumber(last.atl, $locale, 1)} label={$t('ui.week.atl')} />
              <Metric value={formatNumber(last.tsb, $locale, 1)} label={$t('ui.week.tsb')} />
            </StatRow>
          </div>
        {/if}
        <div class="chart">
          <PmcChart history={data.pmc} height={260} label={$t('ui.week.pmc_title')} />
        </div>
        {#if startNote}
          <p class="muted note">
            {$t('ui.week.pmc_start_note', { date: formatDate(startNote, $locale) })}
          </p>
        {/if}
      {/if}
    </Section>
  {/if}
</div>

<style>
  .nav {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-6);
  }

  .nav h2 {
    min-width: 240px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .bars {
    margin-top: var(--space-6);
  }

  .chart {
    margin-top: var(--space-5);
  }

  .stand {
    margin: 0;
    font-size: var(--fs-small);
  }

  .note {
    margin: var(--space-2) 0 0;
    font-size: var(--fs-small);
  }

  .warn {
    margin: var(--space-4) 0 0;
  }
</style>
