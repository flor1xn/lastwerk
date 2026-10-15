<script lang="ts">
  import type { PlanValidation, PlanWarning } from '../backend/types';
  import { formatDate } from '../format';
  import { locale, t } from '../i18n';
  import { hardViolations, ruleInfo, softViolations, sortedWarnings } from '../plan';

  interface Props {
    warnings: PlanWarning[];
    validation: PlanValidation;
  }

  let { warnings, validation }: Props = $props();

  const sorted = $derived(sortedWarnings(warnings));
  const hard = $derived(hardViolations(validation.violations));
  const soft = $derived(softViolations(validation.violations));
  const valid = $derived(validation.schema_errors.length === 0 && hard.length === 0);
</script>

<section class="panel" data-testid="plan-checks">
  <h3>{$t('ui.plan.validation.title')}</h3>
  <p data-testid="plan-validation" data-valid={valid} class:ok={valid} class:error={!valid}>
    {#if validation.schema_errors.length > 0}
      {$t('ui.plan.validation.schema_failed', { n: validation.schema_errors.length })}
    {:else if hard.length > 0}
      {$t('ui.plan.validation.hard', { n: hard.length })}
    {:else}
      {$t('ui.plan.validation.ok')}
    {/if}
    {#if soft.length > 0}
      <span class="muted">{$t('ui.plan.validation.soft', { n: soft.length })}</span>
    {/if}
  </p>
  {#if validation.schema_errors.length > 0}
    <ul>
      {#each validation.schema_errors as line (line)}
        <li class="error">{line}</li>
      {/each}
    </ul>
  {/if}
  {#each hard as v (v.code + (v.date ?? ''))}
    <p class="error" data-testid="plan-hard" data-code={v.code}>
      <strong>{v.code}</strong>
      {ruleInfo($locale, v.code).name}
    </p>
  {/each}

  <h3 class="sub">{$t('ui.plan.warnings.title')}</h3>
  {#if sorted.length === 0}
    <p class="muted" data-testid="plan-no-warnings">{$t('ui.plan.warnings.none')}</p>
  {:else}
    <ul data-testid="plan-warnings">
      {#each sorted as w (w.code + (w.date ?? ''))}
        {@const info = ruleInfo($locale, w.code)}
        <li data-testid="plan-warning" data-code={w.code} data-severity={w.severity}>
          <span class="code" class:warn={w.severity === 'warn'}>{w.code}</span>
          <strong>{info.name}</strong>
          <span class="muted"
            >{$t(`ui.plan.warnings.${w.severity}`)}{#if w.date}, {formatDate(
                w.date,
                $locale,
              )}{/if}</span
          >
          {#if info.explanation}
            <span class="explanation" data-testid="plan-warning-text">{info.explanation}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .sub {
    margin-top: 10px;
  }

  p {
    margin: 4px 0;
  }

  .ok {
    color: var(--ok);
  }

  ul {
    margin: 4px 0 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 8px;
    padding: 3px 0;
    border-bottom: 1px solid var(--border);
  }

  .code {
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 3px;
    font-variant-numeric: tabular-nums;
  }

  .code.warn {
    border-color: var(--warn);
    color: var(--warn);
  }

  .explanation {
    flex-basis: 100%;
    color: var(--muted);
    font-size: var(--fs-small);
  }
</style>
