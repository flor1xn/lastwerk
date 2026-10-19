<script lang="ts">
  import type { Snippet } from 'svelte';

  // Abschnitt ohne Kasten (SPEC 5.5, seit T24): Marke in Versalien, optional eine Aussage in
  // einem Satz, darüber eine feine Linie. Die Klassen stehen in app.css, damit Markup ohne
  // diese Hülle (zum Beispiel Einstellungen mit eigenem <h3>) dasselbe Bild ergibt.
  interface Props {
    /** Marke, klein und in Versalien (gesetzt per CSS). */
    label: string;
    /** Aussage in einem Satz unter der Marke. */
    statement?: string;
    testid?: string;
    /** Rechts neben der Kopfzeile, zum Beispiel eine Zustandsmarke. */
    aside?: Snippet;
    children: Snippet;
  }

  let { label, statement, testid, aside, children }: Props = $props();
</script>

<section class="section" data-testid={testid}>
  <header class="section-head">
    <div class="section-title">
      <h2 class="section-label">{label}</h2>
      {#if statement}
        <p class="section-statement">{statement}</p>
      {/if}
    </div>
    {#if aside}
      {@render aside()}
    {/if}
  </header>
  {@render children()}
</section>
