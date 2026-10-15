<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import type { PlanningInputs, Settings } from '../../lib/backend/types';
  import ErrorNote from '../../lib/components/ErrorNote.svelte';
  import { errorKey } from '../../lib/errors';
  import { t } from '../../lib/i18n';
  import AvailabilityPanel from '../plan/AvailabilityPanel.svelte';
  import GoalPanel from '../plan/GoalPanel.svelte';
  import AppearanceSection from '../settings/AppearanceSection.svelte';
  import AthleteSection from '../settings/AthleteSection.svelte';
  import CoachSetup from '../settings/CoachSetup.svelte';
  import GarminConnect from '../settings/GarminConnect.svelte';
  import ThresholdSection from '../settings/ThresholdSection.svelte';

  interface Props {
    /** Wird nach dem letzten Schritt aufgerufen, nachdem der Abschluss gespeichert ist. */
    onfinish: () => void;
  }

  let { onfinish }: Props = $props();

  // Reihenfolge der Schritte. Überspringbar heißt: mit Hinweis, was dann fehlt.
  const STEPS = ['language', 'thresholds', 'goal', 'coach', 'garmin'] as const;
  type Step = (typeof STEPS)[number];
  const SKIPPABLE: Step[] = ['thresholds', 'goal', 'coach', 'garmin'];

  let index = $state(0);
  let settings = $state<Settings | null>(null);
  let inputs = $state<PlanningInputs | null>(null);
  let loadError = $state<unknown>(null);
  let errorMessage = $state<string | null>(null);
  let finishing = $state(false);

  const step = $derived(STEPS[index]);
  const last = $derived(index === STEPS.length - 1);

  async function loadSettings() {
    settings = await getBackend().getSettings();
  }

  $effect(() => {
    const backend = getBackend();
    void Promise.all([backend.getSettings(), backend.getPlanningInputs()])
      .then(([s, i]) => {
        settings = s;
        inputs = i;
      })
      .catch((e: unknown) => {
        loadError = e;
      });
  });

  function next() {
    if (!last) index += 1;
  }

  function back() {
    if (index > 0) index -= 1;
  }

  function skip() {
    if (last) void finish();
    else next();
  }

  async function finish() {
    errorMessage = null;
    finishing = true;
    try {
      // Als Einstellung gespeichert, nicht im Browser: Backup und Neuinstallation tragen es mit.
      await getBackend().setSettings({ onboarded: true });
      onfinish();
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      finishing = false;
    }
  }
</script>

<main class="wizard" data-testid="wizard">
  {#if loadError}
    <ErrorNote error={loadError} />
  {:else if !settings || !inputs}
    <p class="muted">{$t('common.loading')}</p>
  {:else}
    <header>
      <p class="label" data-testid="wizard-progress">
        {$t('ui.onboarding.step_of', { n: index + 1, total: STEPS.length })}
      </p>
      <h2 data-testid="wizard-title">{$t(`ui.onboarding.${step}.title`)}</h2>
      <p class="lead">{$t(`ui.onboarding.${step}.lead`)}</p>
    </header>

    {#key step}
      <div class="body" data-testid={`wizard-step-${step}`}>
        {#if step === 'language'}
          <AppearanceSection />
        {:else if step === 'thresholds'}
          <ThresholdSection />
          <AthleteSection {settings} onchange={() => void loadSettings()} />
        {:else if step === 'goal'}
          <GoalPanel {inputs} onchange={(i) => (inputs = i)} />
          <AvailabilityPanel {inputs} onchange={(i) => (inputs = i)} />
        {:else if step === 'coach'}
          <!-- Lokaler Coach (T29): Ollama einrichten und ein Modell wählen. Der Download läuft
               im gemeinsamen Zustand weiter, der Assistent wartet nicht darauf. -->
          <section class="panel" data-testid="coach-step">
            <CoachSetup />
          </section>
        {:else}
          <!-- Anmeldung im Fenster von Garmin (SPEC 5.6). Kein Feld für Zugangsdaten. -->
          <section class="panel" data-testid="garmin-step">
            <p>{$t('ui.onboarding.garmin.explain')}</p>
            <p class="muted" data-testid="garmin-risk">{$t('ui.garmin.risk')}</p>
            <GarminConnect />
          </section>
        {/if}
      </div>
    {/key}

    {#if errorMessage}
      <p class="error" role="alert">{$t(errorMessage)}</p>
    {/if}

    {#if SKIPPABLE.includes(step)}
      <p class="muted skip-hint" data-testid="wizard-skip-hint">
        {$t(`ui.onboarding.${step}.if_skipped`)}
      </p>
    {/if}

    <footer>
      <button onclick={back} disabled={index === 0 || finishing} data-testid="wizard-back">
        {$t('ui.onboarding.back')}
      </button>
      <span class="spacer"></span>
      {#if SKIPPABLE.includes(step)}
        <button onclick={skip} disabled={finishing} data-testid="wizard-skip">
          {$t('ui.onboarding.skip')}
        </button>
      {/if}
      {#if last}
        <button
          class="primary"
          onclick={() => void finish()}
          disabled={finishing}
          data-testid="wizard-finish">{$t('ui.onboarding.finish')}</button
        >
      {:else}
        <button class="primary" onclick={next} data-testid="wizard-next">
          {$t('ui.onboarding.next')}
        </button>
      {/if}
    </footer>
  {/if}
</main>

<style>
  .wizard {
    box-sizing: border-box;
    max-width: 760px;
    margin: 0 auto;
    padding: 48px 24px 64px;
  }

  header {
    margin-bottom: 24px;
  }

  .label {
    margin: 0 0 8px;
    font-size: var(--fs-label);
    font-weight: var(--fw-bold);
    text-transform: uppercase;
    letter-spacing: var(--ls-label);
    color: var(--muted);
  }

  h2 {
    margin: 0 0 8px;
    font-size: var(--fs-title);
    line-height: var(--lh-title);
    font-weight: var(--fw-bold);
    letter-spacing: var(--ls-display);
  }

  .lead {
    margin: 0;
    font-size: var(--fs-lead);
    color: var(--muted);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    animation: step-in 160ms ease-out;
  }

  /* Nur transform und opacity (SPEC 5.5, Gestaltung der Oberfläche). */
  @keyframes step-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }

  .skip-hint {
    margin: 16px 0 0;
  }

  footer {
    display: flex;
    gap: 8px;
    margin-top: 24px;
  }

  .spacer {
    flex: 1;
  }
</style>
