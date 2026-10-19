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
    /** Laufende Woche: ausgeklappt und hervorgehoben (T64, SPEC 5.5). Die übrigen Wochen sind
     *  Zeilen, die man aufklappt. */
    current?: boolean;
  }

  let { week, index, current = false }: Props = $props();

  const reasons = $derived((week.reason_keys ?? []).map((k) => keyText($locale, k)));
</script>

{#snippet head()}
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
      <strong data-testid="week-target">{formatNumber(week.target_load, $locale, 1)}</strong></span
    >
    <span
      ><span class="muted">{$t('ui.plan.view.planned')}</span>
      <strong data-testid="week-planned">{formatNumber(weekPlannedLoad(week), $locale, 1)}</strong
      ></span
    >
    <span class="muted">{$t('ui.plan.view.minutes', { n: weekMinutes(week) })}</span>
  </span>
{/snippet}

{#snippet body()}
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
{/snippet}

{#if current}
  <section class="week current" data-testid="plan-week" data-phase={week.phase} data-current="true">
    <p class="now">{$t('ui.plan.view.current_week')}</p>
    <header>{@render head()}</header>
    {@render body()}
  </section>
{:else}
  <details class="week" data-testid="plan-week" data-phase={week.phase}>
    <summary>{@render head()}</summary>
    {@render body()}
  </details>
{/if}

<style>
  .week {
    border-bottom: 1px solid var(--line);
  }

  .week.current {
    padding: var(--space-4) var(--space-5) var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    background: var(--panel);
  }

  .now {
    margin: 0 0 var(--space-1);
    font-size: var(--fs-label);
    font-weight: var(--fw-semibold);
    letter-spacing: var(--ls-label);
    text-transform: uppercase;
    color: var(--accent);
  }

  header,
  summary {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 14px;
    margin-bottom: 4px;
  }

  summary {
    min-height: 44px;
    align-items: center;
    margin: 0;
    padding: var(--space-2) 0;
    cursor: pointer;
    list-style: none;
  }

  summary::-webkit-details-marker {
    display: none;
  }

  /* Pfeil zeigt, dass die Zeile aufklappt; er dreht sich, sonst bewegt sich nichts. */
  summary::before {
    content: '';
    width: 6px;
    height: 6px;
    margin-right: 2px;
    border-right: 1.5px solid var(--muted);
    border-bottom: 1.5px solid var(--muted);
    transform: rotate(-45deg);
    transition: transform 160ms var(--ease-out);
  }

  details[open] > summary::before {
    transform: rotate(45deg);
  }

  details[open] {
    padding-bottom: var(--space-3);
  }

  summary h3,
  header h3 {
    margin: 0;
  }

  .phase {
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-chip);
    font-size: var(--fs-small);
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
    border-radius: var(--radius-chip);
    color: var(--muted);
    font-size: var(--fs-label);
  }

  /* Hart ist kein Fehler: Text und kräftiger Rand statt Warnfarbe. */
  .intensity.hard {
    border-color: var(--text-2);
    color: var(--text);
    font-weight: var(--fw-semibold);
  }

  .why {
    flex-basis: 100%;
    font-size: var(--fs-small);
  }
</style>
