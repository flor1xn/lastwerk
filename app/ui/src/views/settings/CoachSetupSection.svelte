<script lang="ts">
  import { t } from '../../lib/i18n';
  import CoachSetup from './CoachSetup.svelte';

  // Reiter "Coach" der Einstellungen (T29, seit T42 ein eigener Reiter): derselbe Baustein wie im
  // Erststart. Der Hinweis in der Planansicht öffnet diesen Reiter direkt.
</script>

<section class="section" id="coach-setup" data-testid="coach-setup-section">
  <h3>{$t('ui.coach_setup.title')}</h3>
  <p class="muted hint">{$t('ui.coach_setup.statement')}</p>
  <CoachSetup />
</section>

<style>
  .hint {
    margin: 0 0 var(--space-3);
  }
</style>
