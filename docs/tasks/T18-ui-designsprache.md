# T18: Designsprache und Startseite "Heute"

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach M3 (Nutzerwunsch) |
| Voraussetzung | T09 |
| cloudfähig | ja |
| Bereich | `app/ui`, `catalog/locales`, `docs` |
| SPEC | 5.5 (neu), 8.2, 8.4 (R10), 10.4 |

## Ziel

Die Oberfläche folgt der bestätigten Designsprache (SPEC 5.5) und beginnt mit einer Startseite "Heute", die in einem Satz sagt, was zu tun ist und warum.

## Warum

Die Oberfläche aus T06 und T09 war ein Prüfstand: dicht, blau, tabellenlastig. Der Nutzer will ein Werkzeug, das Entscheidungen trifft und erklärt.

## Umfang

- Farbvariablen, Schrift, Abstände und Bausteine in `app/ui/src/app.css` nach SPEC 5.5, dunkles und helles Thema.
- Seitenleiste statt Kopfzeile, Startansicht "Heute".
- `TodayView`: Einheit von heute aus dem aktiven Plan, Form mit Zustand, Verlauf, Woche in Balken.
- Reine Funktionen für Formzustand und heutige Einheit in `lib/today.ts` mit Tests.
- Sprachschlüssel in `de.json` und `en.json`.
- Bestehende Ansichten erben die neuen Variablen und bleiben funktional unverändert.

## Nicht im Umfang

- Karte "Lastwerk hat angepasst" (braucht gespeicherte Planänderungen, SPEC 5.5, Vorschlag).
- Neugestaltung von Woche, Plan, Aktivitäten, Detail und Einstellungen über die Variablen hinaus (eigene Tasks).
- Neue Abhängigkeiten, Remote-Schriften, Bewegung mit Bibliothek.
- Installer (eigener Schritt).

## Vorgaben

- Skills aus `.claude/skills` nutzen (`emil-design-eng`, `break-ui`).
- Texte aus Plänen als Text, nie als HTML (SPEC 9.5).
- Keine Zahl ohne Quelle oder "Faustregel" (Formzustand).

## Abnahme

```
pnpm --dir app/ui lint
pnpm --dir app/ui check
pnpm --dir app/ui test
pnpm --dir app/ui exec playwright test
sh scripts/check-i18n.sh
sh scripts/check-privacy.sh
```

Erfolg: Alle Befehle grün, Rauchtest öffnet "Heute" in Deutsch und Englisch ohne Konsolenfehler.

## Manuelle Schritte

Der Nutzer prüft die Optik im laufenden Programm (`pnpm --dir app/ui exec tauri dev`) und sagt, was nicht stimmt.

## Blocker oder Vermerke

Keine Blocker. Erledigt am 14. Oktober 2026. Die manuelle Prüfung der Optik im laufenden Programm steht aus.

### Ergebnis

- Seitenleiste, neue Farben (dunkel und hell), Karten und Knöpfe nach SPEC 5.5 über `app.css`. Andere Ansichten erben die Variablen und sind sonst unverändert, sie wirken noch dicht und gehören in eigene Tasks.
- Neue Ansicht `TodayView` als Start, `lib/today.ts` mit Tests, Sprachschlüssel `ui.today.*`, `ui.form.*`, `ui.nav.today`.
- Neue Abhängigkeiten: keine. Der Rauchtest lud einmalig Chromium für Playwright (Entwicklungswerkzeug, nicht im Repository).
- Abweichung: Das Diagramm der Startseite nutzt die bestehende uPlot-Hülle, die Legende der Bibliothek ist ausgeblendet, eine eigene steht darunter.
- Offen (SPEC 5.5, Vorschlag): Karte "Lastwerk hat angepasst", braucht gespeicherte Planänderungen.
