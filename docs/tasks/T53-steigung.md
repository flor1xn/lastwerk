# T53: Steigungsbereinigtes Tempo und Höhenmeter je Stunde

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T31, T54 (gleicher Ordner `crates/fit`) |
| Bereich | `crates/fit`, `crates/testkit`, `crates/core`, `app/src-tauri`, `app/ui`, `catalog/locales`, `docs/SPEC.md` (8.1, 14, Anhang A) |
| SPEC | 4.3 (F50), 8.1, 14 (O8), Anhang A (Q37) |

## Ziel

Das Aktivitätsdetail zeigt für Läufe ein steigungsbereinigtes Tempo neben dem gemessenen, nach den Energiekosten des Laufens bei Steigung (Minetti 2002, Q37). Für Rad und Lauf zeigt es Höhenmeter aufwärts und Höhenmeter je Stunde an Anstiegen (VAM).

## Warum

Vorbilder: Strava (Grade Adjusted Pace), Runalyze und Golden Cheetah. Lastwerk liest heute keine Höhe aus FIT, ein Lauf im Gebirge wirkt deshalb langsam. O8 nennt die Steigungskorrektur als offen.

## Umfang

1. `crates/fit`: Felder `altitude`, `enhanced_altitude` und `distance` der Nachricht `record` lesen, Nummern und Skalierung nach öffentlicher Profilbeschreibung (O2). Testkit schreibt sie in synthetische Dateien.
2. Quelle: Polynom der Laufkosten `Cr(i)` aus dem Volltext von Q37 (frei verfügbar über das Repositorium der Universität Brescia) übernehmen, Koeffizienten mit Seitenangabe im Kommentar, Prüfstatus in Anhang A nachtragen.
3. `core`: Steigung aus geglätteter Höhe über eine Strecke (Fenster als Faustregel), bereinigtes Tempo `v * Cr(i) / Cr(0)`. Außerhalb des gemessenen Bereichs von Q37 (Steigung -0,45 bis +0,45) kein Wert.
4. `core`: Höhenmeter aufwärts mit Schwelle gegen Rauschen (Faustregel), VAM für Anstiege über eine Mindestdauer (Konvention, ohne Quelle als Faustregel gekennzeichnet).
5. App: bereinigtes Tempo als zweite Linie und in den Runden, Höhenprofil, Höhenmeter und VAM in der Zusammenfassung.
6. O8 für "Steigungskorrektur im Laufen" aktualisieren, Abweichungen vom Original vermerken.

## Nicht im Umfang

Steigungskorrektur in der Laufbelastung (rTSS, 8.1) und in den Laufbestzeiten (T37); das wäre eine eigene Entscheidung. Barometrische gegen GPS-Höhe unterscheiden.

## Vorgaben

- Referenz: Steigung 0 ergibt gemessenes Tempo. Ein Wert bei positiver Steigung mit Rechenweg aus dem Polynom, sobald die Koeffizienten vorliegen.
- Synthetische Datei mit konstanter Steigung, Ergebnis gegen Handrechnung.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Keine REA-Analyse. Stravas Verfahren ist nicht veröffentlicht und wird nicht nachgebaut; Lastwerk folgt Q37.

## Abnahme

```
sh scripts/check-all.sh
```

- Referenzwerte und synthetische Dateien grün, Datei ohne Höhe zeigt keinen Wert statt 0.
- Attrappe mit flachem und bergigem Lauf, Sichtprüfung.

## Manuelle Schritte

Plausibilität an einem eigenen Berglauf durch den Nutzer.

## Vermerke
