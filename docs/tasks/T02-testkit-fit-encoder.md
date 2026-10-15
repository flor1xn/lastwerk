# T02: FIT-Encoder und synthetische Fixtures (lastwerk-testkit)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M1 |
| Voraussetzung | T00 |
| cloudfähig | ja |
| Bereich | `crates/testkit` |
| SPEC | 11.3, 6.4, 6.5 |

## Ziel

Ein Encoder für FIT-Dateien und Generatoren, die deterministisch kleine, synthetische Dateien erzeugen. Damit sind T03 und T04 ohne echte Daten testbar.

## Warum

Echte FIT-Dateien dürfen nie ins Repository (SPEC 11.1). Der Parser und der Import brauchen aber Eingaben mit bekanntem Inhalt.

## Umfang

- FIT-Encoder: Header (14 Byte), Definitionsnachrichten, Datennachrichten, CRC. Nachrichten mindestens `file_id`, `device_info`, `record`, `lap`, `session`, `activity`.
- Generatoren (SPEC 11.3): konstante Leistung, Intervallprofile, Rampen, Herzfrequenz mit Drift, Lücken, Spitzen und Aussetzer, zwei Dateien derselben Fahrt (Edge und Uhr) mit leichtem Zeitversatz, Dateien ohne Leistung, ohne Herzfrequenz, ohne `session`, beschädigte Dateien (falsche Prüfsumme, abgeschnitten), eine Monitoring-Datei (keine Aktivität), eine leere Datei.
- Bibliotheks-API, die alle Fixtures im Speicher liefert (Name, Bytes, Erwartung). Tests in T03 und T04 nutzen sie direkt.
- Binary `gen-fixtures --out <ordner>`, das dieselben Fixtures als `*.fit` samt `manifest.json` in einen Ordner schreibt (Standard `target/fixtures`). Ausgabe ist deterministisch (zweimal erzeugen ergibt identische Bytes).
- `manifest.json` entsteht aus derselben Quelle: je Datei erwartete Eigenschaften (Art, erwartete Anzahl Aktivitäten, erwarteter Fehlercode, Paarzugehörigkeit).
- Die Fixtures werden **nicht eingecheckt**. Sie entstehen bei Bedarf. So bleibt die Regel "kein `*.fit` im Repository" ohne Ausnahme gültig (SPEC 1.4, 11.4).

## Nicht im Umfang

Dekodieren von FIT-Dateien für den Produktivbetrieb (T03). Für die Tests des Encoders genügt ein minimaler Rücklese-Helfer im Testcode.

## Vorgaben

- Das Format folgt der öffentlich dokumentierten FIT-Protokollbeschreibung. Garmin-Profildateien (`Profile.xlsx`, SDK-Quellen) kommen nicht ins Repository (offen O2, SPEC 14).
- Generatoren sind reine Funktionen von Parametern, ohne Systemzeit und ohne Zufall (feste Seeds).
- SPEC 11.3 spricht von eingecheckten Fixtures unter `tests/fixtures/`, SPEC 12 (M1) von `crates/testkit/fixtures`. Das widerspricht der Regel "kein `*.fit` im Index". Entscheidung dieses Tasks: erzeugen statt einchecken, Ordner `target/fixtures`. Trage die Abweichung in SPEC 11.3 und in den M1-Befehlen von SPEC 12 nach.

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p lastwerk-testkit
cargo run -p lastwerk-testkit --bin gen-fixtures -- --out target/fixtures
cargo run -p lastwerk-testkit --bin gen-fixtures -- --out target/fixtures2
sh scripts/check-privacy.sh
```

Erfolg: Tests grün, beide erzeugten Ordner sind byteidentisch (Test oder Vergleich mit `cmp`), Rücklesetest bestätigt Prüfsumme und Nachrichtenanzahl je Generator, `git ls-files "*.fit"` bleibt leer.

## Manuelle Schritte

Keine.

## Blocker oder Vermerke

Keine Blocker. Stand 13. Oktober 2026:

- Neue Abhängigkeiten in `crates/testkit`: `serde` 1.0.229 (MIT OR Apache-2.0, Erwartungen der Fixtures als Datenstruktur) und `serde_json` 1.0.154 (MIT OR Apache-2.0, `manifest.json`). Keine weiteren, die Argumentauswertung des Binarys nutzt `std::env::args`.
- `crates/testkit` hängt nicht von `lastwerk-core` oder `lastwerk-fit` ab. `lastwerk-fit` bindet das Testkit in T03 als Entwicklungsabhängigkeit ein.
- Profilkonstanten (Nachrichten- und Feldnummern, Enum-Werte, Basistypen) stehen als eigene Definitionen in `crates/testkit/src/profile.rs` nach öffentlicher FIT-Protokoll- und Profilbeschreibung. Keine Profildateien oder SDK-Quellen im Repository (SPEC 14, O2). Produktnummern der Geräte (4242, 4343) sind frei erfunden.
- Die Abweichung "erzeugen statt einchecken, Ordner `target/fixtures`" stand bereits in SPEC 11.3 und in den M1-Befehlen von SPEC 12, dort war nichts nachzutragen.
- Fixture `no_session`: Erwartung ist eine lesbare Aktivität (`activities = 1`, `error = null`, `has_session = false`), weil `FitError` in SPEC 5.4 keinen Fall "ohne session" kennt. Lehnt der Parser in T03 solche Dateien begründet ab, passt T03 die Erwartung in `fixtures.rs` an und vermerkt es.
- Fixtures (19): `power_constant_200w_3600s` (T-NP-1), `power_intervals_100w_300w_30s` (T-NP-2), `power_ramp_100w_300w`, `hr_drift_130_160`, `gaps_short` (5 s und 10 s), `gaps_long` (60 s), `spikes_dropouts` (3000 W, HF 0 und 250), `pair_edge` und `pair_watch` (Paar `ride_pair`, 20 s Versatz, 3600 s gegen 3570 s, Uhr ohne Leistung), `no_power`, `no_heart_rate`, `no_session`, `run_speed`, `swim_pool_25m`, `strength`, `corrupt_crc`, `corrupt_truncated`, `monitoring`, `empty`.
