# T64: Verlauf und Plan in der neuen Sprache

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 |
| cloudfähig | ja |
| Voraussetzung | T62 |
| Bereich | `app/ui/src/views` (History, Activities, Calendar, Progress, Performance, Recovery, Health, Plan), `views/plan`, `lib/components`, `catalog/locales` |
| SPEC | 5.5 |

## Ziel

Verlauf und Plan nutzen die Bausteine aus T62; der Plan stellt die laufende Woche nach vorn.

## Umfang

1. Plan: Mit aktivem Plan oben die laufende Woche ausgeklappt, die übrigen Wochen als Zeilen (Wochennummer, Phase, Ziellast, Planlast, Minuten), aufklappbar. Die drei Fragen nur ohne aktiven Plan oder über "Neuer Plan". Prüfung und Warnungen bleiben, Form der Planwochen als Diagramm im neuen Stil.
2. Aktivitäten: Zeilen 32 hoch, Sportfarbe als Marke, "nicht berechenbar" als leiser Strich mit Erklärung im `title`, Spalten ohne leere Breiten.
3. Kalender, Fortschritt, Leistung, Körper: Diagrammstil aus T62, Kennzahlen mit `Metric`, Zustände als Text, Heatmap ohne Limette.
4. Schmal ab 360 px ohne waagerechtes Scrollen; breite Tabellen scrollen in ihrem eigenen Bereich.

## Abnahme

```
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
```

## Blocker oder Vermerke

Keine.
