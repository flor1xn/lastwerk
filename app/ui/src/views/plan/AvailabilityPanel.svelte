<script lang="ts">
  import { untrack } from 'svelte';
  import { getBackend } from '../../lib/backend';
  import type { AvailabilityDay, PlanningInputs } from '../../lib/backend/types';
  import { errorKey } from '../../lib/errors';
  import { t } from '../../lib/i18n';
  import { WEEKDAYS } from '../../lib/plan';

  interface Props {
    inputs: PlanningInputs;
    onchange: (inputs: PlanningInputs) => void;
  }

  let { inputs, onchange }: Props = $props();

  // Arbeitskopie: Bearbeitet wird erst beim Speichern übernommen.
  let days = $state<AvailabilityDay[]>(untrack(() => inputs.availability.map((d) => ({ ...d }))));
  let errorMessage = $state<string | null>(null);
  let saved = $state(false);

  async function save(event: SubmitEvent) {
    event.preventDefault();
    errorMessage = null;
    saved = false;
    const clean = days.map((d) => ({ ...d, max_minutes: Math.trunc(Number(d.max_minutes)) }));
    if (
      clean.some(
        (d) => !Number.isFinite(d.max_minutes) || d.max_minutes < 0 || d.max_minutes > 1440,
      )
    ) {
      errorMessage = 'ui.error.invalid_value';
      return;
    }
    try {
      const next = await getBackend().setAvailability(clean);
      saved = true;
      onchange(next);
    } catch (e) {
      errorMessage = errorKey(e);
    }
  }
</script>

<section class="panel" data-testid="availability-section">
  <h3>{$t('ui.plan.avail.title')}</h3>
  <p class="muted hint">{$t('ui.plan.avail.hint', { minutes: inputs.default_minutes })}</p>
  {#if inputs.availability_set < 7}
    <p class="muted hint" data-testid="availability-unset">
      {$t('ui.plan.avail.unset', { n: 7 - inputs.availability_set })}
    </p>
  {/if}
  <form onsubmit={save}>
    <table>
      <thead>
        <tr>
          <th></th>
          <th>{$t('ui.plan.avail.minutes')}</th>
          <th>{$t('ui.plan.avail.hard')}</th>
          <th>{$t('ui.plan.avail.rest')}</th>
        </tr>
      </thead>
      <tbody>
        {#each days as day, i (day.weekday)}
          <tr>
            <th scope="row">{$t(`ui.plan.avail.weekday.${WEEKDAYS[i]}`)}</th>
            <td
              ><input
                type="number"
                min="0"
                max="1440"
                step="5"
                bind:value={day.max_minutes}
                disabled={day.fixed_rest}
                data-testid={`avail-minutes-${i}`}
                aria-label={`${$t(`ui.plan.avail.weekday.${WEEKDAYS[i]}`)} ${$t('ui.plan.avail.minutes')}`}
              /></td
            >
            <td
              ><input
                type="checkbox"
                bind:checked={day.allow_hard}
                disabled={day.fixed_rest}
                data-testid={`avail-hard-${i}`}
                aria-label={`${$t(`ui.plan.avail.weekday.${WEEKDAYS[i]}`)} ${$t('ui.plan.avail.hard')}`}
              /></td
            >
            <td
              ><input
                type="checkbox"
                bind:checked={day.fixed_rest}
                data-testid={`avail-rest-${i}`}
                aria-label={`${$t(`ui.plan.avail.weekday.${WEEKDAYS[i]}`)} ${$t('ui.plan.avail.rest')}`}
              /></td
            >
          </tr>
        {/each}
      </tbody>
    </table>
    <p>
      <button class="primary" type="submit" data-testid="avail-save"
        >{$t('ui.plan.avail.save')}</button
      >
    </p>
  </form>
  {#if errorMessage}
    <p class="error" role="alert" data-testid="avail-error">{$t(errorMessage)}</p>
  {/if}
  {#if saved}
    <p class="ok" data-testid="avail-saved">{$t('ui.plan.avail.saved')}</p>
  {/if}
</section>

<style>
  p {
    margin: 4px 0;
  }

  .hint {
    font-size: var(--fs-small);
  }

  input[type='number'] {
    width: 70px;
  }

  th[scope='row'] {
    color: var(--text);
    font-weight: 500;
  }

  .ok {
    color: var(--ok);
  }
</style>
