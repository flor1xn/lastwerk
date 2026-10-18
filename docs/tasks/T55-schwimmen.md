# T55: Schwimmen mit Bahnen, SWOLF und CSS-Test

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T31, T53 (gleicher Ordner `crates/fit`) |
| Bereich | `crates/fit`, `crates/testkit`, `crates/core`, `app/src-tauri`, `app/ui`, `catalog/locales` |
| SPEC | 4.3 (F52), 8.6, Anhang A (Q15) |

## Ziel

Das Detail einer Schwimmeinheit zeigt jede Bahn mit Zeit, Zügen, Lage und Tempo je 100 m, Pausen getrennt, dazu SWOLF je Bahn und im Mittel. Aus einem 400-m- und einem 200-m-Test schlägt Lastwerk eine CSS vor (Q15).

## Warum

Vorbilder: Garmin Connect, Polar und Runalyze zeigen Bahnen und SWOLF. SPEC 2.1 führt Schwimmen als Baustein; Lastwerk liest bisher nur die Beckenlänge.

## Umfang

1. `crates/fit`: Nachricht `length` mit Zeit, Zügen, Lage und Art (aktiv oder Pause) lesen, Nummern nach öffentlicher Profilbeschreibung (O2).
2. `core`: SWOLF je Bahn als Sekunden plus Züge (Konvention, als Faustregel gekennzeichnet; nur bei gleicher Beckenlänge vergleichbar, das steht im Erklärtext). Tempo je 100 m ohne Pausen.
3. CSS-Test: Eingabe zweier Zeiten oder Auswahl zweier Abschnitte, `CSS = (400 - 200) / (t400 - t200)` in m/s nach Q15, als Vorschlag für den Schwellenwert `css_mps` (8.6), nicht automatisch übernommen.
4. App: Bahnenliste und Tempo je Bahn im Detail, CSS-Test unter den Schwellenwerten.

## Nicht im Umfang

Freiwasser ohne Bahnen (zeigt weiter Strecke und Zeit), Erkennung der Lage durch Lastwerk, Technikbewertung.

## Vorgaben

- Referenz CSS mit Rechenweg: 400 m in 360 s, 200 m in 170 s ergibt `200 / 190 = 1,0526 m/s`, also `95,0 s` je 100 m.
- Referenz SWOLF: Bahn 25 m in 22 s mit 16 Zügen ergibt 38.
- Synthetische Datei mit aktiven Bahnen und Pausen.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Keine REA-Analyse nötig.

## Abnahme

```
sh scripts/check-all.sh
```

- Referenzwerte grün, Datei ohne `length` zeigt den bisherigen Stand.
- Attrappe mit 80 Bahnen und mit einer, Sichtprüfung schmal.

## Manuelle Schritte

Eigene Schwimmdatei im Tauri-Fenster prüfen, Bahnen mit Garmin Connect vergleichen.

## Vermerke
