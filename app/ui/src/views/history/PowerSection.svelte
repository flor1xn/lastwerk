<script lang="ts">
  import type { Dashboard } from '../../lib/backend/types';
  import PowerCurveBars from '../../lib/components/PowerCurveBars.svelte';
  import Section from '../../lib/components/Section.svelte';
  import { powerCurveStatement, powerCurveSummary } from '../../lib/dashboard';
  import { t } from '../../lib/i18n';

  // Leistungskurve: beste Leistung je Dauer, letzte 90 Tage gegen die 90 davor (Radeinheiten).
  let { dash }: { dash: Dashboard } = $props();

  const durations = $derived(dash.power_curve.durations);
  const any = $derived(powerCurveSummary(durations) !== 'power_curve.no_data');
  const hasValues = $derived(durations.some((d) => d.current_w !== null));
</script>

<Section
  label={$t('ui.dash.power.title')}
  statement={powerCurveStatement(durations, $t)}
  testid="dash-power"
>
  {#if hasValues || any}
    <PowerCurveBars {durations} testid="power-bars" />
  {/if}
</Section>
