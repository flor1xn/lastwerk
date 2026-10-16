<script lang="ts">
  // Verlauf (T42, E8): alles, was schon war, unter einem Navigationspunkt. Die Reiter ersetzen
  // die früheren eigenen Ansichten. Nur der aktive Reiter wird gezeichnet, damit nicht fünf
  // Ansichten gleichzeitig laden. Der Leitsatz des Reiters steht in der eingebetteten Ansicht,
  // weil er dort aus den Daten entsteht (Kalender, Fortschritt, Erholung).
  import Segmented from '../lib/components/Segmented.svelte';
  import Tabs from '../lib/components/Tabs.svelte';
  import { t } from '../lib/i18n';
  import {
    BODY_TABS,
    bodyTab,
    CALENDAR_MODES,
    calendarMode,
    HISTORY_TABS,
    historyTab,
    selectedActivityId,
  } from '../lib/state';
  import ActivitiesView from './ActivitiesView.svelte';
  import ActivityDetailView from './ActivityDetailView.svelte';
  import CalendarView from './CalendarView.svelte';
  import HealthView from './HealthView.svelte';
  import PerformanceView from './PerformanceView.svelte';
  import ProgressView from './ProgressView.svelte';
  import RecoveryView from './RecoveryView.svelte';
  import WeekView from './WeekView.svelte';

  const tabs = $derived(HISTORY_TABS.map((v) => ({ value: v, label: $t(`ui.history.tab.${v}`) })));
  const modes = $derived(
    CALENDAR_MODES.map((v) => ({ value: v, label: $t(`ui.history.mode.${v}`) })),
  );
  const bodies = $derived(BODY_TABS.map((v) => ({ value: v, label: $t(`ui.history.body.${v}`) })));
</script>

<div class="page history" data-testid="history-view">
  <header class="page-head">
    <h1>{$t('ui.nav.history')}</h1>
  </header>

  <Tabs
    group="history"
    options={tabs}
    value={$historyTab}
    label={$t('ui.history.tabs_label')}
    onchange={(v) => historyTab.set(v)}
  >
    {#if $historyTab === 'calendar'}
      <div class="sub">
        <Segmented
          options={modes}
          value={$calendarMode}
          label={$t('ui.history.calendar_mode')}
          onchange={(m) => calendarMode.set(m)}
          testid="calendar-mode"
        />
      </div>
      {#if $calendarMode === 'month'}
        <CalendarView />
      {:else}
        <WeekView />
      {/if}
    {:else if $historyTab === 'activities'}
      {#if $selectedActivityId === null}
        <ActivitiesView />
      {:else}
        <ActivityDetailView />
      {/if}
    {:else if $historyTab === 'progress'}
      <ProgressView />
    {:else if $historyTab === 'performance'}
      <PerformanceView />
    {:else}
      <div class="sub">
        <Segmented
          options={bodies}
          value={$bodyTab}
          label={$t('ui.history.body_label')}
          onchange={(b) => bodyTab.set(b)}
          testid="body-tab"
        />
      </div>
      {#if $bodyTab === 'recovery'}
        <RecoveryView />
      {:else}
        <HealthView />
      {/if}
    {/if}
  </Tabs>
</div>

<style>
  /* Die Aktivitätenliste braucht die Breite, die anderen Reiter setzen ihr eigenes Maß. */
  .history {
    max-width: none;
  }

  .page-head {
    margin-bottom: var(--space-4);
  }

  .sub {
    margin: 0 0 var(--space-4);
  }
</style>
