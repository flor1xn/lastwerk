<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import {
    ENDURANCE_SPORTS,
    type EnduranceSport,
    type PlanDetail,
    type PlanningInputs,
  } from '../../lib/backend/types';
  import { isIsoDate, mondayOf } from '../../lib/dates';
  import { errorKey } from '../../lib/errors';
  import { formatDate } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { nextMonday } from '../../lib/plan';

  interface Props {
    inputs: PlanningInputs;
    ongenerated: (detail: PlanDetail) => void;
  }

  let { inputs, ongenerated }: Props = $props();

  let weekStart = $state(nextMonday());
  // Ein Zahlenfeld liefert eine Zahl, leer ist null.
  let weeks = $state<number | null>(null);
  let sports = $state<EnduranceSport[]>([]);
  let strength = $state(1);
  let running = $state(false);
  let errorMessage = $state<string | null>(null);
  let done = $state(false);

  const active = $derived(inputs.goals.find((g) => g.active) ?? null);
  // Liegt der Termin des aktiven Ziels vor dem Planbeginn, plant der Planer fortlaufend.
  const eventPast = $derived(
    active?.kind === 'event' &&
      active.event_date !== null &&
      isIsoDate(weekStart) &&
      active.event_date < weekStart,
  );

  function toggle(s: EnduranceSport, checked: boolean) {
    sports = checked ? [...sports, s] : sports.filter((x) => x !== s);
  }

  async function generate(event: SubmitEvent) {
    event.preventDefault();
    errorMessage = null;
    done = false;
    if (!isIsoDate(weekStart)) {
      errorMessage = 'ui.error.invalid_date';
      return;
    }
    if (mondayOf(weekStart) !== weekStart) {
      errorMessage = 'ui.error.not_monday';
      return;
    }
    const count = weeks === null ? null : Number(weeks);
    if (count !== null && !(Number.isInteger(count) && count >= 1 && count <= 26)) {
      errorMessage = 'ui.error.invalid_value';
      return;
    }
    running = true;
    try {
      const detail = await getBackend().generatePlan({
        week_start: weekStart,
        goal_date: null,
        goal_sport: null,
        weeks: count,
        locale: $locale,
        sports,
        strength_per_week: strength,
      });
      done = true;
      ongenerated(detail);
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      running = false;
    }
  }
</script>

<section class="panel" data-testid="generate-section">
  <h3>{$t('ui.plan.generate.title')}</h3>
  {#if eventPast && active?.event_date}
    <p class="warn" data-testid="goal-past">
      {$t('ui.plan.goal.past')}
      ({formatDate(active.event_date, $locale)})
    </p>
  {/if}
  <form onsubmit={generate}>
    <div class="fields">
      <label
        >{$t('ui.plan.generate.week_start')}
        <input type="date" bind:value={weekStart} data-testid="gen-week-start" /></label
      >
      <label
        >{$t('ui.plan.generate.weeks')}
        <input
          type="number"
          min="1"
          max="26"
          bind:value={weeks}
          data-testid="gen-weeks"
          class="short"
        /></label
      >
      <label
        >{$t('ui.plan.generate.strength')}
        <select bind:value={strength} data-testid="gen-strength">
          <option value={0}>0</option>
          <option value={1}>1</option>
          <option value={2}>2</option>
        </select></label
      >
    </div>
    <p class="muted hint">{$t('ui.plan.generate.weeks_hint')}</p>
    <fieldset>
      <legend>{$t('ui.plan.generate.sports')}</legend>
      {#each ENDURANCE_SPORTS as s (s)}
        <label class="check"
          ><input
            type="checkbox"
            checked={sports.includes(s)}
            onchange={(e) => toggle(s, e.currentTarget.checked)}
            data-testid={`gen-sport-${s}`}
          />
          {$t(`sport.${s}`)}</label
        >
      {/each}
    </fieldset>
    <p class="muted hint">{$t('ui.plan.generate.sports_hint')}</p>
    <button class="primary" type="submit" disabled={running} data-testid="gen-run">
      {running ? $t('ui.plan.generate.running') : $t('ui.plan.generate.run')}
    </button>
  </form>
  {#if errorMessage}
    <p class="error" role="alert" data-testid="gen-error">{$t(errorMessage)}</p>
  {/if}
  {#if done}
    <p class="ok" data-testid="gen-done">{$t('ui.plan.generate.done')}</p>
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

  label.check {
    display: inline-flex;
    flex-direction: row;
    align-items: center;
    gap: 4px;
    margin-right: 10px;
    color: var(--text);
    font-size: var(--fs-small);
  }

  .short {
    width: 70px;
  }

  fieldset {
    margin: 8px 0 0;
    padding: 4px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
  }

  legend {
    padding: 0 4px;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  .hint {
    font-size: var(--fs-small);
  }

  .ok {
    color: var(--ok);
  }
</style>
