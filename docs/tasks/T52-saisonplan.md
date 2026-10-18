# T52: Saisonplan mit mehreren Wettkämpfen

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T51 (gleicher Planercode) |
| Bereich | `crates/core` (Planer), `crates/store`, `crates/cli`, `app/src-tauri`, `app/ui`, `catalog`, `schema/plan.schema.json`, `docs/SPEC.md` (7.1, 8.3) |
| SPEC | 4.3 (F49), 7.1, 8.3, 8.4 (R9), Anhang A (Q10, Q11) |

## Ziel

Der Nutzer trägt mehrere Wettkämpfe mit Priorität A, B oder C ein. Der Planer richtet Phasen und Taper an den A-Wettkämpfen aus, gibt B-Wettkämpfen eine kurze Entlastung und behandelt C-Wettkämpfe wie eine harte Einheit. Eine Saisonansicht zeigt je Woche Phase, Ziellast und vorausberechnete Fitness bis zum letzten Wettkampf.

## Warum

Vorbilder: Der Jahresplan von TrainingPeaks (Annual Training Plan) mit Prioritäten A, B, C und Wochenzielen, der Planbaukasten von TrainerRoad mit mehreren Ereignissen. Lastwerk kennt ein Ziel je Plan.

## Umfang

1. Datenmodell: Ziele als Liste mit Datum, Sportart, Priorität und Name; Migration der bestehenden Ziele (ein Ziel wird A). Backup und Schema angepasst.
2. Planer: Phasen rückwärts vom nächsten A-Wettkampf wie bisher (8.3). Zwischen zwei A-Wettkämpfen: Erholung nach dem ersten, dann neuer Aufbau, wenn die Lücke dafür reicht, sonst Erhalt. Länge der Erholung und Mindestlücke als Faustregel in `rules.default.json`.
3. B-Wettkampf: Wochenziel der Wettkampfwoche gesenkt, harte Einheiten mit Abstand wie `race_opener_min_days_before`. Werte als Faustregel; Taper-Prinzip nach Q10, Zahlen aus Q11 nur Sekundärquelle (O8).
4. C-Wettkampf: zählt als harte Einheit für R3 und R4, keine Entlastung.
5. R9 gilt je A-Wettkampf. Zwei A-Wettkämpfe näher als ein Taper erzeugen eine Warnung, keinen Fehler.
6. App: Saisonansicht als Zeitstrahl mit Wochen, Phase, Ziellast und CTL-Vorausberechnung, Wettkämpfe als Marken; Eingabe in der Einrichtung des Plans.

## Nicht im Umfang

Wettkampfprognosen (Nicht-Ziel, SPEC 2.3), Ziel-CTL je Wettkampf als Eingabe, Wettkämpfe mehrerer Sportarten am selben Tag (Triathlon als eigener späterer Task).

## Vorgaben

- Golden-Tests: ein A, zwei A mit großer und kleiner Lücke, A mit B davor, A mit C in der Aufbauphase.
- Bestehende Golden-Pläne mit einem Ziel bleiben unverändert.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Keine REA-Analyse. Wie TrainingPeaks den Jahresplan aufbaut, beschreibt dessen Hilfe; Zahlen daraus werden nicht übernommen.

## Abnahme

```
sh scripts/check-all.sh
```

- Golden-Tests grün, Migration mit alter Datenbank getestet.
- Attrappe der Saisonansicht mit einem und mit fünf Wettkämpfen, Sichtprüfung schmal und breit.

## Manuelle Schritte

Eigene Saison im Tauri-Fenster anlegen, Ergebnis prüfen.

## Vermerke
