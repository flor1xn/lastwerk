<script lang="ts">
  import type { DashRecoveryValue, Dashboard } from '../../lib/backend/types';
  import DeviationRow from '../../lib/components/DeviationRow.svelte';
  import { readinessStatement, signedPercent } from '../../lib/dashboard';
  import { formatDate, formatNumber } from '../../lib/format';
  import { garminSync } from '../../lib/garmin';
  import { locale, t } from '../../lib/i18n';
  import { openHistory } from '../../lib/state';

  // Instrument Erholung (T63, SPEC 5.5): HRV, Schlaf und Ruhepuls des jüngsten Tages gegen den
  // eigenen Schnitt (Basis aus R12, gerechnet in core). Der Zustand kommt als Code aus core.
  // Beschreibt Trainingsdaten, keine Gesundheit (SPEC 2.3).
  let { dash }: { dash: Dashboard } = $props();

  const r = $derived(dash.readiness);
  const low = $derived(r.state === 'readiness.low');
  const normal = $derived(r.state === 'readiness.normal');

  interface Vital {
    key: 'hrv' | 'sleep' | 'resting_hr';
    value: DashRecoveryValue;
    digits: number;
  }

  const vitals = $derived.by((): Vital[] => {
    const out: Vital[] = [];
    if (r.hrv) out.push({ key: 'hrv', value: r.hrv, digits: 0 });
    if (r.sleep_h) out.push({ key: 'sleep', value: r.sleep_h, digits: 1 });
    if (r.resting_hr) out.push({ key: 'resting_hr', value: r.resting_hr, digits: 0 });
    return out;
  });

  const unitKey = (k: Vital['key']) => `ui.dash.readiness.unit_${k}`;
  // Warnfarbe nur dort, wo Lastwerk selbst "deutlich darunter" sagt (HRV und Schlaf bei
  // readiness.low). Der Ruhepuls hat keine Schwelle und bleibt neutral.
  const tone = (v: Vital) =>
    low && v.key !== 'resting_hr' && (v.value.deviation_percent ?? 0) < 0 ? 'warn' : 'neutral';
  const deviationText = (v: Vital) =>
    v.value.deviation_percent === null
      ? $t('ui.dash.readiness.no_baseline')
      : signedPercent(v.value.deviation_percent, $locale, 0);

  // Ohne Werte sagt das Instrument in einem Satz, was fehlt (gleiche Texte wie bisher).
  const emptyText = $derived(
    r.last_date === null && $garminSync.kind === 'running'
      ? $t('ui.dash.readiness.loading')
      : readinessStatement(r, $t, $locale),
  );
</script>

<section class="instrument" data-testid="today-readiness" aria-labelledby="inst-recovery">
  <h2 class="inst-label" id="inst-recovery">{$t('ui.instrument.recovery.title')}</h2>
  {#if vitals.length > 0}
    <div class="rows" data-testid="readiness-values">
      {#each vitals as v (v.key)}
        <DeviationRow
          label={$t(`ui.dash.readiness.${v.key}`)}
          value={formatNumber(v.value.value, $locale, v.digits)}
          unit={$t(unitKey(v.key))}
          deviation={v.value.deviation_percent}
          deviationText={deviationText(v)}
          tone={tone(v)}
          testid={`vital-${v.key}`}
        />
      {/each}
      {#if r.body_battery_high !== null}
        <p class="battery" data-testid="vital-body-battery">
          {$t('ui.dash.readiness.body_battery')}
          <b>{formatNumber(r.body_battery_high, $locale, 0)}</b>
          {#if r.body_battery_low !== null}
            <span class="muted">
              · {$t('ui.dash.readiness.battery_low', {
                value: formatNumber(r.body_battery_low, $locale, 0),
              })}</span
            >
          {/if}
        </p>
      {/if}
    </div>
    <p
      class="state-mark"
      class:warn={low}
      class:ok={normal}
      class:off={!low && !normal}
      data-testid="readiness-state"
    >
      {low || normal ? $t(`ui.instrument.recovery.state.${low ? 'low' : 'normal'}`) : emptyText}
    </p>
    {#if r.date}
      <p class="stand">{$t('ui.dash.readiness.stand', { date: formatDate(r.date, $locale) })}</p>
    {/if}
    {#if low && r.plan_adjusted}
      <p class="adjusted" data-testid="plan-adjusted">{$t('ui.dash.readiness.plan_adjusted')}</p>
    {/if}
    <button type="button" class="inst-more" onclick={() => openHistory('body')}>
      {$t('ui.instrument.recovery.more')}
    </button>
  {:else}
    <p class="inst-empty" data-testid="readiness-empty">{emptyText}</p>
  {/if}
</section>

<style>
  .rows {
    display: flex;
    flex-direction: column;
  }

  .battery {
    margin: 0;
    padding-top: var(--space-2);
    border-top: 1px solid var(--line);
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .battery b {
    margin-left: 4px;
    font-size: var(--fs-lead);
    font-weight: var(--fw-semibold);
    font-stretch: var(--num-stretch);
    color: var(--text);
  }

  .stand {
    margin: 0;
    font-size: var(--fs-small);
    color: var(--muted);
  }

  .adjusted {
    margin: var(--space-1) 0 0;
    max-width: 44ch;
    font-size: var(--fs-small);
    font-weight: var(--fw-medium);
  }
</style>
