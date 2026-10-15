# T32: Fortschritt nach Woche, Monat und Jahr

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 (Auswertung) |
| cloudfähig | ja |
| Voraussetzung | T25 |
| Bereich | `app/src-tauri` (Befehl `get_progress`), `app/ui` (Ansicht "Fortschritt"), `catalog/locales` |
| SPEC | 4.1 (F7), 4.2 (F39), 5.5, 6.4 |

## Ziel

Eine Ansicht "Fortschritt" zeigt Umfang und Last über Wochen, Monate und Jahre, je Sportart, und vergleicht das laufende Jahr mit dem Vorjahr zum selben Stichtag (wie die Jahresübersicht in Garmin Connect, Strava und intervals.icu).

## Warum

F7 (Muss) verlangt Übersichten für Woche, Monat und Jahr. Gebaut ist bisher nur die Woche.

## Umfang

1. Befehl `get_progress(today)`: je Sportart Summen von Dauer, Strecke, Last und Zahl der Einheiten (nur zählende Aktivitäten, SPEC 6.4) für 52 Wochen und 24 Monate, Jahressummen bis zum Stichtag für dieses und das Vorjahr, Summenkurve je Tag des Jahres für beide Jahre (Dauer, Strecke, Last). Nur Summen und Gruppieren, keine Formel.
2. Ansicht "Fortschritt" in der Navigation: Kopf mit Aussage in einem Satz (dieses Jahr gegen Vorjahr), Kennzahlenreihe mit Veränderung, Summenkurve (uPlot), Wochenbalken nach Sportart (SVG, gestapelt, Wert im `title`), Monatstabelle. Umschalter für die Größe (Zeit, Strecke, Last) und die Sportart.
3. Attrappe, Tests, Texte.

## Nicht im Umfang

Ziele für Jahresumfang, Export der Tabellen.

## Vorgaben

- Last zählt nur Aktivitäten mit berechenbarer Last; Einheiten ohne Last zählen bei Dauer und Strecke, nicht bei Last (wie in der Woche).
- Der Vorjahresvergleich endet am gleichen Tag des Jahres (29. Februar zählt im Nicht-Schaltjahr zum 28.).

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo fmt --manifest-path app/src-tauri/Cargo.toml --check && cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings && cargo test --manifest-path app/src-tauri/Cargo.toml --locked
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
sh scripts/check-all.sh
```

Dazu: Attrappe mit synthetischen Werten für jede neue Ansicht (auch leer und extrem), Vitest für Deutung und Formatierung, Playwright-Rauchtest Deutsch und Englisch, Sichtprüfung hell und dunkel, schmal und breit (`break-ui`).

## Manuelle Schritte

Sichtprüfung im echten Tauri-Fenster mit eigener Datenbank durch den Nutzer.

## Vermerke

### Geliefert (15. Oktober 2026)

- Befehl `get_progress(today)` (`service/progress.rs`, DTO `Progress`, `ProgressEntry`) an allen vier Stellen eingetragen. Er liefert Tagessummen je Sportart statt fertiger Wochen und Monate: Die Oberfläche bildet Wochen, Monate, Jahressummen und Summenkurven daraus (`lib/progress.ts`, nur Summen, mit Handrechnungen in `progress.test.ts`). So wechselt die Ansicht Größe und Sportart ohne neuen Aufruf. Zeitraum: früherer Tag aus 1. Januar des Vorjahres und Monatserstem 23 Monate zurück.
- Ansicht "Fortschritt" in der Navigation nach "Woche": Umschalter Zeit, Strecke, Last (`Segmented`, wiederverwendbar), Sportart, Jahresvergleich mit Kennzahlen und Summenkurve (gestrichelt das ganze Vorjahr), 52 Wochen als gestapelte Säulen je Sportart (`WeekStackBars`, HTML statt SVG, damit Schrift und Säulen jede Breite halten), 24 Monate als Tabelle.
- Last: Einheiten ohne berechenbare Last zählen bei Zeit und Strecke, nicht bei Last; die Ansicht nennt ihre Zahl. Ein Monat nur mit solchen Einheiten zeigt keine Last statt 0.
- Attrappe: eigener Generator `mock-progress.ts` über gut zwei Jahre (die Aktivitäten der Attrappe reichen nur 140 Tage zurück), Varianten über `?dashboard=empty|sparse|extreme`.
- Damit ist F7 für Monat und Jahr erfüllt.
