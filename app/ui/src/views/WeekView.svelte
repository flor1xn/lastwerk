<script lang="ts">
  // Reiter Kalender, Umschalter Woche (T42): Last der Woche je Tag und Sportart. Den Verlauf von
  // Fitness, Ermüdung und Form zeigt Heute, deshalb hier kein zweites PMC-Diagramm.
  import { getBackend } from '../lib/backend';
  import type { WeekOverview } from '../lib/backend/types';
  import { viewDefaults } from '../lib/defaults';
  import { addDays, localIso, mondayOf } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import Metric from '../lib/components/Metric.svelte';
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
  const total = $derived(data ? data.totals_by_sport.reduce((s, x) => s + x.load, 0) : 0);
</script>

<div class="page">
  <p class="page-lead" data-testid="lead">{$t('ui.lead.week')}</p>

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

  .warn {
    margin: var(--space-4) 0 0;
  }
</style>
