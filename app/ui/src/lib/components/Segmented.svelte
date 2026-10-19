<script lang="ts" generics="T extends string | number">
  // Umschalter mit wenigen festen Möglichkeiten (Größe, Zeitraum), für die Auswertungsseiten
  // (T32 bis T37). Knöpfe mit aria-pressed in einer Gruppe; die gewählte Möglichkeit trägt
  // Fläche und Gewicht, nicht nur Farbe.
  interface Option {
    value: T;
    label: string;
  }

  interface Props {
    options: Option[];
    value: T;
    label: string;
    onchange: (value: T) => void;
    testid?: string;
  }

  let { options, value, label, onchange, testid }: Props = $props();
</script>

<div class="seg" role="group" aria-label={label} data-testid={testid}>
  {#each options as o (o.value)}
    <button
      type="button"
      class:on={o.value === value}
      aria-pressed={o.value === value}
      data-value={o.value}
      onclick={() => onchange(o.value)}>{o.label}</button
    >
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel);
  }

  button {
    min-height: 32px;
    padding: 4px 14px;
    border: none;
    border-radius: 999px;
    background: none;
    color: var(--muted);
    font-size: var(--fs-body);
    font-weight: var(--fw-medium);
    transition:
      background-color 140ms ease-out,
      color 140ms ease-out;
  }

  button:hover:not(:disabled) {
    color: var(--text);
  }

  button.on {
    background: var(--panel-2);
    color: var(--text);
    font-weight: var(--fw-semibold);
  }

  @media (prefers-reduced-motion: reduce) {
    button {
      transition: none;
    }
  }
</style>
