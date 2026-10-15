# T31: Aktivitätsdetail ausbauen

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 (Auswertung) |
| cloudfähig | ja |
| Voraussetzung | T25 |
| Bereich | `crates/core` (Runden), `app/src-tauri` (`get_activity`), `app/ui` (Detail), `catalog/locales` |
| SPEC | 4.2 (F38), 5.5, 8.1, Anhang A (Q16) |

## Ziel

Die Detailansicht einer Aktivität beantwortet die Fragen, die andere Werkzeuge (TrainingPeaks, intervals.icu, Garmin Connect) im Detail beantworten: Wie gleichmäßig war die Einheit (aerobe Entkopplung, Effizienzfaktor), wie liefen die Runden, wie verteilte sich die Zeit auf die Zonen, wie war das Tempo beim Laufen.

## Warum

Die Rechnung der aeroben Entkopplung liegt seit T25 Teil A in `core`, wird aber nirgends gezeigt (Vermerk T25: "braucht die Messreihe, die nur die Detailansicht liest"). Runden stehen in jeder FIT-Datei und fehlen bisher ganz.

## Umfang

1. `core::metrics::laps`: reine Funktion, die aus Samples und Rundengrenzen (Start relativ zum Aktivitätsbeginn, Dauer) je Runde Dauer, Strecke (aus der Datei), mittlere Leistung, mittlere Herzfrequenz und mittlere Geschwindigkeit bildet. Mittel nur über vorhandene Werte (wie `downsample`), keine erfundenen Nullen. Handgerechneter Referenzwert.
2. `get_activity` liest die Reihe einmal und liefert zusätzlich: `decoupling` (Wert in Prozent und Code aus `decoupling_state`), `efficiency_factor` (NP durch mittlere HF, nur Rad ab 45 min wie im Dashboard), `laps`, Geschwindigkeit in der Messreihe (`speed_mps`).
3. Oberfläche: Kennzahlen als `StatRow` mit Deutung in einem Satz (Entkopplung, EF), Tabelle der Runden (ab zwei Runden), Zeit in Zonen zusätzlich als waagrechte Balken mit Prozent, Tempo-Kurve (min/km) bei Läufen.

## Nicht im Umfang

Karte (F30), Höhenprofil, Bearbeitung von Runden, Intervallerkennung ohne Runden.

## Vorgaben

- Entkopplung und EF tragen den Vermerk "Q16, nur Sekundärquelle" im Code. Grenzen aus T25 (5 Prozent, 45 min, Faustregel), keine neuen Zahlen.
- Ohne Messreihe (Datei fehlt oder geändert) erscheinen Entkopplung und Runden nicht, die Ansicht sagt, warum.

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo fmt --manifest-path app/src-tauri/Cargo.toml --check && cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings && cargo test --manifest-path app/src-tauri/Cargo.toml --locked
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
sh scripts/check-all.sh
```

Dazu: Attrappe mit synthetischen Werten für jede neue Ansicht (auch leer und extrem), Vitest für Deutung und Formatierung, Playwright-Rauchtest Deutsch und Englisch, Sichtprüfung hell und dunkel, schmal und breit (`break-ui`).

## Manuelle Schritte

Sichtprüfung im echten Tauri-Fenster mit eigener Datenbank durch den Nutzer.

## Vermerke

### Geliefert (15. Oktober 2026)

- `core::metrics::laps` (`LapBounds`, `lap_stats`) mit Referenzwerten. Geschwindigkeit je Runde aus Strecke durch Timerzeit der Datei, sonst Mittel der Messwerte.
- `get_activity` liest die FIT-Datei einmal (`read_activity_file`) und liefert zusätzlich `efficiency_factor`, `decoupling` (`percent`, `state`), `laps` (erst ab zwei Runden) und `series.speed_mps`. Unplausible Geschwindigkeiten (nicht endlich oder negativ, SPEC 6.5) fallen weg.
- Detailansicht: Abschnitt "Gleichmäßigkeit" mit Aussage aus dem Code, Kennzahlen nur, wenn mindestens eine berechenbar ist; Rundentabelle (Spalten nur für vorhandene Kanäle, Tempo bei Läufen, km/h sonst); Tempo als dritte Reihe im Verlauf bei Läufen (Achse umgekehrt, schneller oben).
- Zeit in Zonen hatte schon Balken mit Prozent (`ZoneTable`), keine Änderung nötig.
- Entkopplung rechnet für jede Sportart mit Leistung und Herzfrequenz (auch Läufe mit Leistungsmesser), Effizienzfaktor bleibt wie im Dashboard auf Rad beschränkt.
- Anzeigegrenze für Tempo: unter 0,5 m/s kein Tempo (Konvention der Anzeige, `MIN_PACE_SPEED_MPS`).
