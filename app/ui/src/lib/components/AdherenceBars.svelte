<script lang="ts">
  import type { DashWeekAdherence } from '../backend/types';
  import { addDays } from '../dates';
  import { NBSP, visibleWeeks } from '../dashboard';
  import { formatDayMonth, formatNumber, MISSING } from '../format';
  import { locale, t } from '../i18n';

  // Planerfüllung je Planwoche: Die Spur ist die geplante Last der Woche, die Füllung die
  // erledigte, der Strich die bis gestern geplante. Rechts steht das Verhältnis als Zahl, die
  // Länge der Balken ist nur die Form. Wochen ohne Soll bis gestern zeigen den Platzhalter.
  interface Props {
    weeks: DashWeekAdherence[];
    today: string;
    testid?: string;
  }

  let { weeks, today, testid }: Props = $props();

  const shown = $derived(visibleWeeks(weeks, today, 8));
  const scale = $derived(
    Math.max(1, ...shown.map((w) => Math.max(w.planned_total, w.done_to_date))),
  );
  const pct = (v: number) => `${Math.min(100, (v / scale) * 100)}%`;
</script>

<div
  class="rows"
  data-testid={testid}
  role="group"
  aria-label={$t('ui.dash.adherence.chart_label')}
>
  {#each shown as w (w.week_start)}
    {@const current = w.week_start <= today && today <= addDays(w.week_start, 6)}
    <div
      class="row"
      class:future={w.week_start > today}
      data-testid="adherence-week"
      title={$t('ui.dash.adherence.week_title', {
        date: formatDayMonth(w.week_start, $locale),
        done: formatNumber(w.done_to_date, $locale, 0),
        planned: formatNumber(w.planned_to_date, $locale, 0),
        total: formatNumber(w.planned_total, $locale, 0),
      })}
    >
      <span class="week" class:now={current}>{formatDayMonth(w.week_start, $locale)}</span>
      <div class="track" aria-hidden="true">
        <div class="plan" style:width={pct(w.planned_total)}>
          <div
            class="done"
            style:width={w.planned_total > 0
              ? `${Math.min(100, (w.done_to_date / w.planned_total) * 100)}%`
              : '0%'}
          ></div>
          {#if !w.complete && w.planned_to_date > 0 && w.planned_total > 0}
            <div
              class="mark"
              style:left={`${Math.min(100, (w.planned_to_date / w.planned_total) * 100)}%`}
            ></div>
          {/if}
        </div>
        {#if w.done_to_date > w.planned_total}
          <div
            class="over"
            style:left={pct(w.planned_total)}
            style:width={`calc(${pct(w.done_to_date)} - ${pct(w.planned_total)})`}
          ></div>
        {/if}
      </div>
      <span class="ratio">
        {w.ratio_percent === null
          ? MISSING
          : `${formatNumber(w.ratio_percent, $locale, 0)}${NBSP}%`}
      </span>
      <span class="amount muted">
        {formatNumber(w.done_to_date, $locale, 0)} / {formatNumber(w.planned_total, $locale, 0)}
      </span>
    </div>
  {/each}
</div>
<ul class="legend">
  <li><span class="sw done-sw"></span>{$t('ui.dash.adherence.done')}</li>
  <li><span class="sw plan-sw"></span>{$t('ui.dash.adherence.planned')}</li>
  <li><span class="sw mark-sw"></span>{$t('ui.dash.adherence.so_far')}</li>
</ul>

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .row {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr) 52px auto;
    align-items: center;
    gap: var(--space-3);
    font-size: var(--fs-dense);
  }

  .row.future {
    opacity: 0.7;
  }

  .week {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .week.now {
    color: var(--text);
    font-weight: var(--fw-bold);
  }

  .track {
    position: relative;
    height: 12px;
  }

  .plan {
    position: relative;
    height: 100%;
    border: 1px solid var(--muted);
    border-radius: 6px;
    background: transparent;
    overflow: hidden;
  }

  .done {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    background: var(--accent);
  }

  .mark {
    position: absolute;
    top: -1px;
    bottom: -1px;
    width: 2px;
    background: var(--text);
  }

  .over {
    position: absolute;
    top: 0;
    height: 100%;
    border-radius: 0 6px 6px 0;
    background: var(--accent);
    opacity: 0.55;
  }

  .ratio {
    text-align: right;
    font-weight: var(--fw-bold);
    font-variant-numeric: tabular-nums;
  }

  .amount {
    min-width: 7ch;
    text-align: right;
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
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
    width: 12px;
    height: 10px;
    margin-right: 6px;
    border-radius: 3px;
    vertical-align: -1px;
  }

  .done-sw {
    background: var(--accent);
  }

  .plan-sw {
    border: 1px solid var(--muted);
  }

  .mark-sw {
    width: 2px;
    background: var(--text);
    margin-right: 8px;
  }
</style>
