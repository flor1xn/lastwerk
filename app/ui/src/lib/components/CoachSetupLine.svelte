<script lang="ts">
  import { coachSetupOp, percentOf } from '../coach-setup';
  import { t } from '../i18n';

  // Kleine Zeile in der Seitenleiste, solange ein Download oder die Einrichtung von Ollama läuft
  // (T29): Der Nutzer darf den Erststart beenden und die App benutzen, der Download läuft weiter.
  // Ohne Auftrag bleibt die Zeile leer.
  const op = $derived($coachSetupOp);
  const percent = $derived(
    op.kind === 'pull' && op.progress ? percentOf(op.progress.completed, op.progress.total) : null,
  );
</script>

{#if op.kind === 'pull'}
  <p class="line" role="status" aria-live="off" data-testid="sidebar-coach-setup">
    {$t('ui.coach_setup.pulling', { name: op.name })}{percent === null ? ' …' : ` · ${percent} %`}
  </p>
{:else if op.kind === 'install'}
  <p class="line" role="status" aria-live="off" data-testid="sidebar-coach-setup">
    {$t('ui.coach_setup.installing')}
  </p>
{/if}

<style>
  .line {
    margin: 0;
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
    overflow-wrap: anywhere;
  }
</style>
