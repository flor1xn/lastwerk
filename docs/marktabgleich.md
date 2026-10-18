# Marktabgleich vom 18. Oktober 2026

Zweiter Abgleich nach dem vom 15. Oktober 2026 (T31 bis T37). Frage: Was können bestehende Produkte, das Lastwerk noch fehlt, und was davon passt zu den Zielen und Nicht-Zielen der Spezifikation (SPEC 2.2, 2.3)? Ergebnis sind die Tasks T47 bis T59 in [tasks/README.md](tasks/README.md).

Grundlage sind Hilfeseiten, Foren und Berichte der Anbieter, abgerufen am 18. Oktober 2026 über eine Websuche. Herstellerangaben sind ungeprüft übernommen; wo nur Werbetext vorlag, steht das dabei. Preise sind bewusst nicht aufgeführt.

## Produkte

| Produkt | Art | Lokal | Was für Lastwerk zählt |
|---|---|---|---|
| TrainingPeaks (mit WKO) | Web, Apps, Trainerplattform | nein | Performance Management Chart, Jahresplan mit Wettkampfprioritäten, strukturierte Einheiten mit Übertragung auf Garmin, Wahoo und Zwift, Erfüllungsfarben je Einheit, RPE und Befinden nach der Einheit |
| intervals.icu | Web | nein | Fitnessdiagramm, eFTP, Leistungsmodell mit W' und W'-Bilanz, Einheiten mit Import und Export (ZWO, MRC, ERG), Kalenderabgleich mit Garmin, Wellness mit RPE, Schlaf, HRV |
| Golden Cheetah | Desktop, GPL v2 | ja | Critical-Power-Modelle, W'-Bilanz, PMC, Vergleich von Aktivitäten, frei programmierbare Diagramme; 3.8 RC1 (Juli 2026) bringt geplante Aktivitäten |
| Runalyze | Web, proprietär | nein | Monotonie und Strain auf TRIMP-Basis, effektive VO2max aus HF und Tempo, Marathonform, Wettkampfprognosen, aerobe Schwelle aus HRV (DFA alpha1) |
| TrainerRoad | Apps | nein | Adaptive Training (Plan passt sich an verpasste und geschaffte Einheiten an), AI FTP Detection ohne Test (Herstellerangabe), Planbaukasten |
| Garmin Connect | Apps, Web | nein | Trainingsbereitschaft, adaptive Pläne aus wenigen Fragen, Lauf- und Raddynamik, Schwimmen mit Bahnen und SWOLF, Ausrüstung mit Kilometerstand |
| Strava | Apps, Web | nein | Fitness und Frische, steigungsbereinigtes Tempo, ähnliche Aktivitäten, Athlete Intelligence (KI-Zusammenfassungen), Ausrüstung |
| Polar Flow | Apps, Web | nein | Training Load Pro: Cardio Load (TRIMP-artig), Muscle Load (Arbeit in kJ), Perceived Load (RPE mal Dauer) |
| COROS EvoLab | Uhr, App | nein | Belastung, Wirkung, Erholung, Laufprognosen |
| Xert | Web | nein | Fitness Signature, MPA, Adaptive Training Advisor (Modelle ohne öffentliche Quelle) |
| Athletica, AI Endurance | Web, Apps | nein | KI-Pläne und KI-Coach, die Einheiten verschieben (Werbetext der Anbieter) |

Lastwerk bleibt das einzige der Liste, das alles lokal rechnet, jede Formel mit Quelle führt und ein Sprachmodell nur lokal einsetzt. Golden Cheetah ist ebenfalls lokal, aber auf Rad und Datenanalyse ausgerichtet und nicht auf Planung.

## Abgleich der Funktionen

"Teilweise" heißt: Grundlage da, Funktion fehlt.

| Funktion | Vorbilder | Lastwerk heute | Task |
|---|---|---|---|
| RPE nach der Einheit, Kraft ohne FIT nachtragen | TrainingPeaks, intervals.icu, Polar, Garmin | Tabelle `manual_session` ohne Eingabeweg, Kraft zählt nicht in der CTL (O29) | [T47](tasks/T47-rpe-eingabe.md) |
| Monotonie und Strain | Runalyze, Golden Cheetah | fehlt | [T48](tasks/T48-monotonie-strain.md) |
| Critical Power, W', W'-Bilanz | Golden Cheetah, intervals.icu, WKO, Xert | Leistungskurve da, Modell bewusst ausgelassen (T33, keine Quelle) | [T49](tasks/T49-critical-power.md) |
| Einheiten als Datei für Gerät und Zwift | TrainingPeaks, intervals.icu, TrainerRoad | nur Garmin-Push im eingefrorenen Zusatzprogramm (F22), F32 offen | [T50](tasks/T50-workout-dateien.md) |
| Plan passt sich an verpasste Einheiten an | TrainerRoad, Garmin, AI Endurance | Planerfüllung da, F34 und O15 offen | [T51](tasks/T51-neuplanung.md) |
| Saison mit mehreren Wettkämpfen | TrainingPeaks, TrainerRoad | ein Ziel je Plan | [T52](tasks/T52-saisonplan.md) |
| Steigungsbereinigtes Tempo, VAM | Strava, Runalyze, Golden Cheetah | keine Höhe aus FIT gelesen | [T53](tasks/T53-steigung.md) |
| Lauf- und Raddynamik, Arbeit in kJ | Garmin, Runalyze, Golden Cheetah, Polar | nicht gelesen | [T54](tasks/T54-dynamik.md) |
| Schwimmen mit Bahnen, SWOLF, CSS-Test | Garmin, Polar, Runalyze | nur Beckenlänge gelesen | [T55](tasks/T55-schwimmen.md) |
| Ausrüstung mit Kilometerstand | Strava, Garmin, intervals.icu, Runalyze | fehlt | [T56](tasks/T56-ausruestung.md) |
| Aerobe Schwelle aus HRV (DFA alpha1) | Runalyze, intervals.icu | RR-Intervalle nicht gelesen | [T57](tasks/T57-dfa-alpha1.md) |
| Streckenform ohne Kartenkacheln | alle (dort mit Karte) | keine GPS-Daten gelesen, F30 mit Kacheln offen (O3) | [T58](tasks/T58-streckenform.md) |
| Aktivitäten vergleichen | Golden Cheetah, Strava, TrainingPeaks | fehlt | [T59](tasks/T59-vergleich.md) |

## Bewusst nicht übernommen

| Funktion | Vorbild | Grund |
|---|---|---|
| Wettkampfprognosen, Marathonform | Runalyze, Garmin, COROS, Strava | Nicht-Ziel (SPEC 2.3) |
| Verpflegung, Ernährungsplanung | TrainingPeaks | Nicht-Ziel (SPEC 2.3) |
| Überlastungs- oder Verletzungswarnungen, Ampeln für Risiko | TrainerRoad, Garmin | Nicht-Ziel (SPEC 2.3). T48 zeigt Monotonie nur beschreibend |
| FTP-Erkennung durch maschinelles Lernen | TrainerRoad | keine öffentliche Quelle; der FTP-Vorschlag aus T36 bleibt |
| Fitness Signature, MPA, Strain-Skala von WHOOP | Xert, WHOOP | proprietäre Modelle ohne Veröffentlichung |
| Segmente, Bestenlisten, Heatmaps, soziale Funktionen | Strava | Netzwerk und Mehrbenutzerbetrieb (SPEC 2.3, 10) |
| Aufzeichnung, Rollentrainersteuerung | Zwift, TrainerRoad | Nicht-Ziel (SPEC 2.3) |
| Trainermarktplatz, Rechnungen | TrainingPeaks | Nur ein Nutzer (SPEC 2.1) |
| Karte mit Kacheln | alle | bleibt F30, hängt an O3; T58 zeigt die Form ohne Netz |

## Erkundung mit REA

[REA](https://github.com/morluto/rea) (MIT) ist ein MCP-Server mit Kommandozeile, der Programme ohne Quelltext untersucht: native Binärdateien, JavaScript- und Electron-Apps, .NET und Webseiten. Die Tasks T47 bis T59 nennen je einen optionalen Erkundungsschritt. Dafür gelten diese Regeln, zusätzlich zu [CLAUDE.md](../CLAUDE.md) und dem [DISCLAIMER von REA](https://github.com/morluto/rea/blob/main/DISCLAIMER.md):

1. **Nur lokal, nur vom Nutzer eingerichtet.** REA wird nicht im Repository registriert (kein Eintrag in `.mcp.json` oder `.claude/settings.json`). Wer es nutzt, richtet es selbst ein (`npx rea-agents setup`) und startet die Erkundung in einer lokalen Session. Cloud-Sessions brauchen REA nicht; jeder Task ist ohne den Erkundungsschritt vollständig.
2. **Zulässige Ziele.** Programme, die der Nutzer rechtmäßig installiert hat, und öffentliche Seiten ohne Anmeldung. Kein Garmin-Konto, keine Garmin-Dienste, keine Anmeldung mit einem eigenen Konto in der Session (CLAUDE.md, harte Regeln). Keine Umgehung von Lizenzprüfungen oder Schutzmaßnahmen.
3. **Lizenz des Ziels zuerst.** Verbietet die Lizenz des Ziels Reverse Engineering, entfällt der Schritt. Ausnahme ist nur die Interoperabilität (etwa ein Dateiformat oder ein Ablageort auf dem Gerät), wenn die Information nicht anders zu bekommen ist, zum Beispiel aus Handbuch oder Gerät. Ob das im Einzelfall trägt, entscheidet der Nutzer, nicht die Session.
4. **Verhalten verstehen, nichts übernehmen.** Erlaubt ist zu klären, welche Eingaben eine Funktion nimmt, was sie ausgibt, wie sie es zeigt und welches Dateiformat sie schreibt. Kein Code, keine Texte, keine Grafiken, keine Konstanten wandern in Lastwerk. Quelltext unter GPL (Golden Cheetah) wird gelesen, nicht kopiert; Lastwerk steht unter MIT OR Apache-2.0.
5. **REA ist keine Quelle.** Formeln und Zahlen kommen weiter nur aus Anhang A oder tragen "Faustregel". Ein mit REA beobachteter Wert darf als Gegenprobe im Vermerk der Task-Datei stehen, nie im Code.
6. **Befund festhalten.** Im Vermerk der Task-Datei: Datum, Ziel mit Version, Frage, Antwort, Grenzen der Antwort. Keine Screenshots fremder Oberflächen ins öffentliche Repository (SPEC 10.6).

Ehrliche Einordnung: Für die meisten Tasks reichen Hilfeseiten und die Originalarbeiten. REA hilft vor allem dort, wo eine Frage zum Format oder zum Verhalten offen bleibt, etwa beim Ablageort von Workout-Dateien (O4, T50), oder um zu sehen, wie ein installiertes Programm eine Kennzahl darstellt.

## Quellen des Abgleichs

- intervals.icu Forum: [W' und CP](https://forum.intervals.icu/t/w-and-cp-settings-help/1155), [eFTP](https://forum.intervals.icu/t/relationship-between-ftp-estimated-ftp/414), [W'-Bilanz](https://forum.intervals.icu/t/using-w-balance/804), [Wechsel von TrainingPeaks](https://forum.intervals.icu/t/what-would-make-me-switch-from-trainingpeaks/2993)
- TrainingPeaks: [Neuerungen](https://www.trainingpeaks.com/blog/trainingpeaks-feature-updates/)
- TrainerRoad: [AI FTP Detection](https://trainerroad.com/blog/?p=75091), [Adaptive Training](https://trainerroad.com/blog/tag/adaptive-training) (Werbetext)
- Runalyze: [Funktionen](https://runalyze.com/help/article/features), [Glossar](https://runalyze.com/glossary), [Berechnungen](https://blog.runalyze.com/tutorial/runalyze-understanding-the-calculations/), [aerobe Schwelle aus HRV](https://blog.runalyze.com/?p=4428)
- Golden Cheetah: [Startseite mit 3.8 RC1](https://goldencheetah.org/), [Versionshinweise](https://www.goldencheetah.org/RELEASE/release-notes.html)
- Garmin: [Adaptive Trainingspläne](https://www8.garmin.com/manuals/webhelp/GUID-0221611A-992D-495E-8DED-1DD448F7A066/EN-US/GUID-71798B16-A9A9-4D0F-BF64-50DD65AFC0D5.html)
- Polar: [Training Load Pro](https://support.polar.com/en/training-load-pro)
- Strava: [Athlete Intelligence](https://www.advnture.com/news/strava-launches-athlete-intelligence), [Ausblick 2026](https://www.t3.com/active/strava-2026-future-and-challenges) (Presse)
- AI Endurance, Athletica: [Vergleich eines Mitbewerbers](https://www.sensai.fit/blog/ai-endurance-alternatives), [Athletica AI Coach](https://athletica.ai/blog/meet-your-new-24-7-training-partner-athletica-ai-coach) (Werbetext)
- REA: [README](https://github.com/morluto/rea), [DISCLAIMER](https://github.com/morluto/rea/blob/main/DISCLAIMER.md)
