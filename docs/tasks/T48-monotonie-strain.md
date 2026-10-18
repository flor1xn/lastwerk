# T48: Monotonie und Strain je Woche

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T32 (besser nach T47, dann zählt Kraft mit) |
| Bereich | `crates/core`, `app/src-tauri`, `app/ui`, `catalog/locales`, `docs/SPEC.md` (8.2) |
| SPEC | 4.3 (F47), 8.2, 2.3, Anhang A (Q34) |

## Ziel

Für jede ISO-Woche zeigt Lastwerk Monotonie (Mittel der Tageslast geteilt durch ihre Standardabweichung) und Strain (Wochenlast mal Monotonie) nach Foster 1998 (Q34), als Verlauf in "Fortschritt" und als Zahl in der Wochen-Story.

## Warum

Vorbilder: Runalyze führt Monotonie und Strain auf TRIMP-Basis, Golden Cheetah ebenso. Lastwerk zeigt bisher, wie viel trainiert wurde, aber nicht, wie gleichförmig die Last über die Woche verteilt war.

## Umfang

1. `core`: `week_monotony(daily_loads: [f64; 7]) -> Option<MonotonyStrain>`; Tage ohne Training zählen mit Last 0. Standardabweichung 0 ergibt `None` (Monotonie nicht definiert).
2. Vor dem Code die Definition der Standardabweichung (Stichprobe n-1 oder Grundgesamtheit n) am Original Q34 prüfen und in SPEC 8.2 eintragen. Ist das Original nicht greifbar, gilt die Stichprobe als Konvention mit Vermerk.
3. Übertragung kennzeichnen: Foster rechnet mit Session-RPE-Einheiten, Lastwerk mit der Last aus 8.1. Das steht im Code-Kommentar und in SPEC 8.2.
4. App: Verlauf beider Werte je Woche in "Fortschritt", Zahl in der Wochen-Story, Erklärtext.

## Nicht im Umfang

Schwellen, Warnungen oder Farben für "zu monoton" oder "zu viel Strain". Foster berichtet persönliche Schwellen für Erkrankungen; das wäre eine Überlastungsprognose (Nicht-Ziel, SPEC 2.3). Die Werte stehen beschreibend da.

## Vorgaben

Referenzwerte als Konstanten mit Rechenweg (SPEC 11.1), etwa Tageslast `[100, 0, 100, 0, 100, 0, 100]`:

- Mittel `400 / 7 = 57,142857`
- Quadratsumme der Abweichungen `4 * 42,857143^2 + 3 * 57,142857^2 = 7346,94 + 9795,92 = 17142,86`
- Stichprobe: Standardabweichung `sqrt(17142,86 / 6) = 53,4522`, Monotonie `1,0690`, Strain `400 * 1,0690 = 427,6`
- Grundgesamtheit: `sqrt(17142,86 / 7) = 49,4872`, Monotonie `1,1547`, Strain `461,9`

Gleiche Last an allen sieben Tagen ergibt `None`.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Golden Cheetah ist quelloffen (GPL v2): nachlesen, welche Standardabweichung es nutzt, als Gegenprobe im Vermerk. Kein Code übernehmen. REA ist dafür nicht nötig.

## Abnahme

```
sh scripts/check-all.sh
```

- Referenzwerte grün, Grenzfälle (alle Tage gleich, Woche ohne Training) getestet.
- Attrappe mit leerer und extremer Woche, Vitest für Formatierung, Sichtprüfung hell und dunkel.

## Manuelle Schritte

Plausibilität an eigenen Wochen durch den Nutzer (SPEC 11.5).

## Vermerke
