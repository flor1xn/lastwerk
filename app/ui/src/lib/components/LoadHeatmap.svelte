<script lang="ts">
  import { buildHeatmap, calendarStats } from '../dashboard';
  import { formatDate, formatNumber, formatWeekday } from '../format';
  import { locale, t } from '../i18n';
  import Metric from './Metric.svelte';
  import StatRow from './StatRow.svelte';

  // Belastungskalender: ein Jahr Tageslast als Raster aus Wochenspalten (Montag oben). Die
  // Farbe ist nur eine Stufe aus vier (Quartile der Tage mit Last), der genaue Wert steht in
  // `<title>` jeder Zelle, unter dem Raster beim Überfahren und im Text darüber. Tage ohne Last
  // bleiben leer.
  interface Props {
    days: { date: string; load: number }[];
    today: string;
    testid?: string;
  }

  let { days, today, testid }: Props = $props();

  const CELL = 12;
  const GAP = 3;
  const PITCH = CELL + GAP;
  const LEFT = 30;
  const TOP = 20;

  const map = $derived(buildHeatmap(days, today, 365));
  const stats = $derived(calendarStats(days));
  const width = $derived(LEFT + map.cols * PITCH - GAP);
  const height = TOP + 7 * PITCH - GAP;

  let hovered = $state<string | null>(null);
  const hoveredCell = $derived(map.cells.find((c) => c.date === hovered) ?? null);

  const month = (iso: string) =>
    new Intl.DateTimeFormat($locale === 'de' ? 'de-DE' : 'en-US', {
      month: 'short',
      timeZone: 'UTC',
    }).format(new Date(`${iso}T00:00:00Z`));
  // Beschriftete Zeilen: Montag, Mittwoch, Freitag (Tage der Woche 0, 2, 4).
  const rowLabels = [0, 2, 4];
  const weekdayOf = (row: number) =>
    formatWeekday(`2026-06-${String(1 + row).padStart(2, '0')}`, $locale);
</script>

<div class="cal" data-testid={testid}>
  <StatRow testid="calendar-stats">
    <Metric value={String(stats.trainingDays)} label={$t('ui.dash.calendar.days')} />
    <Metric value={formatNumber(stats.total, $locale, 0)} label={$t('ui.dash.calendar.total')} />
    <Metric
      value={formatNumber(stats.bestWeek, $locale, 0)}
      label={$t('ui.dash.calendar.best_week')}
    />
  </StatRow>

  <div class="scroll">
    <svg
      viewBox={`0 0 ${width} ${height}`}
      style:min-width={`${Math.min(width, 640)}px`}
      role="img"
      aria-label={$t('ui.dash.calendar.chart_label')}
    >
      {#each map.months as m (m.date)}
        <text class="axis" x={LEFT + m.col * PITCH} y="12">{month(m.date)}</text>
      {/each}
      {#each rowLabels as row (row)}
        <text class="axis" x="0" y={TOP + row * PITCH + CELL - 2}>{weekdayOf(row)}</text>
      {/each}
      {#each map.cells as c (c.date)}
        <rect
          class="cell l{c.level}"
          class:today={c.date === today}
          class:hover={c.date === hovered}
          x={LEFT + c.col * PITCH}
          y={TOP + c.row * PITCH}
          width={CELL}
          height={CELL}
          rx="3"
          data-date={c.date}
          data-level={c.level}
          role="presentation"
          onmouseenter={() => (hovered = c.date)}
          onmouseleave={() => (hovered = null)}
        >
          <title>
            {formatDate(c.date, $locale)}: {c.load === null
              ? $t('ui.dash.calendar.no_load')
              : formatNumber(c.load, $locale, 1)}
          </title>
        </rect>
      {/each}
    </svg>
  </div>

  <div class="foot">
    <p class="readout muted" aria-live="off" data-testid="calendar-readout">
      {#if hoveredCell}
        {formatWeekday(hoveredCell.date, $locale)}, {formatDate(hoveredCell.date, $locale)}:
        {hoveredCell.load === null
          ? $t('ui.dash.calendar.no_load')
          : $t('ui.dash.calendar.load', { value: formatNumber(hoveredCell.load, $locale, 1) })}
      {:else}
        {$t('ui.dash.calendar.hint')}
      {/if}
    </p>
    <ul class="scale" aria-label={$t('ui.dash.calendar.scale')}>
      <li class="muted">{$t('ui.dash.calendar.less')}</li>
      {#each [0, 1, 2, 3, 4] as level (level)}
        <li><span class="swatch l{level}"></span></li>
      {/each}
      <li class="muted">{$t('ui.dash.calendar.more')}</li>
    </ul>
  </div>
</div>

<style>
  .scroll {
    margin-top: var(--space-5);
    overflow-x: auto;
  }

  svg {
    display: block;
    width: 100%;
    height: auto;
  }

  .axis {
    fill: var(--muted);
    font-size: 11px;
    font-family: var(--font-sans);
  }

  /* Stufen aus dem Akzent, in den Hintergrund gemischt: 0 ist nur die Fläche, 4 die volle Farbe. */
  .l0 {
    fill: var(--panel-2);
    background: var(--panel-2);
  }

  .l1 {
    fill: color-mix(in srgb, var(--accent) 28%, var(--panel-2));
    background: color-mix(in srgb, var(--accent) 28%, var(--panel-2));
  }

  .l2 {
    fill: color-mix(in srgb, var(--accent) 52%, var(--panel-2));
    background: color-mix(in srgb, var(--accent) 52%, var(--panel-2));
  }

  .l3 {
    fill: color-mix(in srgb, var(--accent) 76%, var(--panel-2));
    background: color-mix(in srgb, var(--accent) 76%, var(--panel-2));
  }

  .l4 {
    fill: var(--accent);
    background: var(--accent);
  }

  .cell.today {
    stroke: var(--text);
    stroke-width: 1.5;
  }

  .cell.hover {
    stroke: var(--text);
    stroke-width: 1;
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2) var(--space-5);
    margin-top: var(--space-3);
    font-size: var(--fs-small);
  }

  .readout {
    margin: 0;
    min-height: 1.4em;
    font-variant-numeric: tabular-nums;
  }

  .scale {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .scale li:first-child {
    margin-right: 4px;
  }

  .scale li:last-child {
    margin-left: 4px;
  }

  .swatch {
    display: block;
    width: 12px;
    height: 12px;
    border-radius: 3px;
  }
</style>
