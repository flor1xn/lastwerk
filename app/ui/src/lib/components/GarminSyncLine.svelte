<script lang="ts">
  import { garminSync, progressText } from '../garmin';
  import { t } from '../i18n';

  // Kleine Zeile zum Abgleich mit Garmin: läuft (mit Fortschritt), fertig (neue Aktivitäten)
  // oder Fehler als Text aus der Sprachdatei. Ohne Abgleich in dieser Sitzung bleibt sie leer.
  // Keine Bewegung: Der Text ändert sich während eines Abgleichs oft, eine Animation lenkte ab.

  interface Props {
    testid?: string;
    /** Mit Zahl der Erholungstage, geänderter Schwellen und neu berechneter Aktivitäten
     * (Einstellungen, Erststart). */
    detail?: boolean;
    /** Schmale Seitenleiste: bei einem Fehler nur "unvollständig", der volle Text als Tooltip
     * (manche Fehlertexte sind zwei Sätze lang). */
    short?: boolean;
  }

  let { testid = 'garmin-sync-line', detail = false, short = false }: Props = $props();
</script>

{#if $garminSync.kind !== 'idle'}
  <p class="sync-line" role="status" aria-live="polite" data-testid={testid}>
    {#if $garminSync.kind === 'running'}
      <span class="dot running" aria-hidden="true"></span>
      <span>
        {$t('ui.garmin.sync.running')}{#if $garminSync.progress}<span class="muted">
            · {$t(...progressText($garminSync.progress))}</span
          >{/if}
      </span>
    {:else if $garminSync.kind === 'done'}
      <span class="dot ok" aria-hidden="true"></span>
      <span>
        {detail
          ? $t('ui.garmin.sync.done_detail', {
              count: $garminSync.summary.new_activities,
              days: $garminSync.summary.recovery_days,
            })
          : $t('ui.garmin.sync.done', {
              count: $garminSync.summary.new_activities,
            })}{#if detail && $garminSync.summary.thresholds_changed > 0}<span
            data-testid={`${testid}-thresholds`}
            >, {$t('ui.garmin.sync.thresholds', {
              n: $garminSync.summary.thresholds_changed,
            })}</span
          >{/if}{#if detail && $garminSync.summary.recomputed > 0}<span
            data-testid={`${testid}-recomputed`}
            >, {$t('ui.garmin.sync.recomputed', { n: $garminSync.summary.recomputed })}</span
          >{/if}
      </span>
    {:else}
      <span class="dot warn" aria-hidden="true"></span>
      {#if short}
        <span class="warn" title={$t($garminSync.key)}>{$t('ui.garmin.sync.aborted')}</span>
      {:else}
        <span class="warn">{$t($garminSync.key)}</span>
      {/if}
    {/if}
  </p>
{/if}

<style>
  .sync-line {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
    overflow-wrap: anywhere;
  }

  .dot {
    flex-shrink: 0;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--muted);
    transform: translateY(-1px);
  }

  .dot.running {
    background: var(--accent);
  }

  .dot.ok {
    background: var(--ok);
  }

  .dot.warn {
    background: var(--warn);
  }
</style>
