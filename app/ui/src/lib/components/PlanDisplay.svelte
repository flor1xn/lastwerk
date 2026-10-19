<script lang="ts">
  import type { PlanDetail } from '../backend/types';
  import { localIso } from '../dates';
  import { formatDate, formatNumber } from '../format';
  import { locale, t } from '../i18n';
  import { planTotals } from '../plan';
  import PlanWarnings from './PlanWarnings.svelte';
  import PlanWeekCard from './PlanWeekCard.svelte';
  import PmcChart from './PmcChart.svelte';

  interface Props {
    detail: PlanDetail;
  }

  let { detail }: Props = $props();

  const plan = $derived(detail.plan);
  // Laufende Woche (heute im Plan), sonst die erste: Sie steht ausgeklappt oben (T64).
  const today = localIso();
  const currentIndex = $derived(
    Math.max(
      0,
      plan.weeks.findIndex((w) => w.days.some((d) => d.date === today)),
    ),
  );
  const totals = $derived(planTotals(plan));
  const goalLine = $derived(
    plan.goal.kind === 'event' && plan.goal.event_date
      ? $t('ui.plan.view.goal_event', { date: formatDate(plan.goal.event_date, $locale) })
      : $t('ui.plan.view.goal_continuous'),
  );
  const meta = $derived(
    [
      plan.goal.kind === 'event' && plan.goal.sport
        ? $t('ui.plan.view.meta_sport', { sport: $t(`sport.${plan.goal.sport}`) })
        : null,
      plan.goal.kind === 'event' && plan.goal.priority
        ? $t('ui.plan.view.meta_priority', { priority: plan.goal.priority })
        : null,
      $t('ui.plan.view.meta_source', { source: $t(`ui.plan.source.${plan.source}`) }),
      $t('ui.plan.view.meta_rules', { version: plan.rules_version }),
    ].filter((x): x is string => x !== null),
  );
</script>

<div
  class="plan"
  data-testid="plan-display"
  data-plan-id={plan.plan_id}
  data-status={detail.status}
>
  <section class="panel head">
    <h2>
      {goalLine}
      <span class="status" data-testid="plan-status">{$t(`ui.plan.status.${detail.status}`)}</span>
    </h2>
    <p class="muted">{meta.join(', ')}</p>
    <p data-testid="plan-totals">
      {$t('ui.plan.view.totals', {
        sessions: totals.sessions,
        minutes: totals.minutes,
        load: formatNumber(totals.load, $locale, 1),
      })}
    </p>
    {#if plan.explanation}
      <!-- Text des Sprachmodells: als Text, nie als HTML (SPEC 9.5). -->
      <div class="explain">
        <h3>{$t('ui.plan.view.explanation')}</h3>
        <p class="explanation" data-testid="plan-explanation">{plan.explanation}</p>
      </div>
    {/if}
  </section>

  <PlanWarnings warnings={plan.warnings} validation={detail.validation} />

  {#if plan.weeks[currentIndex]}
    <PlanWeekCard week={plan.weeks[currentIndex]} index={currentIndex + 1} current />
  {/if}

  <div class="weeks" aria-label={$t('ui.plan.view.all_weeks')}>
    {#each plan.weeks as week, i (week.week_start)}
      {#if i !== currentIndex}
        <PlanWeekCard {week} index={i + 1} />
      {/if}
    {/each}
  </div>

  <section class="section" data-testid="plan-projection">
    <h3>{$t('ui.plan.projection.title')}</h3>
    <p class="muted">
      {$t('ui.plan.projection.hint', {
        ctl: formatNumber(detail.ctl0, $locale, 1),
        atl: formatNumber(detail.atl0, $locale, 1),
      })}
    </p>
    <PmcChart history={detail.projection} height={220} label={$t('ui.plan.projection.title')} />
  </section>
</div>

<style>
  .plan {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .weeks {
    border-top: 1px solid var(--line);
  }

  .head p {
    margin: 4px 0;
  }

  .status {
    margin-left: 8px;
    padding: 0 6px;
    border: 1px solid var(--accent);
    border-radius: var(--radius-chip);
    color: var(--accent);
    font-size: var(--fs-label);
    font-weight: 500;
    vertical-align: 1px;
  }

  .explain {
    margin-top: 8px;
  }

  .explanation {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
