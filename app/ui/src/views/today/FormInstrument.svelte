<script lang="ts">
  import type { Dashboard } from '../../lib/backend/types';
  import BandScale from '../../lib/components/BandScale.svelte';
  import Sparkline from '../../lib/components/Sparkline.svelte';
  import { formName } from '../../lib/dashboard';
  import { formatNumber } from '../../lib/format';
  import { garminSync } from '../../lib/garmin';
  import { locale, t } from '../../lib/i18n';
  import { openHistory } from '../../lib/state';

  // Instrument Form (T63, SPEC 5.5): TSB groß, Zustand aus core als Wort, Bandskala, Fitness und
  // Ermüdung, die letzten sechs Wochen TSB als Linie ohne Achsen. Das volle Diagramm mit
  // Prognose steht im Verlauf unter Fortschritt.
  let { dash }: { dash: Dashboard } = $props();

  const form = $derived(dash.form);
  const state = $derived(form ? formName(form.state) : null);
  // Sechs Wochen (42 Tage, Faustregel: lang genug für einen Trend, kurz genug für heute).
  const recent = $derived(dash.pmc.slice(-42).map((p) => p.tsb));
  const minus = (v: string) => v.replace(/^-(?=\d)/, '−');
</script>

<section class="instrument" data-testid="today-form" aria-labelledby="inst-form">
  <h2 class="inst-label" id="inst-form">{$t('ui.instrument.form.title')}</h2>
  {#if form && state}
    <p class="inst-big">
      <span data-testid="form-value">{minus(formatNumber(form.tsb, $locale, 0))}</span>
      <small>TSB</small>
    </p>
    <p class="state-mark {state}" data-testid="form-state">{$t(`ui.form.${state}`)}</p>
    <div class="inst-block">
      <BandScale tsb={form.tsb} {state} testid="form-scale" />
    </div>
    <p class="inst-pair">
      <span><b>{formatNumber(form.ctl, $locale, 0)}</b> {$t('ui.instrument.form.ctl')}</span>
      <span><b>{formatNumber(form.atl, $locale, 0)}</b> {$t('ui.instrument.form.atl')}</span>
    </p>
    {#if recent.length > 1}
      <div class="inst-block">
        <Sparkline
          values={recent}
          baseline={0}
          label={$t('ui.instrument.form.spark_label')}
          color={`var(--form-${state})`}
          testid="form-spark"
        />
      </div>
    {/if}
    <button type="button" class="inst-more" onclick={() => openHistory('progress')}>
      {$t('ui.instrument.form.more')}
    </button>
  {:else}
    <p class="inst-empty">
      {$garminSync.kind === 'running' ? $t('ui.dash.form.loading') : $t('ui.dash.form.empty')}
    </p>
  {/if}
</section>
