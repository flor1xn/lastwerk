<script lang="ts">
  import type { Snippet } from 'svelte';

  // Mehrere Kennzahlen nebeneinander, getrennt durch dünne senkrechte Linien. Kinder sind
  // üblicherweise <Metric>, jedes andere Element funktioniert gleich.
  let { testid, children }: { testid?: string; children: Snippet } = $props();
</script>

<div class="stat-row" data-testid={testid}>
  {@render children()}
</div>
