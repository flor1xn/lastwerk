<script lang="ts">
  import { getBackend } from '../lib/backend';
  import type { ActivityItem, Dashboard, Plan, WeekOverview } from '../lib/backend/types';
  import { addDays, localIso, mondayOf } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import { leadSentence, planTodayOf } from '../lib/dashboard';
  import { formatDate } from '../lib/format';
  import { locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { dataVersion } from '../lib/state';
  import { activePlanId } from '../lib/today';
  import DecisionCard from './today/DecisionCard.svelte';
  import DistributionSection from './today/DistributionSection.svelte';
  import FormSection from './today/FormSection.svelte';
  import InsightsSection from './today/InsightsSection.svelte';
  import RampSection from './today/RampSection.svelte';
  import ReadinessSection from './today/ReadinessSection.svelte';
  import WeekStorySection from './today/WeekStorySection.svelte';

  // Heute (T44, SPEC 5.5 "Aufbau seit T44"): zuerst die Tagesentscheidung als einzige Karte,
  // dann höchstens drei Erkenntnisse, Bereitschaft, Form und die Wochen-Story. Effizienz und
  // Leistungskurve stehen im Verlauf unter Leistung, der Belastungskalender unter Fortschritt.
  interface TodayData {
    dash: Dashboard;
    week: WeekOverview;
    activities: ActivityItem[];
    plan: Plan | null;
  }

  const loader = new Loader<TodayData>();
  const today = localIso();

  $effect(() => {
    void $dataVersion;
    void loader.run(async () => {
      const backend = getBackend();
      const monday = mondayOf(today);
      const [dash, week, plans, activities] = await Promise.all([
        backend.getDashboard(today),
        backend.getWeekOverview(monday, 1),
        backend.listPlans(),
        backend.listActivities({ from: monday, to: addDays(monday, 6), sport: null }),
      ]);
      const id = activePlanId(plans);
      const plan = id ? (await backend.getPlan(id)).plan : null;
      return { dash, week, activities, plan };
    });
  });

  const data = $derived(loader.data);
  const lead = $derived(
    data
      ? leadSentence(
          data.dash.form?.state ?? null,
          data.dash.readiness,
          planTodayOf(data.dash.decision),
          $t,
        )
      : '',
  );
</script>

{#if loader.error}
  <ErrorNote error={loader.error} />
{:else if !data}
  <p class="muted">{$t('common.loading')}</p>
{:else}
  <div class="page today" data-testid="today">
    <header class="page-head">
      <p class="page-eyebrow">{formatDate(today, $locale)}</p>
      <h1>{$t('ui.nav.today')}</h1>
      <p class="page-lead" data-testid="lead">{lead}</p>
    </header>

    <DecisionCard decision={data.dash.decision} plan={data.plan} {today} />
    <InsightsSection dash={data.dash} />
    <ReadinessSection dash={data.dash} />
    <FormSection dash={data.dash} />
    <WeekStorySection
      {today}
      week={data.week}
      activities={data.activities}
      plan={data.plan}
      adherence={data.dash.adherence}
    />

    <div class="grid">
      <RampSection dash={data.dash} />
      <DistributionSection dash={data.dash} />
    </div>
  </div>
{/if}

<style>
  /* Container: Die Spalten richten sich nach der Breite der Seite, nicht des Fensters (die
     Seitenleiste nimmt links Platz). Darunter eine Spalte. */
  .today {
    display: flex;
    flex-direction: column;
    max-width: var(--page-max);
    container-type: inline-size;
  }

  /* Kleinere Abschnitte nebeneinander. Jeder Abschnitt trägt seine Linie oben. */
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    column-gap: var(--space-7);
  }

  @container (min-width: 760px) {
    .grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
