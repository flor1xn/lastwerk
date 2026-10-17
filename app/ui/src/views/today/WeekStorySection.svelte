<script lang="ts">
  import type { ActivityItem, DashAdherence, Plan, WeekOverview } from '../../lib/backend/types';
  import Section from '../../lib/components/Section.svelte';
  import WeekBars from '../../lib/components/WeekBars.svelte';
  import { adherenceStatement } from '../../lib/dashboard';
  import { formatNumber, formatWeekday } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { buildWeekStory } from '../../lib/week-story';

  // Wochen-Story (T44): ersetzt "Diese Woche", Planerfüllung und Belastungskalender auf
  // "Heute". Erledigt gegen geplant ist reine Zählung wie im Kalender (T35), die Deutung der
  // Planerfüllung kommt als Code aus core (bis gestern gerechnet), der Ausblick aus dem Plan.
  interface Props {
    today: string;
    week: WeekOverview;
    activities: ActivityItem[];
    plan: Plan | null;
    adherence: DashAdherence;
  }
  let { today, week, activities, plan, adherence }: Props = $props();

  const story = $derived(buildWeekStory(today, activities, plan));

  const statement = $derived.by(() => {
    if (story.hasPlan) {
      return $t('ui.week_story.statement.plan', {
        done: story.donePast,
        planned: story.plannedPast,
        load: formatNumber(story.load, $locale, 0),
        planned_load: formatNumber(story.plannedLoad, $locale, 0),
      });
    }
    if (story.sessions > 0) {
      return $t('ui.week_story.statement.no_plan', {
        n: story.sessions,
        load: formatNumber(story.load, $locale, 0),
      });
    }
    return $t('ui.week_story.statement.empty');
  });

  const outlook = $derived.by(() => {
    if (!plan) return null;
    const o = story.outlook;
    if (o.covered === 0) return $t('ui.week_story.outlook.none');
    const base = $t('ui.week_story.outlook.days', { hard: o.hardDays, rest: o.restDays });
    return o.nextHard
      ? `${base} ${$t('ui.week_story.outlook.next_hard', { day: formatWeekday(o.nextHard, $locale) })}`
      : base;
  });

  // Zustand je Tag als kurzer Text unter dem Datum, nie nur als Farbe.
  const marks = $derived.by(() => {
    const out: Record<string, { text: string; tone?: 'muted' | 'warn' | 'accent' }> = {};
    for (const d of story.days) {
      if (d.state === 'missed') out[d.date] = { text: $t('ui.cal.state.missed'), tone: 'warn' };
      else if (d.state === 'partial') out[d.date] = { text: $t('ui.cal.state.partial') };
      else if (d.state === 'extra') out[d.date] = { text: $t('ui.cal.state.extra') };
      else if (d.state === 'done') out[d.date] = { text: $t('ui.cal.legend.done') };
      else if (d.state === 'planned') {
        out[d.date] = {
          text: d.hard ? $t('ui.week_story.mark.hard') : $t('ui.cal.legend.planned'),
          tone: d.date === today ? 'accent' : 'muted',
        };
      }
    }
    return out;
  });
</script>

<Section label={$t('ui.week_story.title')} {statement} testid="today-week-story">
  {#if story.hasPlan && story.plannedPast > 0}
    <p class="line" data-testid="week-story-adherence">
      {adherenceStatement(adherence, true, $t, $locale)}
      {#if story.missedDays > 0}
        {$t('ui.week_story.missed', { n: story.missedDays })}
      {/if}
    </p>
  {/if}
  {#if outlook}
    <p class="line" data-testid="week-story-outlook">{outlook}</p>
  {/if}
  <WeekBars days={week.days} compact {marks} />
</Section>

<style>
  .line {
    margin: 0 0 var(--space-3);
    max-width: 68ch;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
</style>
