<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import {
    ENDURANCE_SPORTS,
    type EnduranceSport,
    type PlanDetail,
    type PlanningInputs,
  } from '../../lib/backend/types';
  import { isIsoDate } from '../../lib/dates';
  import { errorKey } from '../../lib/errors';
  import { formatDate, formatNumber } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { nextMonday, WEEKDAYS } from '../../lib/plan';
  import {
    defaultAnswers,
    minutesPerDay,
    SLIDER_MAX,
    SLIDER_MIN,
    SLIDER_STEP,
  } from '../../lib/planner';

  // Die drei Fragen (T26, SPEC 7.2, O31): Ziel, Stunden je Woche, Trainingstage. Eine
  // Primäraktion legt Ziel und Verfügbarkeit fest, lässt den Regelplaner den Plan erzeugen
  // und aktiviert ihn. Die Vorbelegung kommt aus dem aktiven Ziel und der Verfügbarkeit.
  interface Props {
    inputs: PlanningInputs | null;
    /** Der Plan ist erzeugt und aktiv. */
    oncreated: (detail: PlanDetail) => void;
  }

  let { inputs, oncreated }: Props = $props();

  // Die Eingaben starten mit den gespeicherten Werten. Wechseln die Daten von außen
  // (Wiederherstellung), baut die Ansicht dieses Formular neu auf.
  // svelte-ignore state_referenced_locally
  const initial = defaultAnswers(inputs);
  let kind = $state<'event' | 'continuous'>(initial.kind);
  let date = $state(initial.date);
  let sport = $state<EnduranceSport>(initial.sport);
  let hours = $state(initial.hours);
  let days = $state<number[]>(initial.days);
  let running = $state(false);
  let errorKey_ = $state<string | null>(null);
  let done = $state(false);

  const weekStart = nextMonday();
  const fill = $derived(((hours - SLIDER_MIN) / (SLIDER_MAX - SLIDER_MIN)) * 100);
  const perDay = $derived(minutesPerDay(hours, days.length));
  const daysHint = $derived(
    days.length === 0
      ? $t('ui.planner.days_none')
      : days.length === 1
        ? $t('ui.planner.days_one', { minutes: perDay })
        : $t('ui.planner.days_many', { count: days.length, minutes: perDay }),
  );

  function touch() {
    done = false;
    errorKey_ = null;
  }

  function toggleDay(i: number) {
    touch();
    days = days.includes(i) ? days.filter((d) => d !== i) : [...days, i].sort((a, b) => a - b);
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    touch();
    if (days.length === 0) return;
    if (kind === 'event' && (!isIsoDate(date) || date < weekStart)) {
      errorKey_ = 'ui.error.invalid_date';
      return;
    }
    running = true;
    try {
      const detail = await getBackend().quickPlan({
        goal: {
          kind,
          date: kind === 'event' ? date : null,
          sport: kind === 'event' ? sport : null,
        },
        hours_per_week: hours,
        days,
        week_start: weekStart,
        locale: $locale,
      });
      done = true;
      oncreated(detail);
    } catch (e) {
      errorKey_ = errorKey(e);
    } finally {
      running = false;
    }
  }
</script>

<section class="card planner" data-testid="planner" aria-labelledby="planner-title">
  <h2 id="planner-title" class="title">{$t('ui.planner.title')}</h2>
  <form onsubmit={submit} novalidate>
    <div class="q" role="group" aria-labelledby="planner-q1">
      <p class="legend" id="planner-q1">
        <span class="num" aria-hidden="true">1</span>{$t('ui.planner.q_goal')}
      </p>
      <div class="seg" role="radiogroup" aria-labelledby="planner-q1">
        <label class:on={kind === 'event'}>
          <input
            type="radio"
            name="planner-kind"
            value="event"
            bind:group={kind}
            onchange={touch}
            data-testid="planner-kind-event"
          />
          {$t('ui.planner.goal_event')}
        </label>
        <label class:on={kind === 'continuous'}>
          <input
            type="radio"
            name="planner-kind"
            value="continuous"
            bind:group={kind}
            onchange={touch}
            data-testid="planner-kind-continuous"
          />
          {$t('ui.planner.goal_fit')}
        </label>
      </div>
      {#if kind === 'event'}
        <div class="event">
          <label>
            <span class="field-label">{$t('ui.planner.goal_date')}</span>
            <input type="date" bind:value={date} oninput={touch} data-testid="planner-date" />
          </label>
          <label>
            <span class="field-label">{$t('ui.planner.goal_sport')}</span>
            <select bind:value={sport} onchange={touch} data-testid="planner-sport">
              {#each ENDURANCE_SPORTS as s (s)}
                <option value={s}>{$t(`sport.${s}`)}</option>
              {/each}
            </select>
          </label>
        </div>
      {/if}
    </div>

    <div class="q" role="group" aria-labelledby="planner-q2">
      <p class="legend" id="planner-q2">
        <span class="num" aria-hidden="true">2</span>{$t('ui.planner.q_hours')}
      </p>
      <div class="hours">
        <output class="hours-value" data-testid="planner-hours-value"
          >{formatNumber(hours, $locale, hours % 1 === 0 ? 0 : 1)}<span class="unit"
            >{$t('ui.planner.hours_unit')}</span
          ></output
        >
        <input
          class="range"
          type="range"
          min={SLIDER_MIN}
          max={SLIDER_MAX}
          step={SLIDER_STEP}
          bind:value={hours}
          oninput={touch}
          style:--fill="{fill}%"
          aria-label={$t('ui.planner.q_hours')}
          data-testid="planner-hours"
        />
      </div>
      <div class="scale" aria-hidden="true">
        <span>{SLIDER_MIN} {$t('ui.planner.hours_unit')}</span>
        <span>{SLIDER_MAX} {$t('ui.planner.hours_unit')}</span>
      </div>
      <p class="hint">{$t('ui.planner.hours_hint')}</p>
    </div>

    <div class="q" role="group" aria-labelledby="planner-q3">
      <p class="legend" id="planner-q3">
        <span class="num" aria-hidden="true">3</span>{$t('ui.planner.q_days')}
      </p>
      <div class="days">
        {#each WEEKDAYS as key, i (key)}
          <button
            type="button"
            class="day"
            class:on={days.includes(i)}
            aria-pressed={days.includes(i)}
            aria-label={$t(`ui.plan.avail.weekday.${key}`)}
            onclick={() => toggleDay(i)}
            data-testid={`planner-day-${i}`}
          >
            {$t(`ui.planner.weekday_short.${key}`)}
          </button>
        {/each}
      </div>
      <p class="hint" data-testid="planner-days-hint">{daysHint}</p>
    </div>

    <div class="foot">
      <p class="start" data-testid="planner-start">
        {$t('ui.planner.start', { date: formatDate(weekStart, $locale) })}
      </p>
      <button
        class="primary"
        type="submit"
        disabled={running || days.length === 0}
        data-testid="planner-submit"
      >
        {running ? $t('ui.planner.running') : $t('ui.planner.submit')}
      </button>
    </div>
    <div class="status" aria-live="polite">
      {#if errorKey_}
        <p class="error" role="alert" data-testid="planner-error">{$t(errorKey_)}</p>
      {:else if done}
        <p class="ok" data-testid="planner-done">{$t('ui.planner.done')}</p>
      {/if}
    </div>
  </form>
</section>

<style>
  .planner {
    padding: var(--space-5) var(--space-6) var(--space-5);
  }

  .title {
    margin: 0 0 var(--space-4);
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
    letter-spacing: var(--ls-lead);
  }

  .q {
    padding: var(--space-4) 0;
    border-top: 1px solid var(--line);
    min-width: 0;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 0 var(--space-3);
    font-size: var(--fs-body);
    line-height: var(--lh-tight);
    font-weight: var(--fw-semibold);
  }

  .num {
    display: inline-grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--panel-2);
    color: var(--muted);
    font-size: var(--fs-small);
    font-weight: var(--fw-bold);
  }

  .seg {
    display: inline-flex;
    padding: 3px;
    gap: 3px;
    border-radius: var(--radius-button);
    background: var(--panel-2);
    border: 1px solid var(--border);
  }

  .seg label {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-height: 36px;
    padding: 0 var(--space-4);
    border-radius: 9px;
    color: var(--muted);
    font-weight: var(--fw-medium);
    cursor: pointer;
    transition:
      background-color 120ms ease-out,
      color 120ms ease-out;
  }

  .seg label.on {
    background: var(--bg);
    color: var(--text);
    font-weight: var(--fw-semibold);
    box-shadow: 0 1px 0 var(--line);
  }

  .seg input {
    position: absolute;
    inset: 0;
    opacity: 0;
    margin: 0;
    cursor: pointer;
  }

  .seg label:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .event {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3) var(--space-4);
    margin-top: var(--space-3);
  }

  .event label {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .field-label {
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .hours {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  .hours-value {
    flex: none;
    min-width: 4.2ch;
    display: flex;
    align-items: baseline;
    gap: 4px;
    font-size: var(--fs-title);
    line-height: var(--lh-display);
    font-weight: var(--fw-bold);
    letter-spacing: var(--ls-display);
    font-variant-numeric: tabular-nums;
  }

  .unit {
    font-size: var(--fs-body);
    font-weight: var(--fw-medium);
    color: var(--muted);
    letter-spacing: 0;
  }

  .range {
    flex: 1;
    min-width: 0;
    height: 28px;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    -webkit-appearance: none;
    appearance: none;
    cursor: pointer;
  }

  .range::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: 3px;
    background: linear-gradient(
      to right,
      var(--accent) 0%,
      var(--accent) var(--fill),
      var(--panel-2) var(--fill),
      var(--panel-2) 100%
    );
    border: 1px solid var(--border);
  }

  .range::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 22px;
    height: 22px;
    margin-top: -9px;
    border-radius: 50%;
    background: var(--text);
    border: 3px solid var(--bg);
    box-shadow: 0 0 0 1px var(--border);
    transition: transform 120ms ease-out;
  }

  .range:active::-webkit-slider-thumb {
    transform: scale(1.1);
  }

  .range::-moz-range-track {
    height: 6px;
    border-radius: 3px;
    background: var(--panel-2);
    border: 1px solid var(--border);
  }

  .range::-moz-range-progress {
    height: 6px;
    border-radius: 3px;
    background: var(--accent);
  }

  .range::-moz-range-thumb {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text);
    border: 3px solid var(--bg);
  }

  .range:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 4px;
  }

  .scale {
    display: flex;
    justify-content: space-between;
    margin-left: calc(4.2ch + var(--space-4));
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .hint {
    margin: var(--space-2) 0 0;
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
    color: var(--muted);
  }

  .days {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: var(--space-2);
  }

  .day {
    min-height: 44px;
    padding: 0;
    font-weight: var(--fw-semibold);
    color: var(--muted);
    transition:
      background-color 120ms ease-out,
      border-color 120ms ease-out,
      color 120ms ease-out,
      transform 120ms ease-out;
  }

  .day.on {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg);
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3) var(--space-4);
    padding-top: var(--space-4);
    border-top: 1px solid var(--line);
  }

  .start {
    margin: 0;
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .status {
    min-height: 1.4em;
    margin-top: var(--space-2);
  }

  .status p {
    margin: 0;
    font-size: var(--fs-small);
  }

  .ok {
    color: var(--ok);
  }

  @media (max-width: 480px) {
    .planner {
      padding: var(--space-4);
    }
  }
</style>
