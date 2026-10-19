<script lang="ts">
  import type { Dashboard, InsightCode } from '../../lib/backend/types';
  import { insightStatement } from '../../lib/dashboard';
  import { formatWeekday } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { openCoach, view } from '../../lib/state';
  import type { WeekStory } from '../../lib/week-story';

  // Notizen unter den Instrumenten (T63, SPEC 5.5): höchstens drei Zeilen. Zuerst Erkenntnisse
  // aus core (Reihenfolge aus metrics::insights, Faustregel), ohne die, die ein Instrument schon
  // zeigt (jede Aussage an einer Stelle), dann der Ausblick auf die nächsten sieben Tage.
  interface Props {
    dash: Dashboard;
    story: WeekStory;
    hasPlan: boolean;
  }
  let { dash, story, hasPlan }: Props = $props();

  const SHOWN_BY_INSTRUMENT: InsightCode[] = [
    'form.overloaded',
    'readiness.low',
    'adherence.under',
    'adherence.over',
  ];
  const MAX = 3;

  const insights = $derived(
    dash.insights.filter((i) => !SHOWN_BY_INSTRUMENT.includes(i.code)).slice(0, MAX),
  );

  const outlook = $derived.by(() => {
    if (!hasPlan) return null;
    const o = story.outlook;
    if (o.covered === 0) return $t('ui.week_story.outlook.none');
    const base = $t('ui.week_story.outlook.days', { hard: o.hardDays, rest: o.restDays });
    return o.nextHard
      ? `${base} ${$t('ui.week_story.outlook.next_hard', { day: formatWeekday(o.nextHard, $locale) })}`
      : base;
  });

  const hasData = $derived(dash.form !== null || dash.readiness.state !== 'readiness.no_data');
  const quiet = $derived(hasData && dash.insights.length === 0);
</script>

{#if insights.length > 0 || outlook || quiet}
  <section class="notes" data-testid="today-insights" aria-label={$t('ui.insight.title')}>
    <ul>
      {#each insights as item (item.code)}
        {@const text = insightStatement(item.code, dash, $t, $locale)}
        <li data-testid="insight" data-code={item.code}>
          <p>{text}</p>
          <button
            type="button"
            class="link-more"
            aria-label={$t('ui.insight.discuss_label', { text })}
            onclick={() =>
              openCoach({ topic: item.code, prefill: $t(`ui.insight.ask.${item.code}`) })}
          >
            {$t('ui.insight.discuss')}
          </button>
        </li>
      {/each}
      {#if quiet}
        <li data-testid="insight-quiet">
          <p class="muted">{$t('ui.insight.empty')}</p>
        </li>
      {/if}
      {#if outlook && insights.length < MAX}
        <li data-testid="week-story-outlook">
          <p>{outlook}</p>
          <button type="button" class="link-more" onclick={() => view.set('plan')}>
            {$t('ui.today.open_plan')}
          </button>
        </li>
      {/if}
    </ul>
  </section>
{/if}

<style>
  /* Notizen als ruhige Zeilen: Satz links, Verweis rechts, keine Aufzählungszeichen. */
  .notes {
    margin-top: var(--space-4);
    border-top: 1px solid var(--line);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-4);
    min-height: 48px;
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--line);
  }

  p {
    margin: 0;
    max-width: var(--measure);
    overflow-wrap: anywhere;
    color: var(--text-2);
  }
</style>
