<script lang="ts">
  import type { ProposalPreview } from '../../backend/types';
  import { compareCell, type ProposalState } from '../../coach';
  import { formatDate, formatDayMonth, formatNumber } from '../../format';
  import { locale, t } from '../../i18n';
  import { templateName, WEEKDAYS } from '../../plan';

  // Vorschlag des Coachs (SPEC 9.7, 4): zeigt Änderungen und Vorher und Nachher, ändert aber
  // nichts, bis der Nutzer "Übernehmen" wählt. Alle Texte des Modells erscheinen als Text.
  interface Props {
    proposal: ProposalPreview;
    state: ProposalState;
    /** Fehlerschlüssel einer misslungenen Übernahme. */
    error?: string | null;
    onaccept: () => void;
    ondismiss: () => void;
  }

  let { proposal, state, error = null, onaccept, ondismiss }: Props = $props();

  const slot = (s: 'am' | 'pm') => $t(`ui.coach.proposal.slot_${s}`);
  const summary = $derived(proposal.summary);
  const change = $derived(summary.change);

  const changeLines = $derived.by(() => {
    const c = summary.change;
    if (!c) return [];
    const lines: string[] = [
      $t('ui.coach.proposal.change_start', { date: formatDate(c.week_start, $locale) }),
    ];
    if (c.weeks !== null) lines.push($t('ui.coach.proposal.change_weeks', { n: c.weeks }));
    if (c.strength_per_week !== null) {
      lines.push($t('ui.coach.proposal.change_strength', { n: c.strength_per_week }));
    }
    if (c.sports !== null) {
      lines.push(
        $t('ui.coach.proposal.change_sports', {
          list: c.sports.map((s) => $t(`sport.${s}`)).join(', '),
        }),
      );
    }
    return lines;
  });

  const dayLines = $derived.by(() => {
    const days = summary.change?.availability;
    if (!days) return [];
    return days.map((d) => {
      const day = $t(`ui.planner.weekday_short.${WEEKDAYS[d.weekday]}`);
      if (d.fixed_rest || d.max_minutes === 0)
        return $t('ui.coach.proposal.change_day_rest', { day });
      return $t(
        d.allow_hard ? 'ui.coach.proposal.change_day_minutes' : 'ui.coach.proposal.change_day_easy',
        { day, minutes: d.max_minutes },
      );
    });
  });

  function editText(e: ProposalPreview['summary']['edits'][number]): string {
    const date = formatDate(e.date, $locale);
    if (e.op === 'replace') {
      return $t('ui.coach.proposal.edit_replace', {
        date,
        slot: slot(e.slot),
        template: templateName($locale, e.template_id ?? ''),
        minutes: e.duration_min ?? 0,
      });
    }
    if (e.op === 'move' && e.to_date && e.to_slot) {
      return $t('ui.coach.proposal.edit_move', {
        date,
        slot: slot(e.slot),
        to: formatDate(e.to_date, $locale),
        to_slot: slot(e.to_slot),
      });
    }
    return $t('ui.coach.proposal.edit_remove', { date, slot: slot(e.slot) });
  }

  const int = (n: number) => formatNumber(n, $locale, 0);
  const rows = $derived(
    summary.weeks.map((w) => ({
      week: w.week_start,
      minutes: compareCell(w.before?.minutes, w.after?.minutes, int),
      hard: compareCell(w.before?.hard, w.after?.hard, int),
      load: compareCell(w.before?.load, w.after?.load, int),
      removed: w.after === null,
      added: w.before === null,
    })),
  );
</script>

<article class="card proposal" data-testid="proposal" data-kind={proposal.kind} data-state={state}>
  {#if state === 'dismissed'}
    <p class="muted done" data-testid="proposal-dismissed">{$t('ui.coach.proposal.dismissed')}</p>
  {:else if state === 'applied'}
    <p class="ok done" role="status" data-testid="proposal-applied">
      {$t('ui.coach.proposal.applied')}
    </p>
  {:else if state === 'stale'}
    <p class="muted done" data-testid="proposal-stale">{$t('ui.error.proposal_stale')}</p>
  {:else}
    <p class="eyebrow">{$t(`ui.coach.proposal.${proposal.kind}`)}</p>
    {#if proposal.explanation}
      <!-- Text des Modells: als Text, nie als HTML (SPEC 9.5). -->
      <p class="why" data-testid="proposal-explanation">{proposal.explanation}</p>
    {/if}

    {#if summary.edits.length > 0 || change}
      <h4>{$t('ui.coach.proposal.changes')}</h4>
      <ul class="changes" data-testid="proposal-changes">
        {#each summary.edits as e, i (i)}
          <li>{editText(e)}</li>
        {/each}
        {#each changeLines as line (line)}
          <li>{line}</li>
        {/each}
      </ul>
      {#if dayLines.length > 0}
        <ul class="dayslist" data-testid="proposal-days">
          {#each dayLines as line, i (i)}
            <li>{line}</li>
          {/each}
        </ul>
      {/if}
    {/if}

    <h4>{$t('ui.coach.proposal.compare')}</h4>
    <div class="table-wrap">
      <table data-testid="proposal-compare">
        <thead>
          <tr>
            <th>{$t('ui.coach.proposal.week')}</th>
            <th class="num">{$t('ui.coach.proposal.minutes')}</th>
            <th class="num">{$t('ui.coach.proposal.hard')}</th>
            <th class="num">{$t('ui.coach.proposal.load')}</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as r (r.week)}
            <tr data-testid="proposal-week">
              <td>{formatDayMonth(r.week, $locale)}</td>
              <td class="num" class:changed={r.minutes.changed}>{r.minutes.text}</td>
              <td class="num" class:changed={r.hard.changed}>{r.hard.text}</td>
              <td class="num" class:changed={r.load.changed}>{r.load.text}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if summary.warnings > 0}
      <p class="warn small" data-testid="proposal-warnings">
        {$t('ui.coach.proposal.warnings', { n: summary.warnings })}
      </p>
    {/if}

    {#if error}
      <p class="error small" role="alert" data-testid="proposal-error">{$t(error)}</p>
    {/if}
    <div class="actions">
      <button
        class="primary"
        type="button"
        disabled={state === 'applying'}
        onclick={onaccept}
        data-testid="proposal-accept"
      >
        {state === 'applying' ? $t('ui.coach.proposal.applying') : $t('ui.coach.proposal.accept')}
      </button>
      <button
        type="button"
        disabled={state === 'applying'}
        onclick={ondismiss}
        data-testid="proposal-dismiss"
      >
        {$t('ui.coach.proposal.dismiss')}
      </button>
    </div>
  {/if}
</article>

<style>
  .proposal {
    margin-top: var(--space-3);
    padding: var(--space-4);
    border-radius: var(--radius-panel);
    box-shadow: 0 14px 28px -24px var(--shadow);
    animation: card-in 180ms var(--ease-out);
  }

  @keyframes card-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  .eyebrow {
    margin: 0 0 var(--space-2);
    font-size: var(--fs-label);
    font-weight: var(--fw-bold);
    letter-spacing: var(--ls-label);
    text-transform: uppercase;
    color: var(--accent);
  }

  h4 {
    margin: var(--space-3) 0 var(--space-1);
    font-size: var(--fs-label);
    line-height: var(--lh-tight);
    font-weight: var(--fw-bold);
    letter-spacing: var(--ls-label);
    text-transform: uppercase;
    color: var(--muted);
  }

  .why {
    margin: 0;
    font-size: var(--fs-dense);
    line-height: var(--lh-body);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  ul {
    margin: 0;
    padding-left: 18px;
    font-size: var(--fs-dense);
    line-height: var(--lh-body);
    overflow-wrap: anywhere;
  }

  .dayslist {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(7.5rem, 1fr));
    gap: 0 var(--space-3);
    margin-top: var(--space-1);
    padding-left: 18px;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  .table-wrap {
    max-height: 15rem;
    overflow: auto;
    border-top: 1px solid var(--line);
  }

  table {
    font-size: var(--fs-small);
  }

  th,
  td {
    padding: 4px 6px;
    white-space: nowrap;
  }

  thead th {
    position: sticky;
    top: 0;
    background: var(--panel);
  }

  td.changed {
    font-weight: var(--fw-semibold);
    color: var(--text);
  }

  td:not(.changed).num {
    color: var(--muted);
  }

  .small {
    margin: var(--space-2) 0 0;
    font-size: var(--fs-small);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-4);
  }

  .done {
    margin: 0;
    font-size: var(--fs-small);
  }

  .ok {
    color: var(--ok);
  }
</style>
