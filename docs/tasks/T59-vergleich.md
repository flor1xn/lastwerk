# T59: Aktivitäten vergleichen

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T31 |
| Bereich | `app/src-tauri`, `app/ui`, `catalog/locales` |
| SPEC | 4.3 (F56), 5.5 |

## Ziel

Zwei bis vier Aktivitäten derselben Sportart lassen sich nebeneinanderlegen: Leistung, Herzfrequenz und Tempo über Zeit oder Strecke als überlagerte Linien, dazu eine Tabelle mit Dauer, Last, NP, Effizienzfaktor und Entkopplung. Lastwerk schlägt ähnliche Einheiten vor.

## Warum

Vorbilder: Golden Cheetah vergleicht Aktivitäten und Zeiträume, Strava zeigt ähnliche Läufe, TrainingPeaks erlaubt den Vergleich im Analysebereich. In Lastwerk zeigt jede Aktivität nur sich selbst; ob dieselbe Einheit heute leichter fiel als vor drei Monaten, bleibt verborgen.

## Umfang

1. Befehl `get_activity_compare(ids)` mit heruntergerechneten Messreihen je Aktivität (vorhandenes `downsample`) und den Kennzahlen aus T31.
2. Vorschläge: gleiche Sportart, gleiche Vorlage im Plan oder Dauer innerhalb einer Spanne (Faustregel, Parameter in den Einstellungen), neueste zuerst.
3. App: "Vergleichen" aus Liste und Detail, Ansicht mit Achsenwahl Zeit oder Strecke, Farben je Aktivität aus der Designsprache (5.5).

## Nicht im Umfang

Streckenerkennung über GPS, Vergleich ganzer Zeiträume (das macht "Fortschritt"), mehr als vier Aktivitäten.

## Vorgaben

Werte kommen aus den bestehenden Funktionen in `core`; der Task rechnet nichts Neues.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Optional: Golden Cheetah (quelloffen) ansehen, wie es überlagerte Linien und Achsen beschriftet. Keine REA-Analyse nötig, nichts übernehmen.

## Abnahme

```
sh scripts/check-all.sh
```

- Vitest für Vorschlagsregel und Achsen, Playwright-Rauchtest Deutsch und Englisch.
- `break-ui`: vier sehr lange und eine sehr kurze Aktivität, fehlende Kanäle.

## Manuelle Schritte

Zwei eigene gleiche Einheiten im Tauri-Fenster vergleichen.

## Vermerke
