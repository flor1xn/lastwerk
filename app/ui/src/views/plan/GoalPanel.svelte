<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import {
    ENDURANCE_SPORTS,
    type EnduranceSport,
    type PlanningInputs,
    type Priority,
  } from '../../lib/backend/types';
  import { isIsoDate } from '../../lib/dates';
  import { errorKey } from '../../lib/errors';
  import { formatDate } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { describeGoal } from '../../lib/plan';

  interface Props {
    inputs: PlanningInputs;
    onchange: (inputs: PlanningInputs) => void;
  }

  let { inputs, onchange }: Props = $props();

  let kind = $state<'event' | 'continuous'>('event');
  let date = $state('');
  let sport = $state<EnduranceSport | ''>('');
  let priority = $state<Priority>('A');
  let label = $state('');
  let errorMessage = $state<string | null>(null);
  let saved = $state(false);

  const active = $derived(inputs.goals.find((g) => g.active) ?? null);

  async function save(event: SubmitEvent) {
    event.preventDefault();
    errorMessage = null;
    saved = false;
    if (kind === 'event' && !isIsoDate(date)) {
      errorMessage = 'ui.error.invalid_date';
      return;
    }
    try {
      const next = await getBackend().setGoal({
        event_date: kind === 'event' ? date : null,
        sport: kind === 'event' && sport !== '' ? sport : null,
        priority: kind === 'event' ? priority : null,
        label: label.trim() === '' ? null : label,
      });
      saved = true;
      onchange(next);
    } catch (e) {
      errorMessage = errorKey(e);
    }
  }
</script>

<section class="panel" data-testid="goal-section">
  <h3>{$t('ui.plan.goal.title')}</h3>
  <p data-testid="goal-active">
    {#if active}
      <span class="muted">{$t('ui.plan.goal.active')}:</span>
      {describeGoal($t, active, (d) => formatDate(d, $locale))}
      {#if active.label}<span class="muted">({active.label})</span>{/if}
    {:else}
      <span class="muted">{$t('ui.plan.goal.none')}</span>
    {/if}
  </p>
  <form onsubmit={save}>
    <div class="fields">
      <label
        >{$t('ui.plan.goal.kind')}
        <select bind:value={kind} data-testid="goal-kind">
          <option value="event">{$t('ui.plan.goal.kind_event')}</option>
          <option value="continuous">{$t('ui.plan.goal.kind_continuous')}</option>
        </select></label
      >
      {#if kind === 'event'}
        <label
          >{$t('ui.plan.goal.date')}
          <input type="date" bind:value={date} data-testid="goal-date" /></label
        >
        <label
          >{$t('ui.plan.goal.sport')}
          <select bind:value={sport} data-testid="goal-sport">
            <option value="">{$t('ui.plan.goal.sport_auto')}</option>
            {#each ENDURANCE_SPORTS as s (s)}
              <option value={s}>{$t(`sport.${s}`)}</option>
            {/each}
          </select></label
        >
        <label
          >{$t('ui.plan.goal.priority')}
          <select bind:value={priority} data-testid="goal-priority">
            <option value="A">A</option>
            <option value="B">B</option>
            <option value="C">C</option>
          </select></label
        >
      {/if}
      <label class="wide"
        >{$t('ui.plan.goal.label')}
        <input type="text" maxlength="200" bind:value={label} data-testid="goal-label" /></label
      >
    </div>
    <p class="muted hint">{$t('ui.plan.goal.label_hint')}</p>
    <button class="primary" type="submit" data-testid="goal-save">{$t('ui.plan.goal.save')}</button>
  </form>
  {#if errorMessage}
    <p class="error" role="alert" data-testid="goal-error">{$t(errorMessage)}</p>
  {/if}
  {#if saved}
    <p class="ok" data-testid="goal-saved">{$t('ui.plan.goal.saved')}</p>
  {/if}
</section>

<style>
  p {
    margin: 4px 0;
  }

  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 10px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  label.wide {
    flex-basis: 100%;
  }

  .hint {
    font-size: var(--fs-small);
  }

  .ok {
    color: var(--ok);
  }
</style>
