# T63: Heute als Tafel

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 |
| cloudfähig | ja |
| Voraussetzung | T61, T62 |
| Bereich | `app/ui/src/views/TodayView.svelte`, `views/today`, `views/ProgressView.svelte`, `catalog/locales`, `docs/SPEC.md` |
| SPEC | 5.5, 8.7 |

## Ziel

Heute beantwortet über der Falz drei Fragen: Was tue ich heute und warum, wie bin ich dran, wie läuft die Woche.

## Umfang

1. Kopfzeile statt Titel "Heute": Datum, Planwoche und Phase des aktiven Plans.
2. Entscheidungsstreifen (`today/DecisionStrip.svelte`, ersetzt `DecisionCard`): links Urteil in Limette, Vorlage groß, Minuten, hart oder locker, Last; ersetzte Einheit durchgestrichen, wenn die Alternative `planned` existiert. Rechts "Warum" mit Gründen, Regeln, fehlenden Eingängen, Primäraktion "Mit Coach besprechen" mit Kürzel, Alternativen als gewöhnliche Knöpfe mit aufklappenden Einzelheiten wie bisher (ändern den Plan nicht).
3. Drei Instrumente: Form (TSB, Zustand, BandScale, Fitness, Ermüdung, sechs Wochen TSB ohne Achsen), Erholung (DeviationRow für HRV, Schlaf, Ruhepuls, Zustand als Text, Hinweis R12-Tausch), Woche (erledigt von geplant, Last bisher von geplant, WeekStrip, verpasste Tage). Jedes Instrument sagt ohne Daten in einem Satz, was fehlt.
4. Notizen: höchstens drei Zeilen aus `insights`, ohne die Codes, die ein Instrument schon zeigt (`form.overloaded`, `readiness.low`, `adherence.*`), dazu der Ausblick der Woche. Je Zeile ein leiser Knopf.
5. Leerzustand (kein Plan, keine Last, keine Erholungswerte): ein Weg in drei Schritten (Daten holen, Plan in drei Fragen, Coach einrichten) statt einzelner "fehlt"-Sätze.
6. Rampe und Verteilung ziehen nach Verlauf › Fortschritt; das volle Form-Diagramm mit Prognose steht dort ebenfalls.
7. Tests und Rauchtest an die neuen Testkennungen angepasst.

## Abnahme

```
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
```

Bei 1440 mal 900 stehen Entscheidung und alle drei Instrumente ohne Scrollen im Bild (Playwright, Variante `?plan=laufend`).

## Blocker oder Vermerke

Keine.
