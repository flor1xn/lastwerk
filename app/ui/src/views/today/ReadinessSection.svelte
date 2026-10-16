<script lang="ts">
  import type { DashRecoveryValue, Dashboard } from '../../lib/backend/types';
  import Metric from '../../lib/components/Metric.svelte';
  import Section from '../../lib/components/Section.svelte';
  import Sparkline from '../../lib/components/Sparkline.svelte';
  import StatRow from '../../lib/components/StatRow.svelte';
  import { denseSeries, directionOf, readinessStatement, signedPercent } from '../../lib/dashboard';
  import { formatDate, formatNumber } from '../../lib/format';
  import { garminSync } from '../../lib/garmin';
  import { locale, t } from '../../lib/i18n';

  // Bereitschaft: HRV, Ruhepuls und Schlafdauer des jüngsten Tages gegen den eigenen Schnitt der
  // Tage davor (Basis aus R12). Beschreibt Trainingsdaten, keine Gesundheit (SPEC 2.3).
  let { dash }: { dash: Dashboard } = $props();

  const r = $derived(dash.readiness);
  // Während des Abgleichs sagt der leere Zustand, dass die Werte unterwegs sind (T41).
  const statement = $derived(
    r.last_date === null && $garminSync.kind === 'running'
      ? $t('ui.dash.readiness.loading')
      : readinessStatement(r, $t, $locale),
  );
  const low = $derived(r.state === 'readiness.low');

  interface Vital {
    key: 'hrv' | 'resting_hr' | 'sleep';
    value: DashRecoveryValue;
    digits: number;
    values: (number | null)[];
  }

  const vitals = $derived.by((): Vital[] => {
    const series = r.series;
    const dense = (pick: (p: (typeof series)[number]) => number | null) =>
      denseSeries(series, pick, dash.today, 60);
    const out: Vital[] = [];
    if (r.hrv) out.push({ key: 'hrv', value: r.hrv, digits: 0, values: dense((p) => p.hrv_ms) });
    if (r.resting_hr) {
      out.push({
        key: 'resting_hr',
        value: r.resting_hr,
        digits: 0,
        values: dense((p) => p.resting_hr_bpm),
      });
    }
    if (r.sleep_h) {
      out.push({ key: 'sleep', value: r.sleep_h, digits: 1, values: dense((p) => p.sleep_h) });
    }
    return out;
  });

  const unitKey = (k: Vital['key']) => `ui.dash.readiness.unit_${k}`;

  // Richtung und Text der Abweichung. Rot nur dort, wo Lastwerk selbst "deutlich darunter" sagt
  // (HRV und Schlaf bei readiness.low). Der Ruhepuls hat keine Schwelle und bleibt neutral.
  function delta(v: Vital) {
    const d = v.value.deviation_percent;
    if (d === null) return undefined;
    return {
      text: $t('ui.dash.readiness.vs_baseline', { value: signedPercent(d, $locale, 0) }),
      direction: directionOf(d, 0),
      tone: low && d < 0 && v.key !== 'resting_hr' ? ('bad' as const) : ('neutral' as const),
    };
  }
</script>

<Section label={$t('ui.dash.readiness.title')} {statement} testid="today-readiness">
  {#snippet aside()}
    {#if r.date}
      <p class="stand muted">
        {$t('ui.dash.readiness.stand', { date: formatDate(r.date, $locale) })}
      </p>
    {/if}
  {/snippet}

  {#if vitals.length > 0 || r.body_battery_high !== null}
    <div data-testid="readiness-values">
      <StatRow>
        {#each vitals as v (v.key)}
          <div class="vital" data-testid={`vital-${v.key}`}>
            <Metric
              value={formatNumber(v.value.value, $locale, v.digits)}
              unit={$t(unitKey(v.key))}
              label={$t(`ui.dash.readiness.${v.key}`)}
              delta={delta(v)}
            />
            <Sparkline
              values={v.values}
              baseline={v.value.baseline}
              label={$t('ui.dash.readiness.spark_label', {
                name: $t(`ui.dash.readiness.${v.key}`),
              })}
              color={low && v.key !== 'resting_hr' ? 'var(--warn)' : 'var(--accent)'}
            />
            <p class="caption muted">
              {v.value.baseline === null
                ? $t('ui.dash.readiness.no_baseline')
                : $t('ui.dash.readiness.baseline', {
                    value: `${formatNumber(v.value.baseline, $locale, v.digits)} ${$t(unitKey(v.key))}`,
                  })}
            </p>
          </div>
        {/each}
        <!-- Body Battery liefert T22. Bis dahin ist der Wert null und die Spalte fehlt. -->
        {#if r.body_battery_high !== null}
          <div class="vital" data-testid="vital-body-battery">
            <Metric
              value={formatNumber(r.body_battery_high, $locale, 0)}
              label={$t('ui.dash.readiness.body_battery')}
            />
            {#if r.body_battery_low !== null}
              <p class="caption muted">
                {$t('ui.dash.readiness.battery_low', {
                  value: formatNumber(r.body_battery_low, $locale, 0),
                })}
              </p>
            {/if}
          </div>
        {/if}
      </StatRow>
    </div>
  {/if}

  {#if low && r.plan_adjusted}
    <p class="adjusted" data-testid="plan-adjusted">{$t('ui.dash.readiness.plan_adjusted')}</p>
  {/if}
</Section>

<style>
  .stand {
    margin: 0;
    font-size: var(--fs-small);
  }

  .vital {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
    flex: 1 1 0;
  }

  .caption {
    margin: 0;
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
  }

  .adjusted {
    margin: var(--space-5) 0 0;
    padding: var(--space-1) 0 var(--space-1) var(--space-3);
    border-left: 3px solid var(--accent);
    max-width: 62ch;
    font-weight: var(--fw-semibold);
  }
</style>
