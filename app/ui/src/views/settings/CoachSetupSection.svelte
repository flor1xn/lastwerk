<script lang="ts">
  import { onMount } from 'svelte';
  import { coachSetupFocus } from '../../lib/coach-setup';
  import { t } from '../../lib/i18n';
  import CoachSetup from './CoachSetup.svelte';

  // Abschnitt "Coach" der Einstellungen (T29): derselbe Baustein wie im Erststart. Kommt der Nutzer
  // vom Hinweis in der Planansicht, springt die Ansicht hierher.
  let root = $state<HTMLElement | null>(null);

  onMount(() => {
    if ($coachSetupFocus) {
      coachSetupFocus.set(false);
      root?.scrollIntoView?.({ block: 'start' });
    }
  });
</script>

<section class="section" id="coach-setup" bind:this={root} data-testid="coach-setup-section">
  <h3>{$t('ui.coach_setup.title')}</h3>
  <p class="muted hint">{$t('ui.coach_setup.statement')}</p>
  <CoachSetup />
</section>

<style>
  .hint {
    margin: 0 0 var(--space-3);
  }
</style>
