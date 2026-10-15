<script lang="ts">
  import type { Dashboard } from '../../lib/backend/types';
  import AdherenceBars from '../../lib/components/AdherenceBars.svelte';
  import Metric from '../../lib/components/Metric.svelte';
  import Section from '../../lib/components/Section.svelte';
  import { adherenceStatement } from '../../lib/dashboard';
  import { formatNumber } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';

  // Planerfüllung: erledigte gegen geplante Last je Woche des aktiven Plans, bis gestern gerechnet.
  let { dash }: { dash: Dashboard } = $props();

  const a = $derived(dash.adherence);
  const hasPlan = $derived(dash.plan !== null);
</script>

<Section
  label={$t('ui.dash.adherence.title')}
  statement={adherenceStatement(a, hasPlan, $t, $locale)}
  testid="today-adherence"
>
  {#if hasPlan}
    {#if a.ratio_percent !== null}
      <Metric
        value={formatNumber(a.ratio_percent, $locale, 0)}
        unit="%"
        label={$t('ui.dash.adherence.label')}
        testid="adherence-metric"
      />
    {/if}
    {#if a.weeks.length > 0}
      <div class="bars">
        <AdherenceBars weeks={a.weeks} today={dash.today} testid="adherence-bars" />
      </div>
    {/if}
  {/if}
</Section>

<style>
  .bars {
    margin-top: var(--space-5);
  }
</style>
