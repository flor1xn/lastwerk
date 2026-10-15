<script lang="ts">
  import type { PlanListItem, PlanStatus } from '../../lib/backend/types';
  import { formatDate } from '../../lib/format';
  import { locale, t } from '../../lib/i18n';

  interface Props {
    plans: PlanListItem[];
    selected: string | null;
    onselect: (id: string) => void;
    onstatus: (id: string, status: PlanStatus) => void;
  }

  let { plans, selected, onselect, onstatus }: Props = $props();
</script>

<section class="panel" data-testid="plan-list">
  <h3>{$t('ui.plan.list.title')}</h3>
  {#if plans.length === 0}
    <p class="muted" data-testid="plan-list-empty">{$t('ui.plan.list.empty')}</p>
  {:else}
    <table>
      <tbody>
        {#each plans as p (p.id)}
          <tr
            class:selected={p.id === selected}
            data-testid="plan-row"
            data-id={p.id}
            data-status={p.status}
          >
            <td><span class="status {p.status}">{$t(`ui.plan.status.${p.status}`)}</span></td>
            <td>
              {#if p.first_week && p.last_week}
                {$t('ui.plan.list.row', {
                  start: formatDate(p.first_week, $locale),
                  end: formatDate(p.last_week, $locale),
                  weeks: p.weeks,
                })}
              {/if}
              <span class="muted"
                >{$t('ui.plan.list.created', {
                  date: formatDate(p.created_at.slice(0, 10), $locale),
                })}</span
              >
            </td>
            <td class="actions">
              <button
                onclick={() => onselect(p.id)}
                data-testid="plan-open"
                disabled={p.id === selected}
                >{p.id === selected ? $t('ui.plan.list.shown') : $t('ui.plan.list.open')}</button
              >
              {#if p.status !== 'active'}
                <button onclick={() => onstatus(p.id, 'active')} data-testid="plan-activate"
                  >{$t('ui.plan.list.activate')}</button
                >
              {/if}
              {#if p.status !== 'archived'}
                <button onclick={() => onstatus(p.id, 'archived')} data-testid="plan-archive"
                  >{$t('ui.plan.list.archive')}</button
                >
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  tr.selected td {
    background: var(--row-hover);
  }

  td {
    white-space: normal;
  }

  .actions {
    text-align: right;
    white-space: nowrap;
  }

  .status {
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-size: var(--fs-label);
  }

  .status.active {
    border-color: var(--ok);
    color: var(--ok);
  }

  .status.archived {
    color: var(--muted);
  }
</style>
