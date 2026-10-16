<script lang="ts">
  import { setLanguage } from '../../lib/appearance';
  import { getBackend } from '../../lib/backend';
  import type { Locale, PlanningInputs, Settings } from '../../lib/backend/types';
  import ErrorNote from '../../lib/components/ErrorNote.svelte';
  import GarminSyncLine from '../../lib/components/GarminSyncLine.svelte';
  import { errorKey } from '../../lib/errors';
  import { garminStatus, garminSync, refreshGarminStatus } from '../../lib/garmin';
  import { LOCALES, locale, t } from '../../lib/i18n';
  import { dataVersion } from '../../lib/state';
  import QuickPlanner from '../plan/QuickPlanner.svelte';
  import CoachSetup from '../settings/CoachSetup.svelte';
  import GarminConnect from '../settings/GarminConnect.svelte';
  import SensitiveShares from '../settings/SensitiveShares.svelte';

  interface Props {
    /** Wird nach dem letzten Schritt aufgerufen, nachdem der Abschluss gespeichert ist. */
    onfinish: () => void;
  }

  let { onfinish }: Props = $props();

  // Einrichtung (T41, Entscheidung E6): in drei Schritten zum fertigen Plan, ohne Frage nach
  // Schwellenwerten (sie kommen aus Garmin, T39).
  // 1. Garmin verbinden. Die Sprache kommt beim ersten Start vom System und steht oben zur Wahl.
  // 2. Während der erste Abgleich läuft: Freigaben der sensiblen Werte je Bereich und, auf
  //    Wunsch, den lokalen Coach einrichten (nur auf Klick, SPEC 9.2).
  // 3. Die drei Fragen des Planers, vorbelegt aus der eigenen Historie. Ein Klick erzeugt und
  //    aktiviert den Plan und schließt die Einrichtung ab.
  const STEPS = ['garmin', 'extras', 'plan'] as const;
  type Step = (typeof STEPS)[number];

  let index = $state(0);
  let settings = $state<Settings | null>(null);
  let inputs = $state<PlanningInputs | null>(null);
  let loadError = $state<unknown>(null);
  let errorMessage = $state<string | null>(null);
  let finishing = $state(false);

  const step = $derived<Step>(STEPS[index]);
  const connected = $derived($garminStatus?.connected ?? false);
  const syncing = $derived($garminSync.kind === 'running');

  /** Sprache des Systems, wenn Lastwerk sie kennt (sonst Englisch). */
  function systemLocale(): Locale {
    const lang = typeof navigator === 'undefined' ? '' : (navigator.language ?? '');
    const base = lang.slice(0, 2).toLowerCase();
    return (LOCALES as readonly string[]).includes(base) ? (base as Locale) : 'en';
  }

  $effect(() => {
    void refreshGarminStatus();
    void getBackend()
      .getSettings()
      .then(async (s) => {
        // Nur beim ersten Start: Wer die Einrichtung wiederholt, hat seine Sprache gewählt.
        const sys = systemLocale();
        if (!s.onboarded && sys !== s.locale) await setLanguage(sys);
        settings = { ...s, locale: s.onboarded ? s.locale : sys };
      })
      .catch((e: unknown) => {
        loadError = e;
      });
  });

  // Die Fragen des Planers lesen die Historie neu, sobald der Abgleich Aktivitäten bringt.
  $effect(() => {
    void $dataVersion;
    if (step !== 'plan') return;
    void getBackend()
      .getPlanningInputs()
      .then((i) => {
        inputs = i;
      })
      .catch((e: unknown) => {
        loadError = e;
      });
  });

  async function reloadSettings() {
    try {
      settings = await getBackend().getSettings();
    } catch (e) {
      errorMessage = errorKey(e);
    }
  }

  function next() {
    if (index < STEPS.length - 1) index += 1;
  }

  function back() {
    if (index > 0) index -= 1;
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
  {:else if !settings}
    <p class="muted">{$t('common.loading')}</p>
  {:else}
    <header>
      <div class="top">
        <p class="label" data-testid="wizard-progress">
          {$t('ui.onboarding.step_of', { n: index + 1, total: STEPS.length })}
        </p>
        <label class="lang">
          <span class="visually-hidden">{$t('ui.appearance.language')}</span>
          <select
            value={$locale}
            onchange={(e) => void setLanguage(e.currentTarget.value as Locale)}
            data-testid="wizard-language"
          >
            {#each LOCALES as l (l)}
              <option value={l}>{$t(`ui.language.${l}`)}</option>
            {/each}
          </select>
        </label>
      </div>
      <ol class="steps" aria-hidden="true">
        {#each STEPS as s, i (s)}
          <li class:done={i < index} class:current={i === index}>
            {$t(`ui.onboarding.${s}.short`)}
          </li>
        {/each}
      </ol>
      <h2 data-testid="wizard-title">{$t(`ui.onboarding.${step}.title`)}</h2>
      <p class="lead">{$t(`ui.onboarding.${step}.lead`)}</p>
    </header>

    {#key step}
      <div class="body" data-testid={`wizard-step-${step}`}>
        {#if step === 'garmin'}
          <!-- Anmeldung im Fenster von Garmin (SPEC 5.6). Kein Feld für Zugangsdaten. -->
          <section class="panel" data-testid="garmin-step">
            <p>{$t('ui.onboarding.garmin.explain')}</p>
            <p data-testid="garmin-takes">{$t('ui.onboarding.garmin.takes')}</p>
            <p class="muted" data-testid="garmin-risk">{$t('ui.garmin.risk')}</p>
            <GarminConnect />
          </section>
          {#if !connected}
            <p class="muted skip-hint" data-testid="wizard-skip-hint">
              {$t('ui.onboarding.garmin.if_skipped')}
            </p>
          {/if}
        {:else if step === 'extras'}
          {#if connected}
            <section class="panel" data-testid="wizard-sync">
              <p class="part">{$t('ui.onboarding.extras.sync_title')}</p>
              <GarminSyncLine testid="wizard-sync-line" detail />
              {#if syncing}
                <p class="muted">{$t('ui.onboarding.extras.sync_hint')}</p>
              {/if}
            </section>
            <section class="panel" data-testid="wizard-s3">
              <p class="part">{$t('ui.garmin.s3.title')}</p>
              <SensitiveShares {settings} onchange={() => void reloadSettings()} />
            </section>
          {/if}
          <section class="panel" data-testid="wizard-coach">
            <p class="part">{$t('ui.onboarding.extras.coach_title')}</p>
            <p class="muted">{$t('ui.onboarding.extras.coach_hint')}</p>
            <details>
              <summary data-testid="wizard-coach-open">
                {$t('ui.onboarding.extras.coach_open')}
              </summary>
              <CoachSetup />
            </details>
          </section>
        {:else}
          {#if syncing && !inputs?.history}
            <p class="muted" role="status" data-testid="wizard-plan-waiting">
              {$t('ui.onboarding.plan.waiting')}
            </p>
          {:else if inputs?.history}
            <p class="muted" data-testid="wizard-plan-history">
              {$t('ui.onboarding.plan.from_history', { weeks: inputs.history.weeks })}
            </p>
          {/if}
          {#if inputs}
            <!-- Neu aufgebaut, sobald ein Vorschlag aus der Historie da ist. -->
            {#key inputs.history ? 'history' : 'default'}
              <QuickPlanner {inputs} oncreated={() => void finish()} />
            {/key}
          {:else}
            <p class="muted">{$t('common.loading')}</p>
          {/if}
        {/if}
      </div>
    {/key}

    {#if errorMessage}
      <p class="error" role="alert">{$t(errorMessage)}</p>
    {/if}

    <footer>
      <button onclick={back} disabled={index === 0 || finishing} data-testid="wizard-back">
        {$t('ui.onboarding.back')}
      </button>
      <span class="spacer"></span>
      {#if step === 'garmin'}
        <button
          class:primary={connected}
          onclick={next}
          data-testid={connected ? 'wizard-next' : 'wizard-skip'}
        >
          {connected ? $t('ui.onboarding.next') : $t('ui.onboarding.garmin.without')}
        </button>
      {:else if step === 'extras'}
        <button class="primary" onclick={next} data-testid="wizard-next">
          {$t('ui.onboarding.extras.to_plan')}
        </button>
      {:else}
        <button onclick={() => void finish()} disabled={finishing} data-testid="wizard-finish">
          {$t('ui.onboarding.plan.later')}
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

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .label,
  .part {
    margin: 0 0 8px;
    font-size: var(--fs-label);
    font-weight: var(--fw-bold);
    text-transform: uppercase;
    letter-spacing: var(--ls-label);
    color: var(--muted);
  }

  .steps {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    margin: 0 0 16px;
    padding: 0;
    list-style: none;
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .steps li.done {
    color: var(--text);
  }

  .steps li.current {
    color: var(--text);
    font-weight: var(--fw-semibold);
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
    animation: step-in 180ms cubic-bezier(0.23, 1, 0.32, 1);
  }

  /* Nur transform und opacity (SPEC 5.5, Gestaltung der Oberfläche). */
  @keyframes step-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .body {
      animation: none;
    }
  }

  details summary {
    cursor: pointer;
  }

  .skip-hint {
    margin: 0;
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
