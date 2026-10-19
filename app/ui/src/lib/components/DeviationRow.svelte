<script lang="ts">
  // Abweichung von der eigenen Basis (T62, SPEC 5.5): Name, Wert, ein Balken um die Mitte und
  // die Abweichung als Text. Der Balken zeigt bis ±25 % (Faustregel, nur Darstellung); größere
  // Abweichungen stoßen an, der Text nennt den Wert. Werte und Texte kommen fertig formatiert.
  // Bei neuem Wert gleitet der Balken (scaleX), beim ersten Zeichnen nicht.
  interface Props {
    label: string;
    value: string;
    unit: string;
    /** Abweichung in Prozent der Basis, negativ heißt darunter; null ohne Basis. */
    deviation: number | null;
    /** Text der Abweichung, zum Beispiel "−15 %", oder der Hinweis ohne Basis. */
    deviationText: string;
    /** warn färbt Balken und Text, wenn Lastwerk selbst "deutlich darunter" sagt (R12). */
    tone?: 'warn' | 'neutral';
    testid?: string;
  }

  let { label, value, unit, deviation, deviationText, tone = 'neutral', testid }: Props = $props();

  const LIMIT = 25;
  const scale = $derived(deviation === null ? 0 : Math.min(Math.abs(deviation), LIMIT) / LIMIT);
  const below = $derived(deviation !== null && deviation < 0);
</script>

<div class="dev" class:warn={tone === 'warn'} data-testid={testid}>
  <span class="label">{label}</span>
  <span class="value">{value}<small>{unit}</small></span>
  <span class="bar" aria-hidden="true">
    <i class:below style:transform="scaleX({scale})"></i>
  </span>
  <span class="pct">{deviationText}</span>
</div>

<style>
  .dev {
    display: grid;
    grid-template-columns: 5.5em 5.5em minmax(40px, 1fr) 4.5em;
    align-items: center;
    gap: var(--space-3);
    min-height: 40px;
  }

  .dev + :global(.dev) {
    border-top: 1px solid var(--line);
  }

  .label {
    font-size: var(--fs-small);
    color: var(--muted);
    overflow-wrap: anywhere;
  }

  .value {
    font-size: var(--fs-title);
    line-height: 1;
    font-weight: var(--fw-semibold);
    font-stretch: var(--num-stretch);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .value small {
    margin-left: 3px;
    font-size: var(--fs-label);
    font-weight: var(--fw-medium);
    font-stretch: 100%;
    color: var(--muted);
  }

  /* Schiene über die ganze Breite (±25 %), damit eine kleine Abweichung als Lage auf einer Skala
     lesbar ist und nicht als verirrter Strich. Die Mitte ist die eigene Basis. */
  .bar {
    position: relative;
    height: 14px;
    background: linear-gradient(var(--line), var(--line)) center / 100% 2px no-repeat;
    border-radius: 1px;
  }

  .bar::before {
    content: '';
    position: absolute;
    left: 50%;
    top: 1px;
    bottom: 1px;
    width: 2px;
    margin-left: -1px;
    border-radius: 1px;
    background: var(--muted);
  }

  .bar i {
    position: absolute;
    left: 50%;
    top: 4px;
    width: 50%;
    height: 6px;
    border-radius: 2px;
    background: var(--text-2);
    transform-origin: left center;
    transition: transform 200ms var(--ease-in-out);
  }

  .bar i.below {
    left: 0;
    transform-origin: right center;
  }

  .pct {
    text-align: right;
    font-size: var(--fs-small);
    font-weight: var(--fw-semibold);
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    white-space: nowrap;
  }

  .warn .bar i {
    background: var(--warn);
  }

  .warn .pct {
    color: var(--warn);
  }
</style>
