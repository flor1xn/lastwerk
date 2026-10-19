<script lang="ts">
  import { getBackend } from '../lib/backend';
  import type { ActivityItem, Plan } from '../lib/backend/types';
  import { buildMonth, monthCounts, monthRange, shiftMonth, type CalDay } from '../lib/calendar';
  import { localIso } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import { formatDuration, formatNumber, formatWeekday } from '../lib/format';
  import { intlTag, locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { keyText, templateName } from '../lib/plan';
  import { dataVersion, openActivity, view } from '../lib/state';
  import { activePlanId } from '../lib/today';

  // Trainingskalender (T35): ein Monat als Raster, je Tag Plan und Ist, je Woche Soll gegen Ist.
  // Die Zustände sind reine Zählung (lib/calendar.ts), der Text nennt sie, nie nur die Farbe.
  interface MonthData {
    activities: ActivityItem[];
    plan: Plan | null;
  }

  const today = localIso();
  let month = $state(today.slice(0, 7));
  const loader = new Loader<MonthData>();

  $effect(() => {
    void $dataVersion;
    const m = month;
    void loader.run(async () => {
      const backend = getBackend();
      const { first, last } = monthRange(m);
      const [activities, plans] = await Promise.all([
        backend.listActivities({ from: first, to: last, sport: null }),
        backend.listPlans(),
      ]);
      const id = activePlanId(plans);
      const plan = id ? (await backend.getPlan(id)).plan : null;
      return { activities, plan };
    });
  });

  const data = $derived(loader.data);
  const weeks = $derived(data ? buildMonth(month, today, data.activities, data.plan) : []);
  const counts = $derived(monthCounts(weeks, today));
  const monthTitle = $derived(
    new Intl.DateTimeFormat(intlTag($locale), {
      month: 'long',
      year: 'numeric',
      timeZone: 'UTC',
    }).format(new Date(`${month}-01T00:00:00Z`)),
  );
  const weekdays = $derived(
    Array.from({ length: 7 }, (_, i) =>
      formatWeekday(`2026-06-${String(1 + i).padStart(2, '0')}`, $locale),
    ),
  );

  const lead = $derived.by(() => {
    if (!data) return '';
    if (data.plan && counts.plannedPast > 0) {
      return $t('ui.cal.lead.plan', {
        month: monthTitle,
        done: counts.donePast,
        planned: counts.plannedPast,
        missed: counts.missedDays,
      });
    }
    if (counts.sessions === 0) return $t('ui.cal.lead.empty', { month: monthTitle });
    return data.plan
      ? $t('ui.cal.lead.sessions', { month: monthTitle, n: counts.sessions })
      : $t('ui.cal.lead.no_plan', { month: monthTitle, n: counts.sessions });
  });

  const dayNumber = (iso: string) => Number(iso.slice(8, 10));
  const showDay = (d: CalDay) => d.inMonth && (d.activities.length > 0 || d.planned.length > 0);
  const reasons = (keys: string[] | undefined) =>
    (keys ?? []).map((k) => keyText($locale, k)).join(' ');
</script>

<div class="page calendar" data-testid="calendar-view">
  <p class="page-lead" data-testid="lead">{lead}</p>

  <div class="bar">
    <button
      type="button"
      onclick={() => (month = shiftMonth(month, -1))}
      aria-label={$t('ui.cal.previous')}
      data-testid="cal-prev">‹</button
    >
    <h2 class="month" data-testid="cal-month">{monthTitle}</h2>
    <button
      type="button"
      onclick={() => (month = shiftMonth(month, 1))}
      aria-label={$t('ui.cal.next')}
      data-testid="cal-next">›</button
    >
    {#if month !== today.slice(0, 7)}
      <button type="button" onclick={() => (month = today.slice(0, 7))} data-testid="cal-today">
        {$t('ui.cal.today')}
      </button>
    {/if}
    {#if data && !data.plan}
      <button type="button" class="ghost" onclick={() => view.set('plan')}>
        {$t('ui.cal.to_plan')}
      </button>
    {/if}
  </div>

  {#if loader.error}
    <ErrorNote error={loader.error} />
  {:else if !data}
    <p class="muted">{$t('common.loading')}</p>
  {:else}
    <div class="grid" role="grid" aria-label={monthTitle} data-testid="cal-grid">
      <div class="row head" role="row">
        {#each weekdays as w (w)}
          <div class="wd" role="columnheader">{w}</div>
        {/each}
        <div class="wd sum" role="columnheader">{$t('ui.cal.week')}</div>
      </div>
      {#each weeks as w (w.start)}
        <div class="row" role="row">
          {#each w.days as d (d.date)}
            <div
              class="cell {d.state}"
              class:out={!d.inMonth}
              class:today={d.isToday}
              class:show={showDay(d)}
              role="gridcell"
              data-testid="cal-day"
              data-date={d.date}
              data-state={d.state}
            >
              <div class="top">
                <span class="num">
                  <span class="wd-inline">{formatWeekday(d.date, $locale)}</span>
                  {dayNumber(d.date)}
                </span>
                {#if d.state === 'missed' || d.state === 'partial' || d.state === 'extra'}
                  <span class="tag" data-testid="cal-tag">{$t(`ui.cal.state.${d.state}`)}</span>
                {/if}
              </div>
              {#each d.planned as s, i (i)}
                <div class="chip plan" class:hard={s.hard} title={reasons(s.reason_keys)}>
                  <span class="name">{templateName($locale, s.template_id)}</span>
                  <span class="meta">{$t('ui.today.minutes', { n: s.duration_min })}</span>
                </div>
              {/each}
              {#each d.activities as a (a.id)}
                <button
                  type="button"
                  class="chip act"
                  style:--sport={`var(--sport-${a.sport})`}
                  onclick={() => openActivity(a.id)}
                  title={$t('ui.perf.open_activity')}
                  data-testid="cal-activity"
                >
                  <span class="name">{$t(`ui.sport.${a.sport}`)}</span>
                  <span class="meta"
                    >{formatDuration(a.duration_s)}{a.load.value !== null
                      ? ` · ${formatNumber(a.load.value, $locale, 0)}`
                      : ''}</span
                  >
                </button>
              {/each}
            </div>
          {/each}
          <div class="cell sum" role="gridcell" data-testid="cal-week-sum">
            <span class="label">{$t('ui.cal.load')}</span>
            <span class="value">
              {formatNumber(w.load, $locale, 0)}
              {#if w.plannedLoad > 0}
                <span class="muted">/ {formatNumber(w.plannedLoad, $locale, 0)}</span>
              {/if}
            </span>
            <span class="label">{$t('ui.cal.time')}</span>
            <span class="value">
              {formatNumber(w.durationS / 3600, $locale, 1)} h
              {#if w.plannedMin > 0}
                <span class="muted">/ {formatNumber(w.plannedMin / 60, $locale, 1)} h</span>
              {/if}
            </span>
          </div>
        </div>
      {/each}
    </div>
    <ul class="legend">
      <li><span class="sw plan"></span>{$t('ui.cal.legend.planned')}</li>
      <li><span class="sw act"></span>{$t('ui.cal.legend.done')}</li>
      <li><span class="sw missed"></span>{$t('ui.cal.state.missed')}</li>
      <li class="muted">{$t('ui.cal.legend.sum')}</li>
    </ul>
  {/if}
</div>

<style>
  .calendar {
    max-width: var(--page-max);
    container-type: inline-size;
  }

  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin: 0 0 var(--space-4);
  }

  .bar button {
    min-width: 36px;
  }

  .ghost {
    margin-left: auto;
  }

  .month {
    margin: 0 var(--space-2);
    min-width: 10ch;
    font-size: var(--fs-lead);
    line-height: 1.3;
    text-align: center;
  }

  .grid {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--line);
  }

  .row {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr)) minmax(92px, 0.8fr);
    border-bottom: 1px solid var(--line);
  }

  .wd {
    padding: var(--space-2);
    font-size: var(--fs-label);
    font-weight: var(--fw-bold);
    letter-spacing: var(--ls-label);
    text-transform: uppercase;
    color: var(--muted);
  }

  .cell {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 96px;
    min-width: 0;
    padding: var(--space-2);
    border-left: 1px solid var(--line);
  }

  .cell:first-child {
    border-left: none;
  }

  .cell.out {
    opacity: 0.45;
  }

  .cell.today {
    box-shadow: inset 0 0 0 2px var(--accent);
    border-radius: 6px;
  }

  .cell.missed {
    background: repeating-linear-gradient(
      135deg,
      transparent 0 6px,
      color-mix(in srgb, var(--warn) 10%, transparent) 6px 12px
    );
  }

  .top {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 4px;
  }

  .num {
    font-size: var(--fs-small);
    font-weight: var(--fw-semibold);
    font-variant-numeric: tabular-nums;
  }

  .wd-inline {
    display: none;
  }

  .tag {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .missed .tag {
    color: var(--warn);
  }

  .chip {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 3px 6px;
    border-radius: 6px;
    font-size: 12px;
    line-height: 1.3;
    text-align: left;
  }

  .chip .name,
  .chip .meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chip .meta {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .chip.plan {
    border: 1px dashed var(--border);
  }

  .chip.plan.hard {
    border-color: var(--accent);
  }

  .chip.act {
    min-height: 0;
    border: none;
    border-left: 3px solid var(--sport);
    background: var(--panel-2);
    color: var(--text);
    font-weight: var(--fw-medium);
  }

  .chip.act:hover:not(:disabled) {
    background: color-mix(in srgb, var(--sport) 18%, var(--panel-2));
  }

  .cell.sum {
    gap: 0;
    font-size: var(--fs-small);
    background: color-mix(in srgb, var(--panel) 60%, transparent);
  }

  .sum .label {
    color: var(--muted);
    font-size: 11px;
  }

  .sum .value {
    margin-bottom: 4px;
    font-weight: var(--fw-semibold);
    font-variant-numeric: tabular-nums;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px var(--space-4);
    margin: var(--space-3) 0 0;
    padding: 0;
    list-style: none;
    font-size: var(--fs-small);
  }

  .sw {
    display: inline-block;
    width: 14px;
    height: 10px;
    margin-right: 6px;
    border-radius: 3px;
    vertical-align: middle;
  }

  .sw.plan {
    border: 1px dashed var(--border);
  }

  .sw.act {
    border-left: 3px solid var(--sport-running);
    background: var(--panel-2);
  }

  .sw.missed {
    background: color-mix(in srgb, var(--warn) 30%, transparent);
  }

  /* Schmal: Liste der Tage mit Einträgen statt Raster. */
  @container (max-width: 760px) {
    .row.head {
      display: none;
    }

    .row {
      display: flex;
      flex-direction: column;
      border-bottom: none;
    }

    .cell {
      display: none;
      min-height: 0;
      border-left: none;
      border-bottom: 1px solid var(--line);
    }

    .cell.show,
    .cell.today {
      display: flex;
    }

    .cell.sum {
      display: flex;
      flex-direction: row;
      flex-wrap: wrap;
      gap: 0 var(--space-3);
      align-items: baseline;
    }

    .sum .value {
      margin-bottom: 0;
    }

    .wd-inline {
      display: inline;
      margin-right: 4px;
      color: var(--muted);
    }
  }
</style>
