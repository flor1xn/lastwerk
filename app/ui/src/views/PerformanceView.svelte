<script lang="ts">
  import { getBackend } from '../lib/backend';
  import {
    PERF_RANGES,
    type Dashboard,
    type Performance,
    type PerfRange,
  } from '../lib/backend/types';
  import { readChartColors } from '../lib/charts';
  import { curveData, curveOptions, ftpData, ftpOptions } from '../lib/charts-performance';
  import { signedPercent } from '../lib/dashboard';
  import { localIso } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import Section from '../lib/components/Section.svelte';
  import Segmented from '../lib/components/Segmented.svelte';
  import UplotChart from '../lib/components/UplotChart.svelte';
  import { formatDate, formatDuration, formatNumber, formatPace, MISSING } from '../lib/format';
  import { locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import {
    curveCounts,
    curveSummary,
    distanceKey,
    formatCurveDuration,
    formatThresholdValue,
    formatTimeDiff,
    runCounts,
    TABLE_DURATIONS,
    thresholdView,
  } from '../lib/performance';
  import { dataVersion, openActivity, openSettings, resolvedTheme } from '../lib/state';
  import EfficiencySection from './history/EfficiencySection.svelte';
  import PowerSection from './history/PowerSection.svelte';

  // Leistung (T33): Leistungskurve je Zeitraum gegen den gleich langen Zeitraum davor. Weitere
  // Abschnitte (Schwellen T36, Laufbestzeiten T37) folgen auf derselben Seite.
  const loader = new Loader<Performance>();
  const today = localIso();
  let range = $state<PerfRange>('90');

  $effect(() => {
    void $dataVersion;
    const r = range;
    void loader.run(() => getBackend().getPerformance(today, r));
  });

  // Effizienz und Leistungskurve der letzten 90 Tage (seit T44 hier statt auf "Heute"). Sie
  // kommen aus dem Dashboard und hängen nicht am Zeitraum der Seite.
  const dashLoader = new Loader<Dashboard>();
  $effect(() => {
    void $dataVersion;
    void dashLoader.run(() => getBackend().getDashboard(today));
  });

  const data = $derived(loader.data);
  const power = $derived(data?.power ?? []);
  const withValues = $derived(power.filter((p) => p.current !== null));
  const hasPrevious = $derived(power.some((p) => p.previous_w !== null));
  const summary = $derived(curveSummary(power));
  const counts = $derived(curveCounts(power));
  const rangeLabel = $derived($t(`ui.perf.range_long.${range}`));

  const statement = $derived.by(() => {
    if (withValues.length === 0) return $t('ui.perf.power.empty');
    if (range === 'all') return $t('ui.perf.power.all');
    if (summary === 'power_curve.no_data') return $t('ui.perf.power.no_previous');
    return $t(`ui.perf.power.statement.${summary}`, {
      rising: counts.rising,
      known: counts.known,
    });
  });

  let hovered = $state<number | null>(null);
  const hoveredPoint = $derived(hovered === null ? null : (power[hovered] ?? null));

  const options = $derived.by(() => {
    void $resolvedTheme;
    return curveOptions(
      readChartColors(),
      $locale,
      {
        duration: $t('ui.perf.power.duration'),
        current: rangeLabel,
        previous: $t('ui.perf.power.previous'),
      },
      hasPrevious,
    );
  });

  const runBests = $derived(data?.run_bests ?? []);
  const run = $derived(runCounts(runBests));
  const runHasPrevious = $derived(runBests.some((r) => r.previous_s !== null));
  const runStatement = $derived.by(() => {
    if (run.known === 0) return $t('ui.perf.run.empty');
    if (range === 'all') return $t('ui.perf.run.all');
    if (run.compared === 0) return $t('ui.perf.run.plain');
    return $t('ui.perf.run.statement', { faster: run.faster, compared: run.compared });
  });

  const thresholds = $derived(data?.thresholds ?? []);
  const suggestions = $derived(data?.ftp_suggestions ?? []);
  const thr = $derived(thresholdView(thresholds, suggestions));
  const monthLabel = (ym: string) =>
    new Intl.DateTimeFormat($locale === 'de' ? 'de-DE' : 'en-US', {
      month: 'long',
      year: 'numeric',
      timeZone: 'UTC',
    }).format(new Date(`${ym}-01T00:00:00Z`));
  const thrStatement = $derived.by(() => {
    switch (thr.kind) {
      case 'empty':
        return $t('ui.perf.thr.empty');
      case 'others':
        return $t('ui.perf.thr.others');
      case 'no_ftp':
        return $t('ui.perf.thr.no_ftp', {
          value: formatThresholdValue('ftp_w', thr.suggestion.value_w, $locale),
          month: monthLabel(thr.suggestion.month),
        });
      case 'current':
        return $t('ui.perf.thr.current', {
          value: formatThresholdValue('ftp_w', thr.ftp.value, $locale),
          date: formatDate(thr.ftp.valid_from, $locale),
        });
      case 'above':
        return $t('ui.perf.thr.above', {
          month: monthLabel(thr.suggestion.month),
          value: formatThresholdValue('ftp_w', thr.suggestion.value_w, $locale),
          ftp: formatThresholdValue('ftp_w', thr.ftp.value, $locale),
        });
    }
  });
  const hasFtpChart = $derived(
    thresholds.some((t) => t.kind === 'ftp_w') || suggestions.length > 0,
  );
  const ftpChartOptions = $derived.by(() => {
    void $resolvedTheme;
    return ftpOptions(readChartColors(), $locale, {
      date: $t('ui.perf.power.date'),
      ftp: $t('ui.threshold.kind.ftp_w'),
      suggestion: $t('ui.perf.thr.suggestion'),
    });
  });

  const rows = $derived(power.filter((p) => TABLE_DURATIONS.includes(p.duration_s)));
  const rangeOptions = $derived(
    PERF_RANGES.map((r) => ({ value: r, label: $t(`ui.perf.range.${r}`) })),
  );
</script>

<div class="page performance" data-testid="performance">
  <p class="page-lead" data-testid="lead">{$t('ui.lead.performance')}</p>

  {#if dashLoader.data}
    <div class="dash-grid">
      <EfficiencySection dash={dashLoader.data} />
      <PowerSection dash={dashLoader.data} />
    </div>
  {/if}

  <div class="controls">
    <Segmented
      options={rangeOptions}
      value={range}
      label={$t('ui.perf.range_label')}
      onchange={(r) => (range = r)}
      testid="perf-range"
    />
  </div>

  {#if loader.error}
    <ErrorNote error={loader.error} />
  {:else if !data}
    <p class="muted">{$t('common.loading')}</p>
  {:else}
    <Section label={$t('ui.perf.power.title')} {statement} testid="perf-power">
      {#if withValues.length > 0}
        <div class="chart" data-testid="perf-curve">
          <UplotChart
            {options}
            data={curveData(power)}
            height={260}
            label={$t('ui.perf.power.chart_label')}
            oncursor={(i) => (hovered = i)}
          />
          <div class="foot">
            <p class="readout muted" data-testid="perf-readout">
              {#if hoveredPoint}
                {formatCurveDuration(hoveredPoint.duration_s)}:
                {hoveredPoint.current === null
                  ? MISSING
                  : `${formatNumber(hoveredPoint.current.value, $locale, 0)} W`}
                {#if hoveredPoint.previous_w !== null}
                  · {$t('ui.perf.power.previous')}
                  {formatNumber(hoveredPoint.previous_w, $locale, 0)} W
                {/if}
              {:else}
                {$t('ui.perf.power.hint')}
              {/if}
            </p>
            <ul class="legend">
              <li><span class="sw now"></span>{rangeLabel}</li>
              {#if hasPrevious}
                <li><span class="sw before"></span>{$t('ui.perf.power.previous')}</li>
              {/if}
            </ul>
          </div>
        </div>

        <div class="table-scroll">
          <table data-testid="perf-table">
            <thead>
              <tr>
                <th>{$t('ui.perf.power.duration')}</th>
                <th class="num">{$t('ui.perf.power.best')}</th>
                <th>{$t('ui.perf.power.date')}</th>
                {#if hasPrevious}
                  <th class="num">{$t('ui.perf.power.previous')}</th>
                  <th class="num">{$t('ui.perf.power.change')}</th>
                {/if}
              </tr>
            </thead>
            <tbody>
              {#each rows as r (r.duration_s)}
                <tr data-testid="perf-row" data-trend={r.trend}>
                  <td>{formatCurveDuration(r.duration_s)}</td>
                  <td class="num strong">
                    {r.current === null
                      ? MISSING
                      : `${formatNumber(r.current.value, $locale, 0)} W`}
                  </td>
                  <td>
                    {#if r.current}
                      <button
                        class="link"
                        type="button"
                        title={$t('ui.perf.open_activity')}
                        onclick={() => openActivity(r.current!.activity_id)}
                        >{formatDate(r.current.date, $locale)}</button
                      >
                    {:else}
                      <span class="muted">{MISSING}</span>
                    {/if}
                  </td>
                  {#if hasPrevious}
                    <td class="num muted">
                      {r.previous_w === null
                        ? MISSING
                        : `${formatNumber(r.previous_w, $locale, 0)} W`}
                    </td>
                    <td class="num change {r.trend.split('.')[1]}">
                      {r.change_percent === null
                        ? MISSING
                        : signedPercent(r.change_percent, $locale, 1)}
                    </td>
                  {/if}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        <p class="note muted">{$t('ui.perf.power.note')}</p>
      {/if}
    </Section>

    <Section label={$t('ui.perf.run.title')} statement={runStatement} testid="perf-run">
      {#if run.known > 0}
        <div class="table-scroll">
          <table data-testid="run-table">
            <thead>
              <tr>
                <th>{$t('ui.perf.run.distance_label')}</th>
                <th class="num">{$t('ui.perf.run.time')}</th>
                <th class="num">{$t('ui.detail.pace')}</th>
                <th>{$t('ui.perf.power.date')}</th>
                {#if runHasPrevious}
                  <th class="num">{$t('ui.perf.power.previous')}</th>
                  <th class="num">{$t('ui.perf.run.diff')}</th>
                {/if}
              </tr>
            </thead>
            <tbody>
              {#each runBests as r (r.distance_m)}
                <tr data-testid="run-row" class:empty={r.current === null}>
                  <td>{$t(distanceKey(r.distance_m))}</td>
                  <td class="num strong">
                    {r.current === null ? MISSING : formatDuration(r.current.value)}
                  </td>
                  <td class="num">
                    {r.current === null
                      ? MISSING
                      : `${formatPace(r.distance_m / r.current.value)} /km`}
                  </td>
                  <td>
                    {#if r.current}
                      <button
                        class="link"
                        type="button"
                        title={$t('ui.perf.open_activity')}
                        onclick={() => openActivity(r.current!.activity_id)}
                        >{formatDate(r.current.date, $locale)}</button
                      >
                    {:else}
                      <span class="muted">{MISSING}</span>
                    {/if}
                  </td>
                  {#if runHasPrevious}
                    <td class="num muted">
                      {r.previous_s === null ? MISSING : formatDuration(r.previous_s)}
                    </td>
                    <td
                      class="num change"
                      class:rising={r.current !== null &&
                        r.previous_s !== null &&
                        r.current.value < r.previous_s}
                    >
                      {r.current === null || r.previous_s === null
                        ? MISSING
                        : formatTimeDiff(r.current.value - r.previous_s)}
                    </td>
                  {/if}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        <p class="note muted">{$t('ui.perf.run.note')}</p>
      {/if}
    </Section>

    <Section label={$t('ui.perf.thr.title')} statement={thrStatement} testid="perf-thresholds">
      {#if thr.kind === 'above' || thr.kind === 'no_ftp' || thr.kind === 'empty'}
        <div class="actions">
          <button
            type="button"
            onclick={() => openSettings('profile')}
            data-testid="perf-to-settings"
          >
            {$t('ui.perf.thr.to_settings')}
          </button>
        </div>
      {/if}
      {#if hasFtpChart}
        <div class="chart" data-testid="ftp-chart">
          <UplotChart
            options={ftpChartOptions}
            data={ftpData(thresholds, suggestions, today)}
            height={180}
            label={$t('ui.perf.thr.chart_label')}
          />
          <ul class="legend">
            <li><span class="sw now"></span>{$t('ui.threshold.kind.ftp_w')}</li>
            <li><span class="dot"></span>{$t('ui.perf.thr.suggestion')}</li>
          </ul>
        </div>
      {/if}
      {#if thresholds.length > 0}
        <div class="table-scroll">
          <table data-testid="thr-table">
            <thead>
              <tr>
                <th>{$t('ui.threshold.col_kind')}</th>
                <th class="num">{$t('ui.threshold.value')}</th>
                <th>{$t('ui.threshold.valid_from')}</th>
                <th>{$t('ui.perf.thr.origin_label')}</th>
              </tr>
            </thead>
            <tbody>
              {#each thresholds as th, i (i)}
                <tr data-testid="thr-row">
                  <td>
                    {$t(`ui.threshold.kind.${th.kind}`)}{th.sport
                      ? ` · ${$t(`ui.sport.${th.sport}`)}`
                      : ''}
                  </td>
                  <td class="num strong">{formatThresholdValue(th.kind, th.value, $locale)}</td>
                  <td>{formatDate(th.valid_from, $locale)}</td>
                  <td class="muted">{$t(`ui.perf.thr.origin.${th.origin}`)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
      {#if suggestions.length > 0}
        <p class="note muted">{$t('ui.perf.thr.note')}</p>
      {/if}
    </Section>
  {/if}
</div>

<style>
  /* Wie das Raster auf "Heute": zwei Spalten ab 760 px Seitenbreite. */
  .dash-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    column-gap: var(--space-7);
    margin-bottom: var(--space-5);
  }

  @container (min-width: 760px) {
    .dash-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  .performance {
    max-width: var(--page-max);
    container-type: inline-size;
  }

  .controls {
    margin: 0 0 var(--space-5);
  }

  .chart {
    margin-bottom: var(--space-5);
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--space-2) var(--space-4);
    margin-top: var(--space-2);
    font-size: var(--fs-small);
  }

  .readout {
    margin: 0;
    min-height: 1.4em;
  }

  .legend {
    display: flex;
    gap: var(--space-4);
    margin: 0;
    padding: 0;
    list-style: none;
    color: var(--muted);
  }

  .sw {
    display: inline-block;
    width: 16px;
    height: 0;
    margin: 0 6px 3px 0;
    vertical-align: middle;
    border-top: 3px solid var(--power);
  }

  .dot {
    display: inline-block;
    width: 9px;
    height: 9px;
    margin-right: 6px;
    border-radius: 50%;
    background: var(--accent);
  }

  .actions {
    margin: 0 0 var(--space-4);
  }

  .sw.before {
    border-top: 2px dashed var(--muted);
  }

  .table-scroll {
    overflow-x: auto;
  }

  table {
    min-width: 420px;
  }

  tr.empty td {
    color: var(--muted);
  }

  .strong {
    font-weight: var(--fw-semibold);
  }

  .change.rising {
    color: var(--ok);
  }

  .link {
    padding: 0;
    min-height: 0;
    border: none;
    background: none;
    color: var(--text);
    text-decoration: underline;
    text-decoration-color: var(--line);
    text-underline-offset: 3px;
  }

  .link:hover:not(:disabled) {
    text-decoration-color: currentColor;
  }

  .note {
    margin: var(--space-3) 0 0;
    font-size: var(--fs-small);
  }
</style>
