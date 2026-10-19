<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { getBackend } from './lib/backend';
  import { GO_KEYS } from './lib/commands';
  import CoachDrawer from './lib/components/coach/CoachDrawer.svelte';
  import CommandPalette from './lib/components/CommandPalette.svelte';
  import ErrorNote from './lib/components/ErrorNote.svelte';
  import GarminSyncLine from './lib/components/GarminSyncLine.svelte';
  import MigrationNotice from './lib/components/MigrationNotice.svelte';
  import { garminSync, syncOnStart } from './lib/garmin';
  import { setLocale, t } from './lib/i18n';
  import {
    applyTheme,
    bumpData,
    coachOpen,
    fitMigration,
    openSettings,
    paletteOpen,
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

  const active = $derived(ready && !startError && onboarded && !$wizardOpen);

  // "G" wartet kurz auf die zweite Taste (Faustregel: eine Sekunde).
  let goPending = 0;

  function typing(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;
    return (
      target.isContentEditable ||
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement
    );
  }

  // Kürzel (SPEC 5.5, E14): Strg+K (macOS Cmd+K) Befehlspalette, Strg+J Coach, G dann H, P, V
  // oder E springt. Nicht im Assistenten; die G-Kürzel nicht in Eingabefeldern.
  function onWindowKeydown(e: KeyboardEvent) {
    if (!active) return;
    const key = e.key.toLowerCase();
    const mod = (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey;
    if (mod && key === 'k') {
      e.preventDefault();
      paletteOpen.set(!get(paletteOpen));
      return;
    }
    if (mod && key === 'j') {
      e.preventDefault();
      paletteOpen.set(false);
      coachInstant = true;
      toggleCoach();
      return;
    }
    if (e.ctrlKey || e.metaKey || e.altKey || get(paletteOpen) || typing(e.target)) return;
    if (goPending && Date.now() - goPending < 1000 && GO_KEYS[key]) {
      e.preventDefault();
      goPending = 0;
      view.set(GO_KEYS[key]);
      return;
    }
    goPending = key === 'g' && !e.shiftKey ? Date.now() : 0;
  }

  function finishSetup() {
    onboarded = true;
    wizardOpen.set(false);
    view.set('today');
    bumpData();
  }

  const goKey = (v: View) =>
    `G ${Object.keys(GO_KEYS)
      .find((k) => GO_KEYS[k] === v)
      ?.toUpperCase()}`;
</script>

{#snippet icon(name: View | 'coach')}
  <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
    {#if name === 'today'}
      <circle cx="10" cy="10" r="7" /><path d="M10 6v4l2.5 2" />
    {:else if name === 'plan'}
      <rect x="3" y="4" width="14" height="13" rx="2" /><path d="M3 8h14M7 2.5v3M13 2.5v3" />
    {:else if name === 'history'}
      <path d="M3.5 16V9.5M8.5 16V5M13.5 16v-5M2 16h16" />
    {:else if name === 'coach'}
      <path d="M4 4.5h12v8.5H9l-4 3v-3H4z" />
    {:else}
      <circle cx="10" cy="10" r="2.5" />
      <path
        d="M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4"
      />
    {/if}
  </svg>
{/snippet}

{#snippet navButton(v: View)}
  <button
    class="rail-item"
    class:active={$view === v}
    aria-current={$view === v ? 'page' : undefined}
    data-tip={`${$t(`ui.nav.${v}`)} · ${goKey(v)}`}
    data-testid={`nav-${v}`}
    onclick={() => select(v)}
  >
    {@render icon(v)}<span class="name">{$t(`ui.nav.${v}`)}</span>
  </button>
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
    <!-- Leiste mit drei Hauptpunkten (E8), unten Coach und Einstellungen (T61). Sprache und
         Thema stehen nur in den Einstellungen, damit es keine zwei Stellen für dieselbe Wahl gibt. -->
    <nav class="rail" aria-label={$t('ui.nav.label')}>
      <div class="brand" title={$t('app.name')}>
        <svg width="22" height="22" viewBox="0 0 26 26" aria-hidden="true">
          <rect x="2" y="14" width="5" height="10" rx="1.5" fill="var(--muted)" opacity="0.6" />
          <rect x="10.5" y="8" width="5" height="16" rx="1.5" fill="var(--muted)" />
          <rect x="19" y="2" width="5" height="22" rx="1.5" fill="var(--accent)" />
        </svg>
        <span class="name">{$t('app.name')}</span>
      </div>
      {#each VIEWS as v (v)}
        {@render navButton(v)}
      {/each}
      <div class="spacer"></div>
      {#if $garminSync.kind !== 'idle'}
        <!-- Statuspunkt des Abgleichs; der Text steht im Tooltip und wird angesagt. Klick führt
             zu Daten und Garmin. -->
        <button
          type="button"
          class="rail-item sync"
          aria-label={$t('ui.nav.sync_label')}
          data-kind={$garminSync.kind}
          onclick={() => openSettings('data')}
        >
          <span class="sync-dot" aria-hidden="true"></span>
          <span class="tip wide"><GarminSyncLine testid="sidebar-sync" short /></span>
        </button>
      {/if}
      <!-- Der Coach ist keine Ansicht, sondern ein Fenster neben jeder Ansicht (T43). -->
      <button
        class="rail-item"
        class:active={$coachOpen}
        aria-expanded={$coachOpen}
        aria-controls="coach-drawer"
        aria-keyshortcuts="Control+J Meta+J"
        data-tip={`${$t('ui.nav.coach')} · ${$t('ui.coach.drawer.kbd')}`}
        data-testid="nav-coach"
        onclick={clickCoach}
      >
        {@render icon('coach')}<span class="name">{$t('ui.nav.coach')}</span>
      </button>
      {@render navButton('settings')}
    </nav>

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
    <CommandPalette />
  </div>
{/if}

<style>
  .shell {
    display: flex;
    min-height: 100vh;
  }

  /* Leiste 56 px (SPEC 5.5): Symbole, Name im Tooltip und für Bildschirmleser. */
  .rail {
    position: sticky;
    top: 0;
    z-index: 30;
    flex: none;
    width: var(--rail);
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-4) 0;
    background: var(--sidebar);
    border-right: 1px solid var(--line);
  }

  .brand {
    display: grid;
    place-items: center;
    height: 40px;
    margin-bottom: var(--space-3);
  }

  .name {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .rail-item {
    position: relative;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    min-height: 40px;
    padding: 0;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-button);
    color: var(--muted);
  }

  .rail-item svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  @media (hover: hover) and (pointer: fine) {
    .rail-item:hover:not(:disabled) {
      border-color: transparent;
      color: var(--text);
      background: var(--surface-hover);
    }
  }

  /* Aktuelle Ansicht: Fläche und Symbol in der Signalfarbe, keine Kante am Rand. Zwei Merkmale,
     damit nicht die Farbe allein trägt. */
  .rail-item.active {
    background: var(--panel-2);
    color: var(--accent);
  }

  /* Tooltip: erscheint nach kurzer Wartezeit, damit Überfahren nicht flackert, ohne Bewegung.
     Bei Knöpfen als Pseudoelement aus data-tip, damit Name und Textinhalt unberührt bleiben. */
  .rail-item[data-tip]::after,
  .tip {
    content: attr(data-tip);
    position: absolute;
    left: calc(100% + 10px);
    top: 50%;
    z-index: 50;
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 5px 8px;
    border: 1px solid var(--ring);
    border-radius: var(--radius-chip);
    background: var(--panel-2);
    box-shadow: var(--shadow-raised);
    color: var(--text);
    font-size: var(--fs-small);
    font-weight: var(--fw-medium);
    white-space: nowrap;
    pointer-events: none;
    opacity: 0;
    visibility: hidden;
    transform: translateY(-50%);
    transition:
      opacity 120ms ease,
      visibility 0s linear 120ms;
  }

  .tip.wide {
    white-space: normal;
    width: max-content;
    max-width: 280px;
  }

  .rail-item:hover::after,
  .rail-item:focus-visible::after,
  .rail-item:hover .tip,
  .rail-item:focus-visible .tip {
    opacity: 1;
    visibility: visible;
    transition:
      opacity 120ms ease 300ms,
      visibility 0s linear 300ms;
  }

  .spacer {
    flex: 1;
  }

  .sync-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--muted);
  }

  .sync[data-kind='running'] .sync-dot {
    background: var(--accent);
  }

  .sync[data-kind='done'] .sync-dot {
    background: var(--ok);
  }

  .sync[data-kind='error'] .sync-dot {
    background: var(--warn);
  }

  main {
    flex: 1;
    min-width: 0;
    padding: var(--space-5) var(--space-7) var(--space-7);
  }

  /* Ab 1100 px steht der offene Coach neben dem Inhalt statt darüber (T43). */
  @media (min-width: 1100px) {
    .shell.coach-open main {
      margin-right: min(400px, 100vw);
    }
  }

  @media (max-width: 1099px) {
    main {
      padding: var(--space-5) var(--space-5) var(--space-7);
    }
  }

  /* Schmal: Leiste unten mit Namen unter den Symbolen, Inhalt mit 16 px Rand. */
  @media (max-width: 759px) {
    .shell {
      flex-direction: column;
    }

    .rail {
      position: fixed;
      top: auto;
      bottom: 0;
      left: 0;
      right: 0;
      width: auto;
      height: calc(60px + env(safe-area-inset-bottom));
      flex-direction: row;
      justify-content: space-around;
      padding: 0 var(--space-2) env(safe-area-inset-bottom);
      border-right: 0;
      border-top: 1px solid var(--line);
      /* Durchscheinende Leiste über dem Inhalt (Skill apple-design, Materialien): Der Inhalt
         bleibt beim Scrollen darunter ahnbar. Wer weniger Transparenz will, bekommt die volle
         Fläche. */
      background: color-mix(in oklch, var(--sidebar) 80%, transparent);
      backdrop-filter: blur(18px) saturate(140%);
    }

    @media (prefers-reduced-transparency: reduce) {
      .rail {
        background: var(--sidebar);
        backdrop-filter: none;
      }
    }

    .brand,
    .spacer,
    .sync,
    .rail-item[data-tip]::after {
      display: none;
    }

    .rail-item {
      width: auto;
      min-width: 56px;
      height: 52px;
      grid-template-rows: auto auto;
      gap: 2px;
      align-content: center;
    }

    .rail-item .name {
      position: static;
      width: auto;
      height: auto;
      clip-path: none;
      font-size: var(--fs-label);
      line-height: 1;
    }

    /* Unten wie in iOS: Symbol und Name in der Signalfarbe, ohne Fläche. */
    .rail-item.active {
      background: none;
    }

    main {
      padding: var(--space-4) var(--space-4) calc(88px + env(safe-area-inset-bottom));
    }
  }

  main.start {
    padding: 24px;
  }
</style>
