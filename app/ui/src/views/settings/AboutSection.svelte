<script lang="ts">
  import type { Settings } from '../../lib/backend/types';
  import { changesFor } from '../../lib/changelog';
  import { locale, t } from '../../lib/i18n';
  import { view, wizardOpen } from '../../lib/state';

  let { settings }: { settings: Settings } = $props();

  const changes = $derived(changesFor(settings.version, $locale));

  function repeat() {
    wizardOpen.set(true);
    view.set('today');
  }
</script>

<section class="section" data-testid="about-section">
  <h3>{$t('ui.about.title')}</h3>
  <p class="num" data-testid="about-version">
    {$t('ui.about.version', { version: settings.version })}
  </p>
  <h4>{$t('ui.about.whats_new')}</h4>
  {#if changes.length > 0}
    <ul data-testid="about-changes">
      {#each changes as line (line)}
        <li>{line}</li>
      {/each}
    </ul>
  {:else}
    <p class="muted" data-testid="about-none">{$t('ui.about.none')}</p>
  {/if}
  <p class="muted hint">{$t('ui.about.repeat_hint')}</p>
  <p>
    <button onclick={repeat} data-testid="about-repeat">{$t('ui.about.repeat')}</button>
  </p>
</section>

<style>
  h4 {
    margin: 14px 0 6px;
    font-size: var(--fs-small);
    font-weight: var(--fw-semibold);
    color: var(--muted);
  }

  .num {
    margin: 0;
    text-align: left;
  }

  ul {
    margin: 0 0 8px;
    padding-left: 18px;
  }

  li {
    margin: 4px 0;
  }

  .hint {
    margin: 8px 0;
  }
</style>
