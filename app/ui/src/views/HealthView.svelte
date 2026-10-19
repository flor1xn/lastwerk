<script lang="ts">
  import { getBackend } from '../lib/backend';
  import {
    HEALTH_RANGES,
    type HealthNumberKey,
    type HealthPage,
    type HealthPoint,
    type HealthRange,
    type HealthTextKey,
    type SensitiveCategory,
  } from '../lib/backend/types';
  import { denseSeries } from '../lib/dashboard';
  import { daysBetween, localIso } from '../lib/dates';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import Metric from '../lib/components/Metric.svelte';
  import Section from '../lib/components/Section.svelte';
  import Segmented from '../lib/components/Segmented.svelte';
  import Sparkline from '../lib/components/Sparkline.svelte';
  import { formatDate, formatDuration, formatNumber } from '../lib/format';
  import { hasKey, locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { dataVersion, openSettings } from '../lib/state';

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
    /** Sekunden als h:mm:ss statt als Zahl (Rennprognosen). */
    duration?: boolean;
    /** Garmins Schlüssel desselben Tages, als Text neben der Beschriftung. */
    text?: HealthTextKey;
    /** Garmins Zielspanne desselben Tages (Minimum, Maximum), als Text unter dem Wert. */
    target?: [HealthNumberKey, HealthNumberKey];
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
    { field: 'floors_up', digits: 0, unit: 'floors' },
    { field: 'floors_down', digits: 0, unit: 'floors' },
  ];
  const TRAINING: MetricDef[] = [
    { field: 'readiness_score', digits: 0, garmin: true, text: 'readiness_level' },
    { field: 'fitness_age', digits: 0, unit: 'years', garmin: true },
    { field: 'vo2max_running', digits: 1, unit: 'vo2', garmin: true },
    { field: 'vo2max_cycling', digits: 1, unit: 'vo2', garmin: true },
    { field: 'endurance_score', digits: 0, garmin: true },
    { field: 'hill_score', digits: 0, garmin: true },
  ];
  // Seit T40: Trainingsstatus mit Lasten und Belastungsfokus, Rennprognosen, Lauftoleranz.
  const STATUS: MetricDef[] = [
    { field: 'garmin_acute_load', digits: 0, garmin: true, text: 'training_status' },
    { field: 'garmin_chronic_load', digits: 0, garmin: true },
    { field: 'garmin_acwr', digits: 2, garmin: true },
    {
      field: 'load_aerobic_low',
      digits: 0,
      garmin: true,
      target: ['load_aerobic_low_min', 'load_aerobic_low_max'],
    },
    {
      field: 'load_aerobic_high',
      digits: 0,
      garmin: true,
      target: ['load_aerobic_high_min', 'load_aerobic_high_max'],
    },
    {
      field: 'load_anaerobic',
      digits: 0,
      garmin: true,
      target: ['load_anaerobic_min', 'load_anaerobic_max'],
    },
  ];
  const RACE: MetricDef[] = [
    { field: 'race_5k_s', digits: 0, garmin: true, duration: true },
    { field: 'race_10k_s', digits: 0, garmin: true, duration: true },
    { field: 'race_half_s', digits: 0, garmin: true, duration: true },
    { field: 'race_marathon_s', digits: 0, garmin: true, duration: true },
  ];
  const TOLERANCE: MetricDef[] = [
    {
      field: 'run_acute_distance_km',
      digits: 1,
      unit: 'km',
      garmin: true,
      text: 'run_tolerance_level',
    },
    { field: 'run_tolerance_km', digits: 1, unit: 'km', garmin: true },
    { field: 'run_impact_load', digits: 0, garmin: true },
  ];
  // Sensible Werte (S3) je Bereich, nur mit Freigabe (T40, Entscheidung E7).
  const SENSITIVE: { category: SensitiveCategory; defs: MetricDef[] }[] = [
    {
      category: 'body',
      defs: [
        { field: 'weight_kg', digits: 1, unit: 'kg' },
        { field: 'bmi', digits: 1 },
        { field: 'body_fat_pct', digits: 1, unit: 'pct' },
        { field: 'body_water_pct', digits: 1, unit: 'pct' },
        { field: 'muscle_kg', digits: 1, unit: 'kg' },
        { field: 'bone_kg', digits: 1, unit: 'kg' },
      ],
    },
    {
      category: 'blood_pressure',
      defs: [
        { field: 'systolic_mmhg', digits: 0, unit: 'mmhg' },
        { field: 'diastolic_mmhg', digits: 0, unit: 'mmhg' },
        { field: 'pulse_bpm', digits: 0, unit: 'bpm' },
      ],
    },
    {
      category: 'nutrition',
      defs: [
        { field: 'calories_kcal', digits: 0, unit: 'kcal' },
        { field: 'calories_goal_kcal', digits: 0, unit: 'kcal' },
        { field: 'carbs_g', digits: 0, unit: 'g' },
        { field: 'fat_g', digits: 0, unit: 'g' },
        { field: 'protein_g', digits: 0, unit: 'g' },
      ],
    },
    { category: 'hydration', defs: [{ field: 'hydration_ml', digits: 0, unit: 'ml' }] },
  ];

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
  const status = $derived(shown(STATUS));
  const race = $derived(shown(RACE));
  const tolerance = $derived(shown(TOLERANCE));
  const sensitive = $derived(
    SENSITIVE.filter((g) => data?.released.includes(g.category))
      .map((g) => ({ category: g.category, list: shown(g.defs) }))
      .filter((g) => g.list.length > 0),
  );
  const any = $derived(
    daily.length + training.length + status.length + race.length + tolerance.length > 0 ||
      sensitive.length > 0,
  );
  /** Garmins Hinweis zum Belastungsfokus, vom jüngsten Tag mit Hinweis. */
  const balance = $derived(
    [...points].reverse().find((p) => p.load_balance !== null)?.load_balance ?? null,
  );

  // Garmins Schlüssel (Stufe, Status, Hinweis): bekannte aus der Sprachdatei, sonst der
  // Schlüssel lesbar gemacht (nie als HTML).
  const TEXT_NAMESPACE: Record<HealthTextKey, string> = {
    readiness_level: 'level',
    training_status: 'status',
    load_balance: 'balance',
    run_tolerance_level: 'tolerance',
  };
  function keyText(kind: HealthTextKey, value: string): string {
    const key = `ui.health.${TEXT_NAMESPACE[kind]}.${value}`;
    return hasKey('de', key) ? $t(key) : value.replace(/_/g, ' ').trim().toLowerCase();
  }

  function valueText(m: Shown): string {
    const v = m.latest[m.field];
    return m.duration ? formatDuration(v) : formatNumber(v, $locale, m.digits);
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
          value={valueText(m)}
          unit={m.unit ? $t(`ui.health.unit.${m.unit}`) : undefined}
          label={$t(`ui.health.metric.${m.field}`)}
        >
          {#snippet extra()}
            {@const text = m.text ? m.latest[m.text] : null}
            {#if m.text && text}
              <span class="level" data-testid={`health-${m.text.replace(/_/g, '-')}`}
                >{keyText(m.text, text)}</span
              >
            {/if}
          {/snippet}
        </Metric>
        {#if m.target}
          {@const lo = m.latest[m.target[0]]}
          {@const hi = m.latest[m.target[1]]}
          {#if lo !== null && hi !== null}
            <p class="caption" data-testid={`health-${m.field}-target`}>
              {$t('ui.health.target', {
                min: formatNumber(lo, $locale, 0),
                max: formatNumber(hi, $locale, 0),
              })}
            </p>
          {/if}
        {/if}
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
  <p class="page-lead" data-testid="lead">{$t('ui.lead.health')}</p>

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
      <button type="button" onclick={() => openSettings('data')}
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
    {#if status.length > 0}
      <Section
        label={$t('ui.health.status_title')}
        statement={$t('ui.health.status_statement')}
        testid="health-status"
      >
        {@render metrics(status)}
        {#if balance}
          <p class="balance" data-testid="health-balance">
            {$t('ui.health.balance_label')}: {keyText('load_balance', balance)}
          </p>
        {/if}
      </Section>
    {/if}
    {#if race.length > 0}
      <Section
        label={$t('ui.health.race')}
        statement={$t('ui.health.race_statement')}
        testid="health-race"
      >
        {@render metrics(race)}
      </Section>
    {/if}
    {#if tolerance.length > 0}
      <Section
        label={$t('ui.health.tolerance_title')}
        statement={$t('ui.health.tolerance_statement')}
        testid="health-tolerance"
      >
        {@render metrics(tolerance)}
      </Section>
    {/if}
    {#each sensitive as group (group.category)}
      <Section
        label={$t(`ui.health.s3.${group.category}`)}
        statement={$t('ui.health.s3_statement')}
        testid={`health-${group.category.replace(/_/g, '-')}`}
      >
        {@render metrics(group.list)}
      </Section>
    {/each}
    <p class="note muted">{$t('ui.health.note')}</p>
  {/if}
</div>

<style>
  .health {
    max-width: var(--page-max);
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

  .balance {
    margin: var(--space-4) 0 0;
    overflow-wrap: anywhere;
  }

  .note {
    margin: var(--space-3) 0 0;
    font-size: var(--fs-small);
  }
</style>
