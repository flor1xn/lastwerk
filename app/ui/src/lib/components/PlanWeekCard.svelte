<script lang="ts">
  import type { PlanWeek } from '../backend/types';
  import { addDays } from '../dates';
  import { formatDate, formatNumber, formatWeekday } from '../format';
  import { locale, t } from '../i18n';
  import {
    keyText,
    templateDescription,
    templateName,
    weekMinutes,
    weekPlannedLoad,
  } from '../plan';

  interface Props {
    week: PlanWeek;
    /** Nummer der Woche im Plan, ab 1. */
    index: number;
  }

  let { week, index }: Props = $props();

  const reasons = $derived((week.reason_keys ?? []).map((k) => keyText($locale, k)));
</script>

<section class="panel" data-testid="plan-week" data-phase={week.phase}>
  <header>
    <h3>{$t('ui.plan.view.week', { n: index })}</h3>
    <span class="range"
      >{$t('ui.plan.view.week_range', {
        from: formatDate(week.week_start, $locale),
        to: formatDate(addDays(week.week_start, 6), $locale),
      })}</span
    >
    <span class="phase" data-testid="plan-phase">{$t(`ui.plan.phase.${week.phase}`)}</span>
    <span class="kpis">
      <span
        ><span class="muted">{$t('ui.plan.view.target')}</span>
        <strong data-testid="week-target">{formatNumber(week.target_load, $locale, 1)}</strong
        ></span
      >
      <span
        ><span class="muted">{$t('ui.plan.view.planned')}</span>
        <strong data-testid="week-planned">{formatNumber(weekPlannedLoad(week), $locale, 1)}</strong
        ></span
      >
      <span class="muted">{$t('ui.plan.view.minutes', { n: weekMinutes(week) })}</span>
    </span>
  </header>
  {#if reasons.length > 0}
    <ul class="reasons" data-testid="week-reasons" aria-label={$t('ui.plan.view.reasons')}>
      {#each reasons as r, i (i)}
        <li>{r}</li>
      {/each}
    </ul>
  {/if}
  <table>
    <tbody>
      {#each week.days as day (day.date)}
        <tr data-testid="plan-day" data-date={day.date}>
          <th scope="row"
            >{formatWeekday(day.date, $locale)}
            <span class="muted">{formatDate(day.date, $locale).slice(0, 5)}</span></th
          >
          <td>
            {#if day.sessions.length === 0}
              <span class="muted">{$t('ui.plan.view.rest_day')}</span>
            {:else}
              {#each day.sessions as s (s.slot)}
                <div class="session" data-testid="plan-session" data-hard={s.hard}>
                  <span class="sport-dot" style:background={`var(--sport-${s.sport})`}></span>
                  <strong title={templateDescription($locale, s.template_id) ?? undefined}
                    >{templateName($locale, s.template_id)}</strong
                  >
                  <span class="muted">{$t(`ui.plan.slot.${s.slot}`)}</span>
                  <span>{$t('ui.plan.view.minutes', { n: s.duration_min })}</span>
                  <span
                    >{$t('ui.plan.view.load', {
                      n: formatNumber(s.planned_load, $locale, 1),
                    })}</span
                  >
                  <span class="intensity" class:hard={s.hard}
                    >{s.hard ? $t('ui.plan.view.hard') : $t('ui.plan.view.easy')}</span
                  >
                  {#if s.reason_keys && s.reason_keys.length > 0}
                    <span class="muted why" data-testid="session-reasons"
                      >{s.reason_keys.map((k) => keyText($locale, k)).join(' ')}</span
                    >
                  {/if}
                </div>
              {/each}
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</section>

<style>
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 14px;
    margin-bottom: 4px;
  }

  .phase {
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--panel-2);
  }

  .kpis {
    display: flex;
    gap: 4px 14px;
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }

  .reasons {
    margin: 0 0 6px;
    padding-left: 16px;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  th[scope='row'] {
    width: 90px;
    vertical-align: top;
    font-weight: 500;
    color: var(--text);
  }

  td {
    white-space: normal;
  }

  .session {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 10px;
    font-variant-numeric: tabular-nums;
  }

  .sport-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }

  .intensity {
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--muted);
    font-size: var(--fs-label);
  }

  .intensity.hard {
    border-color: var(--danger);
    color: var(--danger);
  }

  .why {
    flex-basis: 100%;
    font-size: var(--fs-small);
  }
</style>
