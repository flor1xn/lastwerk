<script lang="ts">
  import { tick } from 'svelte';
  import { getBackend } from '../backend';
  import type { ActivityItem } from '../backend/types';
  import {
    activityCommands,
    askCoachCommand,
    filterCommands,
    staticCommands,
    type Command,
    type CommandGroup,
  } from '../commands';
  import { addDays, localIso } from '../dates';
  import { locale, t } from '../i18n';
  import { paletteOpen } from '../state';

  // Befehlspalette (T61, E14, SPEC 5.5): Muster "Combobox mit Listbox" der WAI-ARIA Authoring
  // Practices. Öffnet und schließt ohne Bewegung, weil sie mehrmals täglich per Tastatur kommt
  // (Skill emil-design-eng: Raycast öffnet ohne Animation). Gefiltert wird hier, ohne Netz.

  let query = $state('');
  let active = $state(0);
  let input = $state<HTMLInputElement | null>(null);
  let list = $state<HTMLElement | null>(null);
  let activities = $state<ActivityItem[]>([]);
  let returnTo: HTMLElement | null = null;

  const coachKbd = $derived($t('ui.coach.drawer.kbd'));
  const all = $derived([
    ...staticCommands($t, coachKbd),
    ...activityCommands(activities, $t, $locale),
  ]);
  const shown = $derived.by((): Command[] => {
    const hits = filterCommands(all, query);
    return hits.length > 0 || query.trim() === '' ? hits : [askCoachCommand(query, $t)];
  });

  const GROUPS: CommandGroup[] = ['go', 'actions', 'activities', 'history', 'settings'];
  const groups = $derived(
    GROUPS.map((g) => ({ group: g, items: shown.filter((c) => c.group === g) })).filter(
      (g) => g.items.length > 0,
    ),
  );
  // Reihenfolge der Tastatur folgt der Anzeige nach Gruppen.
  const ordered = $derived(groups.flatMap((g) => g.items));

  $effect(() => {
    if ($paletteOpen) {
      const focused = document.activeElement;
      returnTo = focused instanceof HTMLElement && focused !== document.body ? focused : null;
      query = '';
      active = 0;
      void tick().then(() => input?.focus());
      void loadActivities();
    }
  });

  $effect(() => {
    // Neue Eingabe: wieder beim ersten Treffer beginnen.
    void query;
    active = 0;
  });

  $effect(() => {
    if (!list) return;
    const el = list.querySelector<HTMLElement>(`#palette-opt-${active}`);
    el?.scrollIntoView?.({ block: 'nearest' });
  });

  async function loadActivities() {
    const today = localIso();
    try {
      activities = await getBackend().listActivities({
        from: addDays(today, -365),
        to: today,
        sport: null,
      });
    } catch {
      // Ohne Liste bleiben die festen Befehle; ein Fehler hier ist kein Grund für eine Meldung.
      activities = [];
    }
  }

  function close(restore = true) {
    paletteOpen.set(false);
    if (restore && returnTo?.isConnected) returnTo.focus();
    returnTo = null;
  }

  function run(c: Command) {
    close(false);
    c.run();
  }

  function onkeydown(e: KeyboardEvent) {
    const n = ordered.length;
    if (e.key === 'ArrowDown' && n > 0) {
      e.preventDefault();
      active = (active + 1) % n;
    } else if (e.key === 'ArrowUp' && n > 0) {
      e.preventDefault();
      active = (active - 1 + n) % n;
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const c = ordered[active];
      if (c) run(c);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === 'Tab') {
      // Modal: Der Fokus bleibt im Eingabefeld, die Liste bedient man mit den Pfeiltasten.
      e.preventDefault();
    }
  }

  const indexOf = (c: Command) => ordered.indexOf(c);
</script>

{#if $paletteOpen}
  <div class="scrim" role="presentation" onclick={() => close()}></div>
  <div
    class="palette"
    role="dialog"
    aria-modal="true"
    aria-label={$t('ui.palette.label')}
    data-testid="palette"
  >
    <div class="field">
      <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="7" cy="7" r="4.5" />
        <path d="M10.5 10.5 14 14" />
      </svg>
      <input
        bind:this={input}
        bind:value={query}
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-list"
        aria-autocomplete="list"
        aria-activedescendant={ordered.length > 0 ? `palette-opt-${active}` : undefined}
        aria-label={$t('ui.palette.label')}
        placeholder={$t('ui.palette.placeholder')}
        autocomplete="off"
        spellcheck="false"
        data-testid="palette-input"
        {onkeydown}
      />
      <kbd aria-hidden="true">Esc</kbd>
    </div>
    <div
      id="palette-list"
      class="list"
      role="listbox"
      aria-label={$t('ui.palette.label')}
      bind:this={list}
    >
      {#each groups as g (g.group)}
        <div role="group" aria-labelledby="palette-group-{g.group}">
          <div id="palette-group-{g.group}" class="group" role="presentation">
            {$t(`ui.palette.group.${g.group}`)}
          </div>
          {#each g.items as c (c.id)}
            {@const i = indexOf(c)}
            <!-- svelte-ignore a11y_click_events_have_key_events (Tastatur über das Eingabefeld, Combobox-Muster) -->
            <div
              id="palette-opt-{i}"
              class="option"
              class:active={i === active}
              role="option"
              tabindex="-1"
              aria-selected={i === active}
              data-testid="palette-option"
              data-id={c.id}
              onclick={() => run(c)}
              onmousemove={() => (active = i)}
            >
              <span class="label">{c.label}</span>
              {#if c.shortcut}<kbd>{c.shortcut}</kbd>{/if}
            </div>
          {/each}
        </div>
      {/each}
    </div>
    <p class="foot" aria-hidden="true">{$t('ui.palette.hint')}</p>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: var(--scrim);
    /* Leichte Unschärfe trennt die Palette vom Inhalt (Skill apple-design, Materialien). Ohne
       Bewegung: Die Palette öffnet oft und per Tastatur. */
    backdrop-filter: blur(3px);
  }

  .palette {
    position: fixed;
    z-index: 61;
    top: 14vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(620px, calc(100vw - 32px));
    max-height: min(520px, 72vh);
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--ring);
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-float);
    overflow: hidden;
  }

  .field {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--line);
  }

  .field svg {
    flex: none;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.6;
    stroke-linecap: round;
  }

  input {
    flex: 1;
    min-width: 0;
    height: 52px;
    padding: 0;
    background: none;
    border: 0;
    border-radius: 0;
    font-size: var(--fs-lead);
  }

  input:focus-visible {
    outline: none;
  }

  .list {
    flex: 1 1 auto;
    overflow-y: auto;
    padding: var(--space-2);
  }

  .group {
    padding: var(--space-3) var(--space-3) var(--space-1);
    font-size: var(--fs-small);
    font-weight: var(--fw-medium);
    color: var(--muted);
  }

  .option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    min-height: 36px;
    padding: 0 var(--space-3);
    border-radius: var(--radius-button);
    cursor: pointer;
  }

  /* Die gewählte Zeile ist eine Fläche, keine Kante (wie Raycast und Linear). */
  .option.active {
    background: var(--panel-2);
  }

  .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .foot {
    margin: 0;
    padding: var(--space-2) var(--space-4);
    border-top: 1px solid var(--line);
    background: color-mix(in oklch, var(--panel-2) 50%, var(--panel));
    font-size: var(--fs-small);
    color: var(--muted);
  }

  @media (max-width: 759px) {
    .palette {
      top: var(--space-4);
    }
  }
</style>
