<script lang="ts">
  import type { ZoneSchemeTime } from '../backend/types';
  import { formatDuration, formatNumber } from '../format';
  import { locale, t } from '../i18n';
  import EstimatedBadge from './EstimatedBadge.svelte';

  let { scheme }: { scheme: ZoneSchemeTime } = $props();

  const total = $derived(scheme.seconds.reduce((s, x) => s + x, 0));
</script>

<section class="zones" data-testid="zone-scheme" data-scheme={scheme.id}>
  <h4>
    {$t(`zones.${scheme.id}.name`)}
    {#if scheme.estimated}<EstimatedBadge />{/if}
  </h4>
  {#if scheme.estimated}
    <p class="warn small">{$t('zones.estimated_hr_max')}</p>
  {/if}
  <p class="muted small">
    {$t('zones.source')}: {scheme.source} · {$t('zones.status')}: {scheme.status}
  </p>
  <table>
    <tbody>
      {#each scheme.seconds as seconds, i (i)}
        <tr>
          <td class="zone">Z{i + 1}</td>
          <td>{$t(`zones.${scheme.id}.z${i + 1}`)}</td>
          <td class="num">{formatDuration(seconds)}</td>
          <td class="num"
            >{total > 0 ? formatNumber((seconds / total) * 100, $locale, 0) : '·'} %</td
          >
          <td class="barcell">
            <span class="bar" style:width={`${total > 0 ? (seconds / total) * 100 : 0}%`}></span>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</section>

<style>
  h4 {
    margin: 0 0 2px;
    font-size: var(--fs-small);
  }

  .small {
    margin: 0 0 4px;
    font-size: var(--fs-label);
  }

  .zone {
    color: var(--muted);
    width: 28px;
  }

  .barcell {
    width: 100%;
    min-width: 80px;
  }

  .bar {
    display: block;
    height: 8px;
    min-width: 1px;
    background: var(--text-2);
    border-radius: 2px;
  }
</style>
