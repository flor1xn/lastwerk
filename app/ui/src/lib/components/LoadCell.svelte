<script lang="ts">
  import type { LoadInfo } from '../backend/types';
  import { formatNumber, MISSING } from '../format';
  import { locale, t } from '../i18n';
  import { missingText } from '../load';
  import EstimatedBadge from './EstimatedBadge.svelte';

  interface Props {
    load: LoadInfo;
    /** Methode (TSS, hrTSS, ...) hinter dem Wert zeigen. */
    showMethod?: boolean;
    /** Dichte Liste: fehlender Wert als leiser Punkt, der Text bleibt für Bildschirmleser und
     *  der Grund im Tooltip (T64). */
    quiet?: boolean;
  }

  let { load, showMethod = true, quiet = false }: Props = $props();

  // Fehlt ein bestimmter Wert (T39), nennt der Tooltip ihn und wo er einzutragen ist.
  const reason = $derived.by(() => {
    const missing = missingText(load);
    return missing
      ? `${$t(missing.what)}. ${$t(missing.hint)}`
      : $t(`ui.load.status.${load.status}`);
  });
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
  <span class="na" class:quiet data-testid="load" data-status={load.status} title={reason}
    >{#if quiet}<span aria-hidden="true">{MISSING}</span><span class="sr"
        >{$t('common.not_computable')}</span
      >{:else}{$t('common.not_computable')}{/if}</span
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

  .na.quiet {
    font-style: normal;
    cursor: help;
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
