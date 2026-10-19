<script lang="ts">
  import type { Dashboard } from '../../lib/backend/types';
  import Metric from '../../lib/components/Metric.svelte';
  import Section from '../../lib/components/Section.svelte';
  import ShareBars from '../../lib/components/ShareBars.svelte';
  import { distributionStatement } from '../../lib/dashboard';
  import { formatNumber } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';

  // Verteilung: Anteil der Zeit in Zone 1 und 2, je Woche und über 28 Tage, gegen die Grenze aus R8.
  let { dash }: { dash: Dashboard } = $props();

  const d = $derived(dash.distribution);
  const any = $derived(d.weeks.some((w) => w.easy_percent !== null));
</script>

<Section
  label={$t('ui.dash.distribution.title')}
  statement={distributionStatement(d, $t, $locale)}
  testid="dash-distribution"
>
  {#if d.window_easy_percent !== null}
    <Metric
      value={formatNumber(d.window_easy_percent, $locale, 0)}
      unit="%"
      label={$t('ui.dash.distribution.label')}
      testid="distribution-metric"
    />
  {/if}
  {#if any}
    <div class="bars">
      <ShareBars weeks={d.weeks} limit={d.min_easy_percent} testid="distribution-bars" />
    </div>
  {/if}
</Section>

<style>
  .bars {
    margin-top: var(--space-5);
  }
</style>
