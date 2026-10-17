<script lang="ts">
  import type { DashDecision, DashVariant, Plan } from '../../lib/backend/types';
  import { lastPart } from '../../lib/dashboard';
  import { formatDate } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { ruleInfo, templateDescription, templateName } from '../../lib/plan';
  import { openCoach, view } from '../../lib/state';
  import { planStartsAfter, sessionsOn } from '../../lib/today';

  // Tagesentscheidung (T44, SPEC 8.7): die einzige Karte auf "Heute". Sie zeigt, was core
  // entschieden hat, und erfindet nichts dazu. Alternativen zeigen nur Einzelheiten und ändern
  // den Plan nie; die einzige Primäraktion führt in den Coach, der eine Übernahme vorschlagen
  // kann (SPEC 9.7).
  interface Props {
    decision: DashDecision;
    plan: Plan | null;
    today: string;
  }
  let { decision, plan, today }: Props = $props();

  const code = $derived(lastPart(decision.state));
  const main = $derived(decision.main);
  // Leichter ohne passende Vorlage: Die Empfehlung bleibt leer, die Karte sagt warum.
  const lightenEmpty = $derived(decision.state === 'decision.lighten' && main === null);
  const startsOn = $derived(
    decision.state === 'decision.no_plan' && plan ? planStartsAfter(plan, today) : null,
  );
  // Weitere Einheiten des Tages (zweite Einheit am Nachmittag), ohne die maßgebliche.
  const others = $derived.by(() => {
    if (!plan || !main) return [];
    const all = sessionsOn(plan, today);
    const planned = decision.alternatives.find((a) => a.kind === 'planned') ?? main;
    const i = all.findIndex(
      (s) => s.template_id === planned.template_id && s.duration_min === planned.duration_min,
    );
    return i === -1 ? all.slice(1) : all.filter((_, k) => k !== i);
  });

  let open = $state<number | null>(null);
  $effect(() => {
    // Neue Entscheidung, keine offene Alternative mehr.
    void decision;
    open = null;
  });

  function discuss() {
    openCoach({ topic: decision.state, prefill: $t(`ui.decision.ask.${code}`) });
  }

  const meta = (v: DashVariant) =>
    `${$t('ui.today.minutes', { n: v.duration_min })} · ${v.hard ? $t('ui.today.hard') : $t('ui.today.easy')}`;
</script>

<section class="card decision" data-testid="today-decision" data-state={code}>
  <p class="eyebrow">{$t('ui.decision.eyebrow')}</p>
  {#if main}
    <p class="state" data-testid="decision-state">{$t(`ui.decision.state.${code}`)}</p>
    <h2 class="title" data-testid="today-title">{templateName($locale, main.template_id)}</h2>
    <p class="meta" data-testid="decision-meta">
      {$t('ui.today.minutes', { n: main.duration_min })}
      <span class="sep" aria-hidden="true">·</span>
      {main.hard ? $t('ui.today.hard') : $t('ui.today.easy')}
    </p>
  {:else if startsOn}
    <h2 class="title" data-testid="today-title">
      {$t('ui.today.starts_title', { date: formatDate(startsOn, $locale) })}
    </h2>
    <p class="meta muted">{$t('ui.today.starts_hint')}</p>
  {:else}
    <h2 class="title" data-testid="today-title">{$t(`ui.decision.state.${code}`)}</h2>
    <p class="meta muted" data-testid="decision-hint">
      {$t(lightenEmpty ? 'ui.decision.hint.lighten_empty' : `ui.decision.hint.${code}`)}
    </p>
  {/if}

  {#if decision.reasons.length > 0}
    <ul class="reasons" data-testid="decision-reasons">
      {#each decision.reasons as r (r)}
        <li>{$t(`ui.decision.reason.${lastPart(r)}`)}</li>
      {/each}
    </ul>
  {/if}

  {#if decision.rules.length > 0}
    <ul class="rules" aria-label={$t('ui.decision.rules')} data-testid="decision-rules">
      {#each decision.rules as code (code)}
        {@const info = ruleInfo($locale, code)}
        <li class="chip" title={info.explanation ?? ''}>
          <span class="code">{code}</span>{info.name}
        </li>
      {/each}
    </ul>
  {/if}

  {#if decision.missing.length > 0}
    <ul class="missing" data-testid="decision-missing">
      {#each decision.missing as m (m)}
        <li>{$t(`ui.decision.missing.${m}`)}</li>
      {/each}
    </ul>
  {/if}

  {#if others.length > 0}
    <div class="more">
      <p class="eyebrow muted">{$t('ui.today.all_sessions')}</p>
      <ul>
        {#each others as s (s.slot + s.template_id)}
          <li>
            {templateName($locale, s.template_id)},
            {$t('ui.today.minutes', { n: s.duration_min })}
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if decision.alternatives.length > 0}
    <div class="alts" data-testid="decision-alternatives">
      <p class="eyebrow muted">{$t('ui.decision.alternatives')}</p>
      <div class="alt-buttons">
        {#each decision.alternatives as alt, i (alt.kind + alt.template_id)}
          <button
            type="button"
            class="alt"
            aria-expanded={open === i}
            aria-controls="decision-alt-detail"
            data-testid="decision-alt-{alt.kind}"
            onclick={() => (open = open === i ? null : i)}
          >
            {$t(`ui.decision.kind.${alt.kind}`)}
          </button>
        {/each}
      </div>
      {#if open !== null && decision.alternatives[open]}
        {@const alt = decision.alternatives[open]}
        {@const desc = templateDescription($locale, alt.template_id)}
        <div class="alt-detail" id="decision-alt-detail" data-testid="decision-alt-detail">
          <p class="alt-name">{templateName($locale, alt.template_id)}</p>
          <p class="muted">{meta(alt)}</p>
          {#if desc}<p class="muted">{desc}</p>{/if}
          <p class="note">{$t('ui.decision.alt_hint')}</p>
        </div>
      {/if}
    </div>
  {/if}

  <div class="actions">
    <button type="button" class="primary" data-testid="decision-discuss" onclick={discuss}>
      {$t('ui.decision.discuss')}
    </button>
    {#if decision.state === 'decision.no_plan'}
      <button type="button" onclick={() => view.set('plan')}>{$t('ui.today.open_plan')}</button>
    {/if}
  </div>
</section>

<style>
  .decision {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    margin-bottom: var(--space-6);
    padding: var(--space-6) var(--space-6) var(--space-6);
    min-width: 0;
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

  .state {
    margin: 0;
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
    font-weight: var(--fw-semibold);
  }

  .title {
    margin: 0;
    max-width: 20ch;
    overflow-wrap: anywhere;
    text-wrap: balance;
    font-size: var(--fs-display);
    line-height: var(--lh-display);
    font-weight: var(--fw-bold);
    letter-spacing: var(--ls-display);
  }

  .meta {
    margin: 0;
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
    max-width: 52ch;
  }

  .muted,
  .meta.muted {
    color: var(--muted);
  }

  .sep {
    margin: 0 6px;
    color: var(--muted);
  }

  .reasons,
  .missing {
    margin: 0;
    padding: 0;
    list-style: none;
    max-width: 62ch;
    overflow-wrap: anywhere;
  }

  .reasons {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .missing {
    color: var(--muted);
    font-size: var(--fs-small);
  }

  .rules {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .chip {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    max-width: 100%;
    padding: 2px 10px;
    border: 1px solid var(--line);
    border-radius: 999px;
    font-size: var(--fs-small);
    color: var(--muted);
    overflow-wrap: anywhere;
  }

  .code {
    font-weight: var(--fw-bold);
    color: var(--text);
  }

  .more ul {
    margin: 6px 0 0;
    padding-left: 18px;
    color: var(--muted);
  }

  .alts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .alt-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .alt[aria-expanded='true'] {
    border-color: var(--accent);
  }

  .alt-detail {
    padding-left: var(--space-3);
    border-left: 2px solid var(--line);
    max-width: 62ch;
    overflow-wrap: anywhere;
  }

  .alt-detail p {
    margin: 0 0 4px;
  }

  .alt-name {
    font-weight: var(--fw-semibold);
  }

  .note {
    font-size: var(--fs-small);
    color: var(--muted);
  }

  /* Die Einzelheiten blenden kurz ein, damit der Wechsel nicht springt. Einmal je Klick,
     selten genug für Bewegung; aus bei reduzierter Bewegung. */
  @media (prefers-reduced-motion: no-preference) {
    .alt-detail {
      animation: reveal 160ms cubic-bezier(0.23, 1, 0.32, 1);
    }
  }

  @keyframes reveal {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
</style>
