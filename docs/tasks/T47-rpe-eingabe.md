# T47: Session-RPE eingeben und Kraft nachtragen

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T42 |
| Bereich | `crates/store` (Migration, Backup), `crates/cli`, `app/src-tauri`, `app/ui`, `catalog/locales`, `schema` |
| SPEC | 4.3 (F46), 6.2, 8.1, 14 (O14, O29), Anhang A (Q4) |

## Ziel

Nach jeder Aktivität lässt sich eine RPE von 1 bis 10 eintragen, und eine Einheit ohne FIT-Datei (vor allem Kraft) lässt sich mit Datum, Sportart, Dauer und RPE nachtragen. Für Kraft rechnet die Last dann nach Session-RPE (8.1, Q4) und zählt in CTL, ATL und Wochenlast.

## Warum

Vorbilder: TrainingPeaks und intervals.icu fragen RPE und Befinden nach der Einheit ab, Polar führt "Perceived Load" (RPE mal Dauer) als eigene Last, Garmin fragt nach dem Lauf. In Lastwerk gibt es die Tabelle `manual_session` und das Feld `rpe`, aber keinen Eingabeweg. Kraft ist deshalb immer `missing_channel` und fehlt in der CTL, während der Planer Kraft mit Last rechnet (O29).

## Umfang

1. Speicher: RPE je Aktivität als Nutzerdatum (eigene Tabelle, nicht in den abgeleiteten Werten, damit ein Neuaufbau sie nicht löscht, SPEC 6.1). Eingabe und Löschen für `manual_session`. Beides im Backup-JSON und im Schema dazu, Wiederherstellung getestet (F12).
2. Last: Für Kraft mit RPE gilt Session-RPE nach 8.1. Für Rad und Lauf bleibt die gemessene Last vorrangig; die RPE wird dort nur gespeichert und angezeigt.
3. Kommandozeile: `lastwerk rpe set <activity> <1-10>`, `lastwerk session add --date --sport --minutes --rpe`, `lastwerk session rm`.
4. App: RPE-Auswahl im Aktivitätsdetail, "Einheit nachtragen" im Verlauf. Nachgetragene Einheiten erscheinen in Liste, Kalender und Wochenlast mit eigenem Kennzeichen.
5. Erneute Berechnung von CTL und ATL nach jeder Änderung, wie beim Import.

## Nicht im Umfang

- RPE aus Garmin übernehmen (Feld nicht über einen belegten Endpunkt gelesen, 5.6).
- Ein Befinden-Wert neben der RPE (keine Quelle für eine Skala).
- Die Entscheidung über O29 (b) oder (c). Dieser Task setzt Option (a) um und ändert den Planer nicht.

## Vorgaben

- Umrechnung auf die Lastskala bleibt die Faustregel aus 8.1 (O14), Referenz T-SRPE-1 bleibt grün.
- Texte über `catalog/locales/de.json` und `en.json`.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Keine REA-Analyse nötig. Wie TrainingPeaks und intervals.icu die Abfrage nach der Einheit gestalten, zeigen deren Hilfeseiten.

## Abnahme

```
sh scripts/check-all.sh
```

- Test: Krafteinheit 60 min, RPE 6 ergibt Verlaufslast 36 (T-SRPE-1) und erhöht die CTL des Tages wie eine gleich große gemessene Last.
- Test: Backup mit RPE und nachgetragener Einheit, leere Datenbank, Wiederherstellung, gleiche Werte.
- Attrappe und Playwright-Rauchtest Deutsch und Englisch für Eingabe und Nachtrag, `break-ui` für die Eingabe.

## Manuelle Schritte

Sichtprüfung im echten Tauri-Fenster durch den Nutzer.

## Vermerke
