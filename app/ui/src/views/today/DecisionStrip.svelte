<script lang="ts">
  import type { DashDecision, DashVariant, Plan } from '../../lib/backend/types';
  import { lastPart } from '../../lib/dashboard';
  import { formatDate, formatNumber } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { ruleInfo, templateDescription, templateName } from '../../lib/plan';
  import { get } from 'svelte/store';
  import { coachOpen, openCoach, paletteOpen, view } from '../../lib/state';
  import { planStartsAfter, sessionsOn } from '../../lib/today';

  // Entscheidungsstreifen (T63, SPEC 5.5 und 8.7): die einzige erhobene Fläche auf "Heute".
  // Links das Urteil und die Einheit, rechts warum. Er zeigt, was core entschieden hat, und
  // erfindet nichts dazu. Alternativen zeigen nur Einzelheiten und ändern den Plan nie; die
  // Primäraktion führt in den Coach, der eine Übernahme vorschlagen kann (SPEC 9.7).
  interface Props {
    decision: DashDecision;
    plan: Plan | null;
    today: string;
    /** Darunter steht der Weg zum Start mit eigener Primäraktion: Der Coach tritt zurück, damit
     *  die Seite nur eine limettengrüne Aktion hat. */
    quiet?: boolean;
  }
  let { decision, plan, today, quiet = false }: Props = $props();

  const code = $derived(lastPart(decision.state));
  const main = $derived(decision.main);
  // Die geplante Einheit, wenn core eine andere empfiehlt: steht durchgestrichen daneben.
  const replaced = $derived(
    main && main.kind !== 'planned'
      ? (decision.alternatives.find((a) => a.kind === 'planned') ?? null)
      : null,
  );
  // Leichter ohne passende Vorlage: Die Empfehlung bleibt leer, der Streifen sagt warum.
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
  const hasWhy = $derived(
    decision.reasons.length > 0 || decision.rules.length > 0 || decision.missing.length > 0,
  );

  let open = $state<number | null>(null);
  $effect(() => {
    // Neue Entscheidung, keine offene Alternative mehr.
    void decision;
    open = null;
  });

  function discuss() {
    openCoach({ topic: decision.state, prefill: $t(`ui.decision.ask.${code}`) });
  }

  // Kürzel B ("besprechen") auf Heute, nicht in Eingabefeldern, nicht bei offener Palette oder
  // offenem Coach (dort gehört die Tastatur dem Gespräch).
  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key.toLowerCase() !== 'b' || e.ctrlKey || e.metaKey || e.altKey || e.defaultPrevented)
      return;
    const target = e.target;
    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement ||
      (target instanceof HTMLElement && target.isContentEditable)
    )
      return;
    if (get(paletteOpen) || get(coachOpen)) return;
    e.preventDefault();
    discuss();
  }

  const meta = (v: DashVariant) =>
    `${$t('ui.today.minutes', { n: v.duration_min })} · ${v.hard ? $t('ui.today.hard') : $t('ui.today.easy')}`;
</script>

<svelte:window onkeydown={onWindowKeydown} />

<section
  class="card strip"
  data-testid="today-decision"
  data-state={code}
  aria-labelledby="decision-heading"
>
  <div class="what">
    {#if main}
      <p class="verdict" id="decision-heading" data-testid="decision-state">
        {$t(`ui.decision.state.${code}`)}
      </p>
      <h2 class="title" data-testid="today-title">{templateName($locale, main.template_id)}</h2>
      <p class="meta" data-testid="decision-meta">
        <span aria-hidden="true"><b>{main.duration_min}</b> {$t('ui.instrument.decision.min')}</span
        >
        <span>{main.hard ? $t('ui.today.hard') : $t('ui.today.easy')}</span>
        <span
          >{$t('ui.instrument.decision.load')}
          <b>{formatNumber(main.planned_load, $locale, 0)}</b></span
        >
        <span class="sr">{$t('ui.today.minutes', { n: main.duration_min })}</span>
      </p>
      {#if replaced}
        <p class="replaced" data-testid="decision-replaced">
          {$t('ui.instrument.decision.instead')}
          <s
            >{templateName($locale, replaced.template_id)}, {$t('ui.today.minutes', {
              n: replaced.duration_min,
            })}</s
          >
        </p>
      {/if}
    {:else if startsOn}
      <p class="verdict muted" id="decision-heading">{$t('ui.decision.eyebrow')}</p>
      <h2 class="title" data-testid="today-title">
        {$t('ui.today.starts_title', { date: formatDate(startsOn, $locale) })}
      </h2>
      <p class="hint">{$t('ui.today.starts_hint')}</p>
    {:else}
      <p class="verdict muted" id="decision-heading">{$t('ui.decision.eyebrow')}</p>
      <h2 class="title" data-testid="today-title">{$t(`ui.decision.state.${code}`)}</h2>
      <p class="hint" data-testid="decision-hint">
        {$t(lightenEmpty ? 'ui.decision.hint.lighten_empty' : `ui.decision.hint.${code}`)}
      </p>
    {/if}

    {#if others.length > 0}
      <div class="more">
        <p class="label">{$t('ui.today.all_sessions')}</p>
        <ul>
          {#each others as s (s.slot + s.template_id)}
            <li>
              {templateName($locale, s.template_id)}, {$t('ui.today.minutes', {
                n: s.duration_min,
              })}
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>

  <div class="why">
    {#if hasWhy}
      <p class="label">{$t('ui.instrument.decision.why')}</p>
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
        {#each decision.rules as rule (rule)}
          {@const info = ruleInfo($locale, rule)}
          <li class="chip" title={info.explanation ?? ''}>
            <span class="code">{rule}</span>{info.name}
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

    <div class="actions">
      <button
        type="button"
        class:primary={!quiet}
        data-testid="decision-discuss"
        onclick={discuss}
        aria-keyshortcuts="B"
      >
        {$t('ui.decision.discuss')}<kbd aria-hidden="true">B</kbd>
      </button>
      {#if decision.state === 'decision.no_plan'}
        <button type="button" onclick={() => view.set('plan')}>{$t('ui.today.open_plan')}</button>
      {/if}
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
    {#if decision.alternatives.length > 0}
      <div
        class="alts"
        data-testid="decision-alternatives"
        aria-label={$t('ui.decision.alternatives')}
      >
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
  </div>
</section>

<style>
  .strip {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
    gap: var(--space-5) var(--space-7);
    padding: var(--space-5) var(--space-6);
    overflow: hidden;
  }

  /* Limettenkante: dieser Streifen ist das, was jetzt zählt. */
  .strip::before {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--accent);
  }

  .what,
  .why {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .verdict,
  .label {
    margin: 0;
    font-size: var(--fs-label);
    line-height: var(--lh-tight);
    font-weight: var(--fw-semibold);
    letter-spacing: var(--ls-label);
    text-transform: uppercase;
    color: var(--muted);
  }

  .verdict {
    color: var(--accent);
  }

  .verdict.muted {
    color: var(--muted);
  }

  .title {
    margin: 0;
    max-width: 22ch;
    overflow-wrap: anywhere;
    text-wrap: balance;
    font-size: var(--fs-display);
    line-height: 1.05;
    font-weight: var(--fw-semibold);
    letter-spacing: var(--ls-display);
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-5);
    margin: var(--space-1) 0 0;
    color: var(--text-2);
  }

  .meta b {
    font-size: var(--fs-lead);
    font-weight: var(--fw-semibold);
    font-stretch: var(--num-stretch);
    color: var(--text);
  }

  .replaced,
  .hint {
    margin: 0;
    max-width: var(--measure);
    color: var(--muted);
    overflow-wrap: anywhere;
  }

  .hint {
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
  }

  .replaced s {
    text-decoration-thickness: 1px;
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .reasons,
  .missing {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
    max-width: var(--measure);
    overflow-wrap: anywhere;
  }

  .reasons {
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
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
    padding: 1px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-chip);
    font-size: var(--fs-small);
    color: var(--muted);
    overflow-wrap: anywhere;
  }

  .code {
    font-weight: var(--fw-semibold);
    color: var(--text);
  }

  .more ul {
    margin: 4px 0 0;
    padding-left: 18px;
    color: var(--muted);
  }

  .actions [data-testid='decision-discuss'] {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .alt {
    background: transparent;
  }

  .alt[aria-expanded='true'] {
    border-color: var(--border-strong);
    background: var(--panel-2);
  }

  .alt-detail {
    padding-left: var(--space-3);
    border-left: 2px solid var(--line);
    max-width: var(--measure);
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
     selten genug für Bewegung; bei reduzierter Bewegung nur die Deckkraft. */
  .alt-detail {
    animation: reveal 160ms var(--ease-out);
  }

  @keyframes reveal {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  @container (max-width: 820px) {
    .strip {
      grid-template-columns: minmax(0, 1fr);
      padding: var(--space-5);
    }

    .title {
      font-size: var(--fs-title);
    }
  }
</style>
