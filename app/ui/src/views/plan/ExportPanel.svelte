<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import { errorKey } from '../../lib/errors';
  import { t } from '../../lib/i18n';
  import { formatCounts } from '../../lib/plan';

  interface Props {
    /** Der angezeigte Plan, `null` ohne Auswahl. */
    planId: string | null;
    /** Nach einer Wiederherstellung: Daten, Sprache und Thema neu laden. */
    onrestored: () => void;
  }

  let { planId, onrestored }: Props = $props();

  let message = $state<string | null>(null);
  let errorMessage = $state<string | null>(null);
  let confirming = $state(false);
  let restoredHint = $state(false);
  let busy = $state(false);

  async function run(action: () => Promise<string>) {
    message = null;
    errorMessage = null;
    restoredHint = false;
    busy = true;
    try {
      message = await action();
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      busy = false;
    }
  }

  const exportIcs = () =>
    run(async () => {
      const r = await getBackend().exportPlanIcs(planId as string);
      return r.saved ? $t('ui.export.ics_done', { n: r.events ?? 0 }) : $t('ui.export.cancelled');
    });

  const exportJson = () =>
    run(async () => {
      const r = await getBackend().exportPlanJson(planId as string);
      return r.saved ? $t('ui.export.json_done') : $t('ui.export.cancelled');
    });

  const exportBackup = () =>
    run(async () => {
      const r = await getBackend().exportBackup();
      return r.saved && r.counts
        ? $t('ui.export.backup_done', { counts: formatCounts($t, r.counts) })
        : $t('ui.export.cancelled');
    });

  async function restore() {
    confirming = false;
    await run(async () => {
      const r = await getBackend().restoreBackup(true);
      if (!r.restored || !r.counts) return $t('ui.export.cancelled');
      restoredHint = true;
      onrestored();
      return $t('ui.export.restored', { counts: formatCounts($t, r.counts) });
    });
  }
</script>

<section class="panel" data-testid="export-section">
  <h3>{$t('ui.export.title')}</h3>
  <p class="row">
    <button onclick={exportIcs} disabled={busy || planId === null} data-testid="export-ics"
      >{$t('ui.export.ics')}</button
    >
    <button onclick={exportJson} disabled={busy || planId === null} data-testid="export-json"
      >{$t('ui.export.plan_json')}</button
    >
    <button onclick={exportBackup} disabled={busy} data-testid="export-backup"
      >{$t('ui.export.backup')}</button
    >
    <button
      onclick={() => (confirming = true)}
      disabled={busy || confirming}
      data-testid="export-restore">{$t('ui.export.restore')}</button
    >
  </p>
  {#if planId === null}
    <p class="muted hint">{$t('ui.export.need_plan')}</p>
  {/if}
  <p class="muted hint">{$t('ui.export.ics_hint')} {$t('ui.export.privacy')}</p>
  {#if confirming}
    <div
      class="confirm"
      role="alertdialog"
      aria-label={$t('ui.export.restore')}
      data-testid="restore-confirm"
    >
      <p class="warn">{$t('ui.export.restore_confirm')}</p>
      <p class="row">
        <button class="primary" onclick={restore} data-testid="restore-yes"
          >{$t('ui.export.restore_yes')}</button
        >
        <button onclick={() => (confirming = false)} data-testid="restore-no"
          >{$t('ui.export.restore_no')}</button
        >
      </p>
    </div>
  {/if}
  {#if message}
    <p class="ok" role="status" data-testid="export-result">{message}</p>
  {/if}
  {#if restoredHint}
    <p class="warn" data-testid="restore-hint">{$t('ui.export.restored_hint')}</p>
  {/if}
  {#if errorMessage}
    <p class="error" role="alert" data-testid="export-error">{$t(errorMessage)}</p>
  {/if}
</section>

<style>
  p {
    margin: 4px 0;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .hint {
    font-size: var(--fs-small);
  }

  .confirm {
    margin: 6px 0;
    padding: 6px 10px;
    border: 1px solid var(--warn);
    border-radius: var(--radius-chip);
  }

  .ok {
    color: var(--ok);
  }
</style>
