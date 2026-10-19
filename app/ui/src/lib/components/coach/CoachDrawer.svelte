<script lang="ts">
  import { tick } from 'svelte';
  import { t } from '../../i18n';
  import { bodyTab, closeCoach, coachFocus, coachOpen } from '../../state';
  import CoachChat from './CoachChat.svelte';

  // Seitenfenster des Coachs (T43, E9): rechts, nicht modal. Ab 1100 px steht es neben dem
  // Inhalt, darunter liegt es mit Hintergrund darüber. Das Gespräch selbst lebt in
  // `coach-session.svelte.ts`, das Fenster zeigt es nur.

  interface Props {
    /** Das Fenster wurde per Tastenkürzel geöffnet oder geschlossen: ohne Bewegung (Faustregel
     *  nach emil-design-eng: Was man per Tastatur hundertfach auslöst, animiert man nicht). */
    instant?: boolean;
  }

  let { instant = false }: Props = $props();

  let panel = $state<HTMLElement | null>(null);
  // Das Gespräch wird erst beim ersten Öffnen gebaut: Vorher fragt die App keinen Modellserver.
  let everOpened = $state(false);
  // Element, das vor dem Öffnen den Fokus hatte (meist der Knopf oder das Kürzel), für die
  // Rückkehr beim Schließen.
  let returnTo: HTMLElement | null = null;

  $effect(() => {
    if ($coachOpen) {
      everOpened = true;
      const active = document.activeElement;
      returnTo =
        active instanceof HTMLElement && !panel?.contains(active) && active !== document.body
          ? active
          : null;
      void tick().then(() => focusInside());
    } else if (returnTo) {
      const target = returnTo;
      returnTo = null;
      if (panel?.contains(document.activeElement) || document.activeElement === document.body) {
        target.focus();
      }
    }
  });

  function focusInside() {
    if (!panel) return;
    const input = panel.querySelector<HTMLTextAreaElement>('textarea:not(:disabled)');
    (input ?? panel.querySelector<HTMLElement>('[data-testid="coach-close"]'))?.focus();
  }

  // Die Einrichtung und der Status laden nach dem Öffnen; sobald das Eingabefeld erscheint,
  // bekommt es den Fokus, solange er noch im Fenster auf dem Schließen-Knopf steht.
  $effect(() => {
    if (!$coachOpen || !panel) return;
    const observer = new MutationObserver(() => {
      const close = panel?.querySelector('[data-testid="coach-close"]');
      if (document.activeElement === close) {
        const input = panel?.querySelector<HTMLTextAreaElement>('textarea:not(:disabled)');
        input?.focus();
      }
    });
    observer.observe(panel, { childList: true, subtree: true, attributes: true });
    return () => observer.disconnect();
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      closeCoach();
    }
  }

  // Esc schließt auch, wenn der Fokus nirgends steht (etwa nachdem ein Knopf im Fenster
  // verschwunden ist). Steht er im Inhalt daneben, gehört Esc dem Inhalt: Das Fenster ist nicht
  // modal.
  function onWindowKeydown(e: KeyboardEvent) {
    if (!$coachOpen || e.key !== 'Escape' || e.defaultPrevented) return;
    const active = document.activeElement;
    if (active === null || active === document.body) {
      e.preventDefault();
      closeCoach();
    }
  }

  const contextKey = $derived(`ui.coach.context.view.${$coachFocus.view}`);
  const topicKey = $derived(
    $coachFocus.topic ? `ui.coach.context.topic.${$coachFocus.topic.split('.')[0]}` : null,
  );
</script>

<svelte:window onkeydown={onWindowKeydown} />

{#if $coachOpen}
  <!-- Nur unter 1100 px sichtbar (CSS): Klick daneben schließt wie Esc. -->
  <div class="backdrop" role="presentation" data-testid="coach-backdrop" onclick={closeCoach}></div>
{/if}

<aside
  id="coach-drawer"
  class="drawer"
  class:open={$coachOpen}
  class:instant
  aria-label={$t('ui.coach.title')}
  aria-hidden={!$coachOpen}
  inert={!$coachOpen}
  bind:this={panel}
  {onkeydown}
  data-testid="coach-drawer"
  data-open={$coachOpen}
>
  <header class="drawer-head">
    <h2>{$t('ui.coach.title')}</h2>
    <button
      type="button"
      class="close"
      onclick={closeCoach}
      aria-label={$t('ui.coach.drawer.close')}
      title={$t('ui.coach.drawer.close_hint')}
      data-testid="coach-close"
    >
      <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
        <path
          d="M4 4l8 8M12 4l-8 8"
          stroke="currentColor"
          stroke-width="1.6"
          stroke-linecap="round"
        />
      </svg>
    </button>
  </header>
  <p class="context" data-testid="coach-context">
    {$t('ui.coach.context.label', {
      where:
        $t(contextKey, { body: $t(`ui.coach.context.body.${$bodyTab}`) }) +
        (topicKey ? ` · ${$t(topicKey)}` : ''),
    })}
  </p>
  <div class="drawer-body">
    {#if everOpened}
      <CoachChat />
    {/if}
  </div>
</aside>

<style>
  /* Breite 400 px (T43). Sichtbarkeit wechselt erst nach der Bewegung, damit das Fenster
     hinausgleitet und danach weder Tab-Taste noch Bildschirmleser es finden. */
  .drawer {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 40;
    width: min(400px, 100vw);
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    padding: var(--space-4) var(--space-4) var(--space-3);
    background: var(--panel);
    border-left: 1px solid var(--ring);
    transform: translateX(100%);
    visibility: hidden;
    transition:
      transform 200ms var(--ease-drawer),
      visibility 0s linear 200ms;
  }

  /* Öffnen per Klick in 200 ms mit der Kurve für Seitenfenster (SPEC 5.5, Skill
     emil-design-eng: Modale und Seitenfenster 200 bis 500 ms). Per Tastatur ohne Bewegung. */
  .drawer.open {
    transform: none;
    visibility: visible;
    transition:
      transform 200ms var(--ease-drawer),
      visibility 0s;
  }

  .drawer.instant,
  .drawer.instant.open {
    transition: none;
  }

  .drawer-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .drawer-head h2 {
    margin: 0;
    font-size: var(--fs-lead);
    line-height: var(--lh-tight);
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    min-height: 32px;
    padding: 0;
    background: none;
    border-color: transparent;
    color: var(--muted);
  }

  @media (hover: hover) and (pointer: fine) {
    .close:hover:not(:disabled) {
      border-color: transparent;
      color: var(--text);
    }
  }

  .context {
    margin: var(--space-1) 0 var(--space-2);
    font-size: var(--fs-small);
    line-height: var(--lh-tight);
    color: var(--muted);
    overflow-wrap: anywhere;
  }

  .drawer-body {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
  }

  .backdrop {
    display: none;
  }

  /* Schmaler als 1100 px liegt das Fenster über dem Inhalt, mit Hintergrund. */
  @media (max-width: 1099px) {
    .backdrop {
      display: block;
      position: fixed;
      inset: 0;
      z-index: 39;
      background: var(--scrim);
    }

    /* Über dem Inhalt schwebt das Fenster: Tiefe aus dem Schatten der schwebenden Ebene. */
    .drawer {
      box-shadow: var(--shadow-float);
    }
  }

  /* Schmal: Blatt über die volle Breite, über der Leiste unten (SPEC 5.5). */
  @media (max-width: 759px) {
    .drawer {
      width: 100vw;
      border-left: 0;
      padding-bottom: calc(var(--space-3) + env(safe-area-inset-bottom));
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .drawer,
    .drawer.open {
      transition: none;
    }
  }
</style>
