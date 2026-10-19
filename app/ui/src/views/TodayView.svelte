<script lang="ts">
  import { getBackend } from '../lib/backend';
  import type { ActivityItem, Dashboard, Plan } from '../lib/backend/types';
  import { addDays, localIso, mondayOf } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import { formatLongDate } from '../lib/format';
  import { locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { dataVersion, paletteOpen } from '../lib/state';
  import { activePlanId } from '../lib/today';
  import { buildWeekStory } from '../lib/week-story';
  import DecisionStrip from './today/DecisionStrip.svelte';
  import FormInstrument from './today/FormInstrument.svelte';
  import RecoveryInstrument from './today/RecoveryInstrument.svelte';
  import StartPath from './today/StartPath.svelte';
  import TodayNotes from './today/TodayNotes.svelte';
  import WeekInstrument from './today/WeekInstrument.svelte';

  // Heute als Tafel (T63, SPEC 5.5): Kopfzeile, Entscheidungsstreifen, drei Instrumente (Form,
  // Erholung, Woche), höchstens drei Notizen. Ohne Plan, Last und Erholungswerte ersetzt ein Weg
  // in drei Schritten die Instrumente. Form-Verlauf, Rampe und Verteilung stehen im Verlauf unter
  // Fortschritt.
  interface TodayData {
    dash: Dashboard;
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
      const [dash, plans, activities] = await Promise.all([
        backend.getDashboard(today),
        backend.listPlans(),
        backend.listActivities({ from: monday, to: addDays(monday, 6), sport: null }),
      ]);
      const id = activePlanId(plans);
      const plan = id ? (await backend.getPlan(id)).plan : null;
      return { dash, activities, plan };
    });
  });

  const data = $derived(loader.data);
  const story = $derived(data ? buildWeekStory(today, data.activities, data.plan) : null);

  // Woche des aktiven Plans mit Phase, nur wenn der Plan heute abdeckt.
  const planWeek = $derived.by(() => {
    const plan = data?.plan;
    if (!plan) return null;
    const i = plan.weeks.findIndex((w) => w.days.some((d) => d.date === today));
    return i === -1 ? null : { n: i + 1, of: plan.weeks.length, phase: plan.weeks[i].phase };
  });

  const hasData = $derived(
    !!data && (data.dash.form !== null || data.dash.readiness.state !== 'readiness.no_data'),
  );
  const empty = $derived(!!data && !hasData && !data.plan);
</script>

{#if loader.error}
  <ErrorNote error={loader.error} />
{:else if !data || !story}
  <p class="muted">{$t('common.loading')}</p>
{:else}
  <div class="page today" data-testid="today">
    <header class="head">
      <div class="when" data-testid="today-head">
        <h1>{formatLongDate(today, $locale)}</h1>
        {#if planWeek}
          <p class="where" data-testid="today-plan-week">
            {$t('ui.instrument.head.week', { n: planWeek.n, of: planWeek.of })} · {$t(
              `ui.plan.phase.${planWeek.phase}`,
            )}
          </p>
        {/if}
      </div>
      <button
        type="button"
        class="cmd"
        onclick={() => paletteOpen.set(true)}
        title={$t('ui.palette.open')}
        aria-keyshortcuts="Control+K Meta+K"
        data-testid="today-palette"
      >
        <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="7" cy="7" r="4.5" />
          <path d="M10.5 10.5 14 14" />
        </svg>
        <span class="cmd-text">{$t('ui.palette.placeholder')}</span>
        <kbd aria-hidden="true">{$t('ui.palette.kbd')}</kbd>
      </button>
    </header>

    <DecisionStrip decision={data.dash.decision} plan={data.plan} {today} quiet={empty} />

    {#if empty}
      <StartPath {hasData} hasPlan={!!data.plan} />
    {:else}
      <div class="instruments">
        <FormInstrument dash={data.dash} />
        <RecoveryInstrument dash={data.dash} />
        <WeekInstrument {story} {today} />
      </div>
      <TodayNotes dash={data.dash} {story} hasPlan={!!data.plan} />
    {/if}
  </div>
{/if}

<style>
  /* Container: Die Spalten richten sich nach der Breite der Seite, nicht des Fensters. */
  .today {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    container-type: inline-size;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2) var(--space-4);
    min-height: 36px;
  }

  /* Das Datum ist der Titel der Seite (wie Apple Fitness und Oura), Woche und Phase stehen
     leise darunter. Die Navigation sagt schon "Heute". */
  .when {
    min-width: 0;
  }

  .when h1 {
    font-size: var(--fs-title);
  }

  .where {
    margin: 2px 0 0;
    font-size: var(--fs-small);
    color: var(--muted);
  }

  /* Suchfeld als ruhige Fläche ohne Rand: Es lädt ein, drängt sich aber nicht vor. */
  .cmd {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 260px;
    min-height: 36px;
    background: var(--surface-hover);
    border-color: transparent;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  @media (hover: hover) and (pointer: fine) {
    .cmd:hover:not(:disabled) {
      border-color: transparent;
      background: color-mix(in oklch, var(--text) 9%, transparent);
      color: var(--text-2);
    }
  }

  .cmd svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
  }

  .cmd kbd {
    margin-left: auto;
  }

  /* Drei Instrumente nebeneinander, getrennt durch Haarlinien; schmal untereinander. */
  .instruments {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin-top: var(--space-3);
  }

  .instruments > :global(*) {
    padding: var(--space-2) var(--space-6);
  }

  .instruments > :global(:first-child) {
    padding-left: 0;
  }

  .instruments > :global(:last-child) {
    padding-right: 0;
  }

  .instruments > :global(* + *) {
    border-left: 1px solid var(--line);
  }

  @container (max-width: 900px) {
    .instruments {
      grid-template-columns: minmax(0, 1fr);
    }

    .instruments > :global(*) {
      padding: var(--space-5) 0;
    }

    .instruments > :global(* + *) {
      border-left: 0;
      border-top: 1px solid var(--line);
    }
  }

  @container (max-width: 560px) {
    .cmd {
      min-width: 0;
    }

    .cmd-text {
      display: none;
    }
  }
</style>
