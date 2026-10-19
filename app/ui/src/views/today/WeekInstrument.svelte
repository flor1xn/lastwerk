<script lang="ts">
  import WeekStrip from '../../lib/components/WeekStrip.svelte';
  import { formatNumber } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { openHistory } from '../../lib/state';
  import type { WeekStory } from '../../lib/week-story';

  // Instrument Woche (T63, SPEC 5.5): erledigt von geplant bis gestern (Zählung wie im
  // Kalender, T35), Last bisher gegen geplante Last der Woche, sieben Tage als Säulen.
  // Ohne Plan zählt es nur, was gemacht wurde.
  interface Props {
    story: WeekStory;
    today: string;
  }
  let { story, today }: Props = $props();

  const fmt = (v: number) => formatNumber(v, $locale, 0);
</script>

<section class="instrument" data-testid="today-week-story" aria-labelledby="inst-week">
  <h2 class="inst-label" id="inst-week">{$t('ui.week_story.title')}</h2>
  {#if story.hasPlan && story.plannedPast === 0}
    <!-- Erster Tag der Woche: Bis gestern war nichts geplant, also zählt die Woche voraus. -->
    <p class="inst-big" data-testid="week-done">
      {story.plannedSessions}<small>{$t('ui.instrument.week.sessions_planned')}</small>
    </p>
    <p class="inst-pair" data-testid="week-story-adherence">
      <span
        ><b>{fmt(story.load)}</b>
        {$t('ui.instrument.week.load_of', { planned: fmt(story.plannedLoad) })}</span
      >
    </p>
  {:else if story.hasPlan}
    <p class="inst-big" data-testid="week-done">
      {story.donePast}<span class="of">/{story.plannedPast}</span>
      <small>{$t('ui.instrument.week.sessions_until_yesterday')}</small>
    </p>
    <p class="inst-pair" data-testid="week-story-adherence">
      <span
        ><b>{fmt(story.load)}</b>
        {$t('ui.instrument.week.load_of', { planned: fmt(story.plannedLoad) })}</span
      >
      {#if story.missedDays > 0}
        <span class="missed"><b>{story.missedDays}</b> {$t('ui.instrument.week.missed')}</span>
      {/if}
    </p>
  {:else if story.sessions > 0}
    <p class="inst-big" data-testid="week-done">
      {story.sessions}<small>{$t('ui.instrument.week.sessions')}</small>
    </p>
    <p class="inst-pair" data-testid="week-story-adherence">
      <span><b>{fmt(story.load)}</b> {$t('ui.instrument.week.load')}</span>
      <span>{$t('ui.instrument.week.no_plan')}</span>
    </p>
  {:else}
    <p class="inst-empty">{$t('ui.week_story.statement.empty')}</p>
  {/if}
  <div class="inst-block">
    <WeekStrip days={story.days} {today} testid="week-strip" />
  </div>
  <button type="button" class="inst-more" onclick={() => openHistory('calendar')}>
    {$t('ui.instrument.week.more')}
  </button>
</section>

<style>
  .missed {
    color: var(--warn);
  }

  .missed b {
    color: var(--warn);
  }
</style>
