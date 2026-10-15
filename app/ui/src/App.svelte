<script lang="ts">
  import { onMount } from 'svelte';
  import { getBackend } from './lib/backend';
  import ErrorNote from './lib/components/ErrorNote.svelte';
  import CoachSetupLine from './lib/components/CoachSetupLine.svelte';
  import GarminSyncLine from './lib/components/GarminSyncLine.svelte';
  import MigrationNotice from './lib/components/MigrationNotice.svelte';
  import { syncOnStart } from './lib/garmin';
  import { setLanguage, setThemeChoice } from './lib/appearance';
  import type { Locale, Theme } from './lib/backend/types';
  import { locale, LOCALES, setLocale, t } from './lib/i18n';
  import {
    applyTheme,
    bumpData,
    fitMigration,
    theme,
    VIEWS,
    view,
    watchSystemTheme,
    wizardOpen,
    type View,
  } from './lib/state';
  import { windowTitle } from './lib/title';
  import ActivitiesView from './views/ActivitiesView.svelte';
  import ActivityDetailView from './views/ActivityDetailView.svelte';
  import CalendarView from './views/CalendarView.svelte';
  import PerformanceView from './views/PerformanceView.svelte';
  import PlanView from './views/PlanView.svelte';
  import ProgressView from './views/ProgressView.svelte';
  import RecoveryView from './views/RecoveryView.svelte';
  import SettingsView from './views/SettingsView.svelte';
  import Wizard from './views/onboarding/Wizard.svelte';
  import TodayView from './views/TodayView.svelte';
  import WeekView from './views/WeekView.svelte';

  let ready = $state(false);
  // Aus den Einstellungen (Datenbank), nicht aus dem Browser: Der Assistent erscheint nur beim
  // ersten Start oder auf Wunsch.
  let onboarded = $state(true);
  let startError = $state<unknown>(null);

  onMount(() => {
    const stopWatching = watchSystemTheme();
    getBackend()
      .getSettings()
      .then((s) => {
        setLocale(s.locale);
        document.documentElement.lang = s.locale;
        applyTheme(s.theme);
        onboarded = s.onboarded;
        fitMigration.set(s.fit_migration);
        ready = true;
        // Stammen die abgeleiteten Werte von einem älteren Programmstand (T33), liest die App
        // einmal ein, damit neue Kennzahlen auch für vorhandene Dateien entstehen. Erst danach der
        // Abgleich mit Garmin (T23), damit nicht zwei Importe gleichzeitig schreiben.
        void refreshCache(s.cache_outdated).finally(() => void syncOnStart());
      })
      .catch((e: unknown) => {
        startError = e;
      });
    return stopWatching;
  });

  async function refreshCache(outdated: boolean): Promise<void> {
    if (!outdated) return;
    try {
      await getBackend().importFolder(() => {});
      bumpData();
    } catch {
      // Kein Abbruch des Starts: "Neu einlesen" in den Einstellungen bleibt der Weg.
    }
  }

  $effect(() => {
    document.title = windowTitle($t(`ui.nav.${$view}`));
  });

  const themeOrder: Theme[] = ['dark', 'light', 'system'];

  function cycleTheme() {
    const next = themeOrder[(themeOrder.indexOf($theme) + 1) % themeOrder.length];
    void setThemeChoice(next);
  }

  const select = (v: View) => view.set(v);

  function finishSetup() {
    onboarded = true;
    wizardOpen.set(false);
    view.set('today');
    bumpData();
  }
</script>

{#if startError}
  <main class="start"><ErrorNote error={startError} /></main>
{:else if !ready}
  <main class="start"><p class="muted">{$t('common.loading')}</p></main>
{:else if !onboarded || $wizardOpen}
  <Wizard onfinish={finishSetup} />
{:else}
  <div class="shell">
    <aside>
      <div class="brand">
        <svg width="24" height="24" viewBox="0 0 26 26" aria-hidden="true">
          <rect x="2" y="14" width="5" height="10" rx="1.5" fill="var(--muted)" opacity="0.6" />
          <rect x="10.5" y="8" width="5" height="16" rx="1.5" fill="var(--muted)" />
          <rect x="19" y="2" width="5" height="22" rx="1.5" fill="var(--accent)" />
        </svg>
        <span>{$t('app.name')}</span>
      </div>
      <nav aria-label={$t('ui.nav.label')}>
        {#each VIEWS as v (v)}
          <button
            class:active={$view === v}
            aria-current={$view === v ? 'page' : undefined}
            data-testid={`nav-${v}`}
            onclick={() => select(v)}>{$t(`ui.nav.${v}`)}</button
          >
        {/each}
      </nav>
      <div class="spacer"></div>
      <div class="sync-status">
        <GarminSyncLine testid="sidebar-sync" short /><CoachSetupLine />
      </div>
      <div class="prefs">
        <label class="lang">
          <span class="visually-hidden">{$t('ui.appearance.language')}</span>
          <select
            value={$locale}
            onchange={(e) => void setLanguage(e.currentTarget.value as Locale)}
            data-testid="header-language"
          >
            {#each LOCALES as l (l)}
              <option value={l}>{l.toUpperCase()}</option>
            {/each}
          </select>
        </label>
        <button onclick={cycleTheme} title={$t('ui.appearance.theme')} data-testid="header-theme">
          {$t(`ui.theme.${$theme}`)}
        </button>
      </div>
    </aside>

    <main>
      <MigrationNotice />
      {#if $view === 'today'}
        <TodayView />
      {:else if $view === 'calendar'}
        <CalendarView />
      {:else if $view === 'week'}
        <WeekView />
      {:else if $view === 'progress'}
        <ProgressView />
      {:else if $view === 'performance'}
        <PerformanceView />
      {:else if $view === 'recovery'}
        <RecoveryView />
      {:else if $view === 'plan'}
        <PlanView />
      {:else if $view === 'activities'}
        <ActivitiesView />
      {:else if $view === 'detail'}
        <ActivityDetailView />
      {:else}
        <SettingsView />
      {/if}
    </main>
  </div>
{/if}

<style>
  /* Sehr dezenter Verlauf oben im Inhaltsbereich, nur aus Tokens (SPEC 5.5, T24). */
  .shell {
    display: flex;
    min-height: 100vh;
    background:
      radial-gradient(900px 420px at 72% -160px, var(--glow), transparent 70%),
      radial-gradient(640px 360px at 8% -120px, var(--glow-2), transparent 70%);
  }

  aside {
    width: 208px;
    flex-shrink: 0;
    box-sizing: border-box;
    padding: 24px 12px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    position: sticky;
    top: 0;
    height: 100vh;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    font-size: var(--fs-heading);
    line-height: 1.2;
    font-weight: var(--fw-bold);
    letter-spacing: -0.01em;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  nav button {
    background: none;
    border-color: transparent;
    text-align: left;
    padding: 9px 12px;
    color: var(--muted);
    font-size: var(--fs-body);
    font-weight: var(--fw-medium);
  }

  nav button:hover:not(:disabled) {
    border-color: transparent;
    color: var(--text);
  }

  nav button {
    position: relative;
  }

  nav button.active {
    background: var(--panel-2);
    color: var(--text);
    font-weight: var(--fw-semibold);
  }

  /* Marke der aktuellen Ansicht: Limette, nicht allein der Hintergrund. */
  nav button.active::before {
    content: '';
    position: absolute;
    left: 0;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
  }

  .spacer {
    flex: 1;
  }

  .sync-status {
    padding: 0 12px;
    color: var(--muted);
  }

  .sync-status:empty {
    display: none;
  }

  .prefs {
    display: flex;
    gap: 8px;
    padding: 0 4px;
  }

  main {
    flex: 1;
    min-width: 0;
    box-sizing: border-box;
    padding: var(--space-6) var(--space-7) var(--space-7);
    max-width: 1500px;
  }

  main.start {
    padding: 24px;
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
