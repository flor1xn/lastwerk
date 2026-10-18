# T58: Streckenform ohne Kartenkacheln

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T31, T53 (gleicher Ordner `crates/fit`) |
| Bereich | `crates/fit`, `crates/testkit`, `app/src-tauri`, `app/ui`, `crates/mcp` (nur Test), `crates/coach` (nur Test) |
| SPEC | 4.3 (F55, F30), 2.1 (Datenumfang für Claude), 9.3, 10.3 |

## Ziel

Das Aktivitätsdetail zeigt die Form der Strecke als Linie ohne Kartenhintergrund, mit Start, Ende und der Stelle unter dem Zeiger im Diagramm. Kein Netzwerk, keine Kacheln.

## Warum

Alle Vorbilder zeigen eine Karte. Lastwerk hat F30 (Karte mit Kacheln, Standard aus) wegen O3 nicht gebaut. Die Form allein reicht, um eine Einheit wiederzuerkennen und Diagramm und Ort zu verbinden, und braucht keine Entscheidung zu O3.

## Umfang

1. `crates/fit`: `position_lat` und `position_long` der Nachricht `record` lesen (Halbkreise, Umrechnung nach öffentlicher Profilbeschreibung, O2). Nicht in der Datenbank speichern; das Detail liest wie bisher die FIT-Datei.
2. Projektion: lokale gleichabständige Projektion um den Mittelpunkt (Faustregel, für Strecken unter einigen hundert Kilometern ausreichend), Vereinfachung der Linie für die Anzeige.
3. App: SVG-Linie im Detail, Kopplung an den Zeiger der Diagramme.
4. Schutz: Koordinaten gehen nie an `lastwerk-mcp` (2.1) und nie in den Modelleingang des Coachs (9.3). Je ein Test, der das sichert.

## Nicht im Umfang

Kartenkacheln (F30, O3), Wiedererkennen gleicher Strecken, Ausblenden von Start und Ziel für Bildschirmfotos (eigener Task, wenn gewünscht).

## Vorgaben

- Test: synthetische Datei mit Quadrat aus vier Punkten ergibt ein Quadrat in Bildkoordinaten (Handrechnung).
- Datei ohne Positionen: Abschnitt fehlt.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Keine REA-Analyse nötig.

## Abnahme

```
sh scripts/check-all.sh
```

- Tests grün, Netzwerkprüfung der Oberfläche unverändert (keine Anfragen).
- Attrappe mit Rundkurs, Strecke von A nach B und Indoor ohne GPS.

## Manuelle Schritte

Eigene Aktivität im Tauri-Fenster öffnen.

## Vermerke
