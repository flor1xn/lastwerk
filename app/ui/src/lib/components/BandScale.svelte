<script lang="ts">
  import { t } from '../i18n';
  import { FORM_BALANCED_FROM, FORM_FRESH_FROM, FORM_LOADED_FROM, type FormState } from '../today';

  // Bandskala des Formzustands (T62, SPEC 5.5): vier benannte Bereiche, ihre Breiten
  // proportional zum TSB, eine Marke auf dem Wert. Grenzen aus lib/today.ts, dieselben wie in
  // core (R10 und Faustregel). Die Enden der Darstellung (-50 und +25) sind Faustregel; Werte
  // außerhalb stoßen an den Rand, die Zahl steht daneben. Die Marke gleitet bei neuem Wert,
  // damit die Veränderung lesbar wird; beim ersten Zeichnen bewegt sie sich nicht. Der Zustand
  // kommt fertig aus core (Dashboard), die Skala rechnet ihn nicht nach.
  interface Props {
    tsb: number;
    state: FormState;
    testid?: string;
  }

  let { tsb, state, testid }: Props = $props();

  const SCALE_MIN = -50;
  const SCALE_MAX = 25;

  const SEGMENTS: { key: FormState; from: number; to: number }[] = [
    { key: 'overloaded', from: SCALE_MIN, to: FORM_LOADED_FROM },
    { key: 'loaded', from: FORM_LOADED_FROM, to: FORM_BALANCED_FROM },
    { key: 'balanced', from: FORM_BALANCED_FROM, to: FORM_FRESH_FROM },
    { key: 'fresh', from: FORM_FRESH_FROM, to: SCALE_MAX },
  ];

  const pct = $derived(
    ((Math.min(SCALE_MAX, Math.max(SCALE_MIN, tsb)) - SCALE_MIN) / (SCALE_MAX - SCALE_MIN)) * 100,
  );
</script>

<div
  class="band-scale"
  role="img"
  aria-label={$t('ui.instrument.form.scale_label', { state: $t(`ui.form.${state}`) })}
  data-testid={testid}
  data-state={state}
>
  <div class="track" aria-hidden="true">
    {#each SEGMENTS as s (s.key)}
      <span class="seg {s.key}" class:on={s.key === state} style:flex-grow={s.to - s.from}></span>
    {/each}
  </div>
  <div class="rail" aria-hidden="true" style:transform="translateX({pct}%)">
    <span class="marker" data-testid={testid ? `${testid}-marker` : undefined}></span>
  </div>
  <div class="names" aria-hidden="true">
    {#each SEGMENTS as s (s.key)}
      <span class:on={s.key === state} style:flex-grow={s.to - s.from}
        >{$t(`ui.form.${s.key}`)}</span
      >
    {/each}
  </div>
</div>

<style>
  .band-scale {
    position: relative;
    padding-top: 6px;
  }

  .track,
  .names {
    display: flex;
    gap: 2px;
  }

  .seg,
  .names span {
    flex-basis: 0;
    min-width: 0;
  }

  .seg {
    height: 6px;
    border-radius: 2px;
    background: var(--panel-2);
  }

  .seg.on.fresh {
    background: var(--form-fresh);
  }

  .seg.on.balanced {
    background: var(--form-balanced);
  }

  .seg.on.loaded {
    background: var(--form-loaded);
  }

  .seg.on.overloaded {
    background: var(--form-overloaded);
  }

  /* Die Schiene ist so breit wie die Spur, deshalb verschiebt translateX in Prozent der Spur. */
  .rail {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 18px;
    pointer-events: none;
    transition: transform 200ms var(--ease-out);
  }

  .marker {
    position: absolute;
    left: -1px;
    top: 0;
    width: 2px;
    height: 18px;
    border-radius: 1px;
    background: var(--text);
    box-shadow: 0 0 0 2px var(--bg);
  }

  .names {
    margin-top: 8px;
    font-size: var(--fs-label);
    line-height: var(--lh-tight);
    color: var(--muted);
  }

  .names span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .names span.on {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
</style>
