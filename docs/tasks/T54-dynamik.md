# T54: Lauf- und Raddynamik, Arbeit in kJ

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T31 |
| Bereich | `crates/fit`, `crates/testkit`, `crates/core`, `app/src-tauri`, `app/ui`, `catalog/locales` |
| SPEC | 4.3 (F51), 5.4 (lastwerk-fit) |

## Ziel

Das Aktivitätsdetail zeigt, was das Gerät aufgezeichnet hat: Trittfrequenz und Kadenz für Rad und Lauf; für Läufe Bodenkontaktzeit, Balance des Bodenkontakts, vertikale Oszillation, vertikales Verhältnis und Schrittlänge; für Radfahrten die Links-Rechts-Balance und die geleistete Arbeit in kJ. Je Wert Mittel, Verlauf und Runden.

## Warum

Vorbilder: Garmin Connect und Runalyze zeigen Laufdynamik, Golden Cheetah die Pedaldaten, Polar führt die Arbeit in kJ als "Muscle Load". Die Werte liegen in den FIT-Dateien des Nutzers, Lastwerk liest sie nicht.

## Umfang

1. `crates/fit`: Felder `cadence`, `fractional_cadence`, `stance_time`, `stance_time_balance`, `vertical_oscillation`, `vertical_ratio`, `step_length`, `left_right_balance` der Nachricht `record` lesen. Nummern, Einheiten und Skalierung nach öffentlicher Profilbeschreibung (O2), je Feld im Kommentar. Fehlende Felder bleiben `None`.
2. `core`: Mittel je Aktivität und Runde, Arbeit `sum(P * dt)` in kJ (Physik, keine Quelle nötig). Lauffrequenz: FIT speichert je nach Gerät Zyklen, nicht Schritte; die Umrechnung steht nach der Profilbeschreibung im Kommentar.
3. App: Abschnitt "Dynamik" im Detail, nur mit Werten, die die Datei enthält.

## Nicht im Umfang

Bewertungen wie "gut" oder "schlecht" und farbige Bereiche: Garmins Bereiche sind nicht belegt, eine Quelle in Anhang A fehlt. Laufleistung aus Stryd oder Garmin (eigener Task, wenn gewünscht).

## Vorgaben

- Synthetische Dateien mit allen Feldern, mit Teilen davon und ohne; Werte gegen Handrechnung.
- Arbeit: 200 W über 3600 s ergibt `200 * 3600 / 1000 = 720 kJ`.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Keine REA-Analyse. Ob eine eigene Datei die Felder enthält, zeigt der Decoder selbst (lokaler Test mit der eigenen Datei, die Datei nie ins Repository).

## Abnahme

```
sh scripts/check-all.sh
```

- Tests der Felder und der Arbeit grün.
- Attrappe mit vollständiger und leerer Dynamik, Sichtprüfung.

## Manuelle Schritte

Eigene Datei von Uhr und Edge im Tauri-Fenster öffnen, Werte mit Garmin Connect vergleichen und im Vermerk notieren.

## Vermerke
