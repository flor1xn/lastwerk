<script lang="ts">
  import type { Snippet } from 'svelte';

  // Kennzahl (SPEC 5.5, seit T24): großer Wert, Einheit klein daneben, Beschriftung darunter,
  // optional eine Veränderung mit Pfeil und Text. Der Wert kommt fertig formatiert, die Hülle
  // rechnet und formatiert nichts. Fehlt ein Wert, übergibt der Aufrufer den Platzhalter (MISSING).
  interface Delta {
    /** Text neben dem Pfeil, zum Beispiel "+3 zur Vorwoche". */
    text: string;
    direction: 'up' | 'down' | 'flat';
    /** Bewertung der Richtung. Ohne Angabe neutral (grau). */
    tone?: 'good' | 'bad' | 'neutral';
  }

  interface Props {
    value: string;
    unit?: string;
    label: string;
    delta?: Delta;
    /** md: Kennzahl in einer Reihe, lg: Empfehlung, xl: größte Kennzahl der Seite. */
    size?: 'md' | 'lg' | 'xl';
    testid?: string;
    /** Farbe der Reihe (CSS-Wert, zum Beispiel var(--sport-cycling)): kleiner Punkt vor der
     * Beschriftung. Der Text bleibt in Textfarbe, der Punkt trägt die Identität. */
    swatch?: string;
    /** Test-ID nur des Werts, für Tests, die den Text lesen. */
    valueTestid?: string;
    /** Zusatz neben der Beschriftung, zum Beispiel eine Zustandsmarke. */
    extra?: Snippet;
  }

  let {
    value,
    unit,
    label,
    delta,
    size = 'md',
    swatch,
    testid,
    valueTestid,
    extra,
  }: Props = $props();

  // Echtes Minuszeichen (U+2212) statt Bindestrich: Es ist so breit wie eine Ziffer
  // und liegt auf Höhe des Querstrichs, der Bindestrich wirkt vor großen Ziffern zu kurz.
  const shown = $derived(value.replace(/^-(?=\d)/, '\u2212'));
</script>

<div class="metric size-{size}" data-testid={testid}>
  <p class="metric-value">
    <span data-testid={valueTestid}>{shown}</span>
    {#if unit}<span class="metric-unit">{unit}</span>{/if}
  </p>
  <div class="metric-label-row">
    <p class="metric-label">
      {#if swatch}<span class="swatch" style:background={swatch}></span>{/if}{label}
    </p>
    {#if extra}{@render extra()}{/if}
  </div>
  {#if delta}
    <p class="metric-delta {delta.tone ?? 'neutral'}">
      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
        {#if delta.direction === 'up'}
          <path d="M6 10V2M2.5 5.5 6 2l3.5 3.5" />
        {:else if delta.direction === 'down'}
          <path d="M6 2v8M2.5 6.5 6 10l3.5-3.5" />
        {:else}
          <path d="M2 6h8" />
        {/if}
      </svg>
      {delta.text}
    </p>
  {/if}
</div>

<style>
  .swatch {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 6px;
    border-radius: 2px;
    vertical-align: 0;
  }

  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
