<script lang="ts">
  import { untrack } from 'svelte';
  import { getBackend } from '../../lib/backend';
  import type { Settings } from '../../lib/backend/types';
  import { errorKey } from '../../lib/errors';
  import { formatNumber, parseDecimal } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { bumpData } from '../../lib/state';

  interface Props {
    settings: Settings;
    onchange: () => void;
  }

  let { settings, onchange }: Props = $props();

  const show = (v: number) => formatNumber(v, $locale, Number.isInteger(v) ? 0 : 1);

  let tauCtl = $state(untrack(() => show(settings.pmc.tau_ctl_d)));
  let tauAtl = $state(untrack(() => show(settings.pmc.tau_atl_d)));
  let ctl0 = $state(untrack(() => show(settings.pmc.ctl0)));
  let atl0 = $state(untrack(() => show(settings.pmc.atl0)));
  let errorMessage = $state<string | null>(null);
  let saved = $state(false);

  async function save(event: SubmitEvent) {
    event.preventDefault();
    errorMessage = null;
    saved = false;
    const values = [tauCtl, tauAtl, ctl0, atl0].map(parseDecimal);
    if (values.some((v) => v === null)) {
      errorMessage = 'ui.error.invalid_value';
      return;
    }
    const [a, b, c, d] = values as number[];
    try {
      await getBackend().setSettings({ pmc: { tau_ctl_d: a, tau_atl_d: b, ctl0: c, atl0: d } });
      saved = true;
      onchange();
      bumpData();
    } catch (e) {
      errorMessage = errorKey(e);
    }
  }
</script>

<section class="section" data-testid="pmc-section">
  <h3>{$t('ui.pmc.title')}</h3>
  <p class="muted hint">{$t('ui.pmc.hint')}</p>
  <form onsubmit={save}>
    <div class="fields">
      <label
        >{$t('ui.pmc.tau_ctl')}
        <input type="text" inputmode="decimal" size="5" bind:value={tauCtl} /></label
      >
      <label
        >{$t('ui.pmc.tau_atl')}
        <input type="text" inputmode="decimal" size="5" bind:value={tauAtl} /></label
      >
      <label
        >{$t('ui.pmc.ctl0')}
        <input type="text" inputmode="decimal" size="5" bind:value={ctl0} /></label
      >
      <label
        >{$t('ui.pmc.atl0')}
        <input type="text" inputmode="decimal" size="5" bind:value={atl0} /></label
      >
      <button class="primary" type="submit">{$t('ui.pmc.save')}</button>
    </div>
  </form>
  {#if errorMessage}
    <p class="error" role="alert">{$t(errorMessage)}</p>
  {/if}
  {#if saved}
    <p class="ok" data-testid="pmc-saved">{$t('ui.pmc.saved')}</p>
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

  .ok {
    color: var(--ok);
  }
</style>
