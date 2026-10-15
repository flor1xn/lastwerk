<script lang="ts">
  import { getBackend } from '../../lib/backend';
  import {
    SPORTS,
    THRESHOLD_KINDS,
    type Recompute,
    type Sport,
    type ThresholdKind,
    type ThresholdList,
  } from '../../lib/backend/types';
  import { localIso } from '../../lib/dates';
  import EstimatedBadge from '../../lib/components/EstimatedBadge.svelte';
  import ErrorNote from '../../lib/components/ErrorNote.svelte';
  import { errorKey } from '../../lib/errors';
  import { formatDate, formatNumber, parseDecimal } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';
  import { Loader } from '../../lib/loader.svelte';
  import { bumpData, dataVersion } from '../../lib/state';

  const loader = new Loader<ThresholdList>();

  let sport = $state<Sport | ''>('cycling');
  let kind = $state<ThresholdKind>('ftp_w');
  let valueText = $state('');
  let validFrom = $state(localIso());
  let note = $state('');
  let saving = $state(false);
  let errorMessage = $state<string | null>(null);
  let result = $state<Recompute | null>(null);

  $effect(() => {
    void $dataVersion;
    void loader.run(() => getBackend().listThresholds());
  });

  // Die Schwellen für Leistung, Lauf und Schwimmen gelten nur für ihre Sportart, die übrigen
  // für alle. Die Auswahl der Art stellt die passende Sportart vor.
  const DEFAULT_SPORT: Record<ThresholdKind, Sport | ''> = {
    ftp_w: 'cycling',
    lthr_bpm: '',
    hr_max_bpm: '',
    hr_rest_bpm: '',
    run_speed_mps: 'running',
    css_mps: 'swimming',
  };

  function onKindChange() {
    sport = DEFAULT_SPORT[kind];
  }

  const decimals = (k: ThresholdKind, v: number) =>
    k === 'run_speed_mps' || k === 'css_mps' ? 2 : Number.isInteger(v) ? 0 : 1;

  async function save(event: SubmitEvent) {
    event.preventDefault();
    errorMessage = null;
    result = null;
    const value = parseDecimal(valueText);
    if (value === null) {
      errorMessage = 'ui.error.invalid_value';
      return;
    }
    saving = true;
    try {
      const set = await getBackend().setThreshold({
        sport: sport === '' ? null : sport,
        kind,
        value,
        valid_from: validFrom,
        note: note.trim() === '' ? null : note.trim(),
      });
      result = set.recompute;
      valueText = '';
      note = '';
      bumpData();
    } catch (e) {
      errorMessage = errorKey(e);
    } finally {
      saving = false;
    }
  }
</script>

<section class="section" data-testid="threshold-section">
  <h3>{$t('ui.threshold.title')}</h3>
  <p class="muted hint">{$t('ui.threshold.hint')}</p>

  {#if loader.error}
    <ErrorNote error={loader.error} />
  {:else if loader.data}
    {#if loader.data.items.length === 0}
      <p class="muted">{$t('ui.threshold.none')}</p>
    {:else}
      <table data-testid="threshold-table">
        <thead>
          <tr>
            <th>{$t('ui.threshold.col_kind')}</th>
            <th>{$t('ui.threshold.sport')}</th>
            <th class="num">{$t('ui.threshold.value')}</th>
            <th>{$t('ui.threshold.valid')}</th>
            <th>{$t('ui.threshold.note')}</th>
          </tr>
        </thead>
        <tbody>
          {#each loader.data.items as item (item.id)}
            <tr>
              <td>{$t(`ui.threshold.kind.${item.kind}`)}</td>
              <td>{item.sport === null ? $t('ui.sport.all') : $t(`ui.sport.${item.sport}`)}</td>
              <td class="num"
                >{formatNumber(item.value, $locale, decimals(item.kind, item.value))}
                {$t(`ui.threshold.unit.${item.kind}`)}</td
              >
              <td>
                {$t('ui.threshold.valid_range', {
                  from: formatDate(item.valid_from, $locale),
                  until: item.valid_until
                    ? formatDate(item.valid_until, $locale)
                    : $t('ui.threshold.open'),
                })}
              </td>
              <td class="muted">{item.note ?? ''}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {#if loader.data.hr_max_estimate}
      <p data-testid="hr-max-estimate">
        {$t('ui.threshold.hr_max_estimate', {
          bpm: formatNumber(loader.data.hr_max_estimate.bpm, $locale, 1),
        })}
        <EstimatedBadge />
      </p>
    {/if}
  {/if}

  <form onsubmit={save} data-testid="threshold-form">
    <h4>{$t('ui.threshold.add')}</h4>
    <div class="fields">
      <label>
        {$t('ui.threshold.sport')}
        <select bind:value={sport}>
          <option value="">{$t('ui.sport.all')}</option>
          {#each SPORTS as s (s)}
            <option value={s}>{$t(`ui.sport.${s}`)}</option>
          {/each}
        </select>
      </label>
      <label>
        {$t('ui.threshold.col_kind')}
        <select bind:value={kind} onchange={onKindChange} data-testid="threshold-kind">
          {#each THRESHOLD_KINDS as k (k)}
            <option value={k}
              >{$t(`ui.threshold.kind.${k}`)} ({$t(`ui.threshold.unit.${k}`)})</option
            >
          {/each}
        </select>
      </label>
      <label>
        {$t('ui.threshold.value')}
        <input
          type="text"
          inputmode="decimal"
          size="7"
          bind:value={valueText}
          required
          data-testid="threshold-value"
        />
      </label>
      <label>
        {$t('ui.threshold.valid_from')}
        <input type="date" bind:value={validFrom} required data-testid="threshold-from" />
      </label>
      <label>
        {$t('ui.threshold.note')}
        <input type="text" size="18" maxlength="200" bind:value={note} />
      </label>
      <button class="primary" type="submit" disabled={saving}>{$t('ui.threshold.save')}</button>
    </div>
  </form>

  {#if errorMessage}
    <p class="error" role="alert" data-testid="threshold-error">{$t(errorMessage)}</p>
  {/if}
  {#if result}
    <p class="ok" data-testid="threshold-result">
      {#if result.skipped}
        {$t('ui.threshold.saved_not_applied')}
      {:else}
        {$t('ui.threshold.saved_recomputed', { n: result.recomputed, total: result.candidates })}
      {/if}
    </p>
    {#if result.file_missing + result.file_changed > 0}
      <p class="warn">
        {$t('ui.threshold.files_skipped', { n: result.file_missing + result.file_changed })}
      </p>
    {/if}
  {/if}
</section>

<style>
  .hint {
    margin: 4px 0 8px;
  }

  h4 {
    margin: 12px 0 6px;
    font-size: var(--fs-small);
  }

  .fields {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 8px 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--muted);
    font-size: var(--fs-small);
  }

  .ok {
    color: var(--ok);
  }
</style>
