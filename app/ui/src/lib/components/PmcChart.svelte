<script lang="ts">
  import type { PmcPoint } from '../backend/types';
  import { pmcData, pmcOptions, readChartColors } from '../charts';
  import { dashPmcOptions, pmcProjectionData } from '../charts-dashboard';
  import { splitPmc } from '../dashboard';
  import { formatDate, formatNumber, MISSING } from '../format';
  import { locale, t } from '../i18n';
  import { resolvedTheme } from '../state';
  import UplotChart from './UplotChart.svelte';

  // Verlauf von Fitness, Ermüdung und Form mit eigener Legende (Punkt, Name, Wert). Die Werte
  // zeigen den letzten Tag und folgen dem Zeiger. Mit `bands` kommen die Formzustände als
  // dezente Bänder hinzu und mit `projection` die gestrichelte Prognose (Startseite).
  interface Props {
    history: PmcPoint[];
    projection?: PmcPoint[];
    bands?: boolean;
    height?: number;
    label: string;
    testid?: string;
  }

  let { history, projection = [], bands = false, height = 240, label, testid }: Props = $props();

  const split = $derived(splitPmc(history, projection));
  const withProjection = $derived(bands && projection.length > 0);

  const options = $derived.by(() => {
    void $resolvedTheme;
    const labels = {
      date: $t('ui.week.pmc_date'),
      ctl: $t('ui.week.ctl'),
      atl: $t('ui.week.atl'),
      tsb: $t('ui.week.tsb'),
    };
    const colors = readChartColors();
    if (!bands) return pmcOptions(colors, $locale, labels);
    const lastActual = history.length > 0 ? history[history.length - 1].date : null;
    return dashPmcOptions(colors, $locale, labels, {
      projection: withProjection,
      bands: {
        fresh: $t('ui.form.fresh'),
        balanced: $t('ui.form.balanced'),
        loaded: $t('ui.form.loaded'),
        overloaded: $t('ui.form.overloaded'),
      },
      today:
        withProjection && lastActual ? { date: lastActual, label: $t('ui.dash.today') } : undefined,
    });
  });

  const data = $derived(withProjection ? pmcProjectionData(split) : pmcData(history));

  // Index unter dem Zeiger, sonst der letzte Ist-Tag.
  let hover = $state<number | null>(null);
  const shown = $derived.by(() => {
    const all = [...history, ...projection];
    if (all.length === 0) return null;
    const i = hover !== null && hover >= 0 && hover < all.length ? hover : history.length - 1;
    const p = all[Math.max(0, i)];
    return { point: p, projected: i >= history.length };
  });

  const items = $derived([
    { key: 'ctl', label: $t('ui.week.ctl'), color: 'var(--pmc-ctl)', value: shown?.point.ctl },
    { key: 'atl', label: $t('ui.week.atl'), color: 'var(--pmc-atl)', value: shown?.point.atl },
    { key: 'tsb', label: $t('ui.week.tsb'), color: 'var(--pmc-tsb)', value: shown?.point.tsb },
  ]);
</script>

<div class="pmc" data-testid={testid}>
  <UplotChart {options} {data} {height} {label} oncursor={(i) => (hover = i)} />
  <div class="legend" role="group" aria-label={label}>
    <ul>
      {#each items as item (item.key)}
        <li data-testid={`legend-${item.key}`}>
          <span class="line" style:background={item.color} aria-hidden="true"></span>
          <span class="name">{item.label}</span>
          <span class="val">{formatNumber(item.value ?? null, $locale, 1)}</span>
        </li>
      {/each}
      {#if withProjection}
        <li data-testid="legend-projection">
          <span class="line dashed" aria-hidden="true"></span>
          <span class="name">{$t('ui.dash.projection')}</span>
        </li>
      {/if}
    </ul>
    {#if shown}
      <p class="when muted" data-testid="legend-date">
        {formatDate(shown.point.date, $locale)}{#if shown.projected}
          · {$t('ui.dash.projection')}{/if}
      </p>
    {:else}
      <p class="when muted">{MISSING}</p>
    {/if}
  </div>
</div>

<style>
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-1) var(--space-5);
    margin-top: var(--space-3);
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
  }

  ul {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-5);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
  }

  .line {
    width: 14px;
    height: 3px;
    border-radius: 2px;
    align-self: center;
  }

  .line.dashed {
    height: 0;
    border-top: 2px dashed var(--muted);
    background: none;
    border-radius: 0;
  }

  .name {
    color: var(--muted);
  }

  .val {
    font-weight: var(--fw-bold);
    font-variant-numeric: tabular-nums;
    min-width: 3.2ch;
  }

  .when {
    margin: 0;
    font-variant-numeric: tabular-nums;
  }
</style>
