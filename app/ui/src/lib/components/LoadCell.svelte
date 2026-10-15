<script lang="ts">
  import type { LoadInfo } from '../backend/types';
  import { formatNumber } from '../format';
  import { locale, t } from '../i18n';
  import EstimatedBadge from './EstimatedBadge.svelte';

  interface Props {
    load: LoadInfo;
    /** Methode (TSS, hrTSS, ...) hinter dem Wert zeigen. */
    showMethod?: boolean;
  }

  let { load, showMethod = true }: Props = $props();
</script>

{#if load.status === 'ok' && load.value !== null}
  <span class="load" data-testid="load" data-status="ok">
    <span class="value">{formatNumber(load.value, $locale, 1)}</span>
    {#if showMethod && load.method}
      <span class="method">{$t(`ui.load.method.${load.method}`)}</span>
    {/if}
    {#if load.estimated}<EstimatedBadge />{/if}
  </span>
{:else}
  <!-- Fehlende Daten sind nie 0 (SPEC 5.4): Text statt Zahl, der Grund steht im Tooltip. -->
  <span
    class="na"
    data-testid="load"
    data-status={load.status}
    title={$t(`ui.load.status.${load.status}`)}>{$t('common.not_computable')}</span
  >
{/if}

<style>
  .value {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .method {
    color: var(--muted);
    font-size: var(--fs-label);
    margin-left: 3px;
  }

  .na {
    color: var(--muted);
    font-style: italic;
  }
</style>
