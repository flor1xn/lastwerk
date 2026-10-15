<script lang="ts">
  import { onMount } from 'svelte';
  import {
    badgesFor,
    cancelPull,
    coachSetupError,
    coachSetupOp,
    coachSetupPulled,
    coachSetupStatus,
    gbText,
    installOllama,
    installPhaseText,
    percentOf,
    pullModel,
    refreshCoachSetup,
    stateErrorKey,
    stepState,
    useModel,
  } from '../../lib/coach-setup';
  import { formatNumber } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';

  // Einrichtung des lokalen Coachs, gemeinsam für Erststart und Einstellungen (T29, SPEC 9.2):
  // Zustand von Ollama, "Jetzt einrichten" (Windows), die Modellliste mit Empfehlung nach der
  // Hardware und der Fortschritt. Installation und Download laufen im Rust-Teil und im
  // gemeinsamen Zustand (lib/coach-setup.ts), sie laufen weiter, wenn der Nutzer diese Ansicht
  // verlässt. Lastwerk zeigt nie einen Pfad oder den Text einer Fehlermeldung von Ollama.
  // Keine Animation außer dem Fortschrittsbalken (transform), der selten zu sehen ist.

  let rechecking = $state(false);

  onMount(() => {
    void refreshCoachSetup();
  });

  const status = $derived($coachSetupStatus);
  const op = $derived($coachSetupOp);
  const step = $derived(stepState(status, op));
  const working = $derived(op.kind !== 'idle');
  const serverUp = $derived(status?.ollama === 'running');
  const cancelledNow = $derived($coachSetupError === 'ui.error.coach_pull_cancelled');

  const pull = $derived(op.kind === 'pull' ? op : null);
  const pullPercent = $derived(
    pull?.progress ? percentOf(pull.progress.completed, pull.progress.total) : null,
  );

  async function recheck() {
    rechecking = true;
    try {
      await refreshCoachSetup();
    } finally {
      rechecking = false;
    }
  }
</script>

<div class="setup" data-testid="coach-setup-block" data-state={step}>
  {#if !status}
    <p class="muted" data-testid="coach-setup-loading">{$t('common.loading')}</p>
  {:else}
    <p class="state-line">
      <span
        class="pill"
        class:ok={status.ollama === 'running'}
        class:warn-pill={status.ollama !== 'running'}
        data-testid="coach-setup-state"
        data-ollama={status.ollama}
      >
        {$t(`ui.coach_setup.state.${status.ollama}`)}
      </span>
      {#if serverUp}
        <span class="model-line" data-testid="coach-setup-model">
          {status.model
            ? $t('ui.coach_setup.model_active', { model: status.model })
            : $t('ui.coach_setup.no_model')}
        </span>
      {/if}
    </p>

    <p class="muted hardware" data-testid="coach-setup-hardware">
      {status.hardware.vram_gb === null
        ? $t('ui.coach_setup.hardware', { ram: formatNumber(status.hardware.ram_gb, $locale, 0) })
        : $t('ui.coach_setup.hardware_gpu', {
            ram: formatNumber(status.hardware.ram_gb, $locale, 0),
            vram: formatNumber(status.hardware.vram_gb, $locale, 0),
          })}
    </p>

    {#if status.ollama !== 'running' && status.state?.decision === 'no'}
      <p class="muted" data-testid="coach-setup-declined">{$t('ui.coach_setup.declined')}</p>
    {/if}
    {#if status.ollama !== 'running' && status.state?.error_code && !status.state.model_ready}
      <p class="warn" data-testid="coach-setup-last-error">
        {$t('ui.coach_setup.last_error')}
        {$t(stateErrorKey(status.state.error_code))}
      </p>
    {/if}

    {#if step === 'installable'}
      <div class="install" data-testid="coach-setup-install">
        <p class="muted">{$t('ui.coach_setup.install_hint')}</p>
        <p>
          <button
            class="primary"
            type="button"
            disabled={working}
            onclick={() => void installOllama()}
            data-testid="coach-setup-install-button">{$t('ui.coach_setup.install')}</button
          >
        </p>
      </div>
    {:else if step === 'manual'}
      <div class="manual" data-testid="coach-setup-manual">
        <p class="label">{$t('ui.coach_setup.manual.title')}</p>
        <ol>
          <li>{$t('ui.coach_setup.manual.step_install')}</li>
          <li>{$t('ui.coach_setup.manual.step_start')}</li>
          <li>{$t('ui.coach_setup.manual.step_pick')}</li>
        </ol>
        <p>
          <button
            type="button"
            disabled={rechecking || working}
            onclick={() => void recheck()}
            data-testid="coach-setup-recheck"
          >
            {rechecking ? $t('ui.coach_setup.checking') : $t('ui.coach_setup.recheck')}
          </button>
        </p>
      </div>
    {:else if step === 'installing'}
      <div class="installing" role="status" aria-live="polite" data-testid="coach-setup-installing">
        <p>
          {op.kind === 'install' && op.progress
            ? $t(...installPhaseText(op.progress))
            : $t('ui.coach_setup.installing')}
        </p>
        <div
          class="bar"
          role="progressbar"
          aria-label={$t('ui.coach_setup.installing')}
          aria-valuemin="0"
          aria-valuemax="100"
          aria-valuenow={op.kind === 'install' ? (op.progress?.percent ?? undefined) : undefined}
        >
          <span
            class="fill"
            class:indeterminate={!(op.kind === 'install' && op.progress?.percent != null)}
            style:transform={op.kind === 'install' && op.progress?.percent != null
              ? `scaleX(${op.progress.percent / 100})`
              : undefined}
          ></span>
        </div>
      </div>
    {/if}

    {#if step === 'ready' && !$coachSetupPulled}
      <p class="ok-line" data-testid="coach-setup-ready">{$t('ui.coach_setup.ready')}</p>
    {/if}
    {#if $coachSetupPulled}
      <p class="ok-line" role="status" data-testid="coach-setup-pulled">
        {$t('ui.coach_setup.pulled', { name: $coachSetupPulled })}
      </p>
    {/if}

    {#if $coachSetupError}
      <p class={cancelledNow ? 'muted' : 'error'} role="alert" data-testid="coach-setup-error">
        {$t($coachSetupError)}
      </p>
    {/if}

    {#if status.catalog.length > 0}
      <h4 class="label models-title">{$t('ui.coach_setup.models_title')}</h4>
      <ul class="models" data-testid="coach-setup-models">
        {#each status.catalog as entry (entry.name)}
          {@const badges = badgesFor(entry, serverUp ? status.model : null)}
          {@const isPulling = pull?.name === entry.name}
          <li
            class="model"
            class:recommended={badges.includes('recommended')}
            data-testid={`coach-model-${entry.name}`}
            data-fit={entry.fit}
            data-recommended={entry.recommended && entry.fit === 'fits' ? 'true' : 'false'}
          >
            <div class="info">
              <p class="name">
                <span class="id">{entry.name}</span>
                <span class="size num"
                  >{$t('ui.coach_setup.size', {
                    size: formatNumber(entry.size_gb, $locale, 1),
                  })}</span
                >
                {#each badges as badge (badge)}
                  <span class={`pill badge ${badge}`} data-testid={`badge-${badge}`}>
                    {$t(`ui.coach_setup.badge.${badge}`)}
                  </span>
                {/each}
              </p>
              <p class="muted label-text">{$t(entry.label_key)}</p>
              {#if entry.fit !== 'fits'}
                <p class="muted fit-hint" data-testid="fit-hint">
                  {$t(`ui.coach_setup.fit_hint.${entry.fit}`)}
                </p>
              {/if}
              {#if isPulling && pull}
                <div
                  class="pulling"
                  role="status"
                  aria-live="off"
                  data-testid="coach-pull-progress"
                >
                  <div
                    class="bar"
                    role="progressbar"
                    aria-label={$t('ui.coach_setup.pulling', { name: entry.name })}
                    aria-valuemin="0"
                    aria-valuemax="100"
                    aria-valuenow={pullPercent ?? undefined}
                  >
                    <span
                      class="fill"
                      class:indeterminate={pullPercent === null}
                      style:transform={pullPercent === null
                        ? undefined
                        : `scaleX(${pullPercent / 100})`}
                    ></span>
                  </div>
                  <p class="muted progress-text">
                    {#if pull.progress && pull.progress.total > 0}
                      <span class="num" data-testid="coach-pull-numbers"
                        >{$t('ui.coach_setup.progress', {
                          done: gbText(pull.progress.completed, $locale),
                          total: gbText(pull.progress.total, $locale),
                        })}
                        ({pullPercent} %)</span
                      >
                      ·
                    {/if}
                    {$t(`ui.coach_setup.pull_phase.${pull.progress?.phase ?? 'manifest'}`)}
                  </p>
                  <p class="muted progress-text">{$t('ui.coach_setup.background')}</p>
                </div>
              {/if}
            </div>
            <div class="actions">
              {#if isPulling}
                <button
                  type="button"
                  onclick={() => void cancelPull()}
                  data-testid="coach-pull-cancel">{$t('ui.coach_setup.cancel')}</button
                >
              {:else if entry.installed && serverUp}
                {#if entry.name !== status.model}
                  <button
                    type="button"
                    disabled={working}
                    onclick={() => void useModel(entry.name)}
                    data-testid={`coach-use-${entry.name}`}>{$t('ui.coach_setup.use')}</button
                  >
                {/if}
              {:else}
                <button
                  type="button"
                  class:primary={badges.includes('recommended') && step === 'no_model'}
                  disabled={working || !serverUp}
                  onclick={() => void pullModel(entry.name)}
                  data-testid={`coach-load-${entry.name}`}>{$t('ui.coach_setup.load')}</button
                >
              {/if}
            </div>
          </li>
        {/each}
      </ul>
      <p class="muted note">{$t('ui.coach_setup.fits_all')}</p>
    {/if}
  {/if}
</div>

<style>
  .setup {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }

  .setup p {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .state-line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
  }

  .pill.ok {
    color: var(--ok);
  }

  .pill.warn-pill {
    color: var(--warn);
  }

  .model-line {
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .hardware {
    font-size: var(--fs-small);
  }

  .ok-line {
    color: var(--ok);
    font-size: var(--fs-dense);
  }

  .install p,
  .manual p,
  .installing p {
    font-size: var(--fs-dense);
  }

  .manual ol {
    margin: var(--space-2) 0 var(--space-3);
    padding-left: 20px;
    font-size: var(--fs-dense);
  }

  .label,
  .models-title {
    margin: var(--space-3) 0 0;
    font-size: var(--fs-label);
    font-weight: var(--fw-bold);
    letter-spacing: var(--ls-label);
    text-transform: uppercase;
    color: var(--muted);
  }

  /* Modellliste: Zeilen mit feinen Linien, kein Kachelraster (SPEC 5.5). */
  .models {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--line);
  }

  .model {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-2) var(--space-4);
    padding: var(--space-3) 0;
    border-bottom: 1px solid var(--line);
  }

  .info {
    flex: 1 1 260px;
    min-width: 0;
  }

  .actions {
    flex: none;
  }

  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
  }

  .id {
    font-weight: var(--fw-bold);
    font-size: var(--fs-body);
    overflow-wrap: anywhere;
  }

  .size {
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .label-text,
  .fit-hint,
  .progress-text,
  .note {
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
  }

  .badge {
    font-size: var(--fs-label);
    padding: 1px 10px;
  }

  .badge.recommended {
    background: var(--accent);
    color: var(--accent-text);
  }

  .badge.slow {
    background: var(--form-loaded-bg);
    color: var(--form-loaded);
  }

  .badge.too_big {
    background: var(--form-overloaded-bg);
    color: var(--form-overloaded);
  }

  .badge.active {
    background: var(--form-fresh-bg);
    color: var(--form-fresh);
  }

  .pulling {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin-top: var(--space-2);
  }

  /* Fortschritt: nur transform, kein Layout (SPEC 5.5). */
  .bar {
    position: relative;
    height: 6px;
    overflow: hidden;
    border-radius: var(--radius-pill);
    background: var(--panel-2);
  }

  .fill {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: var(--radius-pill);
    background: var(--accent);
    transform-origin: left center;
    transition: transform 250ms linear;
  }

  .fill.indeterminate {
    width: 35%;
    transform: none;
    animation: slide 1.4s ease-in-out infinite;
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
</style>
