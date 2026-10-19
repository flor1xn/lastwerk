<script lang="ts">
  import { getBackend } from '../lib/backend';
  import type { Dashboard, Progress, Sport } from '../lib/backend/types';
  import { readChartColors } from '../lib/charts';
  import { cumulativeData, cumulativeOptions } from '../lib/charts-progress';
  import { directionOf, signedPercent } from '../lib/dashboard';
  import { localIso } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import Metric from '../lib/components/Metric.svelte';
  import Section from '../lib/components/Section.svelte';
  import Segmented from '../lib/components/Segmented.svelte';
  import StatRow from '../lib/components/StatRow.svelte';
  import UplotChart from '../lib/components/UplotChart.svelte';
  import WeekStackBars from '../lib/components/WeekStackBars.svelte';
  import { formatNumber, MISSING } from '../lib/format';
  import { intlTag, locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import {
    changePercent,
    cumulative,
    filterSport,
    metricValue,
    monthly,
    PROGRESS_METRICS,
    sportsPresent,
    weekly,
    yearToDate,
    type ProgressMetric,
  } from '../lib/progress';
  import { dataVersion, resolvedTheme } from '../lib/state';
  import CalendarSection from './history/CalendarSection.svelte';
  import DistributionSection from './history/DistributionSection.svelte';
  import FormHistorySection from './history/FormHistorySection.svelte';
  import RampSection from './history/RampSection.svelte';

  // Fortschritt (T32): dieses Jahr gegen das Vorjahr bis zum selben Tag, Wochenumfang über
  // 52 Wochen, Monate über 24 Monate. Die Oberfläche summiert nur (lib/progress.ts).
  const loader = new Loader<Progress>();
  const today = localIso();

  let metric = $state<ProgressMetric>('duration');
  let sport = $state<Sport | 'all'>('all');

  $effect(() => {
    void $dataVersion;
    void loader.run(() => getBackend().getProgress(today));
  });

  // Form-Verlauf, Rampe und Verteilung (seit T63) und der Belastungskalender der letzten 365 Tage
  // (seit T44) stehen hier statt auf "Heute": Sie sind Trends, keine Frage des Tages.
  const dashLoader = new Loader<Dashboard>();
  $effect(() => {
    void $dataVersion;
    void dashLoader.run(() => getBackend().getDashboard(today));
  });

  const data = $derived(loader.data);
  const all = $derived(data?.entries ?? []);
  const sports = $derived(sportsPresent(all));
  const entries = $derived(filterSport(all, sport === 'all' ? null : sport));
  const ytd = $derived(yearToDate(entries, today));
  const current = $derived(metricValue(ytd.current, metric));
  const previous = $derived(metricValue(ytd.previous, metric));
  const change = $derived(changePercent(current, previous));
  const weeks = $derived(weekly(entries, today));
  const months = $derived(monthly(entries, today));
  const shownSports = $derived(sport === 'all' ? sports : [sport]);

  // Einheit und Nachkommastellen je Größe: Stunden mit einer Stelle, Kilometer und Last ganz.
  const unit = $derived(
    metric === 'duration' ? 'h' : metric === 'distance' ? 'km' : $t('ui.progress.load_unit'),
  );
  const digits = $derived(metric === 'duration' ? 1 : 0);
  const fmt = (v: number) => formatNumber(v, $locale, digits);

  const lead = $derived.by(() => {
    if (all.length === 0) return $t('ui.progress.lead.empty');
    const value = `${fmt(current)} ${unit}`;
    if (change === null) return $t('ui.progress.lead.first', { value });
    return $t('ui.progress.lead.compare', { value, change: signedPercent(change, $locale, 0) });
  });

  const delta = $derived(
    change === null
      ? undefined
      : {
          text: $t('ui.progress.vs_previous', { value: signedPercent(change, $locale, 0) }),
          direction: directionOf(change, 1),
          tone: 'neutral' as const,
        },
  );

  const options = $derived.by(() => {
    void $resolvedTheme;
    return cumulativeOptions(
      readChartColors(),
      $locale,
      {
        day: $t('ui.progress.year.day'),
        current: String(ytd.year),
        previous: String(ytd.year - 1),
      },
      digits,
    );
  });
  const chartData = $derived(
    cumulativeData(
      cumulative(entries, ytd.year, today, metric),
      cumulative(entries, ytd.year - 1, today, metric),
    ),
  );

  const monthName = (ym: string) =>
    new Intl.DateTimeFormat(intlTag($locale), {
      month: 'long',
      year: 'numeric',
      timeZone: 'UTC',
    }).format(new Date(`${ym}-01T00:00:00Z`));

  const metricOptions = $derived(
    PROGRESS_METRICS.map((m) => ({ value: m, label: $t(`ui.progress.metric.${m}`) })),
  );
</script>

{#if loader.error}
  <ErrorNote error={loader.error} />
{:else if !data}
  <p class="muted">{$t('common.loading')}</p>
{:else}
  <div class="page progress" data-testid="progress">
    <p class="page-lead" data-testid="lead">{lead}</p>

    {#if dashLoader.data}
      <FormHistorySection dash={dashLoader.data} />
      <div class="pair">
        <RampSection dash={dashLoader.data} />
        <DistributionSection dash={dashLoader.data} />
      </div>
    {/if}

    <div class="controls">
      <Segmented
        options={metricOptions}
        value={metric}
        label={$t('ui.progress.metric_label')}
        onchange={(m) => (metric = m)}
        testid="progress-metric"
      />
      {#if sports.length > 1}
        <label class="sport">
          <span class="visually-hidden">{$t('ui.progress.sport_label')}</span>
          <select bind:value={sport} data-testid="progress-sport">
            <option value="all">{$t('ui.sport.all')}</option>
            {#each sports as s (s)}
              <option value={s}>{$t(`ui.sport.${s}`)}</option>
            {/each}
          </select>
        </label>
      {/if}
    </div>

    <Section
      label={$t('ui.progress.year.title', { year: ytd.year })}
      statement={$t('ui.progress.year.statement')}
      testid="progress-year"
    >
      {#if all.length === 0}
        <p class="muted">{$t('ui.progress.none')}</p>
      {:else}
        <StatRow>
          <Metric
            value={fmt(current)}
            {unit}
            label={$t('ui.progress.year.current')}
            {delta}
            size="lg"
            testid="progress-current"
          />
          <Metric
            value={ytd.previous.count === 0 ? MISSING : fmt(previous)}
            unit={ytd.previous.count === 0 ? undefined : unit}
            label={$t('ui.progress.year.previous', { year: ytd.year - 1 })}
            testid="progress-previous"
          />
          <Metric
            value={String(ytd.current.count)}
            label={$t('ui.progress.year.sessions')}
            testid="progress-sessions"
          />
        </StatRow>
      {/if}
      {#if metric === 'load' && ytd.current.load_missing > 0}
        <p class="note muted" data-testid="progress-load-missing">
          {$t('ui.progress.load_missing', { n: ytd.current.load_missing })}
        </p>
      {/if}
      {#if all.length > 0}
        <div class="chart">
          <UplotChart
            {options}
            data={chartData}
            height={220}
            label={$t('ui.progress.year.chart_label')}
          />
          <ul class="legend">
            <li><span class="sw now"></span>{ytd.year}</li>
            <li><span class="sw before"></span>{ytd.year - 1}</li>
          </ul>
        </div>
      {/if}
    </Section>

    <Section
      label={$t('ui.progress.weeks.title')}
      statement={$t('ui.progress.weeks.statement')}
      testid="progress-weeks"
    >
      {#if all.length === 0}
        <p class="muted">{$t('ui.progress.none')}</p>
      {:else}
        <WeekStackBars {weeks} {metric} sports={shownSports} {unit} {digits} testid="week-bars" />
      {/if}
    </Section>

    <Section
      label={$t('ui.progress.months.title')}
      statement={$t('ui.progress.months.statement')}
      testid="progress-months"
    >
      {#if all.length === 0}
        <p class="muted">{$t('ui.progress.none')}</p>
      {:else}
        <div class="table-scroll">
          <table data-testid="progress-month-table">
            <thead>
              <tr>
                <th>{$t('ui.progress.months.month')}</th>
                <th class="num">{$t('ui.progress.months.sessions')}</th>
                <th class="num" class:sel={metric === 'duration'}>
                  {$t('ui.progress.metric.duration')}
                </th>
                <th class="num" class:sel={metric === 'distance'}>
                  {$t('ui.progress.metric.distance')}
                </th>
                <th class="num" class:sel={metric === 'load'}>{$t('ui.progress.metric.load')}</th>
              </tr>
            </thead>
            <tbody>
              {#each months as m (m.month)}
                <tr data-testid="progress-month" class:empty={m.total.count === 0}>
                  <td>{monthName(m.month)}</td>
                  <td class="num">{m.total.count === 0 ? MISSING : m.total.count}</td>
                  <td class="num" class:sel={metric === 'duration'}>
                    {m.total.count === 0
                      ? MISSING
                      : `${formatNumber(m.total.duration_s / 3600, $locale, 1)} h`}
                  </td>
                  <td class="num" class:sel={metric === 'distance'}>
                    {m.total.count === 0
                      ? MISSING
                      : `${formatNumber(m.total.distance_m / 1000, $locale, 0)} km`}
                  </td>
                  <td class="num" class:sel={metric === 'load'}>
                    {m.total.count === 0 || m.total.count === m.total.load_missing
                      ? MISSING
                      : formatNumber(m.total.load, $locale, 0)}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </Section>

    {#if dashLoader.data}
      <CalendarSection dash={dashLoader.data} />
    {/if}
  </div>
{/if}

<style>
  .progress {
    container-type: inline-size;
  }

  /* Rampe und Verteilung nebeneinander, jeder Abschnitt trägt seine Linie oben. */
  .pair {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    column-gap: var(--space-7);
  }

  @container (min-width: 760px) {
    .pair {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  .progress {
    max-width: var(--page-max);
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    margin: 0 0 var(--space-5);
  }

  .chart {
    margin-top: var(--space-5);
  }

  .note {
    margin: var(--space-3) 0 0;
    font-size: var(--fs-small);
  }

  .legend {
    display: flex;
    gap: var(--space-4);
    margin: var(--space-2) 0 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .sw {
    display: inline-block;
    width: 16px;
    height: 0;
    margin: 0 6px 3px 0;
    vertical-align: middle;
    border-top: 3px solid var(--accent);
  }

  .sw.before {
    border-top: 2px dashed var(--muted);
  }

  .table-scroll {
    overflow-x: auto;
  }

  table {
    min-width: 480px;
  }

  th.sel,
  td.sel {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }

  tr.empty td {
    color: var(--muted);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
