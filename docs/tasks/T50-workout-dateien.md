# T50: Geplante Einheiten als Workout-Datei (FIT und ZWO)

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja, Gerätetest manuell |
| Voraussetzung | T07, T35 |
| Bereich | `crates/fit` (Schreiben), `crates/testkit`, `crates/cli`, `app/src-tauri`, `app/ui`, `catalog/locales`, `docs/SPEC.md` |
| SPEC | 4.3 (F32), 7.4, 14 (O2, O4) |

## Ziel

Jede geplante Einheit mit Schritten lässt sich als FIT-Workout-Datei speichern (Rad, Lauf, Schwimmen) und Radeinheiten zusätzlich als ZWO-Datei für Zwift. Ziele je Schritt: Rad in Watt aus der FTP, Lauf in Tempo oder Herzfrequenz, je nachdem welcher Schwellenwert vorliegt.

## Warum

Vorbilder: TrainingPeaks, intervals.icu und TrainerRoad bringen Einheiten auf Garmin, Wahoo und Zwift; intervals.icu importiert und exportiert ZWO, MRC und ERG. In Lastwerk landet eine Einheit nur als Text im Kalender (ICS). Der Weg über Garmin Connect liegt im eingefrorenen Zusatzprogramm (F22). F32 ist seit Version 1 offen.

## Umfang

1. `crates/fit`: Schreibteil für Dateityp Workout mit den Nachrichten `file_id`, `workout`, `workout_step`. Nachrichten- und Feldnummern als eigene Konstanten nach öffentlicher Protokollbeschreibung, keine Garmin-Profildateien (O2).
2. Abbildung der Vorlagenschritte (7.4: `warmup`, `steady`, `interval` mit Wiederholungen und Pause) auf Schritte mit Dauer und Zielbereich. Zielbreite je Schritt als Faustregel in `rules.default.json`.
3. ZWO (XML) für Rad mit Leistungszielen relativ zur FTP. Es gibt keine offizielle Spezifikation; das Format folgt öffentlichen Beschreibungen, das steht im Code-Kommentar.
4. Kommandozeile `lastwerk workout export <plan> <datum> --format fit|zwo --out <datei>`, in der App "Als Datei speichern" an der Einheit im Kalender (Speicherdialog, kein fester Pfad).
5. Ohne Schwellenwert für das Ziel: Export ohne Zielbereich mit Hinweis, nicht stillschweigend falsch.

## Nicht im Umfang

- Übertragung in Garmin Connect (bleibt F22, Zusatzprogramm eingefroren).
- Direktes Schreiben auf ein angeschlossenes Gerät. Den Pfad (O4) nennt die App erst, wenn er geklärt ist.
- MRC und ERG (später, wenn gewünscht).

## Vorgaben

- Rundlauf-Test: geschriebene Datei mit dem eigenen Decoder lesen, Schritte, Dauern und Ziele gleich, CRC gültig.
- Golden-Tests für FIT-Bytes und ZWO-Text je Vorlage aus dem Katalog.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

O4: Wohin gehört eine Workout-Datei auf Edge und Uhr? Reihenfolge:

1. Handbuch des Geräts und Ordner auf dem angeschlossenen Gerät ansehen (`GARMIN/NewFiles`, `GARMIN/Workouts`). Eine Testdatei aus diesem Task kopieren und am Gerät prüfen.
2. Nur wenn das keine Antwort gibt und der Nutzer zustimmt: Mit REA statisch untersuchen, wohin Garmin Express Workouts schreibt (Interoperabilität). Keine Anmeldung, kein Netzwerkverkehr zu Garmin, kein Code übernehmen.

Ergebnis in O4 und im Vermerk eintragen.

## Abnahme

```
sh scripts/check-all.sh
```

- Rundlauf und Golden-Tests grün, Export ohne Schwellenwert getestet.
- Playwright-Rauchtest für "Als Datei speichern" mit Attrappe.

## Manuelle Schritte

Datei auf Edge und Uhr kopieren und eine Einheit am Gerät starten; ZWO in Zwift laden. Ergebnis mit Gerät und Firmware im Vermerk.

## Vermerke
