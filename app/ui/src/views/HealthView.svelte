<script lang="ts">
  import { getBackend } from '../lib/backend';
  import {
    HEALTH_RANGES,
    type HealthNumberKey,
    type HealthPage,
    type HealthPoint,
    type HealthRange,
  } from '../lib/backend/types';
  import { denseSeries } from '../lib/dashboard';
  import { daysBetween, localIso } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import Metric from '../lib/components/Metric.svelte';
  import Section from '../lib/components/Section.svelte';
  import Segmented from '../lib/components/Segmented.svelte';
  import Sparkline from '../lib/components/Sparkline.svelte';
  import { formatDate, formatNumber } from '../lib/format';
  import { hasKey, locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { dataVersion, view } from '../lib/state';

  // Gesundheit (T39): Tages- und Trainingswerte, die der Abgleich aus Garmin holt, je mit dem
  // jüngsten Wert, seinem Tag und einem Mini-Verlauf über den Zeitraum. Lastwerk rechnet mit
  // keinem dieser Werte; Garmins eigene Last steht nur zur Ansicht da (E2). Beschreibt
  // Trainingsdaten, keine Gesundheit im medizinischen Sinn (SPEC 2.3). Fehlende Werte sind
  // Lücken, nie 0.
  const today = localIso();
  let days = $state<HealthRange>(90);
  const loader = new Loader<HealthPage>();

  $effect(() => {
    void $dataVersion;
    const d = days;
    void loader.run(() => getBackend().getHealth(today, d));
  });

  const data = $derived(loader.data);
  const points = $derived(data?.points ?? []);

  interface MetricDef {
    field: HealthNumberKey;
    digits: number;
    /** Einheit aus der Sprachdatei, `ui.health.unit.<unit>`. */
    unit?: string;
    /** Garmins eigene Zahl, nicht von Lastwerk gerechnet und nie Rechengrundlage. */
    garmin?: boolean;
  }

  const DAILY: MetricDef[] = [
    { field: 'steps', digits: 0 },
    { field: 'step_distance_km', digits: 1, unit: 'km' },
    { field: 'hr_min_bpm', digits: 0, unit: 'bpm' },
    { field: 'hr_max_bpm', digits: 0, unit: 'bpm' },
    { field: 'respiration_waking_brpm', digits: 1, unit: 'brpm' },
    { field: 'respiration_sleep_brpm', digits: 1, unit: 'brpm' },
    { field: 'spo2_avg_pct', digits: 0, unit: 'pct' },
    { field: 'spo2_low_pct', digits: 0, unit: 'pct' },
    { field: 'intensity_moderate_min', digits: 0, unit: 'min' },
    { field: 'intensity_vigorous_min', digits: 0, unit: 'min' },
    { field: 'stress_avg', digits: 0 },
    { field: 'stress_max', digits: 0 },
  ];
  const TRAINING: MetricDef[] = [
    { field: 'readiness_score', digits: 0, garmin: true },
    { field: 'garmin_acute_load', digits: 0, garmin: true },
    { field: 'fitness_age', digits: 0, unit: 'years', garmin: true },
    { field: 'vo2max_running', digits: 1, unit: 'vo2', garmin: true },
    { field: 'vo2max_cycling', digits: 1, unit: 'vo2', garmin: true },
    { field: 'endurance_score', digits: 0, garmin: true },
    { field: 'hill_score', digits: 0, garmin: true },
  ];
  const HYDRATION: MetricDef[] = [{ field: 'hydration_ml', digits: 0, unit: 'ml' }];

  interface Shown extends MetricDef {
    latest: HealthPoint;
    values: (number | null)[];
    /** Tage, die der Mini-Verlauf zeigt: vom ersten Wert im Zeitraum bis heute. */
    span: number;
  }

  /** Nur Kennzahlen mit mindestens einem Wert im Zeitraum, je mit dem jüngsten Tag. */
  function shown(defs: MetricDef[]): Shown[] {
    if (!data) return [];
    const out: Shown[] = [];
    for (const def of defs) {
      let first: HealthPoint | null = null;
      let latest: HealthPoint | null = null;
      for (const p of points) {
        if (p[def.field] === null) continue;
        first ??= p;
        latest = p;
      }
      if (first === null || latest === null) continue;
      // Der Verlauf beginnt beim ersten Wert: Tageswerte gibt es nur für wenige Wochen, über
      // ein ganzes Jahr gestaucht wären sie ein Strich am rechten Rand. Lücken bleiben Lücken.
      const span = Math.min(data.days, Math.max(2, daysBetween(first.date, today) + 1));
      out.push({
        ...def,
        latest,
        span,
        values: denseSeries(points, (p) => p[def.field], today, span),
      });
    }
    return out;
  }

  const daily = $derived(shown(DAILY));
  const training = $derived(shown(TRAINING));
  const hydration = $derived(data?.sensitive_released ? shown(HYDRATION) : []);
  const any = $derived(daily.length + training.length + hydration.length > 0);

  // Garmins Stufe der Trainingsbereitschaft: bekannte Stufen aus der Sprachdatei, sonst der
  // Schlüssel lesbar gemacht (nie als HTML).
  function levelText(level: string): string {
    const key = `ui.health.level.${level}`;
    return hasKey('de', key) ? $t(key) : level.replace(/_/g, ' ').trim();
  }

  const rangeOptions = $derived(
    HEALTH_RANGES.map((r) => ({ value: r, label: $t(`ui.rec.range.${r}`) })),
  );
</script>

{#snippet metrics(list: Shown[])}
  <ul class="cells">
    {#each list as m (m.field)}
      <li class="cell" data-testid={`health-${m.field}`}>
        <Metric
          value={formatNumber(m.latest[m.field], $locale, m.digits)}
          unit={m.unit ? $t(`ui.health.unit.${m.unit}`) : undefined}
          label={$t(`ui.health.metric.${m.field}`)}
        >
          {#snippet extra()}
            {#if m.field === 'readiness_score' && m.latest.readiness_level}
              <span class="level" data-testid="health-readiness-level"
                >{levelText(m.latest.readiness_level)}</span
              >
            {/if}
          {/snippet}
        </Metric>
        <p class="caption muted">
          {$t('ui.health.as_of', {
            date: formatDate(m.latest.date, $locale),
          })}{#if m.garmin}&nbsp;· {$t('ui.health.from_garmin')}{/if}
        </p>
        <Sparkline
          values={m.values}
          label={$t('ui.health.spark_label', {
            name: $t(`ui.health.metric.${m.field}`),
            days: m.span,
          })}
        />
      </li>
    {/each}
  </ul>
{/snippet}

<div class="page health" data-testid="health">
  <header class="page-head">
    <h1>{$t('ui.nav.health')}</h1>
    <p class="page-lead" data-testid="lead">{$t('ui.lead.health')}</p>
  </header>

  <div class="controls">
    <Segmented
      options={rangeOptions}
      value={days}
      label={$t('ui.perf.range_label')}
      onchange={(d) => (days = d)}
      testid="health-range"
    />
  </div>

  {#if loader.error}
    <ErrorNote error={loader.error} />
  {:else if !data}
    <p class="muted">{$t('common.loading')}</p>
  {:else if !any}
    <div class="empty" data-testid="health-empty">
      <p class="muted">{$t('ui.health.none')}</p>
      <button type="button" onclick={() => view.set('settings')}
        >{$t('ui.health.to_settings')}</button
      >
    </div>
  {:else}
    {#if daily.length > 0}
      <Section
        label={$t('ui.health.daily')}
        statement={$t('ui.health.daily_statement')}
        testid="health-daily"
      >
        {@render metrics(daily)}
      </Section>
    {/if}
    {#if training.length > 0}
      <Section
        label={$t('ui.health.training')}
        statement={$t('ui.health.training_statement')}
        testid="health-training"
      >
        {@render metrics(training)}
      </Section>
    {/if}
    {#if hydration.length > 0}
      <Section
        label={$t('ui.health.hydration')}
        statement={$t('ui.health.hydration_statement')}
        testid="health-hydration"
      >
        {@render metrics(hydration)}
      </Section>
    {/if}
    <p class="note muted">{$t('ui.health.note')}</p>
  {/if}
</div>

<style>
  .health {
    max-width: var(--page-max-wide);
  }

  .controls {
    margin: 0 0 var(--space-5);
  }

  .empty {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }

  .empty p {
    margin: 0;
  }

  /* Kennzahlen auf der Fläche, keine Kacheln (SPEC 5.5): feine Linie über jeder Zelle. */
  .cells {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 200px), 1fr));
    gap: var(--space-5) var(--space-5);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .cell {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
    padding-top: var(--space-3);
    border-top: 1px solid var(--line);
  }

  /* Lange Einheiten (Atemzüge/min) rutschen in schmalen Spalten unter die Zahl. */
  .cell :global(.metric-value) {
    flex-wrap: wrap;
  }

  .caption {
    margin: 0;
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
    overflow-wrap: anywhere;
  }

  .level {
    font-size: var(--fs-label);
    line-height: var(--lh-tight);
    font-weight: var(--fw-semibold);
    overflow-wrap: anywhere;
  }

  .note {
    margin: var(--space-3) 0 0;
    font-size: var(--fs-small);
  }
</style>
