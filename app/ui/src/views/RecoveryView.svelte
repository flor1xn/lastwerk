<script lang="ts">
  import { getBackend } from '../lib/backend';
  import {
    RECOVERY_RANGES,
    type RecoveryMetric,
    type RecoveryPage,
    type RecoveryRange,
  } from '../lib/backend/types';
  import { readChartColors } from '../lib/charts';
  import {
    batteryData,
    batteryOptions,
    loadData,
    loadOptions,
    metricData,
    metricOptions,
    type RecoveryChartKey,
  } from '../lib/charts-recovery';
  import { directionOf, signedPercent } from '../lib/dashboard';
  import { localIso } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import Metric from '../lib/components/Metric.svelte';
  import Section from '../lib/components/Section.svelte';
  import Segmented from '../lib/components/Segmented.svelte';
  import StatRow from '../lib/components/StatRow.svelte';
  import UplotChart from '../lib/components/UplotChart.svelte';
  import { formatDate, formatNumber, formatWeekday, MISSING } from '../lib/format';
  import { locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { dataVersion, openSettings, resolvedTheme } from '../lib/state';

  // Erholung (T34): HRV, Ruhepuls, Schlaf und Body Battery über Wochen und Monate, je mit eigener
  // Basislinie und der Untergrenze aus R12, darunter die Tageslast. Beschreibt Trainingsdaten,
  // keine Gesundheit (SPEC 2.3).
  const today = localIso();
  let days = $state<RecoveryRange>(90);
  const loader = new Loader<RecoveryPage>();

  $effect(() => {
    void $dataVersion;
    const d = days;
    void loader.run(() => getBackend().getRecovery(today, d));
  });

  const data = $derived(loader.data);
  const points = $derived(data?.points ?? []);
  const has = (pick: (i: number) => number | null) => points.some((_, i) => pick(i) !== null);
  const hasHrv = $derived(has((i) => points[i].hrv_ms));
  const hasRhr = $derived(has((i) => points[i].resting_hr_bpm));
  const hasSleep = $derived(has((i) => points[i].sleep_h));
  const hasBattery = $derived(has((i) => points[i].body_battery_high));
  const hasLoad = $derived(has((i) => points[i].load));
  const anyRecovery = $derived(hasHrv || hasRhr || hasSleep || hasBattery);

  const lead = $derived.by(() => {
    if (!data) return '';
    if (data.last_date === null) return $t('ui.rec.lead.empty');
    if (data.state === 'readiness.low') {
      return $t('ui.rec.lead.low', { days: data.days, n: data.low_days });
    }
    if (data.state === 'readiness.normal') {
      return $t('ui.rec.lead.normal', { days: data.days, n: data.low_days });
    }
    return $t('ui.rec.lead.stale', { date: formatDate(data.last_date, $locale) });
  });

  // Mittel der letzten sieben Tage gegen das Mittel des Zeitraums, beschreibend.
  const UNITS: Record<RecoveryMetric, { unit: string; digits: number }> = {
    hrv: { unit: 'ms', digits: 0 },
    resting_hr: { unit: 'bpm', digits: 0 },
    sleep: { unit: 'h', digits: 1 },
    sleep_score: { unit: '', digits: 0 },
    body_battery_high: { unit: '', digits: 0 },
  };
  const summaries = $derived((data?.summary ?? []).filter((s) => s.period !== null));
  const deltaOf = (recent: number | null, period: number | null, metric: RecoveryMetric) => {
    if (recent === null || period === null || period <= 0) return undefined;
    const pct = ((recent - period) / period) * 100;
    return {
      text: $t('ui.rec.vs_period', {
        value: `${formatNumber(period, $locale, UNITS[metric].digits)}${UNITS[metric].unit ? ` ${UNITS[metric].unit}` : ''}`,
        change: signedPercent(pct, $locale, 0),
      }),
      direction: directionOf(pct, 0),
      tone: 'neutral' as const,
    };
  };

  let hovered = $state<number | null>(null);
  const hoveredPoint = $derived(hovered === null ? null : (points[hovered] ?? null));

  const labels = $derived({
    date: $t('ui.perf.power.date'),
    base: $t('ui.rec.base'),
    floor: $t('ui.rec.floor'),
    low: $t('ui.rec.low_day'),
  });
  const colors = $derived.by(() => {
    void $resolvedTheme;
    return readChartColors();
  });
  const chart = (key: RecoveryChartKey, digits: number, color: string) =>
    metricOptions(colors, $locale, { ...labels, value: $t(`ui.rec.metric.${key}`) }, digits, color);
  const cssVar = (name: string, fallback: string) => {
    void $resolvedTheme;
    if (typeof document === 'undefined') return fallback;
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
  };

  const rangeOptions = $derived(
    RECOVERY_RANGES.map((r) => ({ value: r, label: $t(`ui.rec.range.${r}`) })),
  );
</script>

<div class="page recovery" data-testid="recovery">
  <p class="page-lead" data-testid="lead">{lead}</p>

  <div class="controls">
    <Segmented
      options={rangeOptions}
      value={days}
      label={$t('ui.perf.range_label')}
      onchange={(d) => (days = d)}
      testid="rec-range"
    />
  </div>

  {#if loader.error}
    <ErrorNote error={loader.error} />
  {:else if !data}
    <p class="muted">{$t('common.loading')}</p>
  {:else if !anyRecovery}
    <div class="empty">
      <p class="muted">{$t('ui.rec.none')}</p>
      <button type="button" onclick={() => openSettings('data')}>{$t('ui.rec.to_settings')}</button>
    </div>
  {:else}
    {#if summaries.length > 0}
      <StatRow testid="rec-summary">
        {#each summaries as s (s.metric)}
          <Metric
            value={s.recent === null
              ? MISSING
              : formatNumber(s.recent, $locale, UNITS[s.metric].digits)}
            unit={s.recent === null || !UNITS[s.metric].unit ? undefined : UNITS[s.metric].unit}
            label={$t('ui.rec.recent', { metric: $t(`ui.rec.metric.${s.metric}`) })}
            delta={deltaOf(s.recent, s.period, s.metric)}
          />
        {/each}
      </StatRow>
    {/if}

    <p class="readout muted" data-testid="rec-readout">
      {#if hoveredPoint}
        <strong
          >{formatWeekday(hoveredPoint.date, $locale)}, {formatDate(
            hoveredPoint.date,
            $locale,
          )}</strong
        >
        · {$t('ui.rec.metric.hrv')}
        {hoveredPoint.hrv_ms === null
          ? MISSING
          : `${formatNumber(hoveredPoint.hrv_ms, $locale, 0)} ms`}
        · {$t('ui.rec.metric.resting_hr')}
        {hoveredPoint.resting_hr_bpm === null
          ? MISSING
          : `${formatNumber(hoveredPoint.resting_hr_bpm, $locale, 0)} bpm`}
        · {$t('ui.rec.metric.sleep')}
        {hoveredPoint.sleep_h === null
          ? MISSING
          : `${formatNumber(hoveredPoint.sleep_h, $locale, 1)} h`}
        · {$t('ui.rec.load')}
        {hoveredPoint.load === null ? MISSING : formatNumber(hoveredPoint.load, $locale, 0)}
        {#if hoveredPoint.low}
          · <span class="warn">{$t('ui.rec.low_day')}</span>
        {/if}
      {:else}
        {$t('ui.rec.hint')}
      {/if}
    </p>

    {#if hasHrv}
      <Section
        label={$t('ui.rec.metric.hrv')}
        statement={$t('ui.rec.hrv_statement', {
          pct: formatNumber(data.hrv_drop_percent, $locale, 0),
        })}
        testid="rec-hrv"
      >
        <UplotChart
          options={chart('hrv', 1, cssVar('--sport-cycling', '#7cc4e8'))}
          data={metricData(points, 'hrv')}
          height={170}
          label={$t('ui.rec.metric.hrv')}
          oncursor={(i) => (hovered = i)}
        />
      </Section>
    {/if}
    {#if hasRhr}
      <Section
        label={$t('ui.rec.metric.resting_hr')}
        statement={$t('ui.rec.rhr_statement')}
        testid="rec-rhr"
      >
        <UplotChart
          options={chart('resting_hr', 1, colors.heart)}
          data={metricData(points, 'resting_hr')}
          height={150}
          label={$t('ui.rec.metric.resting_hr')}
          oncursor={(i) => (hovered = i)}
        />
      </Section>
    {/if}
    {#if hasSleep}
      <Section
        label={$t('ui.rec.metric.sleep')}
        statement={$t('ui.rec.sleep_statement', {
          pct: formatNumber(data.sleep_deficit_percent, $locale, 0),
        })}
        testid="rec-sleep"
      >
        <UplotChart
          options={chart('sleep', 2, cssVar('--sport-swimming', '#5fd0c0'))}
          data={metricData(points, 'sleep')}
          height={150}
          label={$t('ui.rec.metric.sleep')}
          oncursor={(i) => (hovered = i)}
        />
      </Section>
    {/if}
    {#if hasBattery}
      <Section
        label={$t('ui.rec.metric.body_battery')}
        statement={$t('ui.rec.battery_statement')}
        testid="rec-battery"
      >
        <UplotChart
          options={batteryOptions(colors, $locale, {
            date: labels.date,
            high: $t('ui.rec.battery_high'),
            low: $t('ui.rec.battery_low'),
          })}
          data={batteryData(points)}
          height={150}
          label={$t('ui.rec.metric.body_battery')}
          oncursor={(i) => (hovered = i)}
        />
      </Section>
    {/if}
    {#if hasLoad}
      <Section label={$t('ui.rec.load')} statement={$t('ui.rec.load_statement')} testid="rec-load">
        <UplotChart
          options={loadOptions(colors, $locale, { date: labels.date, load: $t('ui.rec.load') })}
          data={loadData(points)}
          height={120}
          label={$t('ui.rec.load')}
          oncursor={(i) => (hovered = i)}
        />
      </Section>
    {/if}
    <ul class="legend">
      <li><span class="dot"></span>{$t('ui.rec.legend.value')}</li>
      <li><span class="line base"></span>{$t('ui.rec.base')}</li>
      <li><span class="line floor"></span>{$t('ui.rec.floor')}</li>
      <li><span class="dot low"></span>{$t('ui.rec.low_day')}</li>
    </ul>
    <p class="note muted">{$t('ui.rec.note')}</p>
  {/if}
</div>

<style>
  .recovery {
    max-width: var(--page-max-wide);
  }

  .controls {
    margin: 0 0 var(--space-5);
  }

  .empty {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }

  .empty p {
    margin: 0;
  }

  .readout {
    position: sticky;
    top: 0;
    z-index: 1;
    margin: var(--space-4) 0 0;
    padding: var(--space-2) 0;
    min-height: 1.5em;
    background: var(--bg);
    font-size: var(--fs-small);
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px var(--space-4);
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-right: 6px;
    border-radius: 50%;
    background: var(--sport-cycling);
  }

  .dot.low {
    width: 9px;
    height: 9px;
    background: var(--form-overloaded, var(--warn));
  }

  .line {
    display: inline-block;
    width: 16px;
    height: 0;
    margin: 0 6px 3px 0;
    vertical-align: middle;
    border-top: 2px solid var(--text);
  }

  .line.floor {
    border-top: 2px dashed var(--form-overloaded, var(--warn));
  }

  .note {
    margin: var(--space-3) 0 0;
    font-size: var(--fs-small);
  }
</style>
