<script lang="ts">
  import { getBackend } from '../lib/backend';
  import type { ActivityDetail } from '../lib/backend/types';
  import { activitySeriesData, activitySeriesOptions, readChartColors } from '../lib/charts';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import LoadCell from '../lib/components/LoadCell.svelte';
  import Metric from '../lib/components/Metric.svelte';
  import StatRow from '../lib/components/StatRow.svelte';
  import UplotChart from '../lib/components/UplotChart.svelte';
  import ZoneTable from '../lib/components/ZoneTable.svelte';
  import {
    formatDate,
    formatDistance,
    formatDuration,
    formatNumber,
    formatPace,
    formatSpeed,
    MISSING,
  } from '../lib/format';
  import { locale, t } from '../lib/i18n';
  import { missingText, trainingEffectText } from '../lib/load';
  import { Loader } from '../lib/loader.svelte';
  import {
    closeActivity,
    dataVersion,
    openSettings,
    resolvedTheme,
    selectedActivityId,
  } from '../lib/state';

  // Detail an Ort und Stelle im Reiter Aktivitäten (T42): Die Ansicht erscheint nur mit gewählter
  // Aktivität, "Zurück" führt zur Liste.
  const loader = new Loader<ActivityDetail>();

  $effect(() => {
    const id = $selectedActivityId;
    void $dataVersion;
    if (id === null) return;
    void loader.run(() => getBackend().getActivity(id));
  });

  const d = $derived(loader.data);
  const missing = $derived(d ? missingText(d.item.load) : null);
  // Garmins eigene Zahlen, nur zur Ansicht (E2): Lastwerk rechnet nie damit.
  const garmin = $derived(
    d?.garmin_load &&
      (d.garmin_load.training_load !== null || d.garmin_load.training_effect !== null)
      ? d.garmin_load
      : null,
  );
  const hasPower = $derived(!!d && d.series.power_w.some((v) => v !== null));
  const hasHeart = $derived(!!d && d.series.hr_bpm.some((v) => v !== null));
  // Tempo nur bei Läufen (min/km); andere Sportarten zeigen in den Runden km/h.
  const isRun = $derived(d?.item.sport === 'running');
  const hasPace = $derived(
    !!d && isRun && (d.series.speed_mps ?? []).some((v) => v !== null && v > 0),
  );
  const lapsHavePower = $derived(!!d && d.laps.some((l) => l.avg_power_w !== null));
  const lapsHaveHeart = $derived(!!d && d.laps.some((l) => l.avg_hr_bpm !== null));
  const lapsHaveSpeed = $derived(!!d && d.laps.some((l) => l.avg_speed_mps !== null));
  const lapsHaveDistance = $derived(!!d && d.laps.some((l) => l.distance_m !== null));

  const options = $derived.by(() => {
    void $resolvedTheme;
    return activitySeriesOptions(
      readChartColors(),
      $locale,
      {
        time: $t('ui.detail.time'),
        power: $t('ui.detail.power'),
        heart: $t('ui.detail.heart_rate'),
        pace: $t('ui.detail.pace_series'),
      },
      { power: hasPower, heart: hasHeart, pace: hasPace },
    );
  });
</script>

{#if loader.error}
  <ErrorNote error={loader.error} />
{:else if d}
  <div class="head">
    <button onclick={closeActivity} data-testid="detail-back">{$t('ui.detail.back')}</button>
    <h2 data-testid="detail-title">
      {$t(`ui.sport.${d.item.sport}`)} · {formatDate(d.item.local_date, $locale)}
    </h2>
    {#if !d.item.is_primary}
      <span class="flag" title={$t('ui.activities.duplicate_hint')}
        >{$t('ui.activities.duplicate')}</span
      >
    {/if}
  </div>
  <p class="page-lead" data-testid="lead">{$t('ui.lead.detail')}</p>

  <div class="grid">
    <section class="panel">
      <h3>{$t('ui.detail.metrics')}</h3>
      <dl data-testid="metrics">
        <dt>{$t('ui.detail.duration')}</dt>
        <dd>{formatDuration(d.item.duration_s)}</dd>
        <dt>{$t('ui.detail.moving')}</dt>
        <dd>{formatDuration(d.moving_s)}</dd>
        <dt>{$t('ui.detail.distance')}</dt>
        <dd>{formatDistance(d.item.distance_m, $locale)}</dd>
        <dt>{$t('ui.detail.avg_hr')}</dt>
        <dd>
          {d.item.avg_hr === null ? MISSING : `${formatNumber(d.item.avg_hr, $locale, 0)} bpm`}
        </dd>
        <dt>{$t('ui.detail.max_hr')}</dt>
        <dd>{d.max_hr === null ? MISSING : `${formatNumber(d.max_hr, $locale, 0)} bpm`}</dd>
        <dt>{$t('ui.detail.avg_power')}</dt>
        <dd>
          {d.item.avg_power_w === null
            ? MISSING
            : `${formatNumber(d.item.avg_power_w, $locale, 0)} W`}
        </dd>
        <dt>{$t('ui.detail.np')}</dt>
        <dd>{d.np_w === null ? MISSING : `${formatNumber(d.np_w, $locale, 0)} W`}</dd>
        <dt>{$t('ui.detail.load')}</dt>
        <dd><LoadCell load={d.item.load} /></dd>
        <dt title={$t('ui.load.if_hint')}>{$t('ui.load.if')}</dt>
        <dd>
          {d.item.load.intensity_factor === null
            ? MISSING
            : formatNumber(d.item.load.intensity_factor, $locale, 2)}
        </dd>
        {#if garmin}
          <dt>{$t('ui.detail.garmin.load')}</dt>
          <dd data-testid="garmin-load">
            {garmin.training_load === null
              ? MISSING
              : formatNumber(garmin.training_load, $locale, 0)}
            <span class="flag">{$t('ui.detail.garmin.tag')}</span>
          </dd>
          <dt>{$t('ui.detail.garmin.effect_label')}</dt>
          <dd data-testid="garmin-effect">
            {garmin.training_effect === null
              ? MISSING
              : trainingEffectText(garmin.training_effect, $t)}
          </dd>
        {/if}
        <dt>{$t('ui.detail.device')}</dt>
        <dd>{d.device_product ?? MISSING}</dd>
      </dl>
      {#if garmin}
        <p class="muted reason" data-testid="garmin-load-note">{$t('ui.detail.garmin.note')}</p>
      {/if}
      {#if missing}
        <p class="warn reason" data-testid="load-reason" data-missing={d.item.load.missing}>
          {$t(missing.what)}. {$t(missing.hint)}
        </p>
        <p class="reason">
          <button onclick={() => openSettings('profile')} data-testid="load-reason-settings"
            >{$t('ui.load.to_settings')}</button
          >
        </p>
      {:else if d.item.load.status !== 'ok'}
        <p class="warn reason" data-testid="load-reason">
          {$t(`ui.load.status.${d.item.load.status}`)}
        </p>
      {/if}
    </section>

    <section class="panel">
      <h3>{$t('ui.detail.best_power')}</h3>
      {#if d.best_power.length === 0}
        <p class="muted">{$t('ui.detail.no_best_power')}</p>
      {:else}
        <table data-testid="best-power">
          <thead>
            <tr><th>{$t('ui.detail.duration')}</th><th class="num">{$t('ui.detail.power')}</th></tr>
          </thead>
          <tbody>
            {#each d.best_power as b (b.duration_s)}
              <tr>
                <td>{formatDuration(b.duration_s)}</td>
                <td class="num">{formatNumber(b.watts, $locale, 0)} W</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  </div>

  <section class="panel steady" data-testid="detail-steady">
    <h3>{$t('ui.detail.steady_title')}</h3>
    <p class="statement" data-testid="decoupling-statement" data-state={d.decoupling.state}>
      {$t(`ui.detail.statement.${d.decoupling.state}`)}
    </p>
    {#if d.decoupling.percent !== null || d.efficiency_factor !== null}
      <StatRow>
        <Metric
          value={d.decoupling.percent === null
            ? MISSING
            : formatNumber(d.decoupling.percent, $locale, 1)}
          unit={d.decoupling.percent === null ? undefined : '%'}
          label={$t('ui.detail.decoupling_label')}
          testid="decoupling-metric"
        />
        <Metric
          value={d.efficiency_factor === null
            ? MISSING
            : formatNumber(d.efficiency_factor, $locale, 2)}
          unit={d.efficiency_factor === null ? undefined : 'W/bpm'}
          label={$t('ui.detail.ef_label')}
          testid="ef-metric"
        />
      </StatRow>
    {/if}
    <p class="muted hint">{$t('ui.detail.decoupling_hint')}</p>
    <p class="muted hint">
      {d.efficiency_factor === null ? $t('ui.detail.ef_none') : $t('ui.detail.ef_hint')}
    </p>
  </section>

  {#if d.laps.length > 1}
    <section class="panel laps" data-testid="detail-laps">
      <h3>{$t('ui.detail.laps_title')}</h3>
      <p class="muted hint">{$t('ui.detail.laps_statement', { n: d.laps.length })}</p>
      <div class="table-scroll">
        <table>
          <thead>
            <tr>
              <th class="num">{$t('ui.detail.lap')}</th>
              <th class="num">{$t('ui.detail.duration')}</th>
              {#if lapsHaveDistance}<th class="num">{$t('ui.detail.distance')}</th>{/if}
              {#if lapsHaveSpeed}
                <th class="num">{isRun ? $t('ui.detail.pace') : $t('ui.detail.speed')}</th>
              {/if}
              {#if lapsHavePower}<th class="num">{$t('ui.detail.avg_power')}</th>{/if}
              {#if lapsHaveHeart}<th class="num">{$t('ui.detail.avg_hr')}</th>{/if}
            </tr>
          </thead>
          <tbody>
            {#each d.laps as lap, i (i)}
              <tr data-testid="lap-row">
                <td class="num">{i + 1}</td>
                <td class="num">{formatDuration(lap.timer_s ?? lap.elapsed_s)}</td>
                {#if lapsHaveDistance}
                  <td class="num">{formatDistance(lap.distance_m, $locale)}</td>
                {/if}
                {#if lapsHaveSpeed}
                  <td class="num">
                    {#if isRun}
                      {formatPace(lap.avg_speed_mps)}{lap.avg_speed_mps === null ? '' : ' /km'}
                    {:else}
                      {formatSpeed(lap.avg_speed_mps, $locale)}
                    {/if}
                  </td>
                {/if}
                {#if lapsHavePower}
                  <td class="num">
                    {lap.avg_power_w === null
                      ? MISSING
                      : `${formatNumber(lap.avg_power_w, $locale, 0)} W`}
                  </td>
                {/if}
                {#if lapsHaveHeart}
                  <td class="num">
                    {lap.avg_hr_bpm === null
                      ? MISSING
                      : `${formatNumber(lap.avg_hr_bpm, $locale, 0)} bpm`}
                  </td>
                {/if}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {/if}

  <section class="panel chart">
    <h3>{hasPace ? $t('ui.detail.series_title_run') : $t('ui.detail.series_title')}</h3>
    {#if d.series.status === 'ok'}
      <UplotChart
        {options}
        data={activitySeriesData(d, hasPace)}
        height={260}
        label={hasPace ? $t('ui.detail.series_title_run') : $t('ui.detail.series_title')}
      />
    {:else}
      <p class="muted" data-testid="series-status" data-status={d.series.status}>
        {$t(`ui.detail.series_${d.series.status}`)}
      </p>
    {/if}
  </section>

  <section class="panel">
    <h3>{$t('ui.detail.zones')}</h3>
    {#if d.zones.length === 0}
      <p class="muted" data-testid="zones-none">{$t('ui.detail.no_zones')}</p>
    {:else}
      <div class="zone-grid">
        {#each d.zones as scheme (scheme.id)}
          <ZoneTable {scheme} />
        {/each}
      </div>
    {/if}
  </section>
{:else if loader.loading}
  <p class="muted">{$t('common.loading')}</p>
{/if}

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }

  .grid {
    display: grid;
    grid-template-columns: minmax(300px, 1fr) minmax(240px, 0.7fr);
    gap: 10px;
    margin-bottom: 10px;
    align-items: start;
  }

  .chart,
  .steady,
  .laps {
    margin-bottom: 10px;
  }

  .statement {
    margin: 0 0 var(--space-4);
    font-size: var(--fs-lead);
    line-height: 1.3;
    font-weight: var(--fw-semibold);
  }

  .hint {
    margin: var(--space-2) 0 0;
    font-size: var(--fs-small);
  }

  .table-scroll {
    overflow-x: auto;
  }

  .laps table {
    min-width: 420px;
  }

  h3 {
    margin-bottom: 6px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 2px 18px;
    margin: 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }

  .reason {
    margin: 8px 0 0;
  }

  .zone-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 14px;
  }

  .flag {
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-size: var(--fs-label);
  }

  @media (max-width: 900px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
