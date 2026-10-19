# Designsprache: Bestandsaufnahme, Recherche, Richtungen (18. Oktober 2026)

Vermerk zu Phase 1 bis 3 des Auftrags "Designsprache auf ein neues Niveau heben". Er begründet die neue Fassung von SPEC 5.5.

**Entscheidung des Nutzers (19. Oktober 2026)**

- **E12** Richtung B "Instrument" mit den Anleihen aus A (Abschnitt 3, Empfehlung).
- **E13** Schrift Archivo mit Breitenachse ersetzt Cairo.
- **E14** Strg+K öffnet eine Befehlspalette für alles; Text ohne passenden Befehl wird zur Frage an den Coach, Strg+J öffnet den Coach direkt. Löst den Teil "Kürzel" von E9 ab.
- REA ist noch nicht eingerichtet, der Nutzer will es nutzen. Phase 2b folgt nach der Einrichtung durch den Nutzer in einer lokalen Session ([T66](tasks/T66-design-pruefung.md)); der Umbau wartet nicht darauf.

Umsetzung in den Tasks T60 bis T66 ([tasks/README.md](tasks/README.md)).

Fest und nicht Teil der Abwägung: die Farbtokens aus `app.css` (dunkel und hell), die harten Regeln aus [CLAUDE.md](../CLAUDE.md), Kontrast mindestens 4,5 zu 1, Zustand nie nur als Farbe, Tastaturbedienung, `prefers-reduced-motion`.

## 1. Bestandsaufnahme

Grundlage: 52 Bildschirmfotos der Attrappe (`vite preview`), alle Hauptansichten und Reiter, dunkel und hell, 420 und 1440 px breit, dazu die Varianten `?dashboard=empty&decision=no_plan`, `?dashboard=extreme&decision=worst` und `?plan=laufend`. Die Bilder liegen nur lokal im Scratchpad der Session, nicht im Repository.

Ausgangsmaße des Builds (`pnpm run build`, Stand `1e5103a`): JavaScript 550,7 KB (gzip 178,2 KB), CSS 69,3 KB (gzip 13,1 KB), Schrift 50,5 KB (zwei woff2-Dateien).

Kontrast der festen Tokens (WCAG 2.x, gerechnet): `--muted` auf `--bg` 6,98, auf `--panel-2` 5,71; `--accent` auf `--bg` 14,40; `--accent-text` auf `--accent` 14,19; hell `--muted` auf `--bg` 5,45, Weiß auf hellem `--accent` 5,75, `--warn` 5,75. Alle Paare erfüllen 4,5 zu 1.

### Die fünf größten Schwächen

1. **Kein schmales Layout.** Bei 420 px nimmt die Seitenleiste 208 px fest ein, dem Inhalt bleiben rund 150 px. "Noch kein aktiver Plan" bricht Buchstabe für Buchstabe, die Extremvariante wird über 10 000 px hoch (Bilder `today-normal-dark-420`, `today-extreme-dark-420`). Das Tauri-Fenster lässt sich so schmal ziehen, und der Coach legt sich unter 1100 px zusätzlich darüber.
2. **Heute beantwortet "Wie bin ich dran" erst unter der Falz.** Bei 1440 mal 900 sieht man Datum, den Titel "Heute" (der dasselbe sagt wie die Navigation), eine gedämpfte Aussage, die Entscheidungskarte und die Erkenntnisse. Der Formwert TSB steht bei rund 1300 px, die Bereitschaft bei rund 950 px (`today-plan-dark-1440`). In der Karte ist die Vorlage mit 48 px das Größte, das Urteil "Heute wie geplant" steht mit 15 px darüber. Die Karte ist 1100 px breit, der Inhalt nutzt die linke Hälfte.
3. **Stapel gleich lauter Abschnitte, Aussagen doppelt.** Heute hat sieben Abschnitte, jeder mit Marke, einem Satz in 20 px Halbfett und Zahlen. Kein Abschnitt führt, alle Sätze konkurrieren. Dieselbe Tatsache steht mehrfach: "77 % der geplanten Last" in den Erkenntnissen und in der Wochen-Story, der Formzustand im Leitsatz, im Grund der Entscheidung, im Satz des Abschnitts und in der Pille. Im Leerzustand sagen sechs Abschnitte untereinander, was fehlt (`today-empty-light-1440`): eine Wand aus Abwesenheit statt eines Wegs zum Start.
4. **Diagramme mit Rauschen und Fehlern.** Im Form-Verlauf überlagern die Bandbeschriftungen die Linie oder werden am linken Rand abgeschnitten ("AUSGEWOGEN" über der TSB-Linie, nur "H" und "N" sichtbar). Die gefüllten Bänder dominieren, die zackige Ermüdung zieht mehr Blick als die Fitness, 22 Datumsmarken. Unter den Tagesbalken steht viermal "1× nicht berechenbar" kursiv in Warnfarbe, obwohl nichts falsch ist. Limette färbt Primäraktion, Navigation, Marke, drei Sparklines, die Fitnesslinie, die Rampe, die Heatmap und den heutigen Kalendertag; sie zeigt damit nicht mehr, "was jetzt zählt". Die Verteilung färbt Zone 1 und 2 in der Farbe des Radsports.
5. **Plan und Aktivitäten als endlose Listen.** Die Planansicht ist 5200 px hoch: Die drei Fragen stehen oben, auch wenn ein Plan läuft, darunter acht voll ausgeklappte Wochen mit gleich gewichteten Zeilen; "diese Woche" hebt sich nicht ab (`plan-dark-1440`). Die Aktivitätenliste zeigt 81 Zeilen ohne Namen und in jeder zweiten Zeile "nicht berechenbar".

Nebenbefunde für den Umbau: Ein unbekannter Vorlagenname erscheint als roher Schlüssel `template.bike...name` in 48 px (`today-extreme`). In der Attrappe widerspricht die feste Entscheidung den Formdaten ("Form frisch" im Grund, Pille "Ausgewogen" bei TSB −2), weil `fixedDecision` die Form nicht liest; die App selbst ist davon nicht betroffen. Die Seitenleiste hat bei drei Punkten rund 60 % leere Höhe.

### Regeln aus SPEC 5.5 mit Urteil

Das Urteil gilt für die empfohlene Richtung B. Wo A oder C anders entscheiden, steht es in Abschnitt 3.

| Regel heute | Urteil | Grund |
|---|---|---|
| Haltung: Trainer, kein Tabellenblatt; jede Hauptansicht beginnt mit einer Aussage | behalten, schärfen | Die Aussage ist auf Heute die Entscheidung selbst, nicht ein Satz unter dem Titel "Heute". Ein führender Satz je Seite statt einer je Abschnitt |
| Drei Hauptpunkte und Einstellungen (E8) | behalten | Inhalt stimmt (Oura hat fünf Reiter auf drei verdichtet). Die Form ändert sich, siehe Navigation |
| Seitenleiste 208 px mit Text | ändern | 60 % leer, kostet bei schmalem Fenster alles. Schmale Leiste 56 px mit Symbolen, Namen als Tooltip mit Kürzel; unter 760 px Leiste unten |
| Reiter nach WAI-ARIA, wandernder Tabindex | behalten | Richtig und geprüft |
| Reiter bewegen sich nicht | behalten | Wird oft gewechselt; häufige Handlungen bewegen sich nicht (Skill `emil-design-eng`) |
| Jede Wahl an genau einer Stelle | behalten, präzisieren | Die Befehlspalette ist ein Weg zu dieser Stelle oder löst die Handlung aus, sie hält keine eigene Einstellung |
| Strg+K öffnet den Coach (E9) | ändern, Entscheidung des Nutzers | Strg+K öffnet eine Befehlspalette für alles (springen, Reiter, Aktivität suchen, Plan erstellen, Coach fragen). Text ohne passenden Befehl wird mit Eingabe zur Frage an den Coach. Der Coach bekommt zusätzlich Strg+J direkt |
| Coach als nicht modales Seitenfenster rechts, 400 px | behalten, ergänzen | Bewährt. Unter 760 px als Blatt über die volle Breite |
| Heute, Aufbau T44 (Karte, Erkenntnisse, Bereitschaft, Form-Verlauf, Wochen-Story, Rampe, Verteilung) | ändern | Entscheidung als Streifen, darunter drei Instrumente (Form, Erholung, Woche) über der Falz. Erkenntnisse als höchstens drei Notizzeilen, nur was kein Instrument schon zeigt. Rampe und Verteilung sind Trends, keine Tagesfrage: sie gehen nach Verlauf, Fortschritt. Das volle Form-Diagramm steht im Verlauf, Heute zeigt sechs Wochen TSB ohne Achsen |
| Seitentitel `h1` "Heute" mit Datum darüber und Leitsatz | streichen auf Heute | Die Navigation sagt "Heute". Die Kopfzeile trägt Datum, Planwoche und Phase |
| Formzustand: vier Bänder, Grenzen −30 (R10), −10 und +5 (Faustregel) | behalten | Inhalt unverändert. Darstellung neu als Bandskala mit Marke statt Pille |
| Farben, Zustandsfarben in Helligkeit und Text | fest | |
| Limette für das, was jetzt zählt | behalten, strenger | Limette nur für Primäraktion, aktive Navigation, Fokus und das Urteil des Tages. Nie für Datenreihen; Fitness, Sparklines und Heatmap werden neutral oder bekommen ihre Sportfarbe |
| Verlauf oben im Inhaltsbereich (`--glow`, `--glow-2`) | streichen | Dekoration ohne Aufgabe; `--glow-2` ist bläulich und damit ein zweiter Farbton neben der Marke |
| Schrift Cairo | ändern, Entscheidung des Nutzers | Siehe Schriftprüfung unten |
| Skala mit zehn Stufen (60/48/34/32/20/17/15/14/13/12) | ändern | Zu viele, zu nah beieinander (13/14/15, 32/34). Sieben Stufen mit größeren Sprüngen: 64 (Kennzahl), 40 (Entscheidung), 24 (Seitentitel), 17 (Aussage), 14 (Text, Tabellen), 13 (Erläuterung), 12 (Marke in Versalien). Faustregel, geprüft an den Skizzen |
| Kein Kachelraster | behalten | Instrumente sind Spalten auf der Fläche mit Haarlinien, keine Kacheln |
| Höchstens eine Karte je Seite, Karten nur für Bedienbares | behalten | Auf Heute ist der Entscheidungsstreifen die einzige erhobene Fläche |
| Eine Primäraktion je Karte | behalten, ergänzen | Jede Primäraktion zeigt ihr Kürzel (Raycast: Kürzel lernt man durch Sehen) |
| Abschnitt mit Marke und Satz in 20 px | ändern | Ein Satz nur, wo er sagt, was die Zahl nicht sagt. Sonst Marke und Zahl |
| `Metric`, `StatRow` | behalten, neu gesetzt | Zahl in schmaler Breite, Einheit und Marke in normaler |
| Pille mit Punkt | ändern | Quadratische Marke mit Text, ruhiger, passt zur Instrumentsprache |
| Raster 8, Abstände 4 bis 48 | ändern | Raster 4 mit 4, 8, 12, 16, 24, 32, 48, 64. Dichte Zeilen brauchen 4er-Schritte |
| Radien 12 (Knöpfe), 16 bis 20 (Karten), 999 (Pillen) | ändern | 6 (Chips, Tasten), 10 (Knöpfe, Felder), 14 (Flächen). Weniger weich, passt zu einem Messwerkzeug |
| Primäraktionen 44 hoch, übrige 36 | ändern | 40 und 32 bei Maus, 44 bei `pointer: coarse` |
| Seitenbreite 1080 und 1200 | ändern | Eine Breite 1240 für Heute und Verlauf, Fließtext höchstens 64 Zeichen, Arbeitsflächen (Aktivitäten, Kalender) volle Breite |
| Diagramme randlos, eigene Legende, Gitter waagerecht | behalten, ergänzen | Höchstens vier Gitterlinien, Achse nur an den Enden und an Monatswechseln beschriftet (Apple, WWDC22 "Design an effective chart"). Bandnamen außerhalb der Zeichenfläche. Der Zeiger ersetzt die große Zahl über dem Diagramm durch den Wert des Tages, statt eine Legende zu füllen |
| Bewegung nur CSS, `transform` und `opacity`, 120 bis 200 ms, nichts bewegt sich beim Laden | behalten, ändern | Der Ansichtswechsel (160 ms beim Wechsel jeder Ansicht) fällt: häufig und ohne Erkenntnis. Neu: Marken und Balken gleiten 200 ms, wenn sich ein Wert ändert, damit die Änderung lesbar wird. Keine Zählanimation von Zahlen. Palette ohne Bewegung |
| `prefers-reduced-motion` schaltet alles ab | ändern | Bewegung fällt weg, kurze Überblendungen in `opacity` bleiben (weniger, nicht null; Skill `emil-design-eng`) |
| Tabellen in Aktivitäten bewusst dicht | behalten, ergänzen | Name der Aktivität, Sportfarbe als Marke, "nicht berechenbar" als leiser Strich mit Erklärung im Titel |
| Planansicht: drei Fragen oben, Plan darunter | ändern | Mit laufendem Plan oben "Diese Woche", die übrigen Wochen als eingeklappte Zeilen. Die drei Fragen nur ohne Plan oder über "Neuer Plan" |
| Erststart-Assistent | behalten | Gleiche Bausteine, schmal tauglich |
| Abgleichzeile Garmin in der Leiste | ändern | Statuspunkt an der Leiste mit Tooltip, Details in der Palette und in den Einstellungen |

### Schriftprüfung

Gemessen an den woff2-Dateien der Pakete `@fontsource-variable/*` 5.3 (Untermenge latin, bei Bedarf latin-ext), alle unter SIL Open Font License 1.1. Breite ist die Summe der Vorschübe für "Belastung Erholung Leistung Woche" in em. Gerendert in Chromium bei 12, 13, 14 und 64 px auf `--bg`.

| Schrift | Größe latin (+ ext) | x-Höhe | Zeilenhöhe `normal` | Ziffern | Breite | Achsen | Urteil |
|---|---|---|---|---|---|---|---|
| Cairo (heute) | 33,8 (+16,6) KB | 0,50 | 1,87 | gleich breit, kein `tnum` | 15,2 | wght | Nicht breit, aber kleine x-Höhe und sehr hohe Zeilenhöhe; bei 12 px am schwächsten lesbar. Weiche, runde Ziffern |
| Inter | 71,2 KB mit opsz | 0,55 | 1,21 | `tnum` | 17,1 | opsz, wght | Beste Lesbarkeit bei 12 px, optische Größe für Anzeigen. Überall zu sehen, wenig eigen |
| Geist | 44,8 KB mit ext | 0,53 | 1,30 | `tnum` | 16,9 | wght | Klare Ziffern, klein. Eng mit der Marke Vercel verbunden |
| Archivo | 88,0 KB mit wdth | 0,53 | 1,09 | `tnum`, fast gleich | 16,8 | wdth 62 bis 125, wght | Breitenachse: Zahlen schmal und groß, Text normal. Eigener Charakter, gute Lesbarkeit bei 13 px |
| IBM Plex Sans | 64,0 KB mit wdth | 0,52 | 1,30 | gleich breit | 16,1 | wdth, wght | Unterscheidet I, l, 1 am besten. Technisch, etwas kühl |
| Instrument Sans | 56,0 KB mit wdth | 0,51 | 1,22 | `tnum` | 16,9 | wdth, wght (400 bis 700) | Eng gesetzt, bei 12 px gedrängt |

Empfehlung für B: Archivo mit Breitenachse (Zahlen wdth 75, Text 100), nur latin, rund 88 KB statt 50 KB. Die Pakete liefern keine Stilvarianten (`ss01`, `zero`); die Ziffer 0 und der Buchstabe O unterscheiden sich in Archivo ausreichend durch die Breite. Ausweichen auf Inter, wenn Lesbarkeit vor Charakter geht.

## 2. Recherche (nur öffentlich, ohne Anmeldung)

Abgerufen am 18. Oktober 2026. REA (Phase 2b) wurde nicht genutzt; ob der Nutzer es lokal eingerichtet hat, ist offen. Aus den Beobachtungen werden Prinzipien in eigenen Worten; nichts davon ist eine Quelle für Zahlen in Lastwerk.

**Linear** ([Redesign](https://linear.app/now/how-we-redesigned-the-linear-ui), [Farbauffrischung](https://linear.app/now/behind-the-latest-design-refresh)). (1) Themen aus drei Größen (Basis, Akzent, Kontrast) in einem wahrnehmungsgleichen Farbraum statt 98 einzelner Werte. Passt: abgeleitete Töne aus den festen Tokens per `color-mix(in oklch, ...)` statt neuer Hexwerte. (2) Rauschen in Leiste, Reitern und Köpfen gesenkt, Ausrichtung auf kleinster Fläche. Passt. (3) Eine Anzeigeschnitt für Überschriften, der Textschnitt für alles andere. Passt als Prinzip "optische Größe": große Zahlen anders gesetzt als Text. Passt nicht: Listendichte als Grundmuster jeder Seite.

**Apple Fitness und Health** ([WWDC22 "Design an effective chart"](https://developer.apple.com/videos/play/wwdc2022/110340/)). (1) Über dem Diagramm steht die Aussage als Überschrift und der zusammenfassende Wert, nicht eine kleine Achsenbeschriftung. Passt, ersetzt die Legende unter dem Diagramm. (2) Rund vier waagerechte Gitterlinien mit runden Werten, Enden der Zeitachse beschriftet, Vorschaudiagramme ganz ohne Achsen. Passt: Sparklines auf Heute ohne Achsen, Verlauf mit vier Linien. (3) Farbe ergänzt Form und Text, trägt nie allein; Farben gleich schwer gewichten. Passt, ist schon Regel. Passt nicht: Ringe. Sie leben von festen Tageszielen und sind Apples Erkennungszeichen.

**Oura** ([neue App](https://ouraring.com/blog/new-oura-app-experience/)). (1) Heute wie die Titelseite einer Nachrichtenapp: ein Höhepunkt des Tages, ausgewählt nach Zustand und Tageszeit. Passt: Heute zeigt, was heute anders ist. (2) Fünf Reiter auf drei verdichtet. Bestätigt E8. (3) Kennzahlen oben als Einstieg, Tiefe dahinter. Passt. Passt nicht: zusammengesetzte Punktzahlen ohne veröffentlichte Formel (SPEC 2.3, Anhang A).

**WHOOP** ([Startseite](https://www.whoop.com/thelocker/the-all-new-whoop-home-screen/), Inhalt über Suchauszug, Seite selbst gesperrt; [Bericht](https://the5krunner.com/2023/03/28/new-whoop-home-screen-looks-pretty-but-is-it-as-intuitive/)). (1) Drei Instrumente nebeneinander oben, Reihenfolge nach dem Tagesablauf. Passt, Grundlage von Richtung B. (2) Erholung als Ampel. Passt nicht: Zustand nur über Farbe und Risikoampeln sind ausgeschlossen (Marktabgleich, "Bewusst nicht übernommen"). (3) Empfehlung hängt sichtbar an der Erholung. Passt, ist die Tagesentscheidung.

**intervals.icu, TrainingPeaks, Strava** (als Gegenbeispiel; [Forum intervals.icu "It's time to fix the UI"](https://forum.intervals.icu/t/its-time-to-fix-the-ui/101779)). (1) Neue Nutzer lehnen das Werkzeug wegen Überfülle ab; gewünscht ist ein einfacher Start mit Tiefe auf Abruf. Lehre: Heute zeigt kein volles Fitnessdiagramm. (2) Einstellungen über Leisten, Fußzeilen und Fenster verstreut. Lehre: jede Wahl an einer Stelle (gilt schon). (3) Drei Linien auf einer Achse ohne Deutung (PMC, eigene Beobachtung an öffentlichen Hilfeseiten): verständlich nur für Eingeweihte. Lehre: der Zustand steht als Wort und Bandskala, die Kurve ist Beleg, nicht Aussage.

**Raycast** ([A fresh look and feel](https://www.raycast.com/blog/a-fresh-look-and-feel)). (1) Aktionsleiste zeigt zu jeder Aktion das Kürzel; Kürzel lernt man durch wiederholtes Sehen. Passt. (2) Die Palette öffnet ohne Bewegung (Skill `emil-design-eng`). Passt. (3) Kompaktmodus als Wahl des Nutzers. Passt nicht jetzt: eine Dichte, gut gewählt.

**Stripe und Vercel** ([Stripe, accessible color systems](https://stripe.com/blog/accessible-color-systems), [Geist Farben](https://vercel.com/geist/colors), [Geist Typografie](https://vercel.com/geist/typography)). (1) Farbstufen in einem wahrnehmungsgleichen Raum, damit gleiche Stufe gleich hell wirkt. Passt für abgeleitete Töne. (2) Stufen nach Aufgabe benannt (Hintergrund, Hover, Aktiv, Rand, Text). Passt: Tokens wie `--surface-hover` statt `--panel-3`. (3) Typografie nach Rolle (Überschrift, Marke, Fließtext), Ziffern in Marken tabellarisch. Passt.

**Teenage Engineering** ([Gespräch mit D. Möllerstedt](https://read.cv/jdsimcoe/inside-design-at-teenage-engineering), Sekundärquelle). (1) Beschränkung als Ästhetik: ein kleiner Bildschirm, jedes Element mit einer Aufgabe. Passt: jedes Instrument beantwortet genau eine Frage. (2) Farbe zeigt, welches Bedienelement gerade wirkt. Passt zu Limette als "jetzt". (3) Charakter aus Schrift und Proportion statt aus Dekoration. Passt: schmale Zahlen statt Verläufe und Schatten.

**Prinzipien für Lastwerk**

1. Heute beantwortet drei Fragen über der Falz: Urteil und Einheit, Zustand, Woche. Alles andere ist Tiefe.
2. Die Zahl ist die Überschrift. Ein Satz steht nur, wo er sagt, was die Zahl nicht sagt.
3. Jede Aussage an genau einer Stelle.
4. Limette heißt "jetzt": Urteil, Primäraktion, Fokus, aktive Navigation. Daten sind neutral oder sportfarben.
5. Zustand als Wort und Lage auf einer Skala, nie als Farbe allein, nie als Ampel.
6. Bewegung erklärt Veränderung (ein Wert wandert), sonst steht alles still. Was oft passiert, bewegt sich nicht.
7. Tastatur zuerst: Palette für alles, Kürzel sichtbar an der Aktion.
8. Schmal ist kein Sonderfall: jede Ansicht ab 360 px ohne waagerechtes Scrollen.

## 3. Drei Richtungen

Die Skizzen der Seite Heute liegen als statisches HTML im Scratchpad (`konzept/a-tagesbrief.html`, `b-instrument.html`, `c-trainer.html`) mit den festen Farbtokens, gebündelten Schriften und synthetischen Skizzendaten (Szenario "heute leichter als geplant", R12). Sie kommen nicht ins Repository.

### A: Tagesbrief (ruhig, typografisch)

- **Leitgedanke:** Heute ist ein kurzer Brief des Trainers. Die Entscheidung ist ein Satz als Überschrift ("Heute leichter. Grundlagenfahrt, 75 Minuten locker."), darunter der Grund im Satz, darunter "Wie du dran bist" in zwei Zeilen mit Zahlen im Text.
- **Typografie:** Inter mit optischer Größe; Überschrift 40, Aussage 19, Text 15 bis 17. Zahlen fett im Satz, eine schmale Randspalte wiederholt die vier Hauptwerte.
- **Raster:** Lesespalte 680 px, Randspalte 240 px, keine Karten, keine Kästen.
- **Navigation:** obere Leiste mit drei Punkten und Palette.
- **Bewegung:** fast keine; neue Werte blenden in 150 ms über.
- **Stärke:** ruhigste Lösung, in fünf Sekunden lesbar, schmal problemlos. **Risiko:** Zahlen sind nicht die Helden; wer Daten sehen will, liest Sätze. Zahlen stehen doppelt (Satz und Randspalte).

### B: Instrument (dicht, präzise), empfohlen

- **Leitgedanke:** Lastwerk ist ein Messgerät. Heute ist eine Tafel: oben der Entscheidungsstreifen (Urteil in Limette, Vorlage groß, Minuten, Last, die ersetzte Einheit durchgestrichen; rechts "Warum" mit Regel und der Primäraktion mit Kürzel), darunter drei Instrumente nebeneinander. Form: TSB groß, Zustand als Wort, Bandskala mit den vier Bereichen und Marke, Fitness und Ermüdung, sechs Wochen TSB ohne Achsen. Erholung: HRV, Schlaf, Ruhepuls mit Abweichungsbalken um die eigene Basis. Woche: erledigt von geplant, Last bisher, sieben Tagessäulen mit geplant, gemacht, verpasst als Form (gestrichelt, voll, Kreuz). Darunter höchstens drei Notizzeilen mit Kürzel.
- **Typografie:** Archivo, Zahlen in Breite 75, Text in Breite 100. Skala 64, 40, 24, 17, 14, 13, 12.
- **Raster:** 4 px, zwölf Spalten, Instrumente durch Haarlinien getrennt, eine erhobene Fläche je Seite, Radien 6, 10, 14.
- **Navigation:** Leiste 56 px mit Symbolen (Tooltip mit Name und Kürzel), unter 760 px unten. Strg+K Palette für alles, G dann H, P, V springt.
- **Bewegung:** Marke auf der Bandskala und Balken gleiten 200 ms bei neuem Wert; Coach-Fenster 200 ms per Klick, ohne Bewegung per Tastatur; sonst nichts.
- **Stärke:** alle drei Fragen über der Falz bei 1440 mal 900, schmal in zwei Bildschirmhöhen; Zahlen sind Helden; eigener Charakter durch schmale Ziffern und Skalen statt Linien. **Risiko:** kippt bei Überfüllung ins Dashboard; deshalb harte Grenze: drei Instrumente, drei Notizen. Symbolleiste braucht Tooltips und Palette, sonst schlechter auffindbar. Größter Umbau der drei.

### C: Trainer (warm, nahbar)

- **Leitgedanke:** Der Coach spricht auf Heute selbst. Gruß und Urteil als Überschrift ("Heute nehmen wir raus."), eine warme Karte mit der Einheit und einem Ring für die Wochenlast, Form als Bogen, rechts die Coach-Spalte mit festen Sätzen aus den Regeln und vorbereiteten Fragen.
- **Typografie:** Cairo bleibt, Skala gestrafft (46, 34, 26, 16, 13).
- **Raster:** zwei Spalten (Inhalt und Coach), Karten mit Radius 20 bis 24, Verlauf aus `--accent` in der Karte.
- **Navigation:** Seitenleiste wie heute, Coach immer sichtbar auf Heute.
- **Bewegung:** gestaffelter Einstieg beim ersten Öffnen des Tages, Ring füllt sich 600 ms.
- **Stärke:** freundlichster erster Eindruck, Coach im Mittelpunkt. **Risiko:** Ringe und Bögen erinnern an Apple und WHOOP; Kacheln kehren zurück; die Coach-Spalte zeigt Text, der wie Modelltext aussieht, aber feststeht (Verwechslungsgefahr mit 9.7); Cairo behält seine Schwächen; mehr Bewegung als nötig.

### Empfehlung

B, mit zwei Anleihen aus A: die Entscheidung liest sich als ganzer Satz (Urteil und Einheit in einer Zeile, wo Platz ist), und Leerzustände sind ein Weg in drei Schritten statt sechs Abschnitte "fehlt". Gründe: B besteht den Fünf-Sekunden-Test am klarsten (Urteil, Zustand, Woche über der Falz), macht Zahlen zu Helden, wie der Auftrag es verlangt, und sieht nach Lastwerk aus, nicht nach einer der Marken: schmale Ziffern, Bandskalen und Limette nur für "jetzt" gibt es bei keinem der Vorbilder in dieser Form. A ist die sichere Rückfalllösung, C widerspricht mehreren Prinzipien (Ampelnähe, Kacheln, Bewegung).

## 4. Offen

- Richtung (A, B, C oder Mischung) und Schrift wählt der Nutzer.
- Strg+K als Palette für alles ersetzt E9 (Strg+K öffnet den Coach); braucht Zustimmung.
- REA: nur wenn lokal eingerichtet. Mögliche Fragen: Kurven und Dauern beim Öffnen von Paletten und Seitenfenstern in installierten Programmen, Abstufung der Abstände.

## 5. Umsetzung und Prüfung (19. Oktober 2026, T60 bis T66 Teil A)

Umgesetzt in sechs Schnitten, jeder einzeln lauffähig und committet: Tokens und Schrift (T60), Leiste, schmales Layout und Befehlspalette (T61), Bausteine und Diagrammstil (T62), Heute als Tafel (T63), Verlauf und Plan (T64), Einstellungen, Coach-Fenster und Einrichtung (T65). Die Regeln stehen in SPEC 5.5.

Prüfung (T66 Teil A):

- **Tests.** 383 Komponenten- und Einheitentests, 35 Playwright-Tests (Deutsch und Englisch, alle Ansichten, Befehlspalette per Tastatur, Coach per Strg+J, kein waagerechtes Scrollen bei 360 px in Heute, Plan, Verlauf und Einstellungen). `sh scripts/check-all.sh` siehe Bericht des Pushs.
- **Härtetest (break-ui).** Englisch bei 360 px mit `?dashboard=extreme&decision=worst` (Fitness 444, unbekannte Vorlage mit langer ID, zwei Gründe, zwei Regeln, fehlender Planbezug): kein Element läuft über, die Seite scrollt nicht waagerecht, die Vorlage bricht als lesbare ID um. Gefunden und behoben: Die Schiene der Bandskala verbreiterte die Seite (jetzt `overflow: clip`).
- **Bewegung (review-animations).** Bestand: Druck auf Knöpfe (`scale(0.97)`, 120 ms), Tooltip der Leiste (300 ms Wartezeit, 120 ms Deckkraft), Coach-Fenster per Klick (200 ms, `--ease-drawer`, per Tastatur ohne Bewegung), Marke der Bandskala und Abweichungsbalken (200 ms), aufklappende Alternative (160 ms), Vorschlagskarte (180 ms), Pfeil der Wochenzeilen (160 ms). Palette, Reiter und Ansichtswechsel bewegen sich nicht. Befund und Korrektur: Marke und Balken bewegen sich auf der Fläche, deshalb `--ease-in-out` statt `--ease-out`. Offen, bewusst nicht gebaut: Folgetooltips ohne Wartezeit (nur mit Skript möglich, Nutzen gering bei fünf Punkten).
- **Bildrate (animation-performance).** Gemessen mit `requestAnimationFrame` in Chromium ohne Fenster, 1440 mal 900: Coach-Fenster und Marke je 25 Bilder in 400 ms, Mittel 16,7 ms, höchstens 16,8 ms, auch bei vierfacher CPU-Drosselung. Beide Bewegungen nutzen nur `transform`. Grenze: Messung ohne echtes Fenster und ohne GPU des Zielrechners, die Sichtprüfung im Tauri-Fenster steht aus.
- **Kontrast.** Gerechnet aus den Werten, die der Browser für die Tokens und die abgeleiteten Töne (`color-mix`) liefert. Dunkel: niedrigster Wert `--muted` auf `--panel-2` 5,71. Hell: niedrigster Wert `--ok` auf `--bg` 4,79, danach `--muted` auf `--panel-2` 5,07. Alle Textpaare erfüllen 4,5 zu 1.
- **Bündelgröße** (`pnpm run build`). Vorher (Stand `1e5103a`): JavaScript 550,7 KB (gzip 178,2), CSS 69,3 KB (gzip 13,1), Schrift 50,5 KB in zwei Dateien. Nachher: JavaScript 577,8 KB (gzip 186,6), CSS 83,3 KB (gzip 15,1), Schrift 90,1 KB in einer Datei. Zusammen gzip und Schrift rund 50 KB mehr, davon 40 KB für die Breitenachse von Archivo. Alles bleibt lokal gebündelt (`font-src 'self'`).
- **Vorher und nachher.** Bildpaare der eigenen Oberfläche liegen nur lokal im Scratchpad der Session, nicht im Repository.

Teil B (REA) wartet auf die Einrichtung durch den Nutzer, siehe [T66](tasks/T66-design-pruefung.md).

## 6. Nachbesserung: Farbkanten (19. Oktober 2026)

Der Nutzer fand die Kästen mit farbigem Streifen links zu sehr nach Standard-KI. Der Skill `apple-design` kam in `.claude/skills/` hinzu. Entfernt: Limettenkante des Entscheidungsstreifens, Warnkante der Coach-Einrichtung, Zitatlinie der Coach-Antwort, Sportkante der Kalenderchips und der Legende, Limettenkante der Anpassung in Erholung, Linie der Alternativen (jetzt leise Fläche). Die Sportfarbe steht im Kalender als Tönung der ganzen Fläche, der Sportname bleibt Text. Geblieben: Marken der Navigation, der Reiter und der Palette als Zustandsanzeige (SPEC 5.5, Regel 7).
