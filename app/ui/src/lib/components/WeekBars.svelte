<script lang="ts">
  import { SPORTS, type DayLoad } from '../backend/types';
  import { formatDayMonth, formatNumber, formatWeekday } from '../format';
  import { locale, t } from '../i18n';

  let { days }: { days: DayLoad[] } = $props();

  const total = (d: DayLoad) => d.by_sport.reduce((s, x) => s + x.load, 0);
  // Gemeinsame Skala für alle Tage. Ohne Last bleibt sie 1, damit nichts durch 0 teilt.
  const max = $derived(Math.max(1, ...days.map(total)));
</script>

<div class="bars" role="group" aria-label={$t('ui.week.bars_label')}>
  {#each days as day (day.date)}
    {@const sum = total(day)}
    <div class="day" data-testid="day" data-date={day.date}>
      <div class="value">{sum > 0 ? formatNumber(sum, $locale, 0) : ''}</div>
      <div class="column">
        {#each day.by_sport as part (part.sport)}
          <div
            class="segment"
            style:height={`${(part.load / max) * 100}%`}
            style:background={`var(--sport-${part.sport})`}
            title={`${$t(`ui.sport.${part.sport}`)}: ${formatNumber(part.load, $locale, 1)}`}
          ></div>
        {/each}
      </div>
      <div class="label">{formatWeekday(day.date, $locale)}</div>
      <div class="date muted">{formatDayMonth(day.date, $locale)}</div>
      {#if day.not_computable > 0}
        <div class="na" data-testid="day-not-computable">
          {day.not_computable}× {$t('common.not_computable')}
        </div>
      {/if}
    </div>
  {/each}
</div>

<ul class="legend">
  {#each SPORTS as sport (sport)}
    <li>
      <span class="swatch" style:background={`var(--sport-${sport})`}></span>{$t(
        `ui.sport.${sport}`,
      )}
    </li>
  {/each}
</ul>

<style>
  /* Balken auf der Fläche: kein Hintergrundkasten je Tag, nur eine Grundlinie. Schmale Balken
     mit 2 px Abstand zwischen den Segmenten (dataviz: Fläche trennt, kein Rand). */
  .bars {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: var(--space-2);
  }

  .day {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    text-align: center;
    min-width: 0;
  }

  .value {
    height: 22px;
    font-size: var(--fs-dense);
    font-variant-numeric: tabular-nums;
    font-weight: var(--fw-bold);
  }

  .column {
    height: 150px;
    width: min(100%, 56px);
    align-self: center;
    display: flex;
    flex-direction: column-reverse;
    gap: 2px;
    border-bottom: 1px solid var(--border);
  }

  .segment {
    min-height: 3px;
    border-radius: 4px;
  }

  .label {
    margin-top: var(--space-2);
    font-weight: var(--fw-semibold);
  }

  .date {
    font-size: var(--fs-label);
    line-height: var(--lh-tight);
  }

  .na {
    margin-top: 2px;
    font-size: var(--fs-label);
    color: var(--warn);
    font-style: italic;
    white-space: normal;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px var(--space-4);
    margin: var(--space-4) 0 0;
    padding: 0;
    list-style: none;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  .swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-right: 6px;
    border-radius: 3px;
    vertical-align: -1px;
  }
</style>
