# T33: Leistungskurve als eigene Seite

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 (Auswertung) |
| cloudfähig | ja |
| Voraussetzung | T25 |
| Bereich | `crates/core` (Dauern), `crates/store` (Migration, Cache-Version), `app/src-tauri` (Befehl `get_performance`), `app/ui` (Ansicht "Leistung"), `catalog/locales` |
| SPEC | 4.3 (F35), 4.2 (F40), 6.2, 6.3 |

## Ziel

Eine Ansicht "Leistung" zeigt die beste mittlere Leistung je Dauer von 1 s bis 2 h als Kurve (wie WKO, intervals.icu, Golden Cheetah), für einen wählbaren Zeitraum gegen den gleich langen Zeitraum davor, mit Datum und Verweis auf die Aktivität jedes Bestwerts.

## Warum

F35 (Kann). Gespeichert sind bisher nur fünf Dauern (5 s, 1, 5, 20, 60 min), für eine Kurve zu wenig.

## Umfang

1. `BEST_POWER_DURATIONS_S` auf 22 Dauern zwischen 1 s und 2 h erweitern (Konvention, Raster wie üblich in Leistungsprofilen). Die fünf bisherigen bleiben enthalten, damit FTP-Schätzung (8.6) und Dashboard unverändert rechnen.
2. Cache-Version: Setting `cache.derived_version` im Speicher. Ist sie kleiner als die des Programms, behandelt der nächste Import jede bekannte Datei wie geändert und rechnet alle abgeleiteten Werte neu, danach steht die neue Version. Keine Nutzerdaten berührt (SPEC 6.1). Die App liest beim Start ein, wenn der Cache veraltet ist (`Settings.cache_outdated`).
3. Befehl `get_performance(today, range_days)` mit Zeiträumen 42, 90, 365 Tage und "alles": je Dauer Bestwert, Datum, Aktivität, Vorzeitraum, Veränderung, Code aus `power_curve_trend_code`.
4. Ansicht "Leistung" in der Navigation, Abschnitt Leistungskurve: Kurve mit logarithmischer Zeitachse (uPlot), Tabelle der Eckdauern mit Link zur Aktivität.

## Nicht im Umfang

Critical Power, W', eFTP aus Modellen (keine Quelle in Anhang A), Watt je Kilogramm (kein Körpergewicht im Datenmodell).

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

- `core`: `POWER_CURVE_DURATIONS_S` (22 Dauern, enthält die fünf Eckdauern, Test), `compare_power_curves_over` für beliebige Dauern; `compare_power_curves` bleibt auf den Eckdauern (Dashboard, MCP unverändert).
- `store`: Import schreibt das ganze Raster. Migration 0005 legt `cache_meta` an, `DERIVED_CACHE_VERSION = 2`, `Store::cache_outdated`. Ein veralteter Stand liest beim nächsten Import jede bekannte Datei neu (Unit-Test mit Fixtures: Neuimport genau einmal, gleiche IDs, danach wieder `unchanged`).
- App: `Settings.cache_outdated`; die Oberfläche liest beim Start einmal ein und gleicht danach mit Garmin ab (nicht gleichzeitig). Befehl `get_performance(today, range)` an allen vier Stellen, `range` ist `42`, `90`, `365` oder `all` (sonst `invalid_value`). Bei Gleichstand gewinnt der frühere Tag.
- Detailansicht zeigt weiter nur die Eckdauern.
- Seite "Leistung" in der Navigation nach "Fortschritt": Zeitraum als `Segmented`, Kurve mit logarithmischer Achse (feste Teilstriche, schmal weniger), Vorzeitraum gestrichelt, Wert unter dem Zeiger als Text, Tabelle mit zwölf Eckdauern, Tag als Verweis auf die Aktivität, Veränderung mit Deutung.
- Attrappe `mock-performance.ts` (erfundene Kurve ohne Modell), `?cache=alt` für den Start mit veraltetem Cache.
