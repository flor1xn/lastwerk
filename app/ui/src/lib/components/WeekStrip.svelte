<script lang="ts">
  import { formatNumber, formatWeekday } from '../format';
  import { locale, t } from '../i18n';
  import type { StoryDay } from '../week-story';

  // Sieben Tage der Woche als Säulen (T62, SPEC 5.5). Der Zustand steht als Form, nie nur als
  // Farbe: gemacht voll, geplant gestrichelt, verpasst gestrichelt mit Kreuz, heute mit
  // Limettenrand. Die Höhe folgt der Last des Tages (gemacht) oder der geplanten Last, gemessen am
  // höchsten Wert der Woche; das ist Darstellung, keine Rechnung. Jeder Tag trägt seinen Zustand
  // als Text für Bildschirmleser. Ohne Bewegung: Die Woche ändert sich höchstens einmal am Tag.
  interface Props {
    days: StoryDay[];
    today: string;
    testid?: string;
  }

  let { days, today, testid }: Props = $props();

  const max = $derived(Math.max(1, ...days.map((d) => Math.max(d.load, d.plannedLoad))));

  type Look = 'done' | 'planned' | 'missed' | 'none';
  function look(d: StoryDay): Look {
    if (d.state === 'missed') return 'missed';
    if (d.state === 'done' || d.state === 'partial' || d.state === 'extra' || d.state === 'trained')
      return 'done';
    if (d.state === 'planned') return 'planned';
    return 'none';
  }

  function stateText(d: StoryDay): string {
    switch (d.state) {
      case 'missed':
        return $t('ui.cal.state.missed');
      case 'partial':
        return $t('ui.cal.state.partial');
      case 'extra':
        return $t('ui.cal.state.extra');
      case 'done':
      case 'trained':
        return $t('ui.cal.legend.done');
      case 'planned':
        return d.hard ? $t('ui.week_story.mark.hard') : $t('ui.cal.legend.planned');
      case 'rest':
        return $t('ui.instrument.week.rest');
      default:
        return '';
    }
  }

  const height = (d: StoryDay) => {
    const v = look(d) === 'done' ? d.load : d.plannedLoad;
    // Gemacht ohne berechenbare Last: eine flache Säule, damit der Tag nicht leer wirkt.
    return Math.max(look(d) === 'none' ? 0 : 8, (v / max) * 100);
  };
</script>

<ol class="strip" data-testid={testid}>
  {#each days as d (d.date)}
    {@const l = look(d)}
    {@const text = stateText(d)}
    <li class={l} class:today={d.date === today} data-testid="day" data-state={d.state}>
      <span class="col" aria-hidden="true">
        {#if l !== 'none'}<b style:height="{height(d)}%"></b>{:else}<i></i>{/if}
      </span>
      <span class="day"
        >{formatWeekday(d.date, $locale)}{#if l === 'missed'}<span aria-hidden="true">
            ✕</span
          >{/if}</span
      >
      <span class="sr">
        {text}{#if l === 'done' && d.load > 0}, {$t('ui.instrument.week.load_of_day', {
            load: formatNumber(d.load, $locale, 0),
          })}{/if}
      </span>
    </li>
  {/each}
</ol>

<style>
  .strip {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .col {
    position: relative;
    display: flex;
    align-items: flex-end;
    height: 64px;
  }

  b {
    display: block;
    width: 100%;
    border-radius: 3px;
    background: var(--text-2);
  }

  i {
    display: block;
    width: 100%;
    height: 2px;
    border-radius: 1px;
    background: var(--line);
  }

  .planned b,
  .missed b {
    background: transparent;
    border: 1px dashed var(--muted);
  }

  .today b {
    border: 1.5px solid var(--accent);
  }

  .day {
    font-size: var(--fs-label);
    line-height: 1;
    text-align: center;
    color: var(--muted);
    white-space: nowrap;
  }

  .today .day {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }

  .missed .day {
    color: var(--warn);
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
