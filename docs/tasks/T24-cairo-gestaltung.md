# T24: Schrift Cairo und Gestaltung ohne Kachelraster

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | 1.0 |
| Voraussetzung | T20 |
| cloudfähig | ja |
| Bereich | `app/ui`, `catalog/locales` (nur falls Texte nötig), `docs/SPEC.md` 5.5 |
| SPEC | 5.5, 10.2, 10.4 |

## Ziel

Die ganze App nutzt Cairo, gebündelt im Build. Die Ansichten wirken modern und ruhig statt wie ein Raster gleich großer Kacheln. Grundlage für das Dashboard in T25.

## Umfang

1. Schrift: npm-Paket `@fontsource-variable/cairo` (OFL-1.1, Vermerk mit Version), importiert in `app/ui/src/main.ts` oder `app.css`, nur die Untermenge `latin` und `latin-ext` (keine arabischen Dateien ins Bundle, wenn das Paket getrennte Dateien anbietet). `font-family: 'Cairo Variable', system-ui, sans-serif` auf `:root`. Prüfen, ob Cairo `tnum` kennt: falls ja `font-feature-settings: 'tnum'` für Zahlen, falls nein Kennzahlen mit `font-variant-numeric: tabular-nums` testen und, wenn Ziffern springen, eine feste Breite je Ziffer setzen (`.num { min-width: …ch }`). Ergebnis vermerken.
2. Typoskala für Cairo neu: Cairo ist breit und hat große Ober- und Unterlängen; Zeilenhöhen und Größen in `app.css` als Tokens (`--fs-display`, `--fs-metric`, `--fs-title`, `--fs-body`, `--fs-label`, Zeilenhöhen) und überall verwenden.
3. Neue Bausteine statt `.card` überall:
   - `.section` mit Kopfzeile (Label in Versalien, optional Aussage in einem Satz) und feiner Trennlinie, ohne Kasten.
   - `.metric` (großer Wert, Einheit klein daneben, Beschriftung darunter, optional Veränderung mit Pfeil und Text).
   - `.stat-row` für mehrere Kennzahlen nebeneinander, getrennt durch dünne senkrechte Linien.
   - `.card` bleibt für interaktive Dinge (Empfehlung, Vorschlag des Coachs, Formulare).
   - Hintergrund mit sehr dezentem Verlauf oben (nur Tokens, kein Bild).
4. `TodayView`, `WeekView`, `SettingsView` auf die Bausteine umstellen, ohne Inhalte zu ändern (T25 baut das Dashboard aus). Plan, Aktivitäten und Detail nur Schrift und Abstände.
5. Diagramme (`lib/charts.ts`): Achsen und Beschriftung in Cairo, Gitter heller, keine Rahmen.
6. SPEC 5.5 nachziehen (Schrift, Bausteine, Ablösung der Kacheln).

## Vorgaben

- Skills lesen und anwenden: `emil-design-eng` zuerst, `dataviz` für Diagramme, `review-animations` falls Bewegung geändert wird, `break-ui` vor Abschluss.
- Keine Netzwerkzugriffe: Playwright-Guard muss grün bleiben, Schrift kommt aus `dist`.
- Kontrast mindestens 4,5:1 für Text in beiden Themen.
- Neue Abhängigkeit vermerken (Name, Version, Lizenz OFL-1.1, Grund). `pnpm audit` ohne neue Befunde.
- Screenshots (Playwright, de, hell und dunkel) der geänderten Ansichten in den Scratchpad, nicht ins Repository.

## Abnahme

```
pnpm --dir app/ui install --frozen-lockfile
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
pnpm --dir app/ui build
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
```

## Vermerke

- **Neue Abhängigkeit (15. Oktober 2026):** `@fontsource-variable/cairo` 5.3.0, Lizenz OFL-1.1 (SIL Open Font License, erlaubt Einbettung und Weitergabe mit der Schrift; Lizenztext im Paket), Grund: Entscheidung vom 15. Oktober 2026, Cairo überall und lokal gebündelt. Eingetragen in SPEC 5.3. Exakte Version im `package.json` wie bei den übrigen Laufzeitabhängigkeiten. `pnpm audit` ohne Befunde.
- **Untermengen:** Das Paket liefert latin, latin-ext und arabic als getrennte Dateien. `wght.css` des Pakets würde alle drei ins Bundle ziehen, deshalb steht in `app/ui/src/fonts.css` ein eigenes `@font-face` für latin und latin-ext mit den Unicode-Bereichen aus dem Paket. Der Build erzeugt genau `cairo-latin-wght-normal-*.woff2` (33,8 KB) und `cairo-latin-ext-wght-normal-*.woff2` (16,6 KB) in `dist/assets`. Playwright-Prüfung: Die Datei kommt von `127.0.0.1` (Herkunft der Seite), `document.fonts` meldet `loaded`, Guard und CSP (`font-src 'self'`) bleiben grün. Die Datei wird über einen relativen Pfad nach `node_modules/@fontsource-variable/cairo/files/` gelesen, bei einem Versionswechsel des Pakets die Dateinamen prüfen.
- **tnum:** Cairo kennt kein OpenType-Merkmal `tnum` (GSUB hat nur `dnom`, `frac`, `numr`, `rvrn`, `locl`). Gemessen mit fontTools (Instanzen bei Gewicht 300, 400, 600, 650, 800): Alle zehn Ziffern sind bei jedem Gewicht gleich breit (560, 560, 559, 558, 555 Einheiten bei 1000 pro em), ebenso das Minuszeichen U+2212. Zahlen springen daher nicht, eine feste Breite (`.num { min-width }`) ist nicht nötig. `font-variant-numeric: tabular-nums` bleibt als Schutz für die Ersatzschrift gesetzt.
- **Abweichung von SPEC 5.5 (vorher):** Beschriftung in Versalien 12 statt 13 Pixel, in SPEC 5.5 nachgezogen. Kennzahlen 60, Empfehlung 48, Text 15 (Fließtext einen Punkt größer als vorher wegen der kleinen x-Höhe von Cairo).
- **Texte:** neu `ui.lead.today` (de, en), entfallen `ui.today.form_values` (Fitness und Ermüdung stehen jetzt als Kennzahlen). Sonst keine Inhalte geändert, keine neuen Daten.
- **Bausteine für T25:** `Section` (`label`, `statement?`, `testid?`, `aside?`-Snippet, Inhalt), `Metric` (`value`, `unit?`, `label`, `delta? {text, direction: up|down|flat, tone?: good|bad|neutral}`, `size? md|lg|xl`, `swatch?`, `testid?`, `valueTestid?`, `extra?`-Snippet), `StatRow` (`testid?`, Inhalt) in `app/ui/src/lib/components/`, Klassen `.section`, `.metric`, `.stat-row`, `.pill`, `.page`, `.page-head` in `app.css`. Strukturtests in `blocks.test.ts`.
- **Verifikation:** Screenshots (de, hell und dunkel) von Heute, Woche, Einstellungen und der Ansichten Plan, Aktivitäten, Detail liegen im Scratchpad, nicht im Repository. `break-ui`: lange Vorlagennamen ohne Leerzeichen, sechs- bis siebenstellige Kennzahlen, lange Aussagen und schmale Fenster (820 und 900 Pixel) brechen um (`overflow-wrap`, `text-wrap: balance`), nichts läuft aus Karte oder Spalte. Kontrast gerechnet (WCAG): Text 15,2 bis 15,4, gedämpfter Text dunkel 6,5 bis 7,2 und hell 5,0 bis 6,1, Limette als Text hell 5,2, Pillen hell mindestens 5,1.
- **Nicht gemacht:** keine Bewegung geändert (Ansichtswechsel unverändert 160 ms), die Wizard-Ansicht nur in Schrift und Größen angepasst.
