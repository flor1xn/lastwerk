# T14: Normalisierung, Erholungsdaten und Planregel R12

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M4d |
| Voraussetzung | T04, T07, T11 (L1 empfohlen) |
| cloudfähig | ja |
| Bereich | `bridge/`, `schema/`, `crates/store`, `crates/cli`, `crates/core` |
| SPEC | 5.4 (Austausch), 7.3, 8.4 (R12), 12 (M4d), 14 (O9) |

## Ziel

Aus dem Archiv entstehen `exchange/out`-Dateien (FIT, `recovery-day`-JSON). Lastwerk importiert sie, zeigt sie an und nutzt sie in der Planregel R12.

## Umfang

- `bridge`: `normalize --root <archiv> --out <exchange/out>`. Liest `sensitive/` nicht, solange keine Kategorie freigegeben ist. Wiederholbar (gleiche Dateien, gleiche Prüfsummen), ohne Garmin-Aufruf.
- `schema/recovery-day.schema.json` nach SPEC 7.3. Liegt `docs/garmin-feldabdeckung.md` aus L1 vor, gelten dessen Felder (O9). Sonst gilt der Entwurf aus SPEC 7.3 und der Task vermerkt das als vorläufig.
- Lastwerk: Migration für `recovery_day` (SPEC 6.2), `lastwerk import-recovery --dir --db`. Dateien aus `exchange/out` sind nicht vertrauenswürdig: Größenlimit, Schema-Prüfung, keine Pfade aus dem Inhalt, ungültige werden abgelehnt und gezählt.
- Planregel R12 (SPEC 8.4) in `lastwerk-core`: bei HRV deutlich unter Basislinie oder Schlaf unter Basislinie die nächste harte Einheit durch eine lockere ersetzen. Schwellen sind Faustregel und Parameter in `rules.default.json`.

## Nicht im Umfang

Anzeige in der Oberfläche über einen einfachen Verlauf hinaus. Freigabe der Erholungswerte für MCP (T10, O19).

## Vorgaben

- Lastwerk startet das Zusatzprogramm nie und lädt keinen Code daraus (SPEC 5.4).
- Ein Test legt Markerwerte in `sensitive/` und findet sie nicht in der Ausgabe.

## Abnahme

```
cd bridge
uv run pytest -q
uv run pytest -q tests/test_normalize.py
uv run ruff check src tests
uv run ruff format --check src tests
cd ..
cargo run -p lastwerk-cli -- import-recovery --dir target/exchange/out/recovery --db target/m4.db
cargo test -p lastwerk-core plan::recovery
cargo test -p lastwerk-cli --test recovery
cargo test --workspace
sh scripts/check-isolation.sh
sh scripts/check-privacy.sh
sh scripts/check-i18n.sh
```

Abweichung von der ursprünglichen Fassung (wie bei T13 für M4c, in SPEC 12 M4d vermerkt): Der Befehl `normalize --root tests/fixtures/archive --out ../target/exchange/out` entfällt, weil ein Fixture-Archiv `*.fit` enthielte, die nicht im Index stehen dürfen. Stattdessen baut `bridge/tests/test_normalize.py` das Archiv zur Laufzeit (`backfill` gegen die Attrappe des Kontos, `tests/synthetic.py`) und führt `normalize` als Unterprozess aus. Ein Test schreibt dabei mit `--out ../target/exchange/out` nach `target/exchange/out`, damit `import-recovery --dir target/exchange/out/recovery --db target/m4.db` aus dem Wurzelordner wörtlich läuft. Für ungültige Dateien erzeugt `crates/cli/tests/recovery.rs` ein gemischtes Verzeichnis unter `target/` und ruft die CLI auf.

Erfolg (SPEC 12, M4d): alle erzeugten `recovery-day`-Dateien validieren, ungültige werden abgelehnt und gezählt, `normalize` liest `sensitive/` nicht, ist wiederholbar und prüfsummengleich, R12 ersetzt in den Testfällen die nächste harte Einheit und sonst nicht.

## Manuelle Schritte

Mit dem echten Archiv `normalize` und `import-recovery` ausführen (lokal).

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026 in einer Cloud-Session (Linux), vollständig offline mit synthetischen Daten und gesperrtem Socket.

### Abhängigkeiten

Keine neuen. Python: nur Standardbibliothek und die Manifest-Prüfung aus T11 (`manifest.validate_value`), die auch das neue Schema versteht (`const`, `type`, `format: date`, `format: date-time`, `minimum`, `maximum`, `maxLength`); `jsonschema` kommt nicht als Dev-Abhängigkeit hinzu und ist nicht in `bridge/uv.lock`. Rust: `jsonschema` 0.58.6 (MIT, ohne Standardfeatures) und `serde_json` in `lastwerk-cli` waren seit T08 vorhanden, `lastwerk-testkit` und `chrono` in den Tests der CLI ebenso. `Cargo.lock` und `uv.lock` ändern sich nicht.

### Vorläufig bis L1 (O9)

`docs/garmin-feldabdeckung.md` aus L1 liegt nicht vor. Es gilt der Entwurf aus SPEC 7.3, `schema/recovery-day.schema.json` ist exakt dieser Entwurf. Die Zuordnung der Felder der Bibliothek zu den Schema-Feldern (`FIELD_MAP` in `bridge/src/lastwerk_bridge/normalize.py`) ist eine **Annahme** aus der bekannten Ausgabe von `garminconnect` 0.3.17: `hrvSummary.lastNightAvg`, `allMetrics.metricsMap.WELLNESS_RESTING_HEART_RATE[0].value` (Ersatz: `restingHeartRate` aus `heart_rates`), `dailySleepDTO.sleepTimeSeconds` und `dailySleepDTO.sleepScores.overall.value`. Nach der Sonde in L1 sind Namen, Einheiten und die Frage zu prüfen, ob die Bereichsendpunkte (`sleep_daily`, `rhr_daily`, `hrv_range`) die Tagesaufrufe ersetzen können. Die Tests zeigen, dass die Zuordnung austauschbar ist (`extract_fields` mit eigener Tabelle) und nie auf `sensitive/` zeigen darf (`check_field_map`).

### Festlegungen und Abweichungen (in SPEC 5.4, 5.5, 6.2, 7.3, 8.4, 12 M4d und 14 O9 nachgetragen)

- **Schema und Datei.** Pro Tag eine Datei `YYYY-MM-DD.json`, Werte außer Kopf optional, ein Tag ohne verwertbaren Wert bekommt keine Datei. Ein Wert außerhalb der Bereiche des Schemas wird weggelassen und im Bericht gezählt, nie gekürzt. `fetched_at` ist der jüngste Abruf der benutzten Quellen laut Manifest.
- **`normalize`.** Liest nur Manifest-Einträge der Endpunkte `original_fit`, `hrv`, `rhr`, `heart_rates`, `sleep` mit Status `ok` und Pfad unter `activities/` oder `daily/`. `sensitive/` wird nie geöffnet, auch nicht über Pfade im Manifest (`..`, absolut, Symlinks aus dem Archiv heraus oder nach `sensitive/` werden abgewiesen). `--allow-sensitive <kategorie>` ist umgesetzt (wiederholbar, Kategorien aus dem Register), verarbeitet in M4d aber nichts und wird im Bericht vermerkt. Der Bericht `normalize-report.json` im Archiv ist abgeleitet (`DERIVED_FILES`). Exit-Codes: 0 ok, 1 Quellen fehlen oder sind beschädigt (Lauf läuft zu Ende), 2 Archiv fehlt, `--root` und `--out` überlappen oder unbekannte Kategorie.
- **`import-recovery`.** Siehe SPEC 5.4 (cli): 64 KiB je Datei, Name und Feld `date` müssen übereinstimmen, Symlinks sind ungültig, Meldungen ohne Messwerte und ohne Ordnerpfad. Exit-Code 0 auch bei ungültigen Dateien (wie bei fehlerhaften FIT-Dateien), 2 bei fehlendem Ordner.
- **R12.** Parameter in `catalog/rules.default.json`: `hrv_drop_fraction` 0,15, `sleep_deficit_fraction` 0,20, `baseline_days` 7, `min_baseline_days` 4, `max_age_days` 3 (alle Faustregel, Quelle Q6 sekundär). `min_baseline_days` und `max_age_days` sind Ergänzungen zum Auftrag, ohne sie wäre ein Signal aus einer einzelnen oder einer Wochen alten Messung möglich. Die Version der Regeln bleibt 1.0.0, damit kein bestehender Plan sich ändert.
- **`PlanContext` bekommt ein Feld `recovery`.** Jede Stelle, die einen `PlanContext` als Struktur aufbaut, braucht `recovery: Vec::new()` oder `recovery: ...` (geändert: `crates/cli/src/planning.rs`, `crates/cli/src/testctx.rs`, `crates/store/tests/plans.rs`, `crates/core/src/plan/tests.rs`).
- **`plan` und `validate` lesen die Tage aus der Datenbank.** Das ist über den Auftragstext hinaus nötig, damit R12 mit echten Daten greift; ohne importierte Tage ändert sich nichts.
- **Backup.** `UserDataExport` hat `recovery_days`, `backup_version` bleibt 1; eine Sicherung ohne den Schlüssel leert die Tabelle beim Einspielen (SPEC 5.4, store).
- **Attrappen-FIT.** Die Aktivitäts-Attrappen des Kontos sind keine gültigen FIT-Dateien; der Rust-`import` meldet sie als `invalid_header`. Das ist beabsichtigt (nur Bytes werden geprüft, nicht die Aktivität).

### Hinweise für spätere Tasks

- T10 (MCP): `Store::recovery_days(from, to)` liest die Tabelle `recovery_day` (`RecoveryDayRecord`, mit `From<&RecoveryDayRecord> for RecoveryDay`). Die Freigabe (O19) liegt beim MCP-Server, `get_recovery_days` höchstens 90 Tage. `UserDataExport` hat ein neues Feld; wer es als Struktur baut, setzt `recovery_days`.
- T16: `exchange/in/plan.json` ist unverändert; ein Plan nach R12 trägt in `warnings` den Code `R12` mit Stufe `info` und `reason.session.recovery_swap` an der ersetzten Einheit. Die Oberfläche (T09) schlägt `rule.R12.*` und `reason.session.recovery_swap` nach.
- L1: `FIELD_MAP` gegen `docs/garmin-feldabdeckung.md` prüfen, danach SPEC 7.3 und O9 auf endgültig setzen.
