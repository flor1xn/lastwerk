<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import type {
    AddFitFilesResult,
    ImportProgress,
    ImportSummary,
    Settings,
  } from '../../lib/backend/types';
  import Section from '../../lib/components/Section.svelte';
  import { errorKey } from '../../lib/errors';
  import { formatNumber } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { bumpData } from '../../lib/state';

  // Abschnitt "Daten" der Einstellungen (T28, SPEC 5.4, 6.1). Der Nutzer wählt keinen Ordner:
  // Lastwerk legt alle FIT-Dateien im eigenen Datenordner ab, neben der Datenbank. Der Pfad steht
  // nur zur Anzeige da. Zwei Handlungen: "Neu einlesen" liest den Stamm noch einmal ein,
  // "FIT-Dateien hinzufügen" kopiert eigene Dateien (Edge, USB-Stick) hinein. Den Dateidialog
  // öffnet der Rust-Teil, die Oberfläche bekommt nur Zahlen.

  interface Props {
    settings: Settings;
  }

  let { settings }: Props = $props();

  let busy = $state(false);
  let progress = $state<ImportProgress | null>(null);
  let summary = $state<ImportSummary | null>(null);
  let added = $state<AddFitFilesResult | null>(null);
  let errorMessage = $state<string | null>(null);

  async function run(action: () => Promise<void>) {
    errorMessage = null;
    summary = null;
    added = null;
    progress = null;
    busy = true;
    try {
      await action();
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      busy = false;
    }
  }

  const rescan = () =>
    run(async () => {
      summary = await getBackend().importFolder((p) => (progress = p));
      bumpData();
    });

  const addFiles = () =>
    run(async () => {
      const result = await getBackend().addFitFiles((p) => (progress = p));
      // Abbruch im Dateidialog ist keine Meldung wert.
      if (result.cancelled) return;
      added = result;
      summary = result.import;
      if (result.import) bumpData();
    });

  const KNOWN_CODES = [
    'empty_file',
    'too_large',
    'invalid_header',
    'crc_mismatch',
    'truncated',
    'unsupported_version',
    'no_activity',
    'malformed',
    'io',
  ];

  const codeText = (code: string) =>
    KNOWN_CODES.includes(code) ? $t(`ui.import.error_code.${code}`) : code;
</script>

<Section
  label={$t('ui.settings.data')}
  statement={$t('ui.settings.statement')}
  testid="data-section"
>
  <p class="location">
    <span class="muted">{$t('ui.settings.location')}</span>
    <code data-testid="fit-root">{settings.fit_root}</code>
  </p>
  <p class="muted hint">{$t('ui.settings.location_hint')}</p>

  <div class="actions">
    <div class="action">
      <button onclick={() => void rescan()} disabled={busy} data-testid="rescan">
        {$t('ui.settings.rescan')}
      </button>
      <p class="muted hint">{$t('ui.settings.rescan_hint')}</p>
    </div>
    <div class="action">
      <button onclick={() => void addFiles()} disabled={busy} data-testid="add-fit-files">
        {$t('ui.settings.add_files')}
      </button>
      <p class="muted hint">{$t('ui.settings.add_files_hint')}</p>
    </div>
  </div>

  {#if busy && progress}
    <p class="row">
      <progress max={progress.total} value={progress.done} data-testid="import-progress"></progress>
      <span class="muted"
        >{$t('ui.import.progress', { done: progress.done, total: progress.total })}</span
      >
    </p>
  {/if}
  {#if errorMessage}
    <p class="error" role="alert" data-testid="import-error">{$t(errorMessage)}</p>
  {/if}
  {#if added}
    <div data-testid="add-result">
      <p>
        {$t('ui.settings.add_result', {
          selected: added.selected,
          copied: added.copied,
          present: added.already_present,
          rejected: added.rejected,
        })}
      </p>
      {#if added.rejected > 0}
        <p class="muted">{$t('ui.settings.add_rejected')}</p>
      {/if}
    </div>
  {/if}
  {#if summary}
    <div data-testid="import-summary">
      <p>
        {$t('ui.import.summary', {
          found: summary.files_found,
          fresh: summary.new,
          updated: summary.updated,
          unchanged: summary.unchanged,
        })}
      </p>
      <p>
        {$t('ui.import.activities', { primary: summary.primary, duplicates: summary.duplicates })}
      </p>
      <p>
        {$t('ui.import.loads', {
          ok: summary.loads_ok,
          threshold: summary.loads_missing_threshold,
          channel: summary.loads_missing_channel,
          implausible: summary.loads_implausible,
        })}
      </p>
      {#if summary.not_activity > 0}
        <p class="muted">{$t('ui.import.not_activity', { n: summary.not_activity })}</p>
      {/if}
      {#if summary.error_count > 0}
        <p class="warn">
          {$t('ui.import.errors', { n: formatNumber(summary.error_count, $locale, 0) })}
        </p>
        <ul>
          {#each summary.errors_by_code as e (e.code)}
            <li>{codeText(e.code)}: {e.count}</li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
  {#if settings.db_in_sync_folder}
    <p class="warn" role="alert" data-testid="sync-warning">{$t('ui.settings.sync_warning')}</p>
  {/if}
</Section>

<style>
  .location {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px;
    margin: 8px 0 4px;
  }

  code {
    padding: 1px 5px;
    background: var(--panel-2);
    border-radius: 3px;
    word-break: break-all;
  }

  .hint {
    margin: 4px 0 8px;
  }

  /* Zwei Handlungen nebeneinander, auf schmaler Breite untereinander. */
  .actions {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 8px 24px;
    margin: 12px 0;
  }

  .action {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 6px 0;
  }

  progress {
    width: 200px;
  }

  ul {
    margin: 0;
    padding-left: 18px;
  }
</style>
