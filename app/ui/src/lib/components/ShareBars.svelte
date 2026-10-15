<script lang="ts">
  import type { DashWeekShare } from '../backend/types';
  import { formatDayMonth, formatNumber, MISSING } from '../format';
  import { locale, t } from '../i18n';

  // 80/20 je Woche: Jeder Balken ist die Zeit mit Zonen der Woche (100 Prozent), unten der
  // lockere Anteil (Zone 1 und 2), oben der Rest. Die gestrichelte Linie ist die Grenze aus R8.
  // Wochen ohne Zeit in Zonen bleiben leer und tragen den Platzhalter, nie eine 0.
  interface Props {
    weeks: DashWeekShare[];
    /** Grenze in Prozent (R8). */
    limit: number;
    testid?: string;
  }

  let { weeks, limit, testid }: Props = $props();

  const hours = (s: number) => formatNumber(s / 3600, $locale, 1);
</script>

<div class="wrap" data-testid={testid}>
  <div class="bars" role="group" aria-label={$t('ui.dash.distribution.chart_label')}>
    <div class="overlay" aria-hidden="true">
      <div class="limit" style:bottom={`${limit}%`}></div>
    </div>
    {#each weeks as w, i (w.week_start)}
      {@const known = w.easy_percent !== null && w.total_s > 0}
      <div
        class="col"
        class:current={i === weeks.length - 1}
        data-testid="share-week"
        data-below={known && (w.easy_percent ?? 0) < limit ? 'true' : 'false'}
        title={known
          ? $t('ui.dash.distribution.week_title', {
              date: formatDayMonth(w.week_start, $locale),
              share: formatNumber(w.easy_percent, $locale, 0),
              hours: hours(w.total_s),
            })
          : $t('ui.dash.distribution.week_empty', { date: formatDayMonth(w.week_start, $locale) })}
      >
        <div class="value">{known ? formatNumber(w.easy_percent, $locale, 0) : MISSING}</div>
        <div class="track">
          {#if known}
            <div class="hard" style:height={`${100 - (w.easy_percent ?? 0)}%`}></div>
            <div class="easy" style:height={`${w.easy_percent ?? 0}%`}></div>
          {:else}
            <div class="none"></div>
          {/if}
        </div>
        <div class="label">{formatDayMonth(w.week_start, $locale)}</div>
      </div>
    {/each}
  </div>
  <ul class="legend">
    <li><span class="sw easy-sw"></span>{$t('ui.dash.distribution.easy')}</li>
    <li><span class="sw hard-sw"></span>{$t('ui.dash.distribution.hard')}</li>
    <li>
      <span class="sw limit-sw"></span>{$t('ui.dash.distribution.limit', {
        value: formatNumber(limit, $locale, 0),
      })}
    </li>
  </ul>
</div>

<style>
  .wrap {
    position: relative;
  }

  .bars {
    position: relative;
    display: grid;
    grid-template-columns: repeat(8, minmax(0, 1fr));
    gap: var(--space-2);
  }

  .col {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    min-width: 0;
    text-align: center;
  }

  .value {
    height: 22px;
    font-size: var(--fs-dense);
    font-weight: var(--fw-bold);
    font-variant-numeric: tabular-nums;
  }

  .track {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: min(100%, 36px);
    height: 120px;
    align-self: center;
  }

  .easy,
  .hard {
    border-radius: 4px;
    min-height: 3px;
  }

  .easy {
    background: var(--form-fresh);
  }

  .hard {
    background: var(--warn);
    opacity: 0.85;
  }

  .none {
    height: 100%;
    border: 1px dashed var(--border);
    border-radius: 4px;
  }

  .label {
    margin-top: var(--space-2);
    font-size: var(--fs-label);
    line-height: var(--lh-tight);
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .col.current .label {
    color: var(--text);
    font-weight: var(--fw-bold);
  }

  /* Deckschicht genau über der Zeile der Balken (unter dem Wert, über dem Datum). */
  .overlay {
    position: absolute;
    left: 0;
    right: 0;
    top: 22px;
    height: 120px;
    pointer-events: none;
    z-index: 1;
  }

  .limit {
    position: absolute;
    left: 0;
    right: 0;
    height: 0;
    border-top: 1px dashed var(--muted);
  }

  /* Schmale Spalte: Daten wechselweise ausblenden, damit sie nicht ineinanderlaufen. Die erste
     Kindposition gehört der Deckschicht, die Wochen stehen ab Position 2. */
  .wrap {
    container-type: inline-size;
  }

  @container (max-width: 380px) {
    .col:nth-child(3) .label,
    .col:nth-child(5) .label,
    .col:nth-child(7) .label,
    .col:nth-child(8) .label {
      visibility: hidden;
    }
  }

  .limit-sw {
    width: 14px;
    height: 0;
    border-top: 1px dashed var(--muted);
    border-radius: 0;
    vertical-align: 3px;
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
    width: 10px;
    height: 10px;
    margin-right: 6px;
    border-radius: 3px;
    vertical-align: -1px;
  }

  .easy-sw {
    background: var(--form-fresh);
  }

  .hard-sw {
    background: var(--warn);
    opacity: 0.85;
  }
</style>
