<script lang="ts">
  import { onMount } from 'svelte';
  import { getBackend } from './lib/backend';
  import CoachDrawer from './lib/components/coach/CoachDrawer.svelte';
  import ErrorNote from './lib/components/ErrorNote.svelte';
  import GarminSyncLine from './lib/components/GarminSyncLine.svelte';
  import MigrationNotice from './lib/components/MigrationNotice.svelte';
  import { syncOnStart } from './lib/garmin';
  import { setLocale, t } from './lib/i18n';
  import {
    applyTheme,
    bumpData,
    coachOpen,
    fitMigration,
    toggleCoach,
    VIEWS,
    view,
    watchSystemTheme,
    wizardOpen,
    type View,
  } from './lib/state';
  import { windowTitle } from './lib/title';
  import HistoryView from './views/HistoryView.svelte';
  import PlanView from './views/PlanView.svelte';
  import SettingsView from './views/SettingsView.svelte';
  import Wizard from './views/onboarding/Wizard.svelte';
  import TodayView from './views/TodayView.svelte';

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

  const select = (v: View) => view.set(v);

  // Per Tastenkürzel ohne Bewegung (siehe CoachDrawer), per Knopf mit.
  let coachInstant = $state(false);

  function clickCoach() {
    coachInstant = false;
    toggleCoach();
  }

  // Strg+K (macOS: Cmd+K) öffnet und schließt den Coach aus jeder Ansicht, nicht im Assistenten.
  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key.toLowerCase() !== 'k' || !(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
    if (!ready || startError || !onboarded || $wizardOpen) return;
    e.preventDefault();
    coachInstant = true;
    toggleCoach();
  }

  function finishSetup() {
    onboarded = true;
    wizardOpen.set(false);
    view.set('today');
    bumpData();
  }
</script>

{#snippet navButton(v: View)}
  <button
    class:active={$view === v}
    aria-current={$view === v ? 'page' : undefined}
    data-testid={`nav-${v}`}
    onclick={() => select(v)}>{$t(`ui.nav.${v}`)}</button
  >
{/snippet}

<svelte:window onkeydown={onWindowKeydown} />

{#if startError}
  <main class="start"><ErrorNote error={startError} /></main>
{:else if !ready}
  <main class="start"><p class="muted">{$t('common.loading')}</p></main>
{:else if !onboarded || $wizardOpen}
  <Wizard onfinish={finishSetup} />
{:else}
  <div class="shell" class:coach-open={$coachOpen}>
    <aside>
      <div class="brand">
        <svg width="24" height="24" viewBox="0 0 26 26" aria-hidden="true">
          <rect x="2" y="14" width="5" height="10" rx="1.5" fill="var(--muted)" opacity="0.6" />
          <rect x="10.5" y="8" width="5" height="16" rx="1.5" fill="var(--muted)" />
          <rect x="19" y="2" width="5" height="22" rx="1.5" fill="var(--accent)" />
        </svg>
        <span>{$t('app.name')}</span>
      </div>
      <!-- Drei Hauptpunkte (T42, E8), Einstellungen unten. Sprache und Thema stehen nur in den
           Einstellungen, damit es keine zwei Stellen für dieselbe Wahl gibt. -->
      <nav aria-label={$t('ui.nav.label')}>
        {#each VIEWS as v (v)}
          {@render navButton(v)}
        {/each}
        <div class="spacer"></div>
        <div class="sync-status">
          <GarminSyncLine testid="sidebar-sync" short />
        </div>
        <!-- Der Coach ist keine Ansicht, sondern ein Fenster neben jeder Ansicht (T43). -->
        <button
          class="coach-toggle"
          class:active={$coachOpen}
          aria-expanded={$coachOpen}
          aria-controls="coach-drawer"
          aria-keyshortcuts="Control+K Meta+K"
          title={$t('ui.coach.drawer.shortcut')}
          data-testid="nav-coach"
          onclick={clickCoach}
          >{$t('ui.nav.coach')}<kbd aria-hidden="true">{$t('ui.coach.drawer.kbd')}</kbd></button
        >
        {@render navButton('settings')}
      </nav>
    </aside>

    <main>
      <MigrationNotice />
      {#if $view === 'today'}
        <TodayView />
      {:else if $view === 'plan'}
        <PlanView />
      {:else if $view === 'history'}
        <HistoryView />
      {:else}
        <SettingsView />
      {/if}
    </main>
    <CoachDrawer instant={coachInstant} />
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
    flex: 1;
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
    padding: 0 12px 8px;
    color: var(--muted);
  }

  .sync-status:empty {
    display: none;
  }

  main {
    flex: 1;
    min-width: 0;
    box-sizing: border-box;
    padding: var(--space-6) var(--space-7) var(--space-7);
    max-width: 1500px;
  }

  /* Ab 1100 px steht der offene Coach neben dem Inhalt statt darüber (T43). */
  @media (min-width: 1100px) {
    .shell.coach-open main {
      margin-right: min(400px, 100vw);
    }
  }

  nav button.coach-toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  nav button.coach-toggle.active {
    background: var(--panel-2);
    color: var(--text);
  }

  kbd {
    font-family: inherit;
    font-size: var(--fs-small);
    color: var(--muted);
    opacity: 0.8;
  }

  main.start {
    padding: 24px;
  }
</style>
