<script lang="ts">
  import type { Dashboard } from '../../lib/backend/types';
  import Section from '../../lib/components/Section.svelte';
  import { insightStatement } from '../../lib/dashboard';
  import { locale, t } from '../../lib/i18n';
  import { openCoach } from '../../lib/state';

  // Coach-Erkenntnisse (T44): höchstens drei Aussagen aus den Zustandscodes, Reihenfolge aus
  // `metrics::insights` (Faustregel). Jede Zeile öffnet den Coach mit Thema und vorbereiteter
  // Frage; gesendet wird nur, was der Nutzer selbst abschickt.
  let { dash }: { dash: Dashboard } = $props();

  // Ohne Last und ohne Erholungswerte wäre "nichts fällt auf" eine Aussage über Daten, die es
  // nicht gibt; dann sagt der Abschnitt, dass sie fehlen.
  const noData = $derived(dash.form === null && dash.readiness.state === 'readiness.no_data');
</script>

<Section
  label={$t('ui.insight.title')}
  statement={dash.insights.length > 0
    ? undefined
    : noData
      ? $t('ui.insight.no_data')
      : $t('ui.insight.empty')}
  testid="today-insights"
>
  {#if dash.insights.length > 0}
    <ul class="insights">
      {#each dash.insights as item (item.code)}
        {@const text = insightStatement(item.code, dash, $t, $locale)}
        <li data-testid="insight" data-code={item.code}>
          <p>{text}</p>
          <button
            type="button"
            class="ghost"
            aria-label={$t('ui.insight.discuss_label', { text })}
            onclick={() =>
              openCoach({ topic: item.code, prefill: $t(`ui.insight.ask.${item.code}`) })}
          >
            {$t('ui.insight.discuss')}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</Section>

<style>
  .insights {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    padding: var(--space-3) 0;
    border-top: 1px solid var(--line);
  }

  li:first-child {
    border-top: none;
    padding-top: 0;
  }

  p {
    margin: 0;
    max-width: 68ch;
    min-width: 0;
    overflow-wrap: anywhere;
    font-size: var(--fs-body);
  }

  /* Leiser Knopf: gehört zur Zeile, konkurriert nicht mit der Primäraktion der Karte. */
  .ghost {
    flex: none;
    background: transparent;
    border-color: var(--line);
  }
</style>
