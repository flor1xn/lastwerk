<script lang="ts">
  import { t } from '../i18n';
  import { fitMigration } from '../state';

  // Einmalige Meldung nach der Übernahme des früher gewählten FIT-Ordners in den festen Stamm
  // (T28). Nur Zahlen, nie ein Pfad. Bleibt stehen, bis der Nutzer sie ausblendet oder die App
  // neu startet (dann meldet der Rust-Teil nichts mehr). Keine Bewegung: Der Hinweis erscheint
  // einmal beim Laden und braucht keine Aufmerksamkeit durch Animation.
</script>

{#if $fitMigration}
  {@const m = $fitMigration}
  <aside class="notice" role="status" data-testid="migration-notice">
    <div class="text">
      <p class="title">{$t(m.missing ? 'ui.migration.title_missing' : 'ui.migration.title')}</p>
      {#if m.missing}
        <p>{$t('ui.migration.missing')}</p>
      {:else}
        <p>{$t('ui.migration.done', { copied: m.copied, identical: m.identical })}</p>
        {#if m.conflicts > 0}
          <p class="muted">{$t('ui.migration.conflicts', { n: m.conflicts })}</p>
        {/if}
        {#if m.failed > 0}
          <p class="warn">{$t('ui.migration.failed', { n: m.failed })}</p>
        {/if}
      {/if}
    </div>
    <button onclick={() => fitMigration.set(null)} data-testid="migration-dismiss">
      {$t('ui.migration.dismiss')}
    </button>
  </aside>
{/if}

<style>
  .notice {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
    margin: 0 0 var(--space-5);
    padding: var(--space-3) var(--space-4);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }

  .text {
    min-width: 0;
  }

  p {
    margin: 0 0 var(--space-1);
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
  }

  .title {
    font-size: var(--fs-body);
    font-weight: var(--fw-semibold);
  }

  button {
    flex: none;
  }

  @media (max-width: 560px) {
    .notice {
      flex-direction: column;
    }
  }
</style>
