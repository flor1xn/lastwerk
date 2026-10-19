<script lang="ts">
  import { sparkPath, sparkPoint } from '../dashboard';

  // Mini-Verlauf ohne Achsen: eine Linie, ein Punkt am letzten Wert, optional die eigene Basis
  // als gestrichelte Linie. Lücken (null) unterbrechen die Linie, nichts wird aufgefüllt. Der
  // Wert steht immer daneben als Zahl (Metric), die Linie zeigt nur die Form.
  interface Props {
    values: (number | null)[];
    /** Eigene Basis, wird als gestrichelte Waagerechte gezeichnet. */
    baseline?: number | null;
    label: string;
    color?: string;
    testid?: string;
  }

  let { values, baseline = null, label, color = 'var(--text-2)', testid }: Props = $props();

  const W = 200;
  const H = 44;

  // Gemeinsame Skala für Linie und Basis, sonst läge die Basis außerhalb.
  const range = $derived.by(() => {
    const known = values.filter((v): v is number => v !== null);
    if (known.length === 0) return undefined;
    const all = baseline === null ? known : [...known, baseline];
    const min = Math.min(...all);
    const max = Math.max(...all);
    const pad = Math.max((max - min) * 0.12, 0.5);
    return { min: min - pad, max: max + pad };
  });
  const path = $derived(sparkPath(values, W, H, 3, range));
  const lastIndex = $derived(values.reduce<number>((acc, v, i) => (v === null ? acc : i), -1));
  const end = $derived(lastIndex >= 0 ? sparkPoint(values, lastIndex, W, H, 3, range) : null);
  // Einzelne Werte ohne Nachbarn ergäben keine Linie (spärliche Reihen wie ein Wert je Woche):
  // Sie erscheinen als kleiner Punkt, Lücken bleiben Lücken.
  const isolated = $derived(
    values
      .map((v, i) =>
        v !== null && i !== lastIndex && values[i - 1] == null && values[i + 1] == null
          ? sparkPoint(values, i, W, H, 3, range)
          : null,
      )
      .filter((p): p is { x: number; y: number } => p !== null),
  );
  const base = $derived(
    baseline !== null && range ? sparkPoint([baseline, baseline], 0, W, H, 3, range) : null,
  );
</script>

<svg
  class="spark"
  viewBox={`0 0 ${W} ${H}`}
  role="img"
  aria-label={label}
  data-testid={testid}
  preserveAspectRatio="xMidYMid meet"
>
  {#if base}
    <line class="base" x1="3" x2={W - 3} y1={base.y} y2={base.y} />
  {/if}
  {#if path}
    <path d={path} style:stroke={color} />
  {/if}
  {#each isolated as p, i (i)}
    <circle class="single" cx={p.x} cy={p.y} r="1.8" style:fill={color} />
  {/each}
  {#if end}
    <circle cx={end.x} cy={end.y} r="3.2" style:fill={color} />
  {/if}
</svg>

<style>
  .spark {
    display: block;
    width: 100%;
    max-width: 220px;
    height: auto;
    overflow: visible;
  }

  path {
    fill: none;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .base {
    stroke: var(--muted);
    stroke-width: 1;
    stroke-dasharray: 3 4;
    opacity: 0.7;
  }
</style>
