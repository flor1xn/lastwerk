<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import uPlot from 'uplot';
  import 'uplot/dist/uPlot.min.css';

  interface Props {
    options: Omit<uPlot.Options, 'width' | 'height'>;
    data: uPlot.AlignedData;
    height?: number;
    /** Text für Hilfstechniken, das Diagramm selbst ist ein Canvas. */
    label: string;
    /** Meldet den Index des Datenpunkts unter dem Zeiger, null beim Verlassen des Diagramms. */
    oncursor?: (index: number | null) => void;
  }

  let { options, data, height = 240, label, oncursor }: Props = $props();

  let host: HTMLDivElement;
  let plot: uPlot | null = null;
  let width = $state(0);

  // Aufbauen, wenn sich Optionen (Thema, Sprache) oder Daten ändern. Die Breite ist hier
  // ausgenommen, eine neue Breite setzt nur die Größe.
  $effect(() => {
    const opts = options;
    const values = data;
    const h = height;
    if (!host) return;
    const created = new uPlot(
      {
        ...opts,
        hooks: {
          ...opts.hooks,
          setCursor: [
            ...(opts.hooks?.setCursor ?? []),
            (u: uPlot) => untrack(() => oncursor)?.(u.cursor.idx ?? null),
          ],
        },
        width: Math.max(
          untrack(() => width),
          200,
        ),
        height: h,
      },
      values,
      host,
    );
    plot = created;
    // Das Canvas misst und zeichnet mit der Schrift, die beim Aufbau schon geladen ist. Archivo
    // kommt als Datei aus dem Build und kann etwas später da sein: danach Achsen neu messen.
    // jsdom kennt document.fonts nicht (dort ersetzt eine Attrappe das Diagramm ohnehin).
    void document.fonts?.load?.(`13px "Archivo Variable"`).then(() => {
      if (plot === created) created.redraw(false, true);
    });
    return () => {
      plot?.destroy();
      plot = null;
    };
  });

  $effect(() => {
    const w = Math.max(width, 200);
    untrack(() => plot?.setSize({ width: w, height }));
  });

  onMount(() => {
    const observer = new ResizeObserver((entries) => {
      width = Math.floor(entries[0]?.contentRect.width ?? 0);
    });
    observer.observe(host);
    return () => observer.disconnect();
  });
</script>

<div class="chart" bind:this={host} role="img" aria-label={label}></div>

<style>
  .chart {
    width: 100%;
    min-height: 40px;
  }
</style>
