<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte';

  // Reiter einer Seite (T42): Verlauf und Einstellungen. Muster "Tabs" der WAI-ARIA Authoring
  // Practices: role="tablist", nur der aktive Reiter ist per Tab-Taste erreichbar (wandernder
  // Tabindex), Pfeiltasten, Pos1 und Ende wechseln und aktivieren sofort. Für kleine
  // Unterumschalter innerhalb eines Reiters bleibt `Segmented`.
  interface Option {
    value: T;
    label: string;
  }

  interface Props {
    /** Kurzer Name der Gruppe, bildet die Kennungen `tab-<gruppe>-<wert>` und `panel-<gruppe>`. */
    group: string;
    options: Option[];
    value: T;
    /** Zugänglicher Name der Reiterleiste. */
    label: string;
    onchange: (value: T) => void;
    /** Inhalt des aktiven Reiters, steht im zugehörigen tabpanel. */
    children?: Snippet;
  }

  let { group, options, value, label, onchange, children }: Props = $props();

  let list = $state<HTMLElement | null>(null);

  const tabId = (v: T) => `tab-${group}-${v}`;
  const panelId = $derived(`panel-${group}`);

  function select(index: number) {
    const next = options[(index + options.length) % options.length];
    if (!next) return;
    if (next.value !== value) onchange(next.value);
    // Der Fokus folgt der Auswahl; die Schaltfläche existiert schon, nur ihr Tabindex wandert.
    list?.querySelector<HTMLElement>(`[role="tab"][data-value="${next.value}"]`)?.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    const current = options.findIndex((o) => o.value === value);
    let target: number | null = null;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') target = current + 1;
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') target = current - 1;
    else if (e.key === 'Home') target = 0;
    else if (e.key === 'End') target = options.length - 1;
    if (target === null) return;
    e.preventDefault();
    select(target);
  }
</script>

<div
  class="tabs"
  role="tablist"
  aria-label={label}
  data-testid={`tabs-${group}`}
  bind:this={list}
  tabindex="-1"
  {onkeydown}
>
  {#each options as o (o.value)}
    <button
      type="button"
      role="tab"
      id={tabId(o.value)}
      class:on={o.value === value}
      aria-selected={o.value === value}
      aria-controls={panelId}
      tabindex={o.value === value ? 0 : -1}
      data-testid={tabId(o.value)}
      data-value={o.value}
      onclick={() => onchange(o.value)}>{o.label}</button
    >
  {/each}
</div>

{#if children}
  <div
    class="panel-body"
    role="tabpanel"
    id={panelId}
    aria-labelledby={tabId(value)}
    data-testid={panelId}
  >
    {@render children()}
  </div>
{/if}

<style>
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-5);
    margin: 0 0 var(--space-5);
    border-bottom: 1px solid var(--line);
  }

  .tabs:focus {
    outline: none;
  }

  button {
    position: relative;
    min-height: 40px;
    padding: var(--space-2) 0;
    border: none;
    border-radius: 0;
    background: none;
    color: var(--muted);
    font-size: var(--fs-body);
    font-weight: var(--fw-medium);
  }

  button:hover:not(:disabled) {
    color: var(--text);
  }

  button.on {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }

  /* Marke des aktiven Reiters: Limette wie in der Seitenleiste, nicht allein die Farbe des
     Textes. Ohne Gleiten: Reiter wechselt man oft, eine Bewegung würde nur aufhalten. */
  button.on::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 3px;
    border-radius: 3px 3px 0 0;
    background: var(--accent);
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
