<script lang="ts">
  import type { DashPowerDuration } from '../backend/types';
  import { formatPowerDuration, signedPercent, directionOf } from '../dashboard';
  import { formatNumber, MISSING } from '../format';
  import { locale, t } from '../i18n';

  // Leistungskurve: je Dauer die beste mittlere Leistung der letzten 90 Tage (volle Linie) gegen
  // die 90 Tage davor (gedämpft). Die Längen sind je Zeile auf den größeren der beiden Werte
  // bezogen, sie zeigen also nur das Verhältnis. Watt und Veränderung stehen als Zahl daneben.
  interface Props {
    durations: DashPowerDuration[];
    testid?: string;
  }

  let { durations, testid }: Props = $props();

  const width = (v: number | null, d: DashPowerDuration) => {
    const top = Math.max(d.current_w ?? 0, d.previous_w ?? 0);
    return v === null || top <= 0 ? '0%' : `${(v / top) * 100}%`;
  };
</script>

<div class="rows" data-testid={testid} role="group" aria-label={$t('ui.dash.power.chart_label')}>
  {#each durations as d (d.duration_s)}
    <div class="row" data-testid="power-row" data-trend={d.trend}>
      <span class="dur">{formatPowerDuration(d.duration_s)}</span>
      <div class="bars" aria-hidden="true">
        <div class="bar now" style:width={width(d.current_w, d)}></div>
        <div class="bar before" style:width={width(d.previous_w, d)}></div>
      </div>
      <span class="watts">
        {d.current_w === null ? MISSING : formatNumber(d.current_w, $locale, 0)}
        <span class="unit">W</span>
      </span>
      <span class="change {d.trend.split('.')[1]}">
        {#if d.change_percent === null}
          <span class="muted">{MISSING}</span>
        {:else}
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            {#if directionOf(d.change_percent, 1) === 'up'}
              <path d="M6 10V2M2.5 5.5 6 2l3.5 3.5" />
            {:else if directionOf(d.change_percent, 1) === 'down'}
              <path d="M6 2v8M2.5 6.5 6 10l3.5-3.5" />
            {:else}
              <path d="M2 6h8" />
            {/if}
          </svg>
          {signedPercent(d.change_percent, $locale, 1)}
        {/if}
      </span>
    </div>
  {/each}
</div>
<ul class="legend">
  <li><span class="sw now"></span>{$t('ui.dash.power.now')}</li>
  <li><span class="sw before"></span>{$t('ui.dash.power.before')}</li>
</ul>

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .row {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr) 64px 84px;
    align-items: center;
    gap: var(--space-3);
    font-size: var(--fs-body);
  }

  .dur {
    color: var(--muted);
    font-weight: var(--fw-semibold);
  }

  .bars {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .bar {
    height: 7px;
    border-radius: 4px;
    min-width: 2px;
  }

  .bar.now {
    background: var(--power);
  }

  .bar.before {
    background: var(--power);
    opacity: 0.32;
  }

  .watts {
    text-align: right;
    font-weight: var(--fw-bold);
    font-variant-numeric: tabular-nums;
  }

  .unit {
    font-weight: var(--fw-regular);
    color: var(--muted);
    font-size: var(--fs-small);
  }

  .change {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    font-size: var(--fs-small);
    font-weight: var(--fw-semibold);
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }

  /* Anstieg in Grün, Rückgang bleibt neutral: Er kann an fehlenden Maximalbelastungen liegen. */
  .change.rising {
    color: var(--ok);
  }

  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px var(--space-4);
    margin: var(--space-4) 0 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .sw {
    display: inline-block;
    width: 14px;
    height: 7px;
    margin-right: 6px;
    border-radius: 4px;
    background: var(--power);
  }

  .sw.before {
    opacity: 0.32;
  }
</style>
