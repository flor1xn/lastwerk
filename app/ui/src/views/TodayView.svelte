<script lang="ts">
  import { getBackend } from '../lib/backend';
  import type { Dashboard, Plan, WeekOverview } from '../lib/backend/types';
  import { localIso, mondayOf } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import Section from '../lib/components/Section.svelte';
  import WeekBars from '../lib/components/WeekBars.svelte';
  import { leadSentence, type PlanToday } from '../lib/dashboard';
  import { formatDate } from '../lib/format';
  import { locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { keyText, templateName } from '../lib/plan';
  import { dataVersion, view } from '../lib/state';
  import { activePlanId, planCovers, sessionsOn } from '../lib/today';
  import AdherenceSection from './today/AdherenceSection.svelte';
  import CalendarSection from './today/CalendarSection.svelte';
  import DistributionSection from './today/DistributionSection.svelte';
  import EfficiencySection from './today/EfficiencySection.svelte';
  import FormSection from './today/FormSection.svelte';
  import PowerSection from './today/PowerSection.svelte';
  import RampSection from './today/RampSection.svelte';
  import ReadinessSection from './today/ReadinessSection.svelte';

  interface TodayData {
    dash: Dashboard;
    week: WeekOverview;
    plan: Plan | null;
  }

  const loader = new Loader<TodayData>();
  const today = localIso();

  $effect(() => {
    void $dataVersion;
    void loader.run(async () => {
      const backend = getBackend();
      const [dash, week, plans] = await Promise.all([
        backend.getDashboard(today),
        backend.getWeekOverview(mondayOf(today), 1),
        backend.listPlans(),
      ]);
      const id = activePlanId(plans);
      const plan = id ? (await backend.getPlan(id)).plan : null;
      return { dash, week, plan };
    });
  });

  const data = $derived(loader.data);
  const sessions = $derived(data?.plan ? sessionsOn(data.plan, today) : []);
  const covered = $derived(data?.plan ? planCovers(data.plan, today) : false);
  const main = $derived(sessions[0] ?? null);
  const rest = $derived(sessions.slice(1));
  const reasons = $derived(
    main ? (main.reason_keys ?? []).map((k) => keyText($locale, k)).join(' ') : '',
  );

  // Der Plan von heute für die Ein-Satz-Aussage: hart, locker, frei oder ohne Plan.
  const planToday = $derived<PlanToday>(
    main
      ? sessions.some((s) => s.hard)
        ? 'hard'
        : 'easy'
      : data?.plan && covered
        ? 'rest'
        : 'none',
  );
  const lead = $derived(
    data ? leadSentence(data.dash.form?.state ?? null, data.dash.readiness, planToday, $t) : '',
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

    <!-- Die einzige Karte der Seite: Sie ist die Empfehlung, also das, was man anfasst. -->
    <section class="card hero" data-testid="today-hero">
      <p class="eyebrow">{$t('ui.today.eyebrow')}</p>
      {#if main}
        <h2 class="hero-title" data-testid="today-title">
          {templateName($locale, main.template_id)}
        </h2>
        <p class="hero-meta">
          {$t('ui.today.minutes', { n: main.duration_min })}
          <span class="sep" aria-hidden="true">·</span>
          {main.hard ? $t('ui.today.hard') : $t('ui.today.easy')}
        </p>
        {#if reasons}
          <p class="reason">{reasons}</p>
        {/if}
        {#if rest.length > 0}
          <div class="more">
            <p class="eyebrow muted">{$t('ui.today.all_sessions')}</p>
            <ul>
              {#each rest as s (s.slot + s.template_id)}
                <li>
                  {templateName($locale, s.template_id)},
                  {$t('ui.today.minutes', { n: s.duration_min })}
                </li>
              {/each}
            </ul>
          </div>
        {/if}
      {:else if data.plan && covered}
        <h2 class="hero-title" data-testid="today-title">{$t('ui.today.rest_title')}</h2>
        <p class="hero-meta muted">{$t('ui.today.rest_hint')}</p>
      {:else if data.plan}
        <h2 class="hero-title" data-testid="today-title">{$t('ui.today.not_covered')}</h2>
        <div class="actions">
          <button class="primary" onclick={() => view.set('plan')}
            >{$t('ui.today.open_plan')}</button
          >
        </div>
      {:else}
        <h2 class="hero-title" data-testid="today-title">{$t('ui.today.no_plan_title')}</h2>
        <p class="hero-meta muted">{$t('ui.today.no_plan_hint')}</p>
        <div class="actions">
          <button class="primary" onclick={() => view.set('plan')}
            >{$t('ui.today.open_plan')}</button
          >
        </div>
      {/if}
    </section>

    <ReadinessSection dash={data.dash} />
    <FormSection dash={data.dash} />

    <div class="grid">
      <RampSection dash={data.dash} />
      <DistributionSection dash={data.dash} />
      <Section label={$t('ui.today.week_title')} statement={$t('ui.lead.week')} testid="today-week">
        <WeekBars days={data.week.days} />
      </Section>
      <AdherenceSection dash={data.dash} />
      <EfficiencySection dash={data.dash} />
      <PowerSection dash={data.dash} />
    </div>

    <CalendarSection dash={data.dash} />
  </div>
{/if}

<style>
  /* Container: Die Spalten richten sich nach der Breite der Seite, nicht des Fensters (die
     Seitenleiste nimmt links Platz). Darunter eine Spalte. */
  .today {
    display: flex;
    flex-direction: column;
    max-width: var(--page-max-wide);
    container-type: inline-size;
  }

  .hero {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    margin-bottom: var(--space-6);
    padding: var(--space-6) var(--space-6) 36px;
  }

  .eyebrow {
    margin: 0;
    font-size: var(--fs-label);
    font-weight: var(--fw-bold);
    letter-spacing: var(--ls-label);
    text-transform: uppercase;
    color: var(--accent);
  }

  .eyebrow.muted {
    color: var(--muted);
  }

  .hero-title {
    margin: 0;
    max-width: 20ch;
    overflow-wrap: anywhere;
    text-wrap: balance;
    font-size: var(--fs-display);
    line-height: var(--lh-display);
    font-weight: var(--fw-bold);
    letter-spacing: var(--ls-display);
  }

  .hero-meta {
    margin: 0;
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
    max-width: 52ch;
  }

  .hero-meta.muted {
    color: var(--muted);
  }

  .sep {
    margin: 0 6px;
    color: var(--muted);
  }

  .reason {
    margin: 0;
    max-width: 62ch;
    overflow-wrap: anywhere;
    color: var(--muted);
  }

  .more ul {
    margin: 6px 0 0;
    padding-left: 18px;
    color: var(--muted);
  }

  .actions {
    margin-top: var(--space-2);
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
