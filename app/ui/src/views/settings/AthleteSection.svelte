<script lang="ts">
  import { untrack } from 'svelte';
  import { getBackend } from '../../lib/backend';
  import type { Recompute, Settings } from '../../lib/backend/types';
  import { errorKey } from '../../lib/errors';
  import { t } from '../../lib/i18n';
  import { bumpData } from '../../lib/state';

  interface Props {
    settings: Settings;
    onchange: () => void;
  }

  let { settings, onchange }: Props = $props();

  // Das Formular startet mit den gespeicherten Werten und behält danach die Eingabe.
  let sex = $state<'' | 'male' | 'female'>(untrack(() => settings.athlete.sex ?? ''));
  let birthYear = $state(
    untrack(() =>
      settings.athlete.birth_year === null ? '' : String(settings.athlete.birth_year),
    ),
  );
  let saving = $state(false);
  let errorMessage = $state<string | null>(null);
  let result = $state<Recompute | null>(null);

  async function save(event: SubmitEvent) {
    event.preventDefault();
    errorMessage = null;
    result = null;
    const year = birthYear.trim() === '' ? undefined : Number(birthYear);
    if (year !== undefined && !Number.isInteger(year)) {
      errorMessage = 'ui.error.invalid_year';
      return;
    }
    saving = true;
    try {
      const set = await getBackend().setAthlete({
        ...(sex === '' ? {} : { sex }),
        ...(year === undefined ? {} : { birth_year: year }),
      });
      result = set.recompute;
      onchange();
      bumpData();
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      saving = false;
    }
  }
</script>

<section class="section" data-testid="athlete-section">
  <h3>{$t('ui.athlete.title')}</h3>
  <p class="muted hint">{$t('ui.athlete.hint')}</p>
  <form onsubmit={save}>
    <div class="fields">
      <label>
        <span
          >{$t(
            'ui.athlete.sex',
          )}{#if settings.athlete.sex !== null && settings.athlete.sex_origin !== null}
            <span class="origin" data-testid="athlete-sex-origin"
              >&nbsp;· {$t(`ui.athlete.origin.${settings.athlete.sex_origin}`)}</span
            >{/if}</span
        >
        <select bind:value={sex}>
          <option value="">{$t('ui.athlete.unset')}</option>
          <option value="male">{$t('ui.athlete.male')}</option>
          <option value="female">{$t('ui.athlete.female')}</option>
        </select>
      </label>
      <label>
        <span
          >{$t(
            'ui.athlete.birth_year',
          )}{#if settings.athlete.birth_year !== null && settings.athlete.birth_year_origin !== null}
            <span class="origin" data-testid="athlete-birth-year-origin"
              >&nbsp;· {$t(`ui.athlete.origin.${settings.athlete.birth_year_origin}`)}</span
            >{/if}</span
        >
        <input type="text" inputmode="numeric" size="6" maxlength="4" bind:value={birthYear} />
      </label>
      <button class="primary" type="submit" disabled={saving}>{$t('ui.athlete.save')}</button>
    </div>
  </form>
  {#if errorMessage}
    <p class="error" role="alert">{$t(errorMessage)}</p>
  {/if}
  {#if result}
    <p class="ok" data-testid="athlete-result">
      {#if result.skipped}
        {$t('ui.threshold.saved_not_applied')}
      {:else}
        {$t('ui.threshold.saved_recomputed', { n: result.recomputed, total: result.candidates })}
      {/if}
    </p>
  {/if}
</section>

<style>
  .hint {
    margin: 4px 0 8px;
  }

  .fields {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 8px 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  .origin {
    color: var(--muted);
  }

  .ok {
    color: var(--ok);
  }
</style>
