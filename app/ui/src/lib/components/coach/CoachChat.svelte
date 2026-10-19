<script lang="ts">
  import { tick } from 'svelte';
  import { getBackend } from '../../backend';
  import type { CoachStatus, PlanListItem, PromptPreview } from '../../backend/types';
  import ErrorNote from '../ErrorNote.svelte';
  import { COUNTER_FROM, MAX_MESSAGE_CHARS, ruleLabel } from '../../coach';
  import { coachSession, type CoachMsg } from '../../coach-session.svelte';
  import { errorKey } from '../../errors';
  import { locale, t } from '../../i18n';
  import { Loader } from '../../loader.svelte';
  import {
    bumpData,
    coachFocus,
    coachReleased,
    dataVersion,
    openSettings,
    selectedPlanId,
  } from '../../state';
  import ProposalCard from './ProposalCard.svelte';

  // Gespräch mit dem lokalen Coach (T26, SPEC 9.7), seit T43 im Seitenfenster aus jeder Ansicht.
  // Das Modell macht nur Vorschläge, die der Rust-Teil geprüft hat; übernommen wird erst auf
  // "Übernehmen". Antworten sind Anzeigetext und erscheinen nur als Text, nie als HTML oder
  // Markdown (SPEC 9.5). Verlauf und Entwurf stehen in `coach-session.svelte.ts` und überstehen
  // das Schließen des Fensters; mit jeder Nachricht geht der Fokus der Ansicht mit (nur Codes).

  const session = coachSession;
  const status = new Loader<CoachStatus>();
  const plans = new Loader<PlanListItem[]>();
  let rechecking = $state(false);
  let modelError = $state<string | null>(null);
  let listEl = $state<HTMLElement | null>(null);
  let inputEl = $state<HTMLTextAreaElement | null>(null);

  // Vorschau des Eingangs (F24).
  let seen = $state(false);
  let seenData = $state<PromptPreview | null>(null);
  let seenError = $state<unknown>(null);
  let seenLoading = $state(false);
  let seenToken = 0;

  $effect(() => {
    void status.run(() => getBackend().coachStatus());
  });

  // Das Gespräch braucht einen aktiven Plan oder einen Entwurf, wie der Rust-Teil ihn wählt.
  // Neu gelesen, wenn sich Daten ändern oder ein anderer Plan gewählt ist (etwa ein eben
  // erzeugter).
  $effect(() => {
    void $dataVersion;
    void $selectedPlanId;
    void plans.run(() => getBackend().listPlans());
  });
  const hasPlan = $derived((plans.data ?? []).some((p) => p.status !== 'archived'));

  /** Zur Einrichtung in den Einstellungen (T29): Ollama einrichten, Modell wählen. */
  function openSetup() {
    openSettings('coach');
  }

  async function recheck() {
    rechecking = true;
    try {
      await status.run(() => getBackend().coachStatus());
    } finally {
      rechecking = false;
    }
  }

  async function chooseModel(model: string) {
    modelError = null;
    try {
      status.data = await getBackend().coachSetModel(model);
    } catch (e) {
      modelError = errorKey(e);
    }
  }

  const count = $derived([...session.draft].length);
  const canSend = $derived(
    hasPlan && !session.sending && session.draft.trim() !== '' && count <= MAX_MESSAGE_CHARS,
  );
  const chatError = $derived(modelError ?? session.error);

  function scrollDown() {
    void tick().then(() => {
      if (listEl) listEl.scrollTop = listEl.scrollHeight;
    });
  }

  $effect(() => {
    void session.messages.length;
    void session.sending;
    void listEl;
    scrollDown();
  });

  async function send() {
    if (!canSend) return;
    modelError = null;
    const result = await session.send($coachFocus);
    if (result === 'unavailable') void recheck();
  }

  async function accept(m: CoachMsg) {
    const detail = await session.accept(m);
    if (!detail) return;
    // Der neue Plan ist aktiv: Die Planansicht zeigt ihn, alle Ansichten laden neu.
    selectedPlanId.set(detail.plan.plan_id);
    bumpData();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      void send();
    }
  }

  function suggest(text: string) {
    session.draft = text;
    inputEl?.focus();
  }

  async function loadSeen() {
    const mine = ++seenToken;
    seenLoading = true;
    try {
      const data = await getBackend().coachPreview(session.draft, $coachFocus);
      if (mine === seenToken) {
        seenData = data;
        seenError = null;
      }
    } catch (e) {
      if (mine === seenToken) {
        seenError = e;
        seenData = null;
      }
    } finally {
      if (mine === seenToken) seenLoading = false;
    }
  }

  // Solange die Vorschau offen ist, folgt sie Eingabefeld, Verlauf und Fokus.
  $effect(() => {
    if (!seen) return;
    void session.draft;
    void session.messages.length;
    void $coachFocus;
    const timer = setTimeout(() => void loadSeen(), 250);
    return () => clearTimeout(timer);
  });

  const suggestions = ['busy_day', 'more_running', 'low_form'] as const;
  const st = $derived(status.data);
</script>

<div class="coach-chat" data-testid="coach">
  {#if st?.available && (st.models.length > 1 || st.model || session.messages.length > 0)}
    <div class="head">
      <div class="tools">
        {#if st.models.length > 1}
          <label class="model">
            <span class="sr">{$t('ui.coach.model')}</span>
            <select
              value={st.model ?? ''}
              onchange={(e) => void chooseModel(e.currentTarget.value)}
              data-testid="coach-model"
            >
              {#each st.models as m (m)}
                <option value={m}>{m}</option>
              {/each}
            </select>
          </label>
        {:else if st.model}
          <span class="model-name" data-testid="coach-model-name" title={$t('ui.coach.model')}
            >{st.model}</span
          >
        {/if}
        {#if session.messages.length > 0}
          <button
            class="quiet"
            type="button"
            onclick={() => void session.reset()}
            data-testid="coach-reset"
          >
            {$t('ui.coach.reset')}
          </button>
        {/if}
      </div>
    </div>
  {/if}
  <p class="statement">{$t('ui.coach.statement')}</p>

  {#if status.error}
    <ErrorNote error={status.error} />
  {:else if !st}
    <p class="muted small" data-testid="coach-checking">{$t('ui.coach.checking')}</p>
  {:else if !st.available}
    <div class="setup" data-testid="coach-setup" data-reason={st.reason ?? ''}>
      <h4>{$t('ui.coach.setup.title')}</h4>
      <p>{$t('ui.coach.setup.lead')}</p>
      {#if st.reason}
        <p class="reason" data-testid="coach-setup-reason">
          {$t(`ui.coach.setup.reason.${st.reason}`, { url: st.base_url })}
        </p>
      {/if}
      <ol>
        <li>{$t('ui.coach.setup.step_install')}</li>
        <li>
          {$t('ui.coach.setup.step_pull')}
          <code>{$t('ui.coach.setup.command')}</code>
        </li>
        <li>{$t('ui.coach.setup.step_check')}</li>
      </ol>
      <div class="setup-actions">
        <button type="button" class="primary" onclick={openSetup} data-testid="coach-open-setup">
          {$t('ui.coach.setup.open_settings')}
        </button>
        <button type="button" disabled={rechecking} onclick={recheck} data-testid="coach-recheck">
          {rechecking ? $t('ui.coach.setup.checking') : $t('ui.coach.setup.recheck')}
        </button>
      </div>
    </div>
  {:else}
    {#if $coachReleased}
      <div
        class="list"
        role="log"
        aria-label={$t('ui.coach.title')}
        aria-live="polite"
        bind:this={listEl}
        data-testid="coach-list"
      >
        {#if session.messages.length === 0 && !session.sending}
          <div class="empty" data-testid="coach-empty">
            <p class="empty-title">{$t('ui.coach.empty_title')}</p>
            <p class="muted small">{$t('ui.coach.empty_hint')}</p>
            <div class="chips">
              {#each suggestions as s (s)}
                <button
                  type="button"
                  class="chip"
                  disabled={!hasPlan}
                  onclick={() => suggest($t(`ui.coach.suggestion.${s}`))}
                  data-testid={`coach-suggestion-${s}`}
                >
                  {$t(`ui.coach.suggestion.${s}`)}
                </button>
              {/each}
            </div>
          </div>
        {/if}
        {#each session.messages as m (m.id)}
          <div class="row {m.role}" data-testid="coach-message" data-role={m.role}>
            <span class="sr">{m.role === 'user' ? $t('ui.coach.you') : $t('ui.coach.coach')}:</span>
            <!-- Text des Nutzers und des Modells: als Text, nie als HTML (SPEC 9.5). -->
            <p class="bubble" data-testid="coach-text">{m.text}</p>
            {#if m.rejected.length > 0}
              <p class="rejected" data-testid="coach-rejected">
                {$t('ui.coach.proposal.rejected', {
                  codes: m.rejected.map((c) => ruleLabel($locale, c)).join(', '),
                })}
              </p>
            {/if}
            {#if m.proposal}
              <ProposalCard
                proposal={m.proposal}
                state={m.pstate}
                error={m.perror}
                onaccept={() => void accept(m)}
                ondismiss={() => session.dismiss(m)}
              />
            {/if}
          </div>
        {/each}
        {#if session.sending}
          <div class="row coach" data-testid="coach-thinking" aria-busy="true">
            <p class="bubble thinking">
              <span class="dots" aria-hidden="true"><i></i><i></i><i></i></span>
              {$t('ui.coach.thinking')}
            </p>
            <button
              class="quiet"
              type="button"
              onclick={() => void session.cancel()}
              data-testid="coach-cancel"
            >
              {$t('ui.coach.cancel')}
            </button>
          </div>
        {/if}
      </div>

      {#if chatError}
        <p class="error small" role="alert" data-testid="coach-error">{$t(chatError)}</p>
      {/if}
      {#if !hasPlan}
        <p class="muted small" data-testid="coach-no-plan">{$t('ui.error.no_plan')}</p>
      {/if}

      <div class="composer">
        <label class="sr" for="coach-input">{$t('ui.coach.placeholder')}</label>
        <textarea
          id="coach-input"
          rows="2"
          maxlength={MAX_MESSAGE_CHARS}
          placeholder={$t('ui.coach.placeholder')}
          bind:value={session.draft}
          bind:this={inputEl}
          {onkeydown}
          disabled={!hasPlan}
          data-testid="coach-input"></textarea>
        <div class="send-row">
          <span class="counter" class:near={count >= COUNTER_FROM} data-testid="coach-counter">
            {#if count >= COUNTER_FROM}
              {$t('ui.coach.counter', { n: count, max: MAX_MESSAGE_CHARS })}
            {/if}
          </span>
          <button type="button" disabled={!canSend} onclick={send} data-testid="coach-send">
            {$t('ui.coach.send')}
          </button>
        </div>
      </div>
    {:else}
      <!-- Einmalige Freigabe je Sitzung (SPEC 9.7, 4): Was zum Modell geht, ist einsehbar. -->
      <div class="consent" data-testid="coach-consent">
        <h4>{$t('ui.coach.consent.title')}</h4>
        <p>{$t('ui.coach.consent.text', { model: st.model ?? '' })}</p>
        <button
          class="primary"
          type="button"
          onclick={() => coachReleased.set(true)}
          data-testid="coach-consent-accept"
        >
          {$t('ui.coach.consent.accept')}
        </button>
      </div>
    {/if}

    <div class="seen-row">
      <button
        class="link"
        type="button"
        aria-expanded={seen}
        onclick={() => (seen = !seen)}
        data-testid="coach-seen"
      >
        {seen ? $t('ui.coach.seen_close') : $t('ui.coach.seen')}
      </button>
    </div>
    {#if seen}
      <section class="seen" data-testid="coach-preview">
        <h4>{$t('ui.coach.preview.title')}</h4>
        {#if seenError}
          <ErrorNote error={seenError} />
        {:else if !seenData}
          <p class="muted small">{$t('ui.coach.preview.loading')}</p>
        {:else}
          <p class="small" data-testid="coach-preview-target">
            {$t('ui.coach.preview.target', {
              provider: seenData.provider.name,
              url: seenData.provider.base_url,
              model: seenData.provider.model ?? $t('ui.coach.preview.model_auto'),
            })}
          </p>
          <p class="muted small">{$t('ui.coach.preview.note')}</p>
          {#each seenData.messages as m, i (i)}
            <div class="pmsg" data-testid="coach-preview-message" data-role={m.role}>
              <p class="prole">{$t(`ui.coach.preview.role.${m.role}`)}</p>
              <pre>{m.content}</pre>
            </div>
          {/each}
        {/if}
        {#if seenLoading && seenData}<span class="sr" aria-live="polite"
            >{$t('ui.coach.preview.loading')}</span
          >{/if}
      </section>
    {/if}
  {/if}
</div>

<style>
  .coach-chat {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-bottom: var(--space-1);
  }

  .tools {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .model select {
    min-height: 32px;
    max-width: 14rem;
    padding: 2px 8px;
    font-size: var(--fs-small);
  }

  .model-name {
    max-width: 14rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .statement {
    margin: var(--space-1) 0 var(--space-3);
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
    color: var(--muted);
    text-wrap: pretty;
  }

  .small {
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
    margin: var(--space-1) 0;
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  button.quiet {
    min-height: 32px;
    padding: 2px 10px;
    background: none;
    border-color: transparent;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  button.link {
    min-height: 32px;
    padding: 2px 0;
    background: none;
    border: 0;
    color: var(--muted);
    font-size: var(--fs-small);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  @media (hover: hover) and (pointer: fine) {
    button.quiet:hover:not(:disabled) {
      color: var(--text);
    }

    button.link:hover:not(:disabled) {
      border-color: transparent;
      color: var(--text);
    }
  }

  /* Einrichtung ohne Modell. */
  .setup h4 {
    margin: 0 0 var(--space-2);
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
  }

  .setup p {
    margin: 0 0 var(--space-2);
    font-size: var(--fs-body);
  }

  .setup .reason {
    padding: var(--space-2) var(--space-3);
    background: var(--panel-2);
    border-radius: 10px;
  }

  .setup-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .setup ol {
    margin: var(--space-3) 0;
    padding-left: 20px;
    font-size: var(--fs-body);
  }

  .setup li {
    margin-bottom: var(--space-1);
  }

  code {
    display: inline-block;
    margin-top: 2px;
    padding: 1px 8px;
    border-radius: 8px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    font-family: ui-monospace, 'SFMono-Regular', Menlo, Consolas, monospace;
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }

  /* Einmalige Freigabe je Sitzung. */
  .consent {
    padding: var(--space-3) 0 var(--space-2);
  }

  .consent h4 {
    margin: 0 0 var(--space-2);
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
  }

  .consent p {
    margin: 0 0 var(--space-3);
    font-size: var(--fs-body);
    line-height: var(--lh-body);
    text-wrap: pretty;
  }

  /* Verlauf. */
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    flex: 1 1 auto;
    min-height: 14rem;
    overflow-y: auto;
    padding: var(--space-1) 2px var(--space-2);
    overscroll-behavior: contain;
  }

  .empty {
    margin: auto 0;
    padding: var(--space-3) 0;
  }

  .empty-title {
    margin: 0;
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
    font-weight: var(--fw-semibold);
  }

  .chips {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }

  .chip {
    max-width: 100%;
    text-align: left;
    border-radius: 16px;
    font-size: var(--fs-body);
    line-height: var(--lh-tight);
    padding: 6px 14px;
    overflow-wrap: anywhere;
  }

  .row {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: var(--space-1);
  }

  .row.user {
    align-items: flex-end;
  }

  .row.coach {
    align-items: stretch;
  }

  .bubble {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    max-width: 92%;
    font-size: var(--fs-body);
    line-height: var(--lh-body);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .user .bubble {
    background: var(--panel-2);
    border-radius: 16px 16px 4px 16px;
  }

  .coach .bubble {
    max-width: 100%;
    padding-left: 0;
    border-radius: 0;
  }

  .rejected {
    margin: 0;
    padding-left: var(--space-3);
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
    color: var(--warn);
    overflow-wrap: anywhere;
  }

  .thinking {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--muted);
  }

  .dots {
    display: inline-flex;
    gap: 3px;
  }

  .dots i {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: currentColor;
    animation: dot 1.1s ease-in-out infinite;
  }

  .dots i:nth-child(2) {
    animation-delay: 160ms;
  }

  .dots i:nth-child(3) {
    animation-delay: 320ms;
  }

  @keyframes dot {
    0%,
    80%,
    100% {
      opacity: 0.25;
    }
    40% {
      opacity: 1;
    }
  }

  .row.coach button.quiet {
    align-self: flex-start;
    margin-left: var(--space-2);
  }

  /* Eingabe. */
  .composer {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-2);
    padding-top: var(--space-3);
    border-top: 1px solid var(--line);
  }

  textarea {
    width: 100%;
    min-height: 3.4em;
    padding: var(--space-2) var(--space-3);
    border-radius: 12px;
    resize: none;
    font-size: var(--fs-body);
    line-height: var(--lh-tight);
  }

  textarea:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .send-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .counter {
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .counter.near {
    color: var(--warn);
  }

  .seen-row {
    margin-top: var(--space-1);
  }

  /* Vorschau des Eingangs. */
  .seen {
    flex: none;
    max-height: 30vh;
    overflow: auto;
    margin-top: var(--space-2);
    padding: var(--space-3);
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--line);
  }

  .seen h4 {
    margin: 0 0 var(--space-1);
    font-size: var(--fs-label);
    letter-spacing: var(--ls-label);
    text-transform: uppercase;
    color: var(--muted);
  }

  .pmsg {
    margin-top: var(--space-2);
  }

  .prole {
    margin: 0;
    font-size: var(--fs-small);
    font-weight: var(--fw-semibold);
  }

  pre {
    margin: 2px 0 0;
    max-height: 12rem;
    overflow: auto;
    padding: var(--space-2);
    border-radius: 8px;
    background: var(--panel);
    font-family: ui-monospace, 'SFMono-Regular', Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.45;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
