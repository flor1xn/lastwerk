<script lang="ts">
  import type { Sport } from '../backend/types';
  import { formatDate, formatNumber } from '../format';
  import { intlTag, locale, t } from '../i18n';
  import { metricValue, type ProgressMetric, type WeekBucket } from '../progress';

  // Wochenumfang als gestapelte Säulen je Sportart (T32). Farbe trägt die Sportart, der Wert
  // steht im `title` jeder Säule, unter dem Diagramm beim Überfahren und in der Legende als
  // Name; Farbe ist nie allein. Höhe relativ zur stärksten Woche des Zeitraums. HTML statt SVG,
  // damit die Säulen jede Breite füllen und die Schrift ihre Größe behält.
  interface Props {
    weeks: WeekBucket[];
    metric: ProgressMetric;
    sports: Sport[];
    unit: string;
    digits: number;
    testid?: string;
  }

  let { weeks, metric, sports, unit, digits, testid }: Props = $props();

  const max = $derived(Math.max(0, ...weeks.map((w) => metricValue(w.total, metric))));
  let hovered = $state<number | null>(null);

  const monthLabel = (iso: string) =>
    new Intl.DateTimeFormat(intlTag($locale), { month: 'short', timeZone: 'UTC' }).format(
      new Date(`${iso}T00:00:00Z`),
    );

  // Monatsmarke an der ersten Woche, die in einem neuen Monat beginnt (nicht an der ersten).
  const isMark = (i: number) =>
    i > 0 && weeks[i].start.slice(5, 7) !== weeks[i - 1].start.slice(5, 7);

  /** Anteile je Sportart in Prozent der Höhe, von unten nach oben; Nullen fallen weg. */
  function parts(w: WeekBucket): { sport: Sport; pct: number; value: number }[] {
    if (max <= 0) return [];
    return sports
      .map((s) => {
        const value = w.bySport[s] ? metricValue(w.bySport[s]!, metric) : 0;
        return { sport: s, value, pct: (value / max) * 100 };
      })
      .filter((p) => p.value > 0);
  }

  const fmt = (v: number) => `${formatNumber(v, $locale, digits)} ${unit}`;
  const label = (w: WeekBucket) =>
    `${$t('ui.progress.weeks.week_of', { date: formatDate(w.start, $locale) })}: ${fmt(
      metricValue(w.total, metric),
    )}`;
  const hoveredWeek = $derived(hovered === null ? null : (weeks[hovered] ?? null));
</script>

<div class="weeks" data-testid={testid}>
  <div class="plot" role="img" aria-label={$t('ui.progress.weeks.chart_label')}>
    {#each weeks as w, i (w.start)}
      <div
        class="col"
        class:hover={hovered === i}
        data-testid="progress-week"
        title={label(w)}
        role="presentation"
        onmouseenter={() => (hovered = i)}
        onmouseleave={() => (hovered = null)}
      >
        <div class="stack">
          {#each parts(w) as p (p.sport)}
            <span
              class="part"
              style:height={`${p.pct}%`}
              style:background={`var(--sport-${p.sport})`}
            ></span>
          {/each}
        </div>
        {#if isMark(i)}
          <span class="mark">{monthLabel(w.start)}</span>
        {/if}
      </div>
    {/each}
  </div>
  <div class="foot">
    <p class="readout muted" data-testid="progress-week-readout">
      {#if hoveredWeek}
        {label(hoveredWeek)}
        {#each parts(hoveredWeek) as p (p.sport)}
          · {$t(`ui.sport.${p.sport}`)} {formatNumber(p.value, $locale, digits)}
        {/each}
      {:else}
        {$t('ui.progress.weeks.hint')}
      {/if}
    </p>
    <ul class="legend">
      {#each sports as s (s)}
        <li>
          <span class="sw" style:background={`var(--sport-${s})`}></span>{$t(`ui.sport.${s}`)}
        </li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .plot {
    display: flex;
    align-items: stretch;
    gap: 2px;
    height: 140px;
    padding-bottom: 20px;
    border-bottom: 1px solid transparent;
  }

  .col {
    position: relative;
    flex: 1 1 0;
    min-width: 0;
    border-radius: 3px;
  }

  .col.hover {
    background: var(--panel-2);
  }

  .stack {
    display: flex;
    flex-direction: column-reverse;
    height: 100%;
    border-bottom: 1px solid var(--line);
  }

  .part {
    display: block;
    min-height: 1px;
  }

  .part:last-child {
    border-radius: 2px 2px 0 0;
  }

  .mark {
    position: absolute;
    left: 0;
    bottom: -20px;
    font-size: 11px;
    line-height: 16px;
    color: var(--muted);
    white-space: nowrap;
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--space-2) var(--space-4);
    margin-top: var(--space-3);
    font-size: var(--fs-small);
  }

  .readout {
    margin: 0;
    min-height: 1.4em;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px var(--space-4);
    margin: 0;
    padding: 0;
    list-style: none;
    color: var(--muted);
  }

  .sw {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-right: 6px;
    border-radius: 3px;
  }
</style>
