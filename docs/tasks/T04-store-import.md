# T04: Speicher, Duplikate und Import (lastwerk-store, lastwerk-cli)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M1 |
| Voraussetzung | T01, T03 |
| cloudfähig | ja |
| Bereich | `crates/store`, `crates/cli` |
| SPEC | 5.4 (store, cli), 6.1 bis 6.5, 11.4 (Migration), 11.5, 12 (M1) |

## Ziel

SQLite-Cache mit Migrationen, Import per SHA-256 inkrementell, Duplikatregel Edge/Uhr, Wiederaufbau, und die CLI-Befehle `import`, `rebuild`, `plausibility`.

## Umfang

- Schema nach SPEC 6.2 als nummerierte SQL-Migrationen (`PRAGMA user_version`), Test N-1 nach N. Tabellen für Cache und Nutzerdaten (`threshold`, `goal`, `availability`, `plan`, `manual_session`, `setting`). `recovery_day` kommt erst in T14.
- `Store::open`, `upsert_source_file`, `upsert_activity`, `daily_loads`, `thresholds_for` (nur Lesen, gebraucht für die Lastberechnung), `export_user_data`, `import_user_data`, `rebuild_cache` nach SPEC 5.4.
- Import: Ordner rekursiv, SHA-256 je Datei, unveränderte Dateien überspringen, Belastung je Aktivität mit `lastwerk-core` berechnen (ohne Schwelle: Status `missing_threshold`).
- Duplikatregel SPEC 6.4 (Konvention 120 s, 10 Prozent, einstellbar), Primärwahl, `duplicate_of`.
- CLI nach SPEC 5.4: `import`, `rebuild`, `plausibility`. Exit-Codes 0, 1, 2, 3. Importbericht mit Anzahl je Ergebnis (neu, unverändert, Fehler, Duplikate).

## Nicht im Umfang

PMC, Zonen, `thresholds set` (T05), Planspeicherung über das Schema hinaus (T08).

## Vorgaben

- Speicherort der Datenbank konfigurierbar, Standard `%LOCALAPPDATA%\Lastwerk\lastwerk.db` (SPEC 6.1). Tests nutzen Pfade unter `target/`.
- `rusqlite` mit gebündeltem SQLite ist erwartet. Vermerke Version und Lizenz.
- Nutzertabellen bleiben beim `rebuild` unberührt (SPEC 6.3).

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p lastwerk-store
cargo run -p lastwerk-testkit --bin gen-fixtures -- --out target/fixtures
cargo run -p lastwerk-cli -- import --dir target/fixtures --db target/m1.db
cargo run -p lastwerk-cli -- rebuild --dir target/fixtures --db target/m1b.db
cargo test --workspace
```

Erfolg (SPEC 12, M1): Importbericht der Fixtures nennt je Fixture eine Aktivität, das Edge/Uhr-Paar ergibt genau eine primäre Aktivität, beschädigte Dateien erscheinen als Fehler und der Import läuft zu Ende. `import` und `rebuild` erzeugen identische Kennzahlen (Test vergleicht beide Datenbanken). Migrationstest und Test "Nutzerdaten überleben `rebuild`" grün.

## Manuelle Schritte

`lastwerk plausibility --dir local/fit` auf eigenen Daten (SPEC 11.5). Die Ausgabe nicht speichern und nicht committen. Zeigt die Duplikatregel erwartete Paare? Das Ergebnis fließt in O10.

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026.

Neue Abhängigkeiten (alle zu `deny.toml` passend, `cargo-deny` selbst lief in dieser Umgebung nicht, die Lizenzen der aufgelösten Pakete wurden über `cargo metadata` geprüft):

| Crate | Name | Version | Lizenz | Grund |
|---|---|---|---|---|
| store | rusqlite, ohne Standardfeatures, Feature `bundled` | 0.40.2 | MIT | SQLite-Zugriff. `bundled` baut SQLite (gemeinfrei) aus C-Quellen, unabhängig vom System. Ohne Standardfeatures entfallen der WASM-Treiber und der Statement-Cache. Zieht `libsqlite3-sys` 0.38.2 (MIT), `fallible-iterator` und `fallible-streaming-iterator` (MIT/Apache-2.0), `smallvec` (MIT OR Apache-2.0) |
| store | sha2 | 0.11.1 | MIT OR Apache-2.0 | SHA-256 je Datei |
| store | serde (derive), serde_json | 1.0.229, 1.0.154 | MIT OR Apache-2.0 | JSON-Export der Nutzerdaten (beide schon im Workspace) |
| store | chrono, thiserror | 0.4.45, 2.0.21 | MIT OR Apache-2.0 | Datumstypen ohne Systemuhr, Fehlertyp (beide schon im Workspace) |
| cli | clap, Feature `derive` | 4.6.7 | MIT OR Apache-2.0 | Argumente und Hilfetexte |
| cli | chrono mit Feature `clock` | 0.4.45 | MIT OR Apache-2.0 | `Utc::now()` nur in der CLI. `core` und `store` bekommen die Zeit als Parameter |

Entwicklung: `lastwerk-testkit` und `lastwerk-core` (Pfade). `assert_cmd` wird nicht gebraucht, die CLI-Tests starten `env!("CARGO_BIN_EXE_lastwerk")` mit `std::process::Command`.

Änderungen außerhalb von `crates/store` und `crates/cli`, beide klein und begründet:

- `crates/fit`: `list_fit_files` und `read_fit_file` (in `lib.rs` exportiert), damit der Import unveränderte Dateien über die Prüfsumme überspringt, ohne sie zu dekodieren. `scan_dir` bleibt unverändert und der Import nutzt es nicht direkt. Abweichung von der Vorgabe "über `scan_dir`", die sonst jede Datei bei jedem Lauf dekodiert hätte. Vermerkt in SPEC 5.4.
- `crates/testkit`: Alle Radfixtures starteten am selben Zeitpunkt. Die Duplikatregel (SPEC 6.4) hätte sie untereinander zusammengelegt, ein Import des Ordners ergäbe dann nicht je Fixture eine primäre Aktivität. Jedes Fixture außer dem Paar `pair_edge` und `pair_watch` hat jetzt einen eigenen Tag (`ride_on_day`), `gaps_long`, `no_session`, `run_speed`, `swim_pool_25m` und `strength` bleiben am Starttag (andere Sportart oder Sportart `other`). Die Erwartungen im Manifest folgen aus den Parametern und ändern sich entsprechend. Vermerkt in SPEC 11.3.

Abweichungen von der SPEC (jeweils im betroffenen Abschnitt vermerkt):

- 6.2: `source_file.sha256` nicht `UNIQUE`, dafür `rel_path` `UNIQUE`. `activity.source_file_id` `UNIQUE`. `threshold.sport` kennt `all`. `activity_load.method` kennt `none` für Fehlerzeilen. Geschlecht als `setting` `athlete.sex`.
- 5.4: `rebuild_cache` und `import_dir` mit `now`, `opts` und Bericht als Ergebnis. `save_plan` fehlt, sie kommt mit T08. `DailyLoad` steht vorläufig im Store, bis T05 ihn in `core` führt.
- 6.4: Dauerabweichung bezogen auf die längere Dauer, Primärwahl nach Rang statt nach Paaren.
- Texte der CLI stehen als Rust-Zeichenketten in `crates/cli/src/report.rs` und `error.rs` (Deutsch), nicht in `catalog/locales`. Sobald die Oberfläche die Berichte teilt (T06), wandern sie in den Katalog.

Offene Punkte für spätere Tasks:

- Die Belastung entsteht nur beim Import. Nach `thresholds set` (T05) muss die CLI `rebuild` empfehlen oder der Store braucht eine Neuberechnung, die Messpunkte aus den Dateien liest.
- Die manuelle Umkehr der Primärwahl bei Duplikaten (UI, SPEC 6.4) ist nicht umgesetzt.
- `activity_zone_time` und `activity_best_power` sind angelegt, aber leer (T05).

Ergebnis der Fixtures (ohne Schwellenwerte): 19 Dateien, 15 neu, 1 ohne Aktivität (Monitoring), 3 Fehler (`crc_mismatch`, `truncated`, `empty_file`), 14 zählende Aktivitäten, 1 Duplikat (`pair_watch` von `pair_edge`), Belastung 0 berechnet, 14 ohne Schwelle, 1 ohne Messkanal (Kraft ohne RPE).
