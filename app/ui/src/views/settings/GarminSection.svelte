<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import type { BridgeStatus, Settings } from '../../lib/backend/types';
  import { errorKey } from '../../lib/errors';
  import { formatDate } from '../../lib/format';
  import { disconnectGarmin, garminStatus, garminSync, runGarminSync } from '../../lib/garmin';
  import { locale, t } from '../../lib/i18n';
  import GarminConnect from './GarminConnect.svelte';
  import SensitiveShares from './SensitiveShares.svelte';

  // Abschnitt "Garmin" der Einstellungen (T23, SPEC 5.6, 10.1: jede Verbindung ist hier
  // sichtbar). Trennen fragt nach. Der Status des eingefrorenen Zusatzprogramms steht nur noch
  // unter "Erweitert" und wird erst beim Aufklappen gelesen.

  interface Props {
    settings: Settings;
    onchange: () => void;
  }

  let { settings, onchange }: Props = $props();

  let confirming = $state(false);
  let busy = $state(false);
  let disconnected = $state(false);
  let errorMessage = $state<string | null>(null);
  let bridge = $state<BridgeStatus | null>(null);

  const connected = $derived($garminStatus?.connected ?? false);
  const running = $derived($garminSync.kind === 'running');

  function lockedText(iso: string): string {
    const time = new Date(iso).toLocaleString($locale === 'de' ? 'de-DE' : 'en-US', {
      dateStyle: 'short',
      timeStyle: 'short',
    });
    return $t('ui.garmin.locked_until', { time });
  }

  async function syncNow() {
    errorMessage = null;
    disconnected = false;
    await runGarminSync('manual');
  }

  async function toggleOnStart(event: Event & { currentTarget: HTMLInputElement }) {
    const on = event.currentTarget.checked;
    errorMessage = null;
    busy = true;
    try {
      await getBackend().setSettings({ garmin_sync_on_start: on });
      onchange();
    } catch (e) {
      event.currentTarget.checked = !on;
      errorMessage = errorKey(e);
    } finally {
      busy = false;
    }
  }

  async function disconnect() {
    errorMessage = null;
    busy = true;
    try {
      await disconnectGarmin();
      confirming = false;
      disconnected = true;
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      busy = false;
    }
  }

  async function loadBridge() {
    try {
      bridge = await getBackend().getBridgeStatus();
    } catch (e) {
      errorMessage = errorKey(e);
    }
  }

  function onAdvancedToggle(event: Event & { currentTarget: HTMLDetailsElement }) {
    if (event.currentTarget.open && bridge === null) void loadBridge();
  }
</script>

<section class="section" data-testid="garmin-section">
  <h3>{$t('ui.garmin.title')}</h3>
  <p class="muted hint">{$t('ui.garmin.statement')}</p>
  <p class="muted hint" data-testid="garmin-risk">{$t('ui.garmin.risk')}</p>

  <GarminConnect />

  {#if $garminStatus}
    <p class="muted hint" data-testid="garmin-last-sync">
      {$garminStatus.last_sync
        ? $t('ui.garmin.last_sync', { date: formatDate($garminStatus.last_sync, $locale) })
        : $t('ui.garmin.never_synced')}
    </p>
    {#if $garminStatus.locked_until}
      <p class="warn hint" data-testid="garmin-locked">{lockedText($garminStatus.locked_until)}</p>
    {/if}
  {/if}

  {#if connected}
    <p class="row">
      <button
        class="primary"
        onclick={() => void syncNow()}
        disabled={running || busy}
        data-testid="garmin-sync-now">{$t('ui.garmin.sync_now')}</button
      >
      <button
        onclick={() => (confirming = true)}
        disabled={running || busy || confirming}
        data-testid="garmin-disconnect">{$t('ui.garmin.disconnect')}</button
      >
    </p>
    {#if confirming}
      <div
        class="confirm"
        role="alertdialog"
        aria-label={$t('ui.garmin.disconnect')}
        data-testid="garmin-disconnect-confirm"
      >
        <p class="warn">{$t('ui.garmin.disconnect_confirm')}</p>
        <p class="row">
          <button
            class="primary"
            onclick={() => void disconnect()}
            disabled={busy}
            data-testid="garmin-disconnect-yes">{$t('ui.garmin.disconnect_yes')}</button
          >
          <button
            onclick={() => (confirming = false)}
            disabled={busy}
            data-testid="garmin-disconnect-no">{$t('ui.garmin.disconnect_no')}</button
          >
        </p>
      </div>
    {/if}
  {/if}
  {#if disconnected && !connected}
    <p class="muted hint" data-testid="garmin-disconnected">{$t('ui.garmin.disconnected')}</p>
  {/if}

  <label class="switch">
    <input
      type="checkbox"
      checked={settings.garmin_sync_on_start}
      disabled={busy}
      onchange={toggleOnStart}
      data-testid="garmin-sync-on-start"
    />
    <span>{$t('ui.garmin.sync_on_start')}</span>
  </label>
  <p class="muted hint">{$t('ui.garmin.sync_on_start_hint')}</p>

  <div class="sensitive">
    <p class="label">{$t('ui.garmin.s3.title')}</p>
    <SensitiveShares {settings} {onchange} deletable />
    <p class="muted hint" data-testid="garmin-never">{$t('ui.garmin.never_fetched')}</p>
  </div>

  {#if errorMessage}
    <p class="error" role="alert" data-testid="garmin-error">{$t(errorMessage)}</p>
  {/if}

  <details class="advanced" ontoggle={onAdvancedToggle} data-testid="garmin-advanced">
    <summary>{$t('ui.garmin.advanced')}</summary>
    <p class="label">{$t('ui.garmin.bridge_title')}</p>
    <p class="muted hint">{$t('ui.garmin.bridge_hint')}</p>
    <p class="hint" data-testid="bridge-status">
      {#if bridge === null}
        <span class="muted">{$t('common.loading')}</span>
      {:else if bridge.has_data}
        {$t('ui.garmin.bridge_has_data', {
          days: bridge.recovery_days,
          files: bridge.activity_files,
        })}
      {:else}
        <span class="muted">{$t('ui.garmin.bridge_no_data')}</span>
      {/if}
    </p>
    <p class="row">
      <button onclick={() => void loadBridge()} data-testid="bridge-refresh">
        {$t('ui.garmin.bridge_refresh')}
      </button>
    </p>
  </details>
</section>

<style>
  .hint {
    margin: 4px 0 8px;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }

  .sensitive {
    margin-top: 16px;
  }

  .advanced {
    margin-top: 16px;
  }

  .advanced summary {
    cursor: pointer;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  .label {
    margin: 12px 0 4px;
    font-size: var(--fs-small);
    font-weight: var(--fw-semibold);
    color: var(--muted);
  }
</style>
