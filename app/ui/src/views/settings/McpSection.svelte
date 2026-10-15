<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import type { Settings } from '../../lib/backend/types';
  import { errorKey } from '../../lib/errors';
  import { t } from '../../lib/i18n';

  interface Props {
    settings: Settings;
    onchange: () => void;
  }

  let { settings, onchange }: Props = $props();

  // Das Einschalten verlangt eine Bestätigung (SPEC 9.6, Punkt 1), das Ausschalten nicht.
  let confirming = $state(false);
  let errorMessage = $state<string | null>(null);
  let busy = $state(false);

  async function write(enabled: boolean) {
    errorMessage = null;
    busy = true;
    try {
      await getBackend().setSettings({ mcp_recovery_enabled: enabled });
      confirming = false;
      onchange();
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      busy = false;
    }
  }

  function toggle(event: Event & { currentTarget: HTMLInputElement }) {
    const checked = event.currentTarget.checked;
    // Der Schalter zeigt immer den gespeicherten Zustand, nie einen unbestätigten.
    event.currentTarget.checked = settings.mcp_recovery_enabled;
    if (checked) {
      confirming = true;
    } else {
      confirming = false;
      void write(false);
    }
  }
</script>

<section class="section" data-testid="mcp-section">
  <h3>{$t('ui.mcp.title')}</h3>
  <p class="muted hint">{$t('ui.mcp.hint')}</p>
  <label class="switch">
    <input
      type="checkbox"
      checked={settings.mcp_recovery_enabled}
      disabled={busy}
      onchange={toggle}
      data-testid="mcp-recovery-toggle"
    />
    <span>{$t('ui.mcp.recovery')}</span>
  </label>
  <p class="muted hint" data-testid="mcp-recovery-state">
    {settings.mcp_recovery_enabled ? $t('ui.mcp.state_on') : $t('ui.mcp.state_off')}
  </p>
  <p class="muted hint">{$t('ui.mcp.recovery_hint')}</p>
  {#if confirming}
    <div
      class="confirm"
      role="alertdialog"
      aria-label={$t('ui.mcp.recovery')}
      data-testid="mcp-recovery-confirm"
    >
      <p class="warn">{$t('ui.mcp.confirm')}</p>
      <p class="row">
        <button
          class="primary"
          onclick={() => void write(true)}
          disabled={busy}
          data-testid="mcp-recovery-yes">{$t('ui.mcp.confirm_yes')}</button
        >
        <button onclick={() => (confirming = false)} disabled={busy} data-testid="mcp-recovery-no"
          >{$t('ui.mcp.confirm_no')}</button
        >
      </p>
    </div>
  {/if}
  {#if errorMessage}
    <p class="error" role="alert" data-testid="mcp-error">{$t(errorMessage)}</p>
  {/if}
</section>

<style>
  .hint {
    margin: 4px 0 8px;
  }

  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .confirm {
    margin: 6px 0;
    padding: 6px 10px;
    border: 1px solid var(--warn);
    border-radius: 4px;
  }

  .row {
    display: flex;
    gap: 8px;
  }
</style>
