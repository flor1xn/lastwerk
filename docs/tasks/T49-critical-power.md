# T49: Critical Power, W' und W'-Bilanz

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T33 |
| Bereich | `crates/core`, `app/src-tauri`, `app/ui`, `catalog/locales`, `docs/SPEC.md` (8.6, Anhang A) |
| SPEC | 4.3 (F48), 8.6, Anhang A (Q35, Q36) |

## Ziel

Die Seite "Leistung" zeigt Critical Power (CP) und W' aus der Leistungskurve des gewählten Zeitraums nach dem linearen Arbeit-Zeit-Modell (Monod und Scherrer, Q36). Das Aktivitätsdetail zeigt für Radfahrten mit Leistung die W'-Bilanz nach Skiba 2012 (Q35).

## Warum

Vorbilder: Golden Cheetah, intervals.icu, WKO und Xert führen CP und W', intervals.icu und Golden Cheetah zeigen die W'-Bilanz im Verlauf. T33 hat das Modell ausgelassen, weil Anhang A keine Quelle hatte; Q35 und Q36 schließen diese Lücke.

## Umfang

1. Quellen: Q36 (Monod und Scherrer 1965) bibliographisch bestätigen und den Prüfstatus in Anhang A nachtragen; Q35 (Skiba 2012) ist bestätigt, die Gleichungen am Volltext prüfen (DOI 10.1249/MSS.0b013e3182517a80).
2. `core`: `fit_critical_power(points: &[(u32, f64)]) -> Option<CpModel>`. Lineare Regression der Arbeit `P * t` über `t` für Dauern im Fenster `cp.window_s` (Faustregel, Vorschlag 180 bis 1200 s, Parameter in `rules.default.json`). CP ist die Steigung, W' der Achsenabschnitt. Weniger als drei Dauern im Fenster oder W' kleiner oder gleich 0 ergibt `None` mit Code.
3. `core`: `w_prime_balance(samples, cp, w_prime) -> Vec<f64>` nach dem Integralmodell aus Q35 (Verbrauch über CP, exponentielle Erholung darunter). Die Zeitkonstante folgt Q35. In Sekundärdarstellungen steht `tau = 546 * exp(-0,01 * D_CP) + 316` mit `D_CP` als CP minus mittlere Leistung unter CP; der Ausdruck kommt erst nach Prüfung am Volltext in den Code, bis dahin mit Vermerk "nur Sekundärquelle".
4. App: CP und W' mit Fenster und genutzten Punkten auf der Seite "Leistung", Modellgerade über der Kurve; W'-Bilanz als zweite Achse im Detail, niedrigster Wert als Zahl.
5. SPEC 8.6 um das Modell ergänzen, F48 nachtragen.

## Nicht im Umfang

- Drei-Parameter-Modelle, Morton, Leistungsmodelle mit Pmax.
- CP als Ersatz für die FTP oder als Grundlage der Last (8.1 bleibt bei FTP).
- Critical Speed für den Lauf (Folgetask, gleiches Modell auf Strecke und Zeit aus T37).
- Einheiten in Prozent von CP oder W'.

## Vorgaben

Referenz mit Rechenweg (SPEC 11.1): zwei Punkte (180 s, 400 W) und (1200 s, 300 W), Arbeit 72000 J und 360000 J. Steigung `(360000 - 72000) / (1200 - 180) = 282,35 W`, Achsenabschnitt `72000 - 282,35 * 180 = 21176,5 J`. Für den Test drei Punkte auf dieser Geraden wählen. W'-Bilanz: konstante Leistung über CP für eine bekannte Dauer verbraucht genau `(P - CP) * t` (Handrechnung), danach steigt die Bilanz wieder.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Golden Cheetah (GPL v2, quelloffen) als Gegenprobe: Welches Fenster nutzt es für das Zwei-Parameter-Modell, wie stellt es die W'-Bilanz dar? Lesen, nicht kopieren. Eine REA-Analyse ist nur sinnvoll, wenn ein installiertes Programm eine Darstellung zeigt, die die Dokumentation nicht erklärt.

## Abnahme

```
sh scripts/check-all.sh
```

- Referenzwerte grün, `None`-Fälle getestet (zu wenige Punkte, W' negativ).
- Attrappe mit Kurve und Fahrt, auch ohne Leistung und mit Lücken; Sichtprüfung schmal und breit.

## Manuelle Schritte

Plausibilität gegen eigene Werte durch den Nutzer: CP knapp unter oder bei der FTP ist erwartbar, große Abweichung als Vermerk.

## Vermerke
