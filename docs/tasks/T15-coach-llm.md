# T15: Lokales Sprachmodell (lastwerk-coach)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M5 |
| Voraussetzung | T07, T08 |
| cloudfähig | ja (Live-Lauf manuell) |
| Bereich | `crates/coach`, `schema/plan-edit.schema.json`, `crates/cli` |
| SPEC | 9.1 bis 9.5, 5.4 (lastwerk-coach), 7.2, 11.4 (LLM), 12 (M5), 14 (O13) |

## Ziel

Ein lokales Modell darf einen Regelplan anpassen und erklären. Jede Ausgabe wird geprüft. Bei jedem Fehler bleibt der Regelplan.

## Umfang

- `LlmClient`, `Provider` (Ollama, OpenAI-kompatibler lokaler Server), `RefineConfig`, `RefineOutcome`, `refine_plan` nach SPEC 5.4 und 9.4. Höchstens zwei Korrekturaufrufe, dann Fallback mit Grund.
- `base_url` nur Loopback oder ein vom Nutzer bestätigter Host im lokalen Netz. Kein Feld für API-Schlüssel, kein öffentlicher Standardhost.
- `schema/plan-edit.schema.json` nach SPEC 7.2, Länge der Erklärung begrenzt.
- Prompt-Eingang nach SPEC 9.3: nur aggregierte Werte, kein Freitext aus Dateien oder Nutzereingaben. CLI-Befehl `refine` zeigt den exakten Eingang an und verlangt Bestätigung (`--yes` für Skripte).
- HTTP-Client: eine kleine, blockierende Bibliothek ohne Laufzeit-Zwang (Name, Version, Lizenz vermerken). Zeitlimit je Aufruf.
- Attrappen-Client für Tests: gültig, ungültiges JSON, unbekannte Vorlage, Regelverletzung, Injektion im Erklärtext.
- Live-Test mit `#[ignore]` gegen Ollama.

## Nicht im Umfang

Cloud-Anbieter jeder Art. Streaming. Anzeige in der Oberfläche über Text hinaus (die Erklärung wird als Text gerendert, nie als HTML, SPEC 9.5).

## Vorgaben

- Das Modell rechnet nicht: Lasten, CTL, ATL, TSB berechnet `lastwerk-core` nach der Anwendung der Änderungen neu (SPEC 9.1, 9.4).
- Test: In keinem Prompt steht eine Marker-Zeichenkette aus den Fixtures (Namen, Notizen, Pfade).
- Der Client ruft nur die konfigurierte Adresse auf (Test mit Attrappen-Server auf Loopback).

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p lastwerk-coach
cargo test --workspace
```

Erfolg (SPEC 12, M5, offline): Alle Attrappen-Fälle grün einschließlich "höchstens zwei Korrekturen", der Prompt-Eingang enthält keinen Freitext aus Fixtures, die Erklärung wird nie als HTML ausgegeben.

## Manuelle Schritte (nur lokal, Nutzer)

```
cargo test -p lastwerk-coach -- --ignored
cargo run -p lastwerk-cli -- refine target/plan.json --provider ollama --model <name> --db target/m1.db
```

Ollama-Dienst starten, Modell ziehen. 20 Läufe mit festem Eingang: kein Lauf liefert einen Plan, der `validate` verletzt, Fehlschläge enden im Fallback und werden mit Grund gezählt. Antwortzeit auf dem Zielrechner festhalten (offen O13).

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026 in einer Cloud-Umgebung (Linux), vollständig offline mit synthetischen Daten und Attrappen-Servern auf Loopback. **Der Live-Lauf gegen ein echtes Modell und die Messung der Antwortzeit (O13) stehen aus** und sind die manuellen Schritte oben; `crates/coach/tests/live.rs` (`#[ignore]`) ist dafür vorbereitet.

### Abhängigkeiten

Neu in `lastwerk-coach` (alle Lizenzen laut `cargo metadata` und `cargo deny check licenses` geprüft, `deny.toml` bleibt unverändert):

| Name | Version | Lizenz | Grund |
|---|---|---|---|
| ureq | 3.4.2 | MIT OR Apache-2.0 | Blockierender HTTP-Client ohne Laufzeit-Zwang (SPEC 5.3), `default-features = false`: kein TLS, keine Kompression, JSON macht `serde_json` selbst |
| ureq-proto | 0.6.4 | MIT OR Apache-2.0 | Protokollschicht von `ureq` |
| http | 1.5.0 | MIT OR Apache-2.0 | Typen von `ureq`; `coach` nutzt `http::Uri` für die Prüfung der Adresse (statt eines eigenen Parsers) |
| httparse | 1.10.1 | MIT OR Apache-2.0 | Kette von `ureq-proto` |
| base64 | 0.23.1 | MIT OR Apache-2.0 | Kette von `ureq` |
| utf8-zero | 0.8.1 | MIT OR Apache-2.0 | Kette von `ureq` |

Schon im Workspace und jetzt auch in `coach`: `jsonschema` 0.58.6 (ohne Standardfeatures), `serde`, `serde_json`, `chrono`, `thiserror`. `lastwerk-cli` hängt neu von `lastwerk-coach` ab. Die Desktop-App bindet `lastwerk-cli` ein, deshalb hat `app/src-tauri/Cargo.lock` dieselben vier neuen Pakete (`lastwerk-coach`, `ureq`, `ureq-proto`, `utf8-zero`; `http`, `httparse` und `base64` standen dort schon). TLS ist ausgelassen: `https` ist `ProviderError::TlsNotSupported`, begründet in SPEC 5.4 (coach).

### Abnahme (13. Oktober 2026)

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p lastwerk-coach` (50 Tests, 1 ignoriert), `cargo test -p lastwerk-cli --test refine` (15 Tests), `cargo test --workspace`, `sh scripts/check-i18n.sh`, `sh scripts/check-privacy.sh`, `sh scripts/check-isolation.sh` und `cargo deny --offline check licenses bans sources` bestehen.

### Festlegungen und Abweichungen (alle in SPEC 5.3, 5.4, 7.2, 9.5, 11.4, 12 M5 und 14 O13 nachgetragen)

- **Adresse.** `BaseUrl` statt `Url`, nur `http`, Loopback oder mit `--allow-lan` private Adressen, Hostnamen außer `localhost` abgelehnt, keine Zugangsdaten in der Adresse. Öffentliche Hosts auch mit `--allow-lan` abgelehnt (getestet mit `api.openai.com`, `api.anthropic.com`, öffentlicher IP).
- **Fallback trägt `attempts`.** Zusätzlich zu `reason`; `FallbackReason` hat `BaseInvalid`, `Timeout`, `Transport` und `InvalidOutput`.
- **Zu lange Erklärung** wird gekürzt statt als Schemafehler zurückgewiesen (Auftrag: "auf Schemalänge begrenzt").
- **Kein Proxy, keine Weiterleitung.** `ureq` liest sonst Proxy-Variablen aus der Umgebung; für lokale Aufrufe ist das falsch. Beides ist abgeschaltet und getestet (Weiterleitung) beziehungsweise festgelegt (Proxy).
- **Markertest in der CLI** (`crates/cli/tests/refine.rs`), nicht in `coach`: nur dort ist die Datenbank greifbar. `coach` prüft den Eingang zusätzlich mit einer Liste erlaubter Schlüssel und Texte (`crates/coach/tests/input.rs`).
- **`crates/core` blieb unverändert.** `LoadModel::session_load` ist öffentlich und reicht für die Neuberechnung der Lasten.
- **Plan-ID.** `refine_plan` kennt weder Zufall noch Uhr; die CLI vergibt `plan_id` (UUID v4) und `created_at` des neuen Plans.

### Hinweise für spätere Tasks

- Die Oberfläche (F24) braucht für den Aufruf nur `lastwerk_coach::{Provider, RefineInput, refine_plan, RefineConfig}` und `lastwerk_cli::refine`. Der Eingang (`RefineInput::build(...).to_json()`) und die Systemanweisung (`system_prompt(locale)`) sind genau der Text, den die Oberfläche vor dem Senden anzeigen muss. Die Erklärung gehört als Text gerendert (SPEC 9.5).
- Der Live-Lauf für O13: 20 Läufe mit festem Eingang, Fehlschläge und Gründe zählen (`FallbackReason::code()`), Zeit je Lauf festhalten. Ein Skript dafür gibt es nicht; `lastwerk refine ... --yes` in einer Schleife genügt, mit einem Plan, den `lastwerk plan` aus der echten Datenbank erzeugt hat.
