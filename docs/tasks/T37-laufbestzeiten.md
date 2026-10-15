# T37: Bestzeiten im Laufen

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 (Auswertung) |
| cloudfähig | ja |
| Voraussetzung | T33 |
| Bereich | `crates/core` (Bestzeiten), `crates/store` (Migration, Tabelle `activity_best_effort`), `app/src-tauri`, `app/ui` (Ansicht "Leistung"), `catalog/locales` |
| SPEC | 4.2 (F44), 6.2, 6.5 |

## Ziel

Auf der Seite "Leistung" zeigt ein Abschnitt die schnellste Zeit über 1 km, 5 km, 10 km, Halbmarathon und Marathon innerhalb von Läufen, je Zeitraum, mit Tempo, Datum und Verweis auf die Aktivität (wie die Bestleistungen in Garmin Connect und Strava).

## Umfang

1. `core::best_effort`: kürzeste Zeit für eine Strecke aus der Geschwindigkeitsreihe. Strecke je Sekunde aus der Geschwindigkeit, Fenster nie über eine Pause (wie `best_power`), fehlende Geschwindigkeit zählt als Stillstand (unterschätzt nie die Zeit). Handgerechneter Referenzwert.
2. Migration: Tabelle `activity_best_effort (activity_id, distance_m, seconds)` im Cache, beim Import gefüllt für Läufe. Cache-Version aus T33 erhöhen.
3. `get_performance` liefert `run_bests` je Strecke (Zeitraum und Vorzeitraum), die Oberfläche zeigt Zeit, Tempo (min/km), Datum und Link.

## Nicht im Umfang

Wettkampfprognosen (VDOT, Riegel: nicht in Anhang A geprüft), Bestzeiten im Schwimmen und Radfahren.

## Vorgaben

- Strecken in ganzen Metern: 1000, 5000, 10000, 21097, 42195. Die Zeit ist auf ganze Sekunden gerundet nach oben (Fenster in ganzen Sekunden), im Code vermerkt.

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

- `core::best_effort` (`best_efforts`, `BEST_EFFORT_DISTANCES_M`) mit Referenzwerten: konstante Geschwindigkeit, schneller Teil mit Aufrundung, GPS-Sprung und Lücke, Pause. Zwei Zeiger über Präfixsummen je Abschnitt.
- Plausibilität: Laufgeschwindigkeit über 12 m/s (43,2 km/h) zählt als Stillstand (`RUN_SPEED_MAX_MPS`, Faustregel: über der Spitzengeschwindigkeit im Sprint). Abweichung von der Task-Datei: Die Grenze war dort nicht genannt; ohne sie verfälscht ein einziger GPS-Sprung den schnellsten Kilometer.
- Migration 0006 `activity_best_effort` (Cache), Import nur für Läufe, `delete_activity_children` und `clear_cache` berücksichtigen die Tabelle, `DERIVED_CACHE_VERSION = 3`, `Store::best_efforts`. Store-Test mit der Fixture `run_speed`: 1000 m in 286 s, 5000 m in 1429 s.
- `core::metrics::change_percent` ist jetzt öffentlich (vorher nur im Crate), der Service nutzt es für die Veränderung.
- `get_performance` liefert `run_bests` (kürzeste Zeit je Strecke im Zeitraum mit Herkunft, kürzeste im Vorzeitraum). Seite "Leistung": Abschnitt "Bestzeiten im Laufen" mit Zeit, Tempo, Tag als Verweis, Zeitraum davor und Unterschied (schneller grün).
