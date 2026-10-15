<script lang="ts">
  import { getBackend } from '../lib/backend';
  import { SPORTS, type ActivityItem, type Sport } from '../lib/backend/types';
  import { addDays, isIsoDate, localIso } from '../lib/dates';
  import { viewDefaults } from '../lib/defaults';
  import ErrorNote from '../lib/components/ErrorNote.svelte';
  import LoadCell from '../lib/components/LoadCell.svelte';
  import { formatDate, formatDistance, formatDuration, formatNumber, MISSING } from '../lib/format';
  import { locale, t } from '../lib/i18n';
  import { Loader } from '../lib/loader.svelte';
  import { dataVersion, openActivity } from '../lib/state';

  type SortKey = 'date' | 'duration' | 'load';

  const loader = new Loader<ActivityItem[]>();
  const today = localIso();

  let from = $state(addDays(today, -viewDefaults.listDays));
  let to = $state(today);
  let sport = $state<Sport | ''>('');
  let showDuplicates = $state(true);
  let sortKey = $state<SortKey>('date');
  let sortDesc = $state(true);

  const validRange = $derived(isIsoDate(from) && isIsoDate(to) && from <= to);

  $effect(() => {
    void $dataVersion;
    if (!validRange) return;
    const filter = { from, to, sport: sport === '' ? null : sport };
    void loader.run(() => getBackend().listActivities(filter));
  });

  function sortValue(a: ActivityItem, key: SortKey): number | null {
    if (key === 'date') return Date.parse(a.start_utc);
    if (key === 'duration') return a.duration_s;
    return a.load.status === 'ok' ? a.load.value : null;
  }

  const rows = $derived.by(() => {
    const list = (loader.data ?? []).filter((a) => showDuplicates || a.is_primary);
    const direction = sortDesc ? -1 : 1;
    return [...list].sort((a, b) => {
      const va = sortValue(a, sortKey);
      const vb = sortValue(b, sortKey);
      // Nicht berechenbare Lasten stehen immer am Ende, nie als 0 einsortiert.
      if (va === null && vb === null) return 0;
      if (va === null) return 1;
      if (vb === null) return -1;
      return (va - vb) * direction;
    });
  });

  function sortBy(key: SortKey) {
    if (sortKey === key) sortDesc = !sortDesc;
    else {
      sortKey = key;
      sortDesc = true;
    }
  }

  const ariaSort = (key: SortKey) =>
    sortKey === key ? (sortDesc ? 'descending' : 'ascending') : 'none';
</script>

<div class="head">
  <h2>{$t('ui.activities.title')}</h2>
  <label>{$t('ui.activities.from')} <input type="date" bind:value={from} /></label>
  <label>{$t('ui.activities.to')} <input type="date" bind:value={to} /></label>
  <label>
    {$t('ui.activities.sport')}
    <select bind:value={sport}>
      <option value="">{$t('ui.sport.all')}</option>
      {#each SPORTS as s (s)}
        <option value={s}>{$t(`ui.sport.${s}`)}</option>
      {/each}
    </select>
  </label>
  <label class="check">
    <input type="checkbox" bind:checked={showDuplicates} />
    {$t('ui.activities.show_duplicates')}
  </label>
  <span class="muted" data-testid="activity-count">
    {$t('ui.activities.count', { n: rows.length })}
  </span>
</div>
<p class="page-lead" data-testid="lead">{$t('ui.lead.activities')}</p>

{#if !validRange}
  <p class="error" role="alert">{$t('ui.error.invalid_date')}</p>
{:else if loader.error}
  <ErrorNote error={loader.error} />
{:else if loader.data && rows.length === 0}
  <p class="muted">{$t('ui.activities.empty')}</p>
{:else if loader.data}
  <div class="panel table-wrap">
    <table data-testid="activity-table">
      <thead>
        <tr>
          <th aria-sort={ariaSort('date')}>
            <button class="sort" onclick={() => sortBy('date')}>{$t('ui.activities.date')}</button>
          </th>
          <th>{$t('ui.activities.sport')}</th>
          <th class="num" aria-sort={ariaSort('duration')}>
            <button class="sort" onclick={() => sortBy('duration')}
              >{$t('ui.activities.duration')}</button
            >
          </th>
          <th class="num">{$t('ui.activities.distance')}</th>
          <th aria-sort={ariaSort('load')}>
            <button class="sort" onclick={() => sortBy('load')}>{$t('ui.activities.load')}</button>
          </th>
          <th class="num" title={$t('ui.load.if_hint')}>{$t('ui.load.if')}</th>
          <th>{$t('ui.activities.flags')}</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as a (a.id)}
          <tr
            class:duplicate={!a.is_primary}
            tabindex="0"
            data-testid="activity-row"
            data-id={a.id}
            onclick={() => openActivity(a.id)}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                openActivity(a.id);
              }
            }}
          >
            <td>{formatDate(a.local_date, $locale)}</td>
            <td>{$t(`ui.sport.${a.sport}`)}</td>
            <td class="num">{formatDuration(a.duration_s)}</td>
            <td class="num">{formatDistance(a.distance_m, $locale)}</td>
            <td><LoadCell load={a.load} /></td>
            <td class="num"
              >{a.load.intensity_factor === null
                ? MISSING
                : formatNumber(a.load.intensity_factor, $locale, 2)}</td
            >
            <td>
              {#if !a.is_primary}
                <span
                  class="flag"
                  data-testid="duplicate-flag"
                  title={$t('ui.activities.duplicate_hint')}>{$t('ui.activities.duplicate')}</span
                >
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
    margin-bottom: 10px;
  }

  .head label {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--muted);
  }

  .head .check {
    color: var(--text);
  }

  .table-wrap {
    padding: 0;
    overflow-x: auto;
  }

  tbody tr {
    cursor: pointer;
  }

  tbody tr:hover,
  tbody tr:focus-visible {
    background: var(--row-hover);
  }

  tr.duplicate {
    color: var(--muted);
  }

  .sort {
    background: none;
    border: none;
    padding: 0;
    color: inherit;
    font-weight: inherit;
  }

  th[aria-sort='ascending'] .sort::after {
    content: ' ▲';
    font-size: var(--fs-label);
  }

  th[aria-sort='descending'] .sort::after {
    content: ' ▼';
    font-size: var(--fs-label);
  }

  .flag {
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-size: var(--fs-label);
  }
</style>
