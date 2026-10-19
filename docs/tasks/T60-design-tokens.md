# T60: Tokens und Schrift der Designsprache "Instrument"

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 |
| cloudfähig | ja |
| Voraussetzung | keine |
| Bereich | `app/ui` (`app.css`, `fonts.css`, `package.json`), `docs/SPEC.md`, `THIRD-PARTY-NOTICES.md` |
| SPEC | 5.5, 10.4 |

## Ziel

Die Grundlage der neuen Designsprache (E12, E13): Archivo statt Cairo, sieben Schriftstufen, Raster 4, Radien 6, 10 und 14, abgeleitete Töne aus den festen Farben, Bewegungsregeln. Danach sieht jede Ansicht schon ruhiger und dichter aus, ohne dass Markup sich ändert.

## Warum

Bestandsaufnahme in [../designsprache.md](../designsprache.md): Cairo hat eine kleine x-Höhe und eine Zeilenhöhe von 1,87, die Skala hat zehn zu nahe Stufen, Limette färbt Datenreihen, der Ansichtswechsel bewegt sich bei jedem Klick.

## Umfang

1. `@fontsource-variable/archivo` 5.3.0 statt `@fontsource-variable/cairo`; `fonts.css` bindet nur `latin-standard-normal` (Achsen wght und wdth) ein, die Datei liegt im Build, `font-src 'self'` bleibt.
2. `app.css`: Skala `--fs-metric` 64, `--fs-display` 40, `--fs-title` 24, `--fs-lead` 17, `--fs-body` 14, `--fs-small` 13, `--fs-label` 12; `--fs-dense` und `--fs-heading` entfallen oder zeigen auf eine der sieben Stufen. Zahlen in `--num-stretch` 75 %. Raster 4 (`--space-1` bis `--space-8`: 4, 8, 12, 16, 24, 32, 48, 64). Radien `--radius-chip` 6, `--radius-button` 10, `--radius-card` 14.
3. Abgeleitete Töne als Tokens nach Aufgabe (`--surface-hover`, `--accent-soft`, `--accent-line`, `--text-2`), gebildet mit `color-mix(in oklch, ...)` aus den festen Tokens. Kein neuer Farbton.
4. Limette nur für Urteil, Primäraktion, Fokus, aktive Navigation: `--pmc-ctl` wird neutral (`--text`), Sparklines und Heatmap nicht mehr in `--accent`.
5. `--glow`, `--glow-2` und der Verlauf im Inhaltsbereich entfallen. Der Ansichtswechsel (`main > *`, 160 ms) entfällt. `prefers-reduced-motion` entfernt Bewegung, lässt Überblendungen in `opacity` bis 150 ms stehen.
6. Knöpfe 32 hoch, Primäraktion 40, bei `pointer: coarse` 44.
7. Keine toten Alt-Stile: jede Regel in `app.css` wird genutzt oder fällt.

## Nicht im Umfang

Markup der Ansichten (T61 bis T65).

## Vorgaben

Kontrast jedes Textpaars mindestens 4,5 zu 1 in beiden Themen. Lizenz OFL 1.1 in `THIRD-PARTY-NOTICES.md` nachtragen.

## Abnahme

```
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-all.sh --quick
```

Build enthält eine woff2-Datei von Archivo und keine von Cairo.

## Manuelle Schritte

Sichtprüfung im Tauri-Fenster (Schrift wird geladen, keine CSP-Meldung).

## Blocker oder Vermerke

Keine.
