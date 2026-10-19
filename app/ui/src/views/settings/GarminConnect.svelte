<script lang="ts">
  import GarminSyncLine from '../../lib/components/GarminSyncLine.svelte';
  import {
    connectGarmin,
    garminLogin,
    garminStatus,
    garminStatusError,
    refreshGarminStatus,
  } from '../../lib/garmin';
  import { t } from '../../lib/i18n';

  // Verbinden mit Garmin, gemeinsam für Erststart und Einstellungen: Zustand als Pille, Knopf
  // für das Anmeldefenster, Ergebnis der Anmeldung und Zeile des Abgleichs. Lastwerk zeigt kein
  // Feld für Zugangsdaten (SPEC 10.2): Passwort und Code gibt der Nutzer nur bei Garmin ein.

  $effect(() => {
    void refreshGarminStatus();
  });

  const connected = $derived($garminStatus?.connected ?? false);
  const waiting = $derived($garminLogin.kind === 'waiting');
</script>

<div class="connect" data-testid="garmin-connect">
  <p class="state">
    {#if $garminStatus}
      <span
        class="state-mark"
        class:ok={connected}
        class:off={!connected}
        data-testid="garmin-state"
      >
        {connected ? $t('ui.garmin.connected') : $t('ui.garmin.not_connected')}
      </span>
    {/if}
    {#if !connected}
      <button
        class="primary"
        onclick={() => void connectGarmin()}
        disabled={waiting}
        data-testid="garmin-connect-button">{$t('ui.garmin.connect')}</button
      >
    {/if}
  </p>
  {#if waiting}
    <p class="muted hint" data-testid="garmin-waiting">{$t('ui.garmin.waiting')}</p>
  {:else if $garminLogin.kind === 'done'}
    <p
      class={$garminLogin.ok ? 'hint' : 'warn hint'}
      role="status"
      data-testid="garmin-login-result"
    >
      {$t($garminLogin.key)}
    </p>
  {/if}
  {#if $garminStatusError}
    <p class="error hint" role="alert" data-testid="garmin-status-error">
      {$t($garminStatusError)}
    </p>
  {/if}
  <GarminSyncLine testid="garmin-sync-state" detail />
</div>

<style>
  .connect {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .state {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    margin: 0;
  }

  .hint {
    margin: 0;
  }
</style>
