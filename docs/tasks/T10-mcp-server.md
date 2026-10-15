# T10: MCP-Server lastwerk-mcp

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M7 |
| Voraussetzung | T05, T08 |
| cloudfähig | ja (Rauchtest mit Claude manuell) |
| Bereich | `crates/mcp` |
| SPEC | 5.4 (lastwerk-mcp), 9.6, 10.2, 11.4 (MCP-Datenschutz), 12 (M7), 14 (O19, O20) |

## Ziel

Ein schreibgeschützter MCP-Server über stdio, der nur gefilterte Kennzahlen, Pläne und Schwellenwerte liefert. Die Filtergrenze liegt im Server und ist getestet.

## Umfang

- Server auf Basis von `rmcp` (Paketname, Version, Lizenz und Transport vor der ersten Nutzung prüfen und vermerken). Nur stdio.
- Datenbank schreibgeschützt öffnen (`SQLITE_OPEN_READ_ONLY`).
- Ausgabetypen mit fester Feldliste: `ActivityView`, `FitnessView`, `WeeklyLoadView` (SPEC 5.4). Datenbankzeilen werden nie direkt serialisiert.
- Werkzeuge nach SPEC 5.4: `get_fitness_status`, `get_weekly_load`, `list_activities`, `get_activity`, `get_current_plan`, `get_plan_week`, `get_thresholds`, `explain_rule`, `get_recovery_days`. Alle mit `readOnlyHint = true` und `openWorldHint = false`. Typisierte Argumente mit Bereichs- und Längengrenzen.
- `get_recovery_days`: Standard gesperrt (Einstellung in der Datenbank, Standard aus). Ohne Freigabe ein Fehler. Existiert die Tabelle `recovery_day` noch nicht (kommt mit T14), liefert das Werkzeug nach Freigabe eine leere Liste.
- `--list-tools` und `--db <datei>` als Argumente.
- Lokales Aufrufprotokoll (Werkzeug, Parameter, Zeilenzahl, keine Nutzdaten).

## Nicht im Umfang

Schreibende Werkzeuge jeder Art. Zugriff auf `exchange/`, das Archiv oder das Garmin-Konto. Strava-Daten. Verbindung des Garmin-nahen Zusatzprogramms mit einem MCP-Client.

## Vorgaben

- Ausgaben enthalten keine Koordinaten, Aktivitätsnamen, Notizen, Gerätekennungen, Dateipfade und keine Plan-Freitexte (SPEC 5.4, 9.6).
- Uhrzeiten nicht ausgeben, nur Datum (offen O20, Standard Datum).
- Die Freigabe der Erholungswerte ist eine eigene Einstellung (offen O19, Standard aus). Entscheide nichts darüber hinaus.

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p lastwerk-mcp
cargo run -p lastwerk-mcp -- --db target/m1.db --list-tools
cargo test --workspace
```

Erfolg (SPEC 12, M7): `--list-tools` nennt genau die Werkzeuge aus 5.4, jedes mit `readOnlyHint = true` (ein Test schlägt bei jedem zusätzlichen Werkzeug fehl). Der Markertest findet in keiner Ausgabe Koordinaten, Namen, Notizen, Seriennummern oder Pfade der synthetischen Datenbank. `get_recovery_days` liefert ohne Freigabe einen Fehler. Ein `INSERT` über die Verbindung des Servers schlägt fehl. Das Aufrufprotokoll enthält keine Nutzdaten.

## Manuelle Schritte

1. Konfiguration im README ergänzen (Claude Desktop und Claude Code).
2. Rauchtest: mit Claude verbinden und fragen "Wie ist meine Form diese Woche?". Antwort muss die Werte aus `get_fitness_status` nennen.
3. Der Nutzer entscheidet vorher, ob er echte Daten an einen Cloud-Client geben will (SPEC 9.6, Punkt 1). Remote-Sessions bleiben bei synthetischen Daten.

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026. Alle Befehle unter "Abnahme" bestehen, dazu `sh scripts/check-privacy.sh`, `check-isolation.sh` und `check-i18n.sh`. Die manuellen Schritte unten stehen noch aus (Rauchtest mit Claude, Entscheidung zu echten Daten).

Neue Abhängigkeiten (Lizenzen über `cargo deny --offline check licenses bans sources` gegen `deny.toml` geprüft, alle bestanden, `deny.toml` blieb unverändert):

| Crate | Name | Version | Lizenz | Grund |
|---|---|---|---|---|
| mcp | rmcp, Features `server`, `macros`, `transport-io` | 3.5.1 | Apache-2.0 | Offizielles Rust-SDK der MCP-Organisation (SPEC 5.3, Q24), die neueste Version auf crates.io (`cargo search`). Nur stdio, kein HTTP, kein Client. Zieht `rmcp-macros` (Apache-2.0), `pastey` (MIT OR Apache-2.0), `tokio-util` (MIT), `futures` (MIT OR Apache-2.0), `tracing` (MIT), `bytes` (MIT) und `thiserror` (schon im Baum) |
| mcp | tokio, Features `rt`, `macros` (über rmcp zusätzlich `io-std`, `io-util`, `sync`, `time`) | 1.53.3 | MIT | Laufzeit, die rmcp verlangt. Ein Thread (`current_thread`) genügt für einen Client über stdio. `tokio-macros` (MIT) |
| mcp | schemars, Feature `chrono04` | 1.2.3 | MIT | JSON-Schema der Argumente und Ausgaben (rmcp erzeugt daraus `inputSchema` und `outputSchema`), dieselbe Hauptversion wie in rmcp. Zieht `schemars_derive` (MIT), `serde_derive_internals` (MIT OR Apache-2.0), `dyn-clone` (MIT OR Apache-2.0), `darling` und `ident_case` (MIT) |
| mcp | clap, Feature `derive` | 4.6.7 | MIT OR Apache-2.0 | Argumente `--db`, `--log`, `--list-tools` mit Exit-Code 2 bei Fehlern, wie in `lastwerk-cli` (schon im Workspace) |
| mcp | jsonschema, ohne Standardfeatures | 0.58.6 | MIT | Prüft den Plan vor der Ausgabe gegen `plan.schema.json`, wie in `lastwerk-cli` (schon im Workspace). Keine Auflösung von Verweisen über Netz oder Dateisystem |
| mcp (dev) | rusqlite, Feature `bundled` | 0.40.2 | MIT | Nur im Test: legt die Tabelle `recovery_day` an, die erst T14 migriert (schon im Workspace, gleiche Version wie `lastwerk-store`) |
| mcp, store | serde_json, chrono, serde | wie im Workspace | MIT OR Apache-2.0 | JSON und Datumstypen, `chrono` in `mcp` mit `clock` für "heute" und das Protokoll |

`rmcp` 3.5.1 weicht in der API von älteren Versionen ab (`ServerConfig` statt `ServerInfo`, `ToolRouter`, `Parameters`, `Json`). Die Nutzung wurde am Quelltext in `~/.cargo/registry/src/*/rmcp-3.5.1` und `rmcp-macros-3.5.1` geprüft, nicht geraten.

Änderungen außerhalb von `crates/mcp`, klein und begründet:

- `crates/store`: `Store::open_read_only(path)` (`SQLITE_OPEN_READ_ONLY` und `PRAGMA query_only`, legt nichts an, migriert nicht), `Store::recovery_days_between(from, to)` mit `RecoveryDayRecord` (leere Liste, solange die Tabelle fehlt). Neue Datei `src/readonly.rs`, Tests in `tests/readonly.rs`. Kein neuer Fehlerfall in `StoreError`, damit `app/src-tauri` unverändert baut. Alle anderen Lesezugriffe gab es schon (`week_history`, `daily_loads`, `first_load_date`, `zone_times_of`, `best_powers_of`, `activity_loads_of`, `loads_between`, `activities_between`, `threshold_lookup`, `list_thresholds`, `active_plan`, `get_setting`).
- `README.md`: Abschnitt zur Konfiguration in Claude Desktop und Claude Code (manueller Schritt 1 ist damit erledigt).
- `docs/SPEC.md`: "Umsetzung in T10" in 5.4 (store und mcp), Abschnitt 11.4, Abhängigkeitstabelle 5.3 und Q24.
- `crates/core` blieb unverändert.

Abweichungen von der SPEC (alle in 5.4, "Umsetzung in T10", mit Grund vermerkt):

- Antworten sind Objekte mit einem Feld für die Liste (`days`, `weeks`, `activities` mit `truncated`, `sports`), weil der strukturierte Inhalt einer MCP-Antwort ein Objekt sein muss.
- Zahlen in den Ausgabetypen sind `f64` und gerundet, nicht `f32` (sonst `66.69999694824219` im JSON). `best_power_w` ist `Option` und fehlt in `list_activities`.
- `sport` ist bei `get_weekly_load` und `list_activities` optional.
- `--db` ist Pflicht, es gibt keinen Standardpfad und kein `LASTWERK_DB`. Begründung: ein Client soll nie unbemerkt eine andere Datenbank lesen. Der übliche Pfad steht im README.
- `explain_rule` nennt den Kurznamen `title` statt `name`.
- `created_at` im Plan verliert die Uhrzeit (O20), `explanation` entfällt, das Ergebnis wird gegen das Schema geprüft.
- `get_fitness_status` lässt Tage vor dem ersten Tag mit Last und nach heute aus.
- `get_recovery_days` liefert keinen Schlafwert, nur HRV, Ruhepuls und Schlafdauer (SPEC 5.4).

Offen und nicht Teil dieses Tasks:

- Die Oberfläche hat keinen Schalter für den Server und für `mcp.recovery_enabled`, keine Bestätigung nach 9.6 Punkt 1 und keine Anzeige des Aufrufprotokolls (SPEC 5.4: "in der Oberfläche einsehbar"). Bis dahin bleibt `get_recovery_days` gesperrt, der Server läuft nur, wenn der Nutzer ihn in seinen Client einträgt.
- Es gibt keinen CLI-Befehl, der einen Plan auf `active` setzt (`Store::set_plan_status`). Ohne aktiven Plan antworten `get_current_plan` und `get_plan_week` mit einem Fehler. Ein Befehl `lastwerk plan activate <id>` gehört zu T09 oder einem eigenen Task.
- Argumente, die schon beim Einlesen scheitern (falsches Datum, Zahl als Text, unbekanntes Feld), beantwortet `rmcp` 3.5.1 selbst mit einem Werkzeugfehler (`isError`). Sie erreichen die Werkzeugmethode nicht und stehen deshalb nicht im Aufrufprotokoll (sie lesen nichts aus der Datenbank). Ein unbekanntes Werkzeug ist ein Protokollfehler (-32602). Wertegrenzen (Bereiche, Regelcode) prüft `tools.rs`, solche Aufrufe stehen mit Status `invalid_params` im Protokoll.
- Der MCP-Inspector (`@modelcontextprotocol/inspector` 2.10.1) lief einmal von Hand im CLI-Modus gegen den Server und listete die Werkzeuge. Er ist nicht Teil der CI.
- Die Frage "Wie ist meine Form diese Woche?" in einem echten Client (manueller Schritt 2) wurde nicht ausgeführt: In dieser Umgebung gibt es keinen Client und keine echten Daten.
