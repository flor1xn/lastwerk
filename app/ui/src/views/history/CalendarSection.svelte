<script lang="ts">
  import type { Dashboard } from '../../lib/backend/types';
  import LoadHeatmap from '../../lib/components/LoadHeatmap.svelte';
  import Section from '../../lib/components/Section.svelte';
  import { calendarStatement } from '../../lib/dashboard';
  import { locale, t } from '../../lib/i18n';

  // Belastungskalender: ein Jahr Tageslast. Ohne Last sagt der Abschnitt nur, was fehlt.
  let { dash }: { dash: Dashboard } = $props();

  const any = $derived(dash.load_days.some((d) => d.load > 0));
</script>

<Section
  label={$t('ui.dash.calendar.title')}
  statement={calendarStatement(dash.load_days, $t, $locale)}
  testid="dash-calendar"
>
  {#if any}
    <LoadHeatmap days={dash.load_days} today={dash.today} testid="calendar" />
  {/if}
</Section>
