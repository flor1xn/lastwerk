# T51: Neuplanung nach verpassten Einheiten

| Feld | Wert |
|---|---|
| Status | offen, Entscheidung O15 vor Beginn |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T35, T44 |
| Bereich | `crates/core` (Planer), `crates/cli`, `app/src-tauri`, `app/ui`, `catalog` (Begründungsschlüssel, Texte), `docs/SPEC.md` (8.3, 14) |
| SPEC | 4.3 (F34), 8.3, 8.4, 14 (O15) |

## Ziel

Ist eine Einheit der laufenden Woche verpasst, bietet Heute an, den Rest der Woche neu zu planen. Der Nutzer sieht vorher, was sich ändert, und bestätigt. Der Planer bleibt deterministisch und hält alle Leitplanken ein.

## Warum

Vorbilder: TrainerRoad Adaptive Training passt den Plan an geschaffte und verpasste Einheiten an, Garmins adaptive Pläne ändern sich mit Form und Terminen, AI Endurance und Athletica verschieben Einheiten. In Lastwerk zählt die Planerfüllung verpasste Einheiten, der Plan bleibt aber stehen. F34 und O15 sind seit Version 1 offen.

## Umfang

Vorschlag für das Verhalten (Faustregel), vom Nutzer vor Beginn zu bestätigen oder zu ändern (O15):

1. Auslöser: Eine geplante Einheit vor heute in der laufenden Woche hat keine passende Aktivität (Regel der Planerfüllung aus T35).
2. Verpasstes wird nicht nachgeholt. Das Wochenziel für den Rest ist das ursprüngliche Ziel minus die schon erledigte Last, gedeckelt durch R1 und R2.
3. Eine verpasste harte Einheit darf auf den nächsten Tag mit `allow_hard` rutschen, wenn R3 und R4 erfüllt bleiben, sonst entfällt sie.
4. Der Rest der Woche durchläuft die Schritte 3 bis 7 aus 8.3 mit den Tagen ab heute. Folgewochen bleiben, ihr Startwert ergibt sich wie bisher aus der Historie.
5. Neue Begründungsschlüssel (`reason.week.replanned`, `reason.session.moved_after_missed`) in beiden Sprachen.
6. App: Hinweis auf Heute, Vorher-Nachher je Tag, Bestätigen oder Verwerfen. Der Coach kann die Änderung über die bestehenden Wege erklären (9.7), rechnet aber nicht.

## Nicht im Umfang

- Anpassung an "zu leicht" oder "zu schwer" empfundene Einheiten (bräuchte T47 und eine Regel mit Quelle).
- Automatische Änderung ohne Bestätigung.
- Änderungen an Folgewochen über den Startwert hinaus.

## Vorgaben

- Golden-Tests: verpasste leichte Einheit, verpasste harte Einheit mit und ohne freien harten Tag, verpasste Langeinheit, ganze Woche verpasst. Gleiche Eingabe ergibt gleichen Plan.
- Jeder neue Plan besteht `validate` (8.4).
- SPEC 8.3 um den Ablauf ergänzen, O15 als entschieden mit Datum eintragen.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Keine REA-Analyse. Die adaptiven Pläne von TrainerRoad und Garmin beruhen auf nicht veröffentlichten Modellen; ihr Verhalten ist kein Maßstab für Regeln mit Quelle oder Faustregel.

## Abnahme

```
sh scripts/check-all.sh
```

- Golden-Tests grün, Leitplanken in jedem Fall erfüllt.
- Playwright-Rauchtest für Hinweis, Vorschau, Bestätigen und Verwerfen.

## Manuelle Schritte

Entscheidung O15 durch den Nutzer vor Beginn. Danach eine echte verpasste Einheit im Tauri-Fenster durchspielen.

## Vermerke
