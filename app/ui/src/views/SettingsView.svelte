<script lang="ts">
  import { getBackend } from '../lib/backend';
  import type { Settings } from '../lib/backend/types';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import Tabs from '../lib/components/Tabs.svelte';
  import { t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { SETTINGS_TABS, settingsTab } from '../lib/state';
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

  // Vier Reiter statt einer Spalte mit zehn Abschnitten (T42): Daten und Garmin, Coach, Profil und
  // Schwellen, App. Sprache und Thema stehen nur noch hier (Reiter App), nicht in der Seitenleiste.
  const loader = new Loader<Settings>();

  function reload() {
    void loader.run(() => getBackend().getSettings());
  }

  reload();

  const tabs = $derived(
    SETTINGS_TABS.map((v) => ({ value: v, label: $t(`ui.settings.tab.${v}`) })),
  );
</script>

<div class="page settings">
  <header class="page-head">
    <h1>{$t('ui.nav.settings')}</h1>
  </header>

  <Tabs
    group="settings"
    options={tabs}
    value={$settingsTab}
    label={$t('ui.settings.tabs_label')}
    onchange={(v) => settingsTab.set(v)}
  >
    <p class="page-lead" data-testid="lead">{$t(`ui.lead.settings_${$settingsTab}`)}</p>
    {#if loader.error}
      <ErrorNote error={loader.error} />
    {:else if loader.data}
      {#if $settingsTab === 'data'}
        <DataSection settings={loader.data} />
        <GarminSection settings={loader.data} onchange={reload} />
        <McpSection settings={loader.data} onchange={reload} />
      {:else if $settingsTab === 'coach'}
        <CoachSetupSection />
      {:else if $settingsTab === 'profile'}
        <AthleteSection settings={loader.data} onchange={reload} />
        <ThresholdSection />
        <PmcSection settings={loader.data} onchange={reload} />
      {:else}
        <AppearanceSection />
        <UpdateSection />
        <AboutSection settings={loader.data} />
      {/if}
    {:else}
      <p class="muted">{$t('common.loading')}</p>
    {/if}
  </Tabs>
</div>

<style>
  /* Eine Spalte: Formulare lesen sich schmal besser, und die Abschnitte stehen mit ihren Linien
     untereinander statt als Kachelraster. */
  .settings {
    max-width: 760px;
  }

  .page-head {
    margin-bottom: var(--space-4);
  }
</style>
