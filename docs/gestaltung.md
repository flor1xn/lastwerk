# Lastwerk-CI: Gestaltung

Verbindlich für jede Oberfläche in `app/ui`, auch für neue Features. Stand 19. Oktober 2026. Die Begründungen, die Recherche zu Linear, Apple Fitness, Oura, WHOOP, Raycast, Stripe, Vercel und Teenage Engineering sowie die verworfenen Richtungen stehen in [designsprache.md](designsprache.md), die Einzelheiten der Ansichten in SPEC 5.5. Bei Widerspruch gilt SPEC 5.5. Wer eine Regel brechen will, ändert erst dieses Dokument mit Grund.

Mechanisch geprüft wird ein Teil davon durch `app/ui/src/lib/design-rules.test.ts` (läuft mit `pnpm test`, also auch in der CI). Der Rest ist Prüfung mit dem Auge, siehe Checkliste am Ende.

## 1. Haltung

Lastwerk ist ein Messgerät mit einem Trainer daneben. Es sieht nach Werkzeug aus, nicht nach Vorlage. Drei Sätze tragen alles:

1. **Die Zahl ist die Überschrift.** Ein Satz steht nur, wo er sagt, was die Zahl nicht sagt.
2. **Ordnung kommt aus Schrift, Abstand und Fläche.** Nicht aus Rahmen, Kanten, Versalien, Pillen oder Farbe.
3. **Limette heißt "jetzt".** Urteil des Tages, Primäraktion, Fokus, aktive Navigation. Sonst nichts.

## 2. Was Lastwerk nie tut

Diese Muster sind typisch für generierte Standardoberflächen. Sie sind verboten, auch wenn sie im Einzelfall "ordentlich" aussehen.

| Muster | Stattdessen |
|---|---|
| Kasten mit farbigem Streifen an einer Seite (`border-left: 3px solid` in Akzent-, Warn- oder Sportfarbe, auch als `::before` oder `inset`-Schatten) | Fläche (`--panel`, `--panel-2`) oder gar kein Kasten; Zustand als Wort |
| Kleine Versalien mit Sperrung als Überschrift ("FORM", "WARUM") | Überschrift normal geschrieben, `--fs-heading`, halbfett, `--text` |
| Umrandete Kennzeichen, Pillen, Badges ("Aufbau", "leicht", "Aktiv", "R12") | Wort in der Zeile, Nebenangaben mit `.meta-line` und Mittelpunkt |
| Quadrat oder Punkt vor einem Zustandswort | Das Wort in seiner Farbe (`.state-mark`) |
| Unterstrichener Textverweis "Mehr anzeigen" | `.link-more` (Text mit Winkel ›), in Instrumenten `.inst-more` |
| Mehrere gleich laute Karten untereinander | Höchstens eine erhobene Fläche je Ansicht (`.card`), alles andere auf der Fläche |
| Rand und Schatten in voller Stärke zugleich | Hauchlinie `--ring` plus `--shadow-raised`, oder nur Fläche |
| Linien zwischen Kennzahlen in einer Reihe | Abstand (`.stat-row`) |
| Farbige Tönung ganzer Chips nach Sportart | Neutrale Fläche mit `.sport-dot` vor dem Namen |
| Limette für Daten, Hervorhebung oder "hart" | Neutrale Töne, Halbfett, kräftigerer Strich |
| Eigene Hex- oder rgb-Werte in Komponenten | Tokens aus `app.css`, abgeleitete Töne per `color-mix(in oklch, ...)` |
| Ampelfarben als einziger Träger eines Zustands | Wort plus Lage auf einer Skala |
| Bewegung beim Laden, Zählanimation, `transition: all` | Bewegung nur, wenn ein Wert sich ändert (Abschnitt 8) |

## 3. Farbe

Fest, nicht verhandelbar: die Tokens in `app/ui/src/app.css` (SPEC 5.5, Farben). Dunkel zuerst, hell gleichwertig.

- **Fläche:** `--bg` (Grund), `--panel` (erhobene Fläche, Arbeitsfläche), `--panel-2` (Feld, gewählte Zeile, Chip), `--surface-hover` (Überfahren, Suchfeld).
- **Linie:** `--line` (Haarlinie zwischen Abschnitten und Zeilen), `--ring` (Hauch um erhobene und schwebende Flächen), `--border` (Felder und Knöpfe), `--border-strong` (Hervorhebung ohne Farbe).
- **Text:** `--text`, `--text-2` (Aussagen, Nebentext mit Gewicht), `--muted` (Beschriftung, Einheit, Hinweis).
- **Signal:** `--accent` nur für "jetzt" (Abschnitt 1). Je Ansicht genau eine limettengrüne Fläche: die Primäraktion.
- **Zustand:** `--ok`, `--warn`, `--danger` und die Formfarben `--form-*` nur am Wort des Zustands, an der Marke einer Skala und an Diagrammen. Nie als Kante.
- **Sport:** `--sport-*` nur als `.sport-dot` und in Diagrammen.

Kontrast jedes Textpaars mindestens 4,5 zu 1 in beiden Themen. Wer einen neuen abgeleiteten Ton einführt, rechnet den Kontrast nach und trägt ihn in designsprache.md ein.

## 4. Schrift

Archivo mit Breitenachse, gebündelt, nie aus dem Netz. Zahlen schmal (`--num-stretch`, 75 %), Text normal breit. Ziffern tabellarisch.

| Token | Größe | Rolle |
|---|---|---|
| `--fs-metric` | 64 | größte Kennzahl eines Instruments |
| `--fs-display` | 40 | Titel der Tagesentscheidung, große Kennzahl |
| `--fs-title` | 24 | Seitentitel (auf Heute das Datum) |
| `--fs-lead` | 17 | Aussage in einem Satz |
| `--fs-heading` | 15 | Überschrift eines Abschnitts, Instruments, Panels |
| `--fs-body` | 14 | Fließtext, Tabellen, Felder |
| `--fs-small` | 13 | Beschriftung, Einheit, Nebenangaben |
| `--fs-label` | 12 | Tastenkürzel, Achsen, Fußnoten |

- **Laufweite je Größe** (Skill `apple-design`, Typografie): `--ls-display` für 40 und 64, `--ls-title` für 24, `--ls-lead` für 15 bis 17, `--ls-small` für 12 und 13, Fließtext 0. Andere Werte nur negativ.
- **Hierarchie aus Größe, Gewicht und Farbe zusammen.** Überschriften halbfett in `--text`, Beschriftungen mittel in `--muted`. Nie Versalien.
- **Satzbau:** Überschriften und Knöpfe im Satzanfang groß, sonst klein ("Diese Woche", "Mit Coach besprechen"). Keine Gedankenstriche, Nebenangaben mit Mittelpunkt.
- Code und Pfade in `--font-mono`.

## 5. Raster, Maße, Radien

- Raster 4: `--space-1` bis `--space-8` (4, 8, 12, 16, 24, 32, 48, 64). Keine anderen Abstände in neuen Komponenten.
- Radien: `--radius-chip` 6 (Tastenkürzel, Tooltip, Kalenderchip), `--radius-button` 10 (Knöpfe, Felder, Rückfragen), `--radius-card` 14 (Flächen). Runde Formen nur für Punkte und Schrittnummern. Kapseln nur im Umschalter `Segmented`.
- Seitenbreite `--page-max` 1240, Fließtext höchstens `--measure` (64 Zeichen). Arbeitsflächen (Aktivitäten, Kalender) volle Breite.
- Knöpfe 32 hoch, Primäraktion 40, mit grobem Zeiger 44.
- Ab 360 px Breite scrollt keine Ansicht waagerecht (Playwright prüft das).

## 6. Flächen und Tiefe

Nach dem Skill `apple-design`, Abschnitt Materialien: Tiefe zeigt Hierarchie, sie ist kein Schmuck.

| Ebene | Baustein | Aussehen |
|---|---|---|
| Grund | Seite | `--bg`, Abschnitte nur durch Abstand und eine Haarlinie `--line` |
| Arbeitsfläche | `.panel` | `--panel`, Hauch `--ring`, kein Schatten |
| Erhoben | `.card` | `--panel`, Hauch `--ring`, `--shadow-raised`; höchstens eine je Ansicht |
| Schwebend | Palette, Coach, Tooltip | `--panel` oder `--panel-2`, `--shadow-float`, darunter `--scrim` |
| Durchscheinend | Leiste unten (schmal) | `--sidebar` zu 80 %, `backdrop-filter: blur`; bei `prefers-reduced-transparency` volle Fläche |

Keine durchscheinende Fläche auf einer anderen durchscheinenden. Unschärfe höchstens 20 px.

## 7. Bausteine

Neue Oberflächen setzen sich aus diesen Bausteinen zusammen. Wer einen neuen braucht, beschreibt ihn hier.

- **Seitenkopf:** `h1` in `--fs-title`, darunter `.page-lead` als ein Satz. Auf Heute ist das Datum der Titel, Woche und Phase stehen leise darunter.
- **Abschnitt** (`Section`, `.section`): Haarlinie oben, Überschrift `h3` oder `.section-label`, Aussage `.section-statement` nur bei Bedarf.
- **Instrument** (`.instrument`): Überschrift, große schmale Zahl (`.inst-big`), Zustand als Wort, Beleg darunter, Verweis `.inst-more` unten.
- **Kennzahl** (`Metric`, `.metric`) und **Reihe** (`StatRow`, `.stat-row`): Zahl, Einheit klein, Beschriftung darunter. Getrennt durch Abstand.
- **Zustand** (`.state-mark` mit `fresh`, `balanced`, `loaded`, `overloaded`, `ok`, `warn`, `off`): nur das Wort.
- **Nebenangaben** (`.meta-line`): Spans in einer Zeile, Mittelpunkt als Trenner.
- **Verweis** (`.link-more`, `.inst-more`): Text mit Winkel, ohne Unterstrich. Unterstrichene Verweise nur in Tabellen (Datum führt zur Aktivität).
- **Knöpfe:** `button.primary` (eine je Ansicht, Limette), `button` (Fläche `--panel-2`), `button.ghost` (Nebenhandlung, wiederholte Handlung in Listen, Alternativen). Jeder Knopf gibt beim Drücken nach (`scale(0.97)`). Kürzel als `kbd` am Knopf.
- **Rückfrage** (`.confirm`): leicht getönte Fläche in Warnfarbe, Text sagt, was passiert, zwei Knöpfe.
- **Sportart** (`.sport-dot` mit `style:--sport`): runder Punkt vor dem Namen.
- **Reiter** (`Tabs`): Unterstrich 2 px in Limette unter dem aktiven Reiter, ohne Bewegung.
- **Tabellen:** dicht, Kopf in `--fs-small` `--muted`, Zeilen durch `--line` getrennt, Zahlen rechtsbündig.
- **Diagramme:** randlos, höchstens vier Gitterlinien, Achse nur an Enden und Monatswechseln beschriftet, Aussage als Überschrift darüber (Skill `dataviz`, SPEC 5.5).

## 8. Bewegung

Nach den Skills `emil-design-eng`, `animate` und `apple-design`.

- Bewegung erklärt eine Veränderung (Marke wandert, Balken wächst, Fenster kommt von seiner Seite). Sonst steht alles still.
- Was oft passiert, bewegt sich nicht: Palette, Reiter, Ansichtswechsel, alles per Tastatur.
- Nur `transform` und `opacity`. Kurven `--ease-out` (Eintritt), `--ease-in-out` (auf der Fläche), `--ease-drawer` (Seitenfenster). Nie `ease-in`, nie `transition: all`.
- Dauer 120 bis 200 ms, Seitenfenster 200 ms. Überfahren nur hinter `@media (hover: hover) and (pointer: fine)`.
- Bei `prefers-reduced-motion` fällt jede Ortsveränderung weg, kurze Überblendungen bleiben.

## 9. Zugang

- Zustand nie nur als Farbe: Wort, Form (gestrichelt, voll, Kreuz) oder Lage auf einer Skala.
- Jede Handlung per Tastatur erreichbar, Fokusrahmen 2 px Limette.
- Muster der WAI-ARIA Authoring Practices für Reiter, Palette, Rückfragen.
- Texte für Nutzer nur über `catalog/locales/de.json` und `en.json`.

## 10. Checkliste für jedes neue Feature

Vor dem Commit, zusätzlich zu `sh scripts/check-all.sh`:

1. Beantwortet die Ansicht zuerst ihre Frage, mit der Zahl als Überschrift?
2. Nur Bausteine aus Abschnitt 7, nur Tokens aus Abschnitt 3 bis 5?
3. Nichts aus der Liste in Abschnitt 2? (`pnpm test` prüft Kanten, Versalien, Sperrung, Pillen, Farbwerte, Schriften, Bewegung.)
4. Höchstens eine erhobene Fläche und genau eine Primäraktion?
5. Hell und dunkel, 360 und 1440 px angesehen (`vite preview` mit der Attrappe)? Härtetest mit Skill `break-ui`?
6. Bewegt sich etwas? Dann Skill `review-animations`.

## 11. Vorbilder und was wir von ihnen nehmen

Kurzfassung, Quellen in designsprache.md, Abschnitt 2.

- **Apple Fitness und Health:** Datum als Titel, Aussage über dem Diagramm, Zahlen groß, Tiefe statt Rahmen, Laufweite je Größe.
- **Linear:** wenige Grundfarben, abgeleitete Töne im wahrnehmungsgleichen Farbraum, Rauschen in Köpfen und Leisten gesenkt.
- **Raycast:** Palette ohne Bewegung, gewählte Zeile als Fläche, Kürzel am Befehl.
- **Oura und WHOOP:** ein Höhepunkt des Tages oben, drei Instrumente darunter. Nicht übernommen: Ampeln und Punktzahlen ohne Formel.
- **Teenage Engineering:** Charakter aus Schrift und Proportion, jedes Element mit einer Aufgabe.
