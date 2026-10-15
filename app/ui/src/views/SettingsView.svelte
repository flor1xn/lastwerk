<script lang="ts">
  import { getBackend } from '../lib/backend';
  import type { Settings } from '../lib/backend/types';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import { t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import AboutSection from './settings/AboutSection.svelte';
  import AppearanceSection from './settings/AppearanceSection.svelte';
  import AthleteSection from './settings/AthleteSection.svelte';
  import CoachSetupSection from './settings/CoachSetupSection.svelte';
  import DataSection from './settings/DataSection.svelte';
  import GarminSection from './settings/GarminSection.svelte';
  import McpSection from './settings/McpSection.svelte';
  import PmcSection from './settings/PmcSection.svelte';
  import ThresholdSection from './settings/ThresholdSection.svelte';
  import UpdateSection from './settings/UpdateSection.svelte';

  const loader = new Loader<Settings>();

  function reload() {
    void loader.run(() => getBackend().getSettings());
  }

  reload();
</script>

<div class="page settings">
  <header class="page-head">
    <h1>{$t('ui.nav.settings')}</h1>
    <p class="page-lead" data-testid="lead">{$t('ui.lead.settings')}</p>
  </header>

  {#if loader.error}
    <ErrorNote error={loader.error} />
  {:else if loader.data}
    <DataSection settings={loader.data} />
    <GarminSection settings={loader.data} onchange={reload} />
    <CoachSetupSection />
    <ThresholdSection />
    <AthleteSection settings={loader.data} onchange={reload} />
    <PmcSection settings={loader.data} onchange={reload} />
    <McpSection settings={loader.data} onchange={reload} />
    <AppearanceSection />
    <UpdateSection />
    <AboutSection settings={loader.data} />
  {:else}
    <p class="muted">{$t('common.loading')}</p>
  {/if}
</div>

<style>
  /* Eine Spalte: Formulare lesen sich schmal besser, und die Abschnitte stehen mit ihren Linien
     untereinander statt als Kachelraster. */
  .settings {
    max-width: 760px;
  }
</style>
