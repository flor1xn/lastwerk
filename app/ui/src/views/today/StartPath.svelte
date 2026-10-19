<script lang="ts">
  import { t } from '../../lib/i18n';
  import { openSettings, view } from '../../lib/state';

  // Leerzustand von "Heute" (T63, SPEC 5.5): Fehlen Plan, Last und Erholungswerte zugleich,
  // steht hier ein Weg in drei Schritten statt einzelner Sätze, was fehlt. Erledigte Schritte
  // sind abgehakt; der Coach ist freiwillig.
  interface Props {
    hasData: boolean;
    hasPlan: boolean;
  }
  let { hasData, hasPlan }: Props = $props();

  const steps = $derived([
    { key: 'data', done: hasData, go: () => openSettings('data') },
    { key: 'plan', done: hasPlan, go: () => view.set('plan') },
    { key: 'coach', done: false, go: () => openSettings('coach') },
  ]);
</script>

<section class="start" data-testid="today-start" aria-labelledby="start-title">
  <h2 class="inst-label" id="start-title">{$t('ui.instrument.start.title')}</h2>
  <ol>
    {#each steps as s, i (s.key)}
      <li class:done={s.done} data-testid="start-step" data-step={s.key}>
        <span class="num" aria-hidden="true">{s.done ? '✓' : i + 1}</span>
        <div class="text">
          <p class="name">{$t(`ui.instrument.start.${s.key}.name`)}</p>
          <p class="hint">{$t(`ui.instrument.start.${s.key}.hint`)}</p>
        </div>
        {#if s.done}
          <span class="state-mark ok">{$t('ui.instrument.start.done')}</span>
        {:else}
          <button
            type="button"
            class:primary={i === steps.findIndex((x) => !x.done)}
            onclick={s.go}
          >
            {$t(`ui.instrument.start.${s.key}.action`)}
          </button>
        {/if}
      </li>
    {/each}
  </ol>
</section>

<style>
  .start {
    padding-top: var(--space-5);
  }

  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--line);
  }

  li {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4) 0;
    border-bottom: 1px solid var(--line);
  }

  .num {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--panel-2);
    font-weight: var(--fw-semibold);
    font-stretch: var(--num-stretch);
    color: var(--text-2);
  }

  .done .num {
    color: var(--ok);
  }

  .text p {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .name {
    font-size: var(--fs-lead);
    font-weight: var(--fw-semibold);
  }

  .hint {
    max-width: var(--measure);
    color: var(--muted);
  }

  .done .name {
    color: var(--text-2);
  }

  @media (max-width: 559px) {
    li {
      grid-template-columns: 32px minmax(0, 1fr);
    }

    li > :last-child {
      grid-column: 2;
      justify-self: start;
    }
  }
</style>
