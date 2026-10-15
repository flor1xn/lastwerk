# T08: Schema, Validierung, ICS-Export und Backup

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M3 |
| Voraussetzung | T07, T04 |
| cloudfähig | ja |
| Bereich | `schema/`, `crates/store`, `crates/cli` |
| SPEC | 7.1, 4.1 (F10 bis F13), 10.2 (ICS), 11.4 (Schema, Golden), 12 (M3) |

## Ziel

Plan als JSON nach Schema, `validate`, `export-ics`, `backup` und Wiederherstellung, alles über die CLI. Version 1 ist damit ohne Oberfläche für den Plan vollständig.

## Umfang

- `schema/plan.schema.json` (JSON Schema 2020-12) nach SPEC 7.1. Negativtests je Pflichtfeld.
- CLI: `plan`, `validate`, `export-ics`, `backup` (und `restore`) nach SPEC 5.4. `plan --out` schreibt `plan.json`, `validate` liefert Exit-Code 1 bei harter Verletzung.
- ICS-Export: eine Veranstaltung je Einheit, nur Titel, Zeit und Dauer, keine Orte, keine Schwellenwerte (SPEC 10.2). Titel aus den Sprachdateien.
- `save_plan`, `export_user_data`, `import_user_data` (Schwellenwerte, Ziele, Verfügbarkeit, Pläne, Einstellungen) mit Versionsfeld im Backup.
- Wiederherstellung: Datenbank löschen, `backup.json` einspielen, `rebuild` ergibt dieselben Schwellenwerte, Ziele und Pläne (SPEC 6.3).
- Golden-Tests (`insta`) für Plan-JSON, ICS und PMC-CSV.

## Nicht im Umfang

Oberfläche (T09), Schema für Modell-Änderungen (T15), Schema für Erholungstage (T14).

## Vorgaben

- Schema-Prüfung mit der Bibliothek `jsonschema` (Dev-Abhängigkeit oder Laufzeit, Version und Lizenz vermerken). `plan.ics` wird im Test mit der Bibliothek `icalendar` eingelesen (Dev-Abhängigkeit).
- Jeder erzeugte Plan besteht das Schema (Test über die Property-Eingaben aus T07).

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p lastwerk-testkit --bin gen-fixtures -- --out target/fixtures
cargo run -p lastwerk-cli -- import --dir target/fixtures --db target/m1.db
cargo run -p lastwerk-cli -- plan --db target/m1.db --week-start 2027-01-04 --goal-date 2027-05-01 --out target/plan.json
cargo run -p lastwerk-cli -- validate target/plan.json --db target/m1.db
cargo run -p lastwerk-cli -- export-ics target/plan.json --out target/plan.ics
cargo run -p lastwerk-cli -- backup --db target/m1.db --out target/backup.json
cargo test --workspace
```

Erfolg (SPEC 12, M3): `validate` endet mit Exit-Code 0 und meldet keine harte Verletzung, `plan.json` besteht das Schema, jede Pflichtfeld-Verletzung im Negativtest wird erkannt, `plan.ics` lässt sich einlesen und enthält je Einheit eine Veranstaltung ohne Ortsangaben, Wiederherstellungstest grün.

## Manuelle Schritte

Keine.

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026.

Neue Abhängigkeiten (alle zu `deny.toml` passend, `cargo-deny` selbst lief in dieser Umgebung nicht, die Lizenzen der aufgelösten Pakete wurden über `cargo metadata` geprüft):

| Crate | Name | Version | Lizenz | Grund |
|---|---|---|---|---|
| cli | jsonschema, ohne Standardfeatures | 0.58.6 | MIT | Schema-Prüfung (SPEC 5.3), als Laufzeitabhängigkeit, damit `validate` und `plan` das Schema prüfen. Ohne die Standardfeatures `resolve-http`, `resolve-file`, `tls-aws-lc-rs` und `idna` entfallen `reqwest`, `rustls` und jede Netzwerk- und Dateiauflösung von Verweisen. Zieht `referencing`, `jsonschema-regex`, `jsonschema-value` (alle MIT), `fancy-regex` (MIT), `regex` (MIT OR Apache-2.0), `fluent-uri` (MIT), `email_address` (MIT), `uuid-simd` (MIT), `num-*` (MIT OR Apache-2.0), `ahash` und `hashbrown` (MIT OR Apache-2.0), `foldhash` (Zlib) und `borrow-or-share` (MIT-0) |
| cli | icalendar, ohne Standardfeatures | 0.17.14 | MIT/Apache-2.0 | ICS-Erzeugung. Mit dem Feature `parser` (zieht `nom` und `nom-language`, beide MIT) nur als Entwicklungsabhängigkeit für den Einlesetest |
| cli | uuid, nur Feature `v4` | 1.28.0 | Apache-2.0 OR MIT | Plan-ID. `core` darf keinen Zufall erzeugen, deshalb nur in der CLI. Nutzt `getrandom` (MIT OR Apache-2.0). Im Baum stehen jetzt zwei Versionen (0.3.4 und 0.4.3, `multiple-versions` steht auf `warn`) |
| cli (dev) | proptest, ohne Standardfeatures, Feature `std` | 1.12.0 | MIT OR Apache-2.0 | Eigenschaftstest der Schemaprüfung, schon im Baum durch `core` |
| cli | serde_json | 1.0.154 | MIT OR Apache-2.0 | JSON der Pläne (schon im Workspace) |

`deny.toml`: Die Lizenz `MIT-0` kam neu hinzu, weil `borrow-or-share` (über `fluent-uri` und `jsonschema`) sie trägt. MIT-0 ist MIT ohne die Pflicht zur Namensnennung und damit mindestens so freizügig wie MIT. Alle anderen Lizenzen standen schon auf der Liste (`aho-corasick` unter `Unlicense OR MIT` wird über MIT erlaubt).

Änderungen außerhalb von `schema/`, `crates/store` und `crates/cli`, klein und begründet:

- `catalog/locales/de.json` und `en.json`: neuer Abschnitt `sport` mit den fünf Sportartnamen (Titel im ICS-Export). Beide Dateien haben weiter dieselben Schlüssel (`sh scripts/check-i18n.sh`).
- `deny.toml`: `MIT-0`, siehe oben.
- `crates/core` blieb unverändert.

Entscheidungen und Abweichungen von der SPEC (jeweils im betroffenen Abschnitt vermerkt):

- 5.4 store: `save_plan(&Plan, PlanStatus)` statt `save_plan(&Plan)`, dazu `save_plan_for_goal`, `list_plans`, `load_plan`, `active_plan`, `set_plan_status`. Höchstens ein aktiver Plan (Konvention). `set_goal`, `list_goals`, `active_goal` (höchstens ein aktives Ziel), `set_availability`, `list_availability`, `week_history`, `pmc_state_at`. `UserDataExport.format_version` heißt `backup_version` (Alias beim Lesen) und trägt `exported_at`.
- 5.4 cli: zusätzlich `restore`, `goal set|list`, `availability set|list` und die Optionen `--goal-sport`, `--weeks`, `--locale`, `--sports`, `--strength` bei `plan`.
- 10.2: Die Beschreibung der Veranstaltungen enthält die Vorlagenbeschreibung und die Texte der Begründungsschlüssel (feste Texte aus den Sprachdateien). Startzeiten am 07:00 und pm 17:00 als floating time. Kein Ort, keine Koordinate, keine Zeitzone, keine Zahlen aus dem Plan.
- Schema, Regeln, Katalog und Sprachdateien sind per `include_str!` in `lastwerk-cli` eingebettet (Konvention wie beim Zonenkatalog in T05).
- Die Zeitkonstanten und Startwerte des PMC liegen noch nicht als Einstellung vor (kommt mit T06). `plan` und `validate` rechnen mit den Standardwerten 42 und 7 Tage und Start 0. Seit dem 14. Oktober 2026 lesen sie die Einstellungen `pmc.*` (SPEC 5.4).
- Die CLI-Texte stehen weiter als Zeichenketten im Code, nicht in `catalog/locales` (siehe T04).

Hinweise für spätere Tasks:

- T09 (Planansicht): `Store::list_plans`, `load_plan`, `active_plan`, `set_plan_status` und `save_plan_for_goal` sind die Zugriffe auf Pläne. Die Ansicht schlägt Texte unter `template.<id>.name` und `.description`, `reason.week.*`, `reason.session.*`, `rule.R**.name` und `.explanation` und `sport.*` nach. Die Schemaprüfung (`schema.rs`), die Kontextbildung (`planning::generate`, `validation_context`) und der ICS-Export (`ics.rs`) liegen im Binary-Crate `lastwerk-cli`. Braucht die App sie (Plan erzeugen, prüfen, exportieren), verschiebt T09 diese Module in ein Bibliotheks-Crate und teilt sie mit der CLI. Das Frontend prüft das Schema nicht selbst und macht keine Netzwerkzugriffe.
- T10 (MCP): `get_current_plan` liest `Store::active_plan` (Status `active`, `draft` und `archived` stehen in `list_plans`), `get_plan_week` ein Element von `plan.weeks`. `lastwerk plan` speichert Pläne als `draft`, aktiv wird ein Plan über `Store::set_plan_status`. Es gibt noch keinen CLI-Befehl dafür; wer ihn braucht, ergänzt `lastwerk plan activate <id>`.
- T15 (LLM): Der Roundtrip eines Plans ist `serde_json::from_str::<Plan>` und `plan_json` (Schemaprüfung vor dem Schreiben). Ein veränderter Plan trägt `source = llm`, wird mit `schema::plan_errors` und `validate_plan` gegen den Kontext aus `planning::validation_context` geprüft und mit `save_plan_for_goal` als `draft` abgelegt.
- T16: Der Plan nach `exchange/in/plan.json` ist die Datei, die `lastwerk plan --out` schreibt, mit `schema_version` 1.
