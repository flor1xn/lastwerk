<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import {
    SENSITIVE_CATEGORIES,
    shareKey,
    type SensitiveCategory,
    type Settings,
  } from '../../lib/backend/types';
  import { errorKey } from '../../lib/errors';
  import { t } from '../../lib/i18n';
  import { bumpData } from '../../lib/state';

  // Freigabe sensibler Werte (S3) je Bereich (T40, Entscheidung E7): Körper und Gewicht,
  // Blutdruck, Ernährung, Hydration. Standard aus, ohne Freigabe kein Abruf (SPEC 5.5). In den
  // Einstellungen mit Löschen je Bereich, in der Einrichtung nur die Schalter.

  interface Props {
    settings: Settings;
    onchange: () => void;
    /** Löschen je Bereich anbieten (Einstellungen). */
    deletable?: boolean;
  }

  let { settings, onchange, deletable = false }: Props = $props();

  let busy = $state(false);
  let errorMessage = $state<string | null>(null);
  let confirming = $state<SensitiveCategory | null>(null);
  let deleted = $state<{ category: SensitiveCategory; n: number } | null>(null);

  async function toggle(c: SensitiveCategory, event: Event & { currentTarget: HTMLInputElement }) {
    const on = event.currentTarget.checked;
    const input = event.currentTarget;
    errorMessage = null;
    busy = true;
    try {
      await getBackend().setSettings({ [shareKey(c)]: on });
      onchange();
    } catch (e) {
      input.checked = !on;
      errorMessage = errorKey(e);
    } finally {
      busy = false;
    }
  }

  async function remove(c: SensitiveCategory) {
    errorMessage = null;
    busy = true;
    try {
      deleted = { category: c, n: await getBackend().deleteSensitiveData(c) };
      confirming = null;
      bumpData();
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      busy = false;
    }
  }
</script>

<div data-testid="garmin-sensitive">
  <ul class="shares">
    {#each SENSITIVE_CATEGORIES as c (c)}
      <li>
        <div class="line">
          <label class="switch">
            <input
              type="checkbox"
              checked={settings[shareKey(c)]}
              disabled={busy}
              onchange={(e) => void toggle(c, e)}
              data-testid={`garmin-share-${c}`}
            />
            <span>{$t(`ui.garmin.s3.toggle.${c}`)}</span>
          </label>
          {#if deletable}
            <button
              class="ghost"
              onclick={() => {
                deleted = null;
                confirming = c;
              }}
              disabled={busy || confirming !== null}
              data-testid={`garmin-delete-${c}`}>{$t('ui.garmin.s3.delete')}</button
            >
          {/if}
        </div>
        {#if confirming === c}
          <div
            class="confirm"
            role="alertdialog"
            aria-label={$t('ui.garmin.s3.delete')}
            data-testid="garmin-delete-confirm"
          >
            <p class="warn">
              {$t('ui.garmin.s3.delete_confirm', { name: $t(`ui.health.s3.${c}`) })}
            </p>
            <p class="row">
              <button
                class="primary"
                onclick={() => void remove(c)}
                disabled={busy}
                data-testid="garmin-delete-yes">{$t('ui.garmin.s3.delete_yes')}</button
              >
              <button
                onclick={() => (confirming = null)}
                disabled={busy}
                data-testid="garmin-delete-no">{$t('ui.garmin.s3.delete_no')}</button
              >
            </p>
          </div>
        {/if}
        {#if deleted?.category === c}
          <p class="hint" role="status" data-testid="garmin-deleted">
            {$t('ui.garmin.s3.deleted', { n: deleted.n })}
          </p>
        {/if}
      </li>
    {/each}
  </ul>
  <p class="muted hint">{$t('ui.garmin.s3.hint')}</p>
  {#if errorMessage}
    <p class="error" role="alert" data-testid="garmin-sensitive-error">{$t(errorMessage)}</p>
  {/if}
</div>

<style>
  .shares {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 8px;
  }

  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .hint {
    margin: 4px 0 8px;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
