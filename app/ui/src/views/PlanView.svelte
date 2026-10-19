<script lang="ts">
  import { untrack } from 'svelte';
  import { get } from 'svelte/store';
  import { getBackend } from '../lib/backend';
  import type { PlanDetail, PlanListItem, PlanStatus, PlanningInputs } from '../lib/backend/types';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import PlanDisplay from '../lib/components/PlanDisplay.svelte';
  import { t, setLocale } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { applyTheme, bumpData, dataVersion, openCoach, selectedPlanId } from '../lib/state';
  import AvailabilityPanel from './plan/AvailabilityPanel.svelte';
  import ExportPanel from './plan/ExportPanel.svelte';
  import GeneratePanel from './plan/GeneratePanel.svelte';
  import GoalPanel from './plan/GoalPanel.svelte';
  import PlanList from './plan/PlanList.svelte';
  import QuickPlanner from './plan/QuickPlanner.svelte';

  const listLoader = new Loader<PlanListItem[]>();
  const inputsLoader = new Loader<PlanningInputs>();
  const detailLoader = new Loader<PlanDetail>();
  let actionError = $state<unknown>(null);
  // Die drei Fragen stehen nur ohne aktiven Plan oder nach "Neuer Plan" (T64, SPEC 5.5). Einmal
  // offen, bleiben sie offen, damit ein eben erstellter Plan seine Bestätigung zeigen kann.
  let plannerOpen = $state<boolean | null>(null);

  $effect(() => {
    void $dataVersion;
    void inputsLoader.run(() => getBackend().getPlanningInputs());
    void listLoader.run(() => getBackend().listPlans());
  });

  // Ohne Auswahl (oder mit einem Plan, den es nicht mehr gibt) zeigt die Ansicht den aktiven
  // Plan, sonst den neuesten. Geprüft wird nur, wenn sich die Liste ändert: Ein eben erzeugter
  // Plan ist gewählt, bevor die neu geladene Liste ihn enthält.
  $effect(() => {
    const list = listLoader.data;
    if (!list) return;
    if (untrack(() => plannerOpen) === null) plannerOpen = !list.some((p) => p.status === 'active');
    const current = untrack(() => get(selectedPlanId));
    if (current !== null && list.some((p) => p.id === current)) return;
    selectedPlanId.set((list.find((p) => p.status === 'active') ?? list[0])?.id ?? null);
  });

  $effect(() => {
    const id = $selectedPlanId;
    if (id === null) {
      detailLoader.data = null;
      return;
    }
    void $dataVersion;
    void detailLoader.run(() => getBackend().getPlan(id));
  });

  // Ein Plan ist neu (über die drei Fragen erzeugt): anzeigen, Liste und Eingaben neu lesen.
  // Ziel und Verfügbarkeit können sich dabei geändert haben.
  function created(detail: PlanDetail) {
    detailLoader.data = detail;
    selectedPlanId.set(detail.plan.plan_id);
    void listLoader.run(() => getBackend().listPlans());
    void inputsLoader.run(() => getBackend().getPlanningInputs());
  }

  // Ein Plan aus den Feineinstellungen ist ein Entwurf; die Eingaben ändern sich dabei nicht.
  function generated(detail: PlanDetail) {
    detailLoader.data = detail;
    selectedPlanId.set(detail.plan.plan_id);
    void listLoader.run(() => getBackend().listPlans());
  }

  async function changeStatus(id: string, status: PlanStatus) {
    actionError = null;
    try {
      listLoader.data = await getBackend().setPlanStatus(id, status);
      if (id === $selectedPlanId || status === 'active') {
        const current = $selectedPlanId;
        if (current !== null) void detailLoader.run(() => getBackend().getPlan(current));
      }
    } catch (e) {
      actionError = e;
    }
  }

  async function restored() {
    // Sprache und Thema stehen in den Einstellungen und können sich mit ändern.
    try {
      const s = await getBackend().getSettings();
      setLocale(s.locale);
      document.documentElement.lang = s.locale;
      applyTheme(s.theme);
    } catch {
      // Die Anzeige bleibt, wie sie ist.
    }
    bumpData();
  }
</script>

<div class="page plan-page">
  <header class="page-head">
    <h1>{$t('ui.nav.plan')}</h1>
    <p class="page-lead" data-testid="lead">{$t('ui.lead.plan')}</p>
  </header>

  {#if listLoader.error}
    <ErrorNote error={listLoader.error} />
  {:else if inputsLoader.error}
    <ErrorNote error={inputsLoader.error} />
  {/if}

  <!-- Einspaltig (T43): Das Gespräch liegt im Seitenfenster des Coachs, nicht mehr hier. -->
  <div class="layout">
    <div class="main">
      {#if inputsLoader.data && plannerOpen}
        <!-- Die Fragen starten neu, wenn die Daten von außen wechseln (Wiederherstellung). -->
        {#key $dataVersion}
          <QuickPlanner inputs={inputsLoader.data} oncreated={created} />
        {/key}
      {/if}

      {#if actionError}
        <ErrorNote error={actionError} />
      {/if}
      {#if detailLoader.error}
        <ErrorNote error={detailLoader.error} />
      {:else if detailLoader.data}
        <div class="talk">
          {#if !plannerOpen}
            <button type="button" onclick={() => (plannerOpen = true)} data-testid="plan-new">
              {$t('ui.plan.new_plan')}
            </button>
          {/if}
          <button type="button" onclick={() => openCoach()} data-testid="plan-talk-coach">
            {$t('ui.plan.talk_to_coach')}
          </button>
        </div>
        <PlanDisplay detail={detailLoader.data} />
      {:else if !listLoader.loading && $selectedPlanId === null}
        <p class="muted" data-testid="plan-empty">{$t('ui.plan.view.empty')}</p>
      {/if}
    </div>
  </div>

  <details class="fine" data-testid="fine-tuning">
    <summary>
      <span class="fine-title">{$t('ui.planner.fine_title')}</span>
      <span class="fine-hint">{$t('ui.planner.fine_hint')}</span>
    </summary>
    <div class="fine-body">
      <div class="fine-col">
        {#if inputsLoader.data}
          {#key $dataVersion}
            <GoalPanel inputs={inputsLoader.data} onchange={(i) => (inputsLoader.data = i)} />
            <AvailabilityPanel
              inputs={inputsLoader.data}
              onchange={(i) => (inputsLoader.data = i)}
            />
            <GeneratePanel inputs={inputsLoader.data} ongenerated={generated} />
          {/key}
        {/if}
      </div>
      <div class="fine-col">
        <PlanList
          plans={listLoader.data ?? []}
          selected={$selectedPlanId}
          onselect={(id) => selectedPlanId.set(id)}
          onstatus={changeStatus}
        />
        <ExportPanel planId={$selectedPlanId} onrestored={restored} />
      </div>
    </div>
  </details>
</div>

<style>
  /* Die Spalten der Feineinstellungen richten sich nach der Breite der Seite, nicht des
     Fensters (Seitenleiste links, Coach rechts nehmen Platz). */
  .plan-page {
    max-width: var(--page-max);
    container-type: inline-size;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .talk {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-2);
  }

  .fine {
    margin-top: var(--space-6);
    border-top: 1px solid var(--line);
    padding-top: var(--space-3);
  }

  .fine > summary {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--space-2) 0;
    cursor: pointer;
    list-style: none;
  }

  .fine > summary::-webkit-details-marker {
    display: none;
  }

  .fine > summary:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 6px;
  }

  .fine-title {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--fs-body);
    font-weight: var(--fw-semibold);
  }

  .fine-title::before {
    content: '';
    width: 7px;
    height: 7px;
    border-right: 2px solid var(--muted);
    border-bottom: 2px solid var(--muted);
    transform: rotate(-45deg);
    transition: transform 160ms var(--ease-out);
  }

  .fine[open] > summary .fine-title::before {
    transform: rotate(45deg);
  }

  .fine-hint {
    padding-left: calc(7px + var(--space-2) + 2px);
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .fine-body {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-3);
    align-items: start;
    margin-top: var(--space-3);
  }

  .fine-col {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }

  @container (max-width: 640px) {
    .fine-body {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
