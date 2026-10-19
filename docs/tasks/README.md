# Tasks

Jeder Task ist ein abgeschlossener Arbeitsauftrag für eine Claude-Code-Session (Cloud oder lokal). Die Spezifikation ([../SPEC.md](../SPEC.md)) bleibt die Wahrheit, die Task-Dateien schneiden die Meilensteine aus SPEC 12 in Stücke, die in eine Session passen. Regeln für Sessions: [../../CLAUDE.md](../../CLAUDE.md). Einrichtung und Startbefehle: [../remote-sessions.md](../remote-sessions.md).

## Übersicht

Status: `offen`, `in Arbeit`, `in Prüfung`, `blockiert`, `erledigt`. Wer einen Task abschließt, setzt den Status hier und in der Task-Datei.

| Task | Titel | Meilenstein | Status | Voraussetzung | cloudfähig | Bereich |
|---|---|---|---|---|---|---|
| [T00](T00-geruest.md) | Gerüst, CI, Datenschutzprüfungen | M0 | erledigt | | ja | Wurzel |
| [T01](T01-core-belastung.md) | Belastung je Aktivität | M1 | erledigt | T00 | ja | crates/core |
| [T02](T02-testkit-fit-encoder.md) | FIT-Encoder und synthetische Fixtures | M1 | erledigt | T00 | ja | crates/testkit |
| [T03](T03-fit-parser.md) | FIT einlesen | M1 | erledigt | T02 | ja | crates/fit |
| [T04](T04-store-import.md) | Speicher, Duplikate, Import | M1 | erledigt | T01, T03 | ja | crates/store, cli |
| [T05](T05-core-pmc-zonen.md) | Zonen, Fitness, Schwellenwerte | M2 | erledigt | T04 | ja | crates/core, store, cli |
| [T06](T06-ui-geruest.md) | Tauri-Shell und Übersichten | M2 | erledigt | T05 | ja | app |
| [T07](T07-core-planer.md) | Regelbasierter Planer | M3 | erledigt | T05 | ja | crates/core, catalog |
| [T08](T08-schemas-export.md) | Schema, Validierung, ICS, Backup | M3 | erledigt | T07, T04 | ja | schema, crates/* |
| [T09](T09-ui-plan-release.md) | Planansicht, Release-Workflow (Version 1) | M3 | erledigt | T06, T08 | ja | app, .github |
| [T10](T10-mcp-server.md) | MCP-Server | M7 | erledigt | T05, T08 | ja, Rauchtest manuell | crates/mcp |
| [T11](T11-bridge-archivkern.md) | Zusatzprogramm: Archivkern offline | M4a | erledigt | T00 | ja | bridge |
| [L1](L1-lokal-garmin-sonde.md) | Garmin-Anmeldung und Sonde | M4a | erledigt | T11 | **nein** | lokal |
| [T12](T12-bridge-backfill-sync.md) | Zusatzprogramm: backfill, sync, coverage | M4b | erledigt | T11 | ja, Live manuell | bridge |
| [T13](T13-bridge-usb.md) | Zusatzprogramm: USB-Kopie (optional) | M4c | erledigt | T11 | ja | bridge |
| [T14](T14-bridge-normalize-recovery.md) | Normalisierung, Erholung, Regel R12 | M4d | erledigt | T04, T07, T11 | ja | bridge, crates |
| [T15](T15-coach-llm.md) | Lokales Sprachmodell | M5 | erledigt | T07, T08 | ja, Live manuell | crates/coach |
| [T16](T16-bridge-push-pwa.md) | Workouts (Trockenlauf), PWA-Entscheidung | M6 | erledigt | T08, T11 | ja | bridge, docs |
| [T17](T17-installer.md) | Windows-Installer, Update auf Knopfdruck | nach M3 | in Prüfung | T09, T18 | **nein**, Secret manuell | app, scripts, .github |
| [T18](T18-ui-designsprache.md) | Designsprache und Startseite "Heute" | nach M3 | erledigt | T09 | ja | app/ui, catalog |
| [T19](T19-erststart.md) | Erststart-Assistent, Designsprache überall, Version in der App | nach M3 | erledigt | T17, T18 | ja, Live manuell | app, catalog, .github |
| [T20](T20-entscheidungswende.md) | Entscheidungswende für 1.0 (Garmin in Rust, Chat, Cairo) | 1.0 | erledigt | T19 | ja | docs |
| [T21](T21-garmin-crate.md) | Crate lastwerk-garmin: Token, Client, Drosselung | 1.0 | erledigt | T20 | ja, Live in T23 | crates/garmin |
| [T22](T22-garmin-sync.md) | Abgleich mit Garmin: Aktivitäten und Erholung | 1.0 | erledigt | T21 | ja, Live in T23 | crates/cli, store |
| [T23](T23-garmin-app.md) | Garmin in der App: Anmeldefenster, Abgleich, Erststart | 1.0 | in Prüfung | T22, T24 | ja, **Live manuell** | app |
| [T24](T24-cairo-gestaltung.md) | Schrift Cairo und Gestaltung ohne Kachelraster | 1.0 | erledigt | T20 | ja | app/ui |
| [T25](T25-dashboard.md) | Kennzahlen und Dashboard | 1.0 | erledigt | T24, T22 | ja | crates/core, app |
| [T26](T26-planer-chat.md) | Planer mit drei Fragen und Gespräch mit dem Coach | 1.0 | in Prüfung | T20, T25 | ja, Live manuell | crates/coach, app |
| [T27](T27-release-1-0.md) | Version 1.0 | 1.0 | erledigt | T21 bis T26, T28 | ja, Release manuell | Wurzel, app, docs |
| [T28](T28-feste-ablage.md) | Feste Ablage der FIT-Dateien ohne Ordnerwahl | 1.0 | erledigt | T23 | ja | app, crates/cli |
| [T29](T29-ollama-einrichtung.md) | Ollama im Installer und im Erststart einrichten | 1.0 | in Prüfung | T26, T27 | ja, **Live manuell** | app, crates/coach, catalog |
| [T30](T30-auto-release.md) | Installer und Update immer aktuell (automatischer Release) | nach 1.0 | in Prüfung | T17, T27 | ja, erster Lauf nach Push | .github, scripts, docs |
| [T31](T31-detail-ausbau.md) | Aktivitätsdetail: Entkopplung, Effizienz, Runden, Tempo | nach 1.0 | erledigt | T25 | ja | crates/core, app |
| [T32](T32-fortschritt.md) | Fortschritt nach Woche, Monat und Jahr | nach 1.0 | erledigt | T25 | ja | app |
| [T33](T33-leistungskurve.md) | Leistungskurve als eigene Seite | nach 1.0 | erledigt | T25 | ja | crates/core, store, app |
| [T34](T34-erholung.md) | Erholung über Monate | nach 1.0 | erledigt | T22, T25 | ja | crates/core, app |
| [T35](T35-kalender.md) | Trainingskalender je Monat | nach 1.0 | erledigt | T25, T26 | ja | app |
| [T36](T36-schwellen-verlauf.md) | Verlauf der Schwellenwerte | nach 1.0 | erledigt | T33 | ja | app |
| [T37](T37-laufbestzeiten.md) | Bestzeiten im Laufen | nach 1.0 | erledigt | T33 | ja | crates/core, store, app |
| [T38](T38-oeffentlich.md) | Vorbereitung auf ein öffentliches Repository | nach 1.0 | erledigt | T30 | ja, Umstellung durch den Eigentümer | .github, scripts, docs, crates |
| [T39](T39-garmin-vollabzug.md) | Garmin-Vollabzug und Schwellenwerte aus Garmin | nach 1.0 | erledigt | T22, T23, T28 | ja, **Live manuell** | crates/garmin, store, cli, app, catalog |
| [T40](T40-garmin-vollabzug-2.md) | Garmin-Vollabzug Teil 2: Verläufe, Trainingsstatus, S3 je Bereich | nach 1.0 | erledigt | T39 | ja, **Live manuell** | crates/garmin, store, cli, app, catalog |
| [T41](T41-einrichtung-neu.md) | Einrichtung neu, in drei Schritten zum fertigen Plan | nach 1.0 | erledigt | T39, T40 | ja, **Live manuell** | app, catalog, docs |
| [T42](T42-navigation-verlauf.md) | Navigation radikal aufräumen, Verlauf und Einstellungen in Reitern | nach 1.0 | erledigt | T41 | ja | app/ui, catalog, docs |
| [T43](T43-coach-seitenfenster.md) | Coach als globales Seitenfenster mit Ansichtskontext | nach 1.0 | in Prüfung | T42 | ja, Live manuell | crates/coach, app, catalog, docs |
| [T44](T44-tagesentscheidung.md) | Tagesentscheidung, Coach-Erkenntnisse und Wochen-Story auf Heute | nach 1.0 | erledigt | T41 (Teil A), T43 (Teil B) | ja | crates/core, app, catalog, docs |
| [T45](T45-coach-strom.md) | Coach-Antworten als Strom | nach 1.0 | offen | T43 | ja, Live manuell | crates/coach, app |
| [T46](T46-ci-haertung.md) | CI straffen und Schlüsselbund ohne zbus | nach 1.0 | erledigt | T38 | ja | .github, scripts, crates/garmin, docs |
| [T47](T47-rpe-eingabe.md) | Session-RPE eingeben und Kraft nachtragen | nach 1.0 | offen | T42 | ja | crates/store, cli, app, catalog, schema |
| [T48](T48-monotonie-strain.md) | Monotonie und Strain je Woche | nach 1.0 | offen | T32 | ja | crates/core, app, catalog, docs |
| [T49](T49-critical-power.md) | Critical Power, W' und W'-Bilanz | nach 1.0 | offen | T33 | ja | crates/core, app, catalog, docs |
| [T50](T50-workout-dateien.md) | Geplante Einheiten als Workout-Datei (FIT und ZWO) | nach 1.0 | offen | T07, T35 | ja, Gerätetest manuell | crates/fit, testkit, cli, app, catalog, docs |
| [T51](T51-neuplanung.md) | Neuplanung nach verpassten Einheiten | nach 1.0 | offen, Entscheidung O15 vor Beginn | T35, T44 | ja | crates/core, cli, app, catalog, docs |
| [T52](T52-saisonplan.md) | Saisonplan mit mehreren Wettkämpfen | nach 1.0 | offen | T51 | ja | crates/core, store, cli, app, catalog, schema, docs |
| [T53](T53-steigung.md) | Steigungsbereinigtes Tempo und Höhenmeter je Stunde | nach 1.0 | offen | T31, T54 | ja | crates/fit, testkit, core, app, catalog, docs |
| [T54](T54-dynamik.md) | Lauf- und Raddynamik, Arbeit in kJ | nach 1.0 | offen | T31 | ja | crates/fit, testkit, core, app, catalog |
| [T55](T55-schwimmen.md) | Schwimmen mit Bahnen, SWOLF und CSS-Test | nach 1.0 | offen | T31, T53 | ja | crates/fit, testkit, core, app, catalog |
| [T56](T56-ausruestung.md) | Ausrüstung mit Kilometerstand | nach 1.0 | offen | T42 | ja | crates/store, cli, app, catalog, schema |
| [T57](T57-dfa-alpha1.md) | Hinweis auf die aerobe Schwelle aus HRV (DFA alpha1) | nach 1.0 | offen | T31, T55 | ja | crates/fit, testkit, core, app, catalog, docs |
| [T58](T58-streckenform.md) | Streckenform ohne Kartenkacheln | nach 1.0 | offen | T31, T53 | ja | crates/fit, testkit, app |
| [T59](T59-vergleich.md) | Aktivitäten vergleichen | nach 1.0 | offen | T31 | ja | app, catalog |
| [T60](T60-design-tokens.md) | Tokens und Schrift der Designsprache "Instrument" | nach 1.0 | erledigt | keine | ja | app/ui, docs |
| [T61](T61-geruest-palette.md) | Gerüst, schmales Layout und Befehlspalette | nach 1.0 | erledigt | T60 | ja | app/ui, catalog, docs |
| [T62](T62-bausteine-instrumente.md) | Bausteine und Diagrammstil | nach 1.0 | erledigt | T60 | ja | app/ui |
| [T63](T63-heute-tafel.md) | Heute als Tafel | nach 1.0 | erledigt | T61, T62 | ja | app/ui, catalog, docs |
| [T64](T64-verlauf-plan.md) | Verlauf und Plan in der neuen Sprache | nach 1.0 | offen | T62 | ja | app/ui, catalog |
| [T65](T65-einstellungen-coach-einrichtung.md) | Einstellungen, Coach-Fenster und Einrichtung | nach 1.0 | offen | T61, T62 | ja | app/ui, catalog |
| [T66](T66-design-pruefung.md) | Prüfung der Designsprache und Abgleich mit REA | nach 1.0 | offen | T60 bis T65 | Teil A ja, Teil B lokal | app/ui, docs |

## Reihenfolge

```
T00 ─┬─ T01 ───────────┐
     ├─ T02 ─ T03 ─────┴─ T04 ─ T05 ─┬─ T06 ─┐
     │                                ├─ T07 ─┴─ T08 ─ T09   (Version 1 nach T09)
     │                                └────────── T10
     └─ T11 ─┬─ L1 (lokal)
             ├─ T12
             ├─ T13 (optional)
             └─ T14 (braucht auch T04, T07)
T15 braucht T07 und T08. T16 braucht T08 und T11.

Version 1.0 (ab T20, bridge/ eingefroren):
T20 ─┬─ T21 ─ T22 ─┐
     ├─ T24 ───────┼─ T23 ─ T25 (Teil B) ─ T26 (Teil B) ─ T27
     ├─ T25 (Teil A, crates/core) ────┘
     └─ T26 (Teil A, crates/coach) ───┘
```

Auswertung nach 1.0 (Abgleich mit TrainingPeaks, intervals.icu, WKO, Golden Cheetah, Garmin Connect, Strava; Auswahl vom 15. Oktober 2026):

```
T25 ─┬─ T31, T32, T34, T35
     └─ T33 ─┬─ T36
             └─ T37
```

Garmin-Vollabzug und Einrichtung (16. Oktober 2026):

```
T39 ─ T40 ─ T41
```

Aufräumen, Tagesentscheidung und Coach-Seitenfenster (16. Oktober 2026):

```
T41 ─┬─ T42 ─ T43 ─┬─ T44 (Teil B) 
     │             └─ T45
     └─ T44 (Teil A, crates/core) ─┘
```

Marktabgleich vom 18. Oktober 2026 (Auswahl und Begründung in [../marktabgleich.md](../marktabgleich.md); die Erkundung mit REA ist je Task optional und nur lokal):

```
T42 ─┬─ T47 ─ (T48 zählt danach Kraft mit)
     └─ T56
T32 ─ T48          T33 ─ T49          T07, T35 ─ T50          T31 ─ T59
T35, T44 ─ T51 ─ T52                  (T51 erst nach Entscheidung O15)
T31 ─ T54 ─ T53 ─┬─ T55 ─ T57         (crates/fit: nacheinander, nie parallel)
                 └─ T58
```

Designsprache "Instrument" vom 19. Oktober 2026 (Begründung und Entscheidungen E12 bis E14 in [../designsprache.md](../designsprache.md); alle in `app/ui`, deshalb nacheinander):

```
T60 ─┬─ T61 ─┬─ T63 ─┐
     └─ T62 ─┼─ T64 ─┼─ T66
             └─ T65 ─┘
```

Vorschlag für die Reihenfolge nach Nutzen: T47 (schließt O29 a, Kraft zählt), T50 (Einheiten aufs Gerät), T51 (F34), T49, T54, T53, dann der Rest.

Version 1 ist der Stand nach T09 (M1 bis M3, SPEC 12). T10 kann vor dem Zusatzprogramm gezogen werden, weil es ohne Garmin-Zugang auskommt.

## Eine Session starten

Die Befehle stehen in [../remote-sessions.md](../remote-sessions.md). Kurz: Auftrag "Bearbeite docs/tasks/T01-core-belastung.md" an eine Cloud-Session auf dem Repository `flor1xn/lastwerk`.

## Parallele Sessions

Ergebnisse gehen direkt nach `main`. Deshalb gilt:

- Gleichzeitig nur Tasks in verschiedenen Ordnern (zum Beispiel T11 in `bridge/` neben T01 in `crates/core`). Zwei Tasks im selben Crate nacheinander.
- Jede Session führt vor dem Push `git pull --rebase origin main` aus.
- Konflikt in `Cargo.lock`: die Version von `origin/main` übernehmen und `cargo check --workspace` ausführen, damit sie neu geschrieben wird. Konflikt in der Tabelle oben: nur die eigene Zeile neu setzen.
- Ein Task verändert fremde Ordner nur, wenn die Task-Datei es unter "Umfang" nennt.

## Aufbau einer Task-Datei

Kopf mit Status, Meilenstein, Voraussetzung, cloudfähig, Bereich und SPEC-Abschnitten. Danach: **Ziel**, **Warum**, **Umfang**, **Nicht im Umfang**, **Vorgaben**, **Abnahme** (Befehle und Erfolgskriterien), **Manuelle Schritte** (nur lokal oder durch den Nutzer), **Blocker oder Vermerke**.
