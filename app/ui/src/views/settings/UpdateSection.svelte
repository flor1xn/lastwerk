<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import type { UpdateInfo } from '../../lib/backend/types';
  import { errorKey } from '../../lib/errors';
  import { t } from '../../lib/i18n';

  // Update nur auf Klick (SPEC 10.1): kein Aufruf beim Öffnen der Ansicht, die URL kennt allein
  // der Rust-Teil. Installieren verlangt eine zweite Bestätigung, weil die App neu startet.
  let info = $state<UpdateInfo | null>(null);
  let busy = $state(false);
  let installing = $state(false);
  let confirming = $state(false);
  let errorMessage = $state<string | null>(null);

  async function check() {
    errorMessage = null;
    confirming = false;
    busy = true;
    try {
      info = await getBackend().checkUpdate();
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      busy = false;
    }
  }

  async function install() {
    errorMessage = null;
    busy = true;
    installing = true;
    try {
      await getBackend().installUpdate(true);
      info = null;
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      busy = false;
      installing = false;
      confirming = false;
    }
  }
</script>

<section class="section" data-testid="update-section">
  <h3>{$t('ui.update.title')}</h3>
  <p class="muted hint">{$t('ui.update.hint')}</p>
  {#if info}
    <p class="muted hint" data-testid="update-current">
      {$t('ui.update.current', { version: info.current })}
    </p>
  {/if}
  <p class="row">
    <button onclick={() => void check()} disabled={busy} data-testid="update-check">
      {busy && !installing ? $t('ui.update.checking') : $t('ui.update.check')}
    </button>
  </p>
  {#if info && !info.available}
    <p class="muted" data-testid="update-none">{$t('ui.update.none')}</p>
  {/if}
  {#if info?.available && info.version}
    <p data-testid="update-available">{$t('ui.update.available', { version: info.version })}</p>
    {#if confirming}
      <div class="confirm" role="alertdialog" aria-label={$t('ui.update.install')}>
        <p class="warn">{$t('ui.update.confirm')}</p>
        <p class="row">
          <button
            class="primary"
            onclick={() => void install()}
            disabled={busy}
            data-testid="update-yes">{$t('ui.update.install')}</button
          >
          <button onclick={() => (confirming = false)} disabled={busy}
            >{$t('ui.mcp.confirm_no')}</button
          >
        </p>
      </div>
    {:else}
      <p class="row">
        <button class="primary" onclick={() => (confirming = true)} data-testid="update-install">
          {$t('ui.update.install')}
        </button>
      </p>
    {/if}
  {/if}
  {#if installing}
    <p class="muted" data-testid="update-installing">{$t('ui.update.installing')}</p>
  {/if}
  {#if errorMessage}
    <p class="error" role="alert" data-testid="update-error">{$t(errorMessage)}</p>
  {/if}
</section>

<style>
  .hint {
    margin: 4px 0 8px;
  }

  .row {
    display: flex;
    gap: 8px;
  }
</style>
