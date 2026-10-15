<script lang="ts">
  // Ersatz für UplotChart in Komponententests: jsdom hat kein Canvas.
  let { label }: { label: string } = $props();
</script>

<div role="img" aria-label={label} data-testid="chart-stub"></div>
