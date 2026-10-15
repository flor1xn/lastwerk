# T22: Abgleich mit Garmin (Aktivitäten und Erholung)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | 1.0 |
| Voraussetzung | T21 |
| cloudfähig | ja (nur Attrappen), Livetest in T23 |
| Bereich | `crates/cli`, `crates/store`, `schema/recovery-day.schema.json`, `crates/garmin` (nur Ergänzungen) |
| SPEC | 5.6, 6.1, 6.2, 7.3 |

## Ziel

`lastwerk garmin sync` (und dieselbe Bibliotheksfunktion für die App) holt neue Aktivitäten als FIT-Dateien und Erholungswerte und schreibt sie in Speicher und FIT-Ablage.

## Umfang

1. Migration `crates/store/migrations/0004_recovery_garmin.sql`: Spalten `body_battery_high`, `body_battery_low` (INTEGER, 0 bis 100) in `recovery_day`. `RecoveryDay` in `crates/store/src/recovery.rs` erweitern, Backup und Wiederherstellung berücksichtigen (alte Backups ohne Felder bleiben lesbar). Schema `recovery-day` um die zwei optionalen Felder erweitern, `schema_version` bleibt 1, wenn die Felder optional sind (sonst 2 mit Begründung).
2. Neue Setting-Schlüssel in `lastwerk-store`: `garmin.last_sync` (Datum), `garmin.sync_on_start` (`true`/`false`, Standard `true`), `garmin.backfill_days` (Standard 365, Faustregel, 1 bis 1825).
3. `crates/cli/src/garmin.rs` (Bibliothek): `pub fn sync(store, client, fit_dir, today, progress: &mut dyn FnMut(SyncProgress)) -> Result<SyncSummary>`.
   - Fenster: ohne `garmin.last_sync` `today - backfill_days`, sonst `last_sync - 7 Tage` (wie `sync --refresh-days 7`, SPEC 5.5).
   - Aktivitäten: Liste seitenweise (100) bis zur ersten Aktivität vor dem Fenster. Je Aktivität, deren Datei `garmin/fit/<activity_id>.fit` noch fehlt: ZIP holen, mit Crate `zip` (Vermerk) nur Einträge `*.fit` lesen, Größenlimit 256 MiB entpackt, Name aus der Aktivitätskennung bilden, nie aus dem ZIP. Atomar schreiben (temporär, dann umbenennen).
   - Danach normaler Import des Ordners `fit_dir` über die bestehende Importfunktion (`lastwerk-store` / `commands.rs` `import`), damit Duplikatregel 6.4 und Belastung greifen.
   - Erholung: HRV über `hrv_range` (Spannen höchstens 336 Tage, `hrvSummaries[].calendarDate` und `.lastNightAvg`), Ruhepuls über `rhr_range` (`[].calendarDate`, `.value`), Body Battery über `body_battery` (Spannen höchstens 31 Tage, Höchst- und Tiefstwert aus `bodyBatteryValuesArray[][1]`), Schlaf je Tag über `sleep_day` nur für die letzten 90 Tage des Fensters (Faustregel: 7-Tage-Basis und 90-Tage-Trend reichen, spart Aufrufe) mit `dailySleepDTO.sleepTimeSeconds` und `dailySleepDTO.sleepScores.overall.value`. Feldnamen laut `docs/garmin-feldabdeckung.md`; wo der Bericht nur Strukturen zeigt (Range-Endpunkte), Annahme im Code-Kommentar kennzeichnen und O9 in SPEC 14 nachtragen. Plausibilitätsgrenzen aus `bridge/src/lastwerk_bridge/normalize.py:70-75` übernehmen (Quelle dort nennen), Werte außerhalb verwerfen. Schreiben über `Store::upsert_recovery_day` mit `source = "garmin-app"`.
   - Am Ende `garmin.last_sync = today`, nur wenn kein Abbruch durch Sperre. `SyncSummary`: neue Aktivitäten, importierte, Erholungstage, Aufrufe, Fehler je Art.
   - Fortschritt: Phase (`activities`, `recovery`, `import`), erledigt, gesamt (soweit bekannt).
4. CLI: `lastwerk garmin status` (verbunden ja/nein, `display_name` nicht ausgeben, nur ob vorhanden; letzter Abgleich), `lastwerk garmin sync [--days N]`, `lastwerk garmin logout`. Keine Anmeldung in der CLI (die braucht das Fenster der App), Meldung verweist darauf.
5. FIT-Ablage: `paths.rs` um `<Datenordner>/Lastwerk/garmin/fit` ergänzen, `LASTWERK_GARMIN_FIT` überschreibt (für Tests).

## Nicht im Umfang

Tauri, Anmeldefenster (T23), Dashboard (T25), Stress, weitere Datenarten, Archiv.

## Vorgaben

- Tests mit Attrappe des `GarminClient` (Trait oder Testserver aus T21), synthetische ZIPs mit FIT aus `lastwerk-testkit`, synthetische JSON-Antworten. Fälle: erster Lauf, Folgelauf mit Überlappung, bekannte Datei wird nicht erneut geladen, ZIP ohne FIT, übergroßes ZIP, Zip-Slip-Name im ZIP, unplausible Werte, Sperre mitten im Lauf (kein `last_sync`).
- Kein Pfad in Meldungen (wie T04).

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
```

## Vermerke

### Abhängigkeiten (Stand 15. Oktober 2026, crates.io)

Neue direkte Abhängigkeiten von `crates/cli`. `deny.toml` ist unverändert, `cargo deny check` meldet `advisories ok, bans ok, licenses ok, sources ok`.

| Name | Version | Lizenz | Grund |
|---|---|---|---|
| zip | 8.6.0 | MIT | Entpacken des ZIP, das Garmin als FIT-Original liefert. Nur lesen. `default-features = false` und die eine Funktion `deflate-flate2-zlib-rs` (Deflate mit dem reinen Rust-Backend `zlib-rs`). Die Funktion `deflate` hätte zusätzlich `zopfli` (Komprimierer) mitgebracht, `aes-crypto`, `bzip2`, `lzma`, `zstd`, `ppmd`, `time` bleiben aus. Gewählt wurde 8.6.0 (25. April 2026) statt der Reihe 9.0, weil 9.0.0 erst fünf Tage alt ist |
| lastwerk-garmin | Pfad | MIT OR Apache-2.0 | Client, Token-Ablage und Fehlercodes (T21). Keine neue Fremdabhängigkeit, aber `lastwerk-cli` und damit `app/src-tauri` ziehen dadurch `ureq` mit TLS, `rustls`, `ring` und `keyring` (T21, SPEC 5.3) in ihre Kette |

Neue Pakete in `Cargo.lock` (fünf): `zip` 8.6.0 (MIT), `flate2` 1.1.10 (MIT OR Apache-2.0), `zlib-rs` 0.6.8 (Zlib), `crc32fast` 1.5.2 (MIT OR Apache-2.0), `typed-path` 0.12.3 (MIT OR Apache-2.0). `indexmap` und `memchr` waren schon im Baum. Die Zeile steht auch in SPEC 5.3.

Folge für T23: `app/src-tauri` hat ein eigenes `Cargo.lock` und eine eigene `cargo deny`-Prüfung und bindet `lastwerk-cli` ein. Dort ist `cargo deny --manifest-path app/src-tauri/Cargo.toml --config deny.toml check` nach dem ersten Bau mit T22 und T21 neu auszuführen, und das Lockfile wird mit dem nächsten Bau aktualisiert. T22 hat `app/` nicht angefasst.

### Festlegungen und Abweichungen

Ausführlich in SPEC 5.4 unter `lastwerk-cli` und `lastwerk-store` (jeweils "Umsetzung in T22") sowie 6.2, 7.3. Kurz:

1. **`RecoveryDay` heißt im Code `RecoveryDayRecord`** (`crates/store/src/recovery.rs`); erweitert um `body_battery_high` und `body_battery_low` (`Option<u8>`). `RecoveryDayExport` und `schema/recovery-day.schema.json` bekommen dieselben Felder als optionale. `schema_version` und `backup_version` bleiben 1; Test mit einer Sicherung im Format von T14.
2. **Quelle als Trait.** `trait GarminSource`, implementiert für `GarminClient`. Die Funktion der App ist `garmin::sync_with_client(store, &GarminClient, fit_dir, today, now, progress)`; `sync` nimmt `SyncOptions { today, now, days }`. Abweichung von der Aufgabe: zusätzlicher Parameter `now` (der Speicher liest keine Uhr) und `days` für `--days`.
3. **Abbruch durch Garmin ist ein Ergebnis, kein Fehler.** `Ok(SyncSummary)` mit `aborted`, damit geholte Dateien importiert und gesammelte Erholungstage geschrieben werden. `Err(SyncError)` gibt es nur für lokale Fehler (Speicher, Dateisystem). Abbruch bei `not_connected`, `auth_failed`, `throttled`, `locked`, `network`, `store_failed` und nach 5 Fehlern in Folge (Faustregel). `garmin.last_sync` nur bei einem Lauf ohne Abbruch; die Aufgabe nannte nur die Sperre, der Grund für alle Abbrüche ist, dass auch ein Netz- oder Anmeldeabbruch ein unvollständiges Fenster hinterlässt.
4. **Rohantwort von `rhr_range`.** `GarminClient::rhr_range` gibt die Rohantwort des Endpunkts zurück (`allMetrics.metricsMap.WELLNESS_RESTING_HEART_RATE[]`). Der Bericht (`rhr_daily`: `[].calendarDate/.value`) zeigt die Form nach dem Umbau der Python-Bibliothek. Der Abgleich liest beide Formen. Annahme, in O9 nachgetragen.
5. **Schlafdauer 0 s ist keine Messung** und bleibt leer (die Bridge hätte 0 gespeichert); sonst läse R12 "null Stunden geschlafen" statt "unbekannt". Faustregel.
6. **Anzeigename nicht abgelegt.** Die Aufgabe nennt "`display_name` nur ob vorhanden" in `status`. Er wird nirgends gespeichert (er identifiziert das Konto, die Sicherung enthielte ihn), `status` arbeitet ohne Netz und kann ihn deshalb nicht prüfen; er wird bei jedem Lauf mit einem Aufruf neu geholt. `status` zeigt stattdessen, ob Token abgelegt sind.
7. **Spannen.** HRV 336 und Body Battery 31 Tage (gemessen in L1), Ruhepuls 365 Tage (Faustregel, Docstring der Bibliothek "bis etwa ein Jahr"). Schlaf nur die letzten 90 Tage des Fensters.
8. **Eigener Ort der Ablage und Sperrdatei:** `<Datenordner>/Lastwerk/garmin/fit` und `.../garmin/lockout.json`. Die Sperrdatei liegt neben, nicht im FIT-Ordner.
9. **Einschränkung `source_file.rel_path`.** Zwei Wurzelordner für den Import (eigener FIT-Ordner und `garmin/fit`) teilen sich die Tabelle `source_file`; siehe SPEC 5.4 "Grenze der Ablage". Offen für T23.

### Tests

`cargo test -p lastwerk-cli` und `-p lastwerk-store`, nur synthetische Daten, keine echte Quelle, kein Netz, kein Schlüsselbund:

- `crates/cli/tests/garmin_sync.rs` (18 Tests, Attrappe `Fake` der Quelle): Fensterregeln, erster Lauf (Aufrufe, Dateinamen, alle sechs Werte, Fortschritt in der Reihenfolge der Phasen), seitenweises Lesen der Liste bis vor das Fenster (250 Einträge, 3 Seiten), Folgelauf mit 7 Tagen Überlappung (nur die neue Datei wird geladen, nachträglich geänderte HRV ersetzt den alten Wert), vorhandene Datei wird nicht erneut geladen, ZIP ohne FIT, kein ZIP, leere Antwort, Zip-Slip-Namen im Archiv (nichts außerhalb des Ordners), unplausible Werte (7 verworfen, Grenzwerte gelten), Tag ohne gültigen Wert legt keine Zeile an, Mischen ohne Überschreiben mit Null (Quelle der Altzeile bleibt, wo nichts kommt), Antwort mit Tagen außerhalb der Spanne, Sperre mitten in den Aktivitäten und mitten in der Erholung (kein `last_sync`, geholte Daten bleiben), die anderen Abbruchgründe, Einzelfehler gegenüber fünf in Folge, Meldungen ohne Pfad, Token und Namen, `status` und `logout` ohne Netz, Sicherung samt Einstellungen.
- `crates/cli/src/garmin/fit_zip.rs` (10 Tests): Kopieren mit Limit (liest nie mehr als Limit plus eins), Stored und Deflate, atomares Schreiben ohne `.part`-Rest, ZIP ohne FIT, kein ZIP, **übergroßer Eintrag** (Limit am Test auf 100 KiB statt 256 MiB gesetzt, genau am Limit geht durch, ein Byte mehr nicht), Zip-Slip, beschädigter Eintrag (CRC), erster passender Eintrag.
- `crates/cli/src/garmin/parse.rs` (4 Tests): jedes Feld, `null`, falsche Typen, Grenzen, beide Formen des Ruhepulses.
- `crates/store`: Migration 3 auf 4 mit Erholungszeile, Einstellungen (Standard, Rundlauf, Grenzen), Body Battery in Upsert und Sicherung, **alte Sicherung ohne Body Battery** spielt sich ein, Wertebereiche in der Sicherung. `crates/cli/tests/recovery.rs`: `import-recovery` übernimmt die Felder und lehnt Werte außerhalb ab. `main.rs`: Argumente von `garmin`, `--days` nur 1 bis 1825, kein `login`.

### Nicht geprüft (Livetest in T23)

- Die Feldnamen der Bereichsendpunkte (O9) und die Form von `bodyBatteryValuesArray` (`[Zeitstempel, Wert]`) gegen ein echtes Konto.
- Ob der Bau mit `zip`/`zlib-rs` unter Windows und macOS durchläuft (reines Rust, aber nicht gebaut).
- `GarminClient` als `GarminSource`: die Weiterleitung ist trivial und nicht gegen den Attrappen-Server aus T21 getestet.
