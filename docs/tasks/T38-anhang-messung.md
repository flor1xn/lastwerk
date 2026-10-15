# T38 Anhang: Effizienz messen und gezielt optimieren (Teil C2)

Zwei Messungen: Die erste (Abschnitt "Ergebnisse vorher und nachher") stammt vom Stand vor T33 und T37; die zweite am Ende ("Nachmessung auf dem zusammengeführten Stand") gilt für `main` mit Leistungskurve und Bestzeiten und ist die maßgebliche.

Stand 15. Oktober 2026. Alle Daten sind synthetisch (crates/testkit), nichts davon liegt im Repository.

## Messaufbau

- Rechner: Cloud-Container, Intel Xeon @ 2,10 GHz, `nproc` = 4, 15 GB RAM, Linux 6.18. Die Maschine ist geteilt, die Last lag bei den Endmessungen bei 1,1 bis 1,2 (`uptime`). Rauschen von etwa 10 Prozent ist üblich, auffällige Messungen wurden wiederholt, kleine Vergleiche zusätzlich an einen Kern gebunden (`taskset -c 3`).
- Build: `cargo build --release --locked`, Rust nach `rust-toolchain.toml`.
- Daten: `gen-bench` (crates/testkit/src/bin/gen-bench.rs, Modul `lastwerk_testkit::bench`), fester Startwert `DEFAULT_SEED = 0x5EED_0038`, linearer Kongruenzgenerator, keine Zufallsquelle.

| Satz | Inhalt | Dateien | Messpunkte | Byte |
|---|---|---|---|---|
| long10 | Radfahrten zu 5 h, 1 Hz, Leistung, Herzfrequenz, Kadenz, Geschwindigkeit | 10 | 180 000 | 2,7 MB |
| long50 | wie long10 | 50 | 900 000 | 13,5 MB |
| hist1 | 1 Jahr, Rad, Lauf, Schwimmen, Kraft, meist um 1 h | 300 | 1 110 546 | 15,4 MB |
| hist5 | 5 Jahre, wie hist1 | 1500 | 5 600 377 | 77,0 MB |

Befehle zum Nachmessen (aus dem Wurzelverzeichnis, hyperfine im PATH):

```text
sh scripts/bench.sh                      # baut, erzeugt alle Sätze in mktemp -d, misst, räumt auf
BIN_DIR=<ordner> sh scripts/bench.sh     # misst fertige Binaries (lastwerk, gen-bench, bench_phases)
cargo run --release -p lastwerk-testkit --bin gen-bench -- --out DIR --set history --years 5 --count 1500
cargo run --release -p lastwerk-store --example bench_phases -- --set long --count 50 --only parse --runs 21
```

`scripts/bench.sh` misst mit `hyperfine --warmup 1 --runs 7` und gibt Median und Minimum aus: Erstimport (`lastwerk import`, frische Datenbank mit Schwellen für alle Sportarten), zweiter Import (alle Dateien unverändert), `pmc` und `plan` über hist5. Das Beispiel `bench_phases` (crates/store/examples) misst die Rechenphasen im Speicher mit `std::time::Instant`, 7 Läufe je Phase. Hotspots wurden mit `valgrind --tool=callgrind` gesucht (perf fehlt im Container), Teilzeiten des Imports mit vorübergehender `Instant`-Messung je Abschnitt (nicht eingecheckt).

## Ergebnisse vorher und nachher

Vorher: Stand d25debd (vor T38-C2). Nachher: 429f739. Beide mit denselben Daten und demselben Skript direkt hintereinander gemessen. Median, in Klammern Minimum, in ms.

### CLI (mit Datenbank)

| Messung | vorher | nachher | Änderung |
|---|---|---|---|
| import long10 | 61,2 (49,0) | 38,1 (35,2) | -38 % |
| import long50 | 288,5 (270,1) | 129,9 (107,5) | -55 % |
| import hist1 | 328,7 (296,6) | 158,3 (146,0) | -52 % |
| import hist5 | 1652,1 (1563,8) | 694,6 (662,8) | -58 % |
| reimport long10 | 7,5 (6,7) | 8,7 (6,7) | Rauschen, Minimum gleich |
| reimport long50 | 21,3 (17,0) | 18,6 (16,6) | gleich |
| reimport hist1 | 23,8 (20,3) | 22,9 (21,8) | gleich |
| reimport hist5 | 105,8 (95,6) | 106,1 (92,7) | gleich |
| pmc hist5 (5 Jahre) | 7,3 (6,5) | 7,0 (6,5) | gleich |
| plan hist5 (12 Wochen) | 10,7 (9,9) | 10,8 (9,5) | gleich |

Der Ausreißer beim Median von reimport long10 liegt im Rauschen (Minimum gleich, Messung wiederholt). Der zweite Import ändert keinen Code im Pfad (Lesen, SHA-256, eine Abfrage je Datei).

### Rechenphasen (bench_phases, ohne Datenbank)

| Phase | long10 vorher | nachher | long50 vorher | nachher | hist1 vorher | nachher | hist5 vorher | nachher |
|---|---|---|---|---|---|---|---|---|
| sha256 | 1,8 | 1,8 | 12,7 | 11,7 | 13,9 | 13,0 | 68,8 | 68,6 |
| parse (mit CRC) | 28,3 | 12,9 | 125,0 | 66,1 | 146,4 | 51,0 | 759,5 | 337,4 |
| plausibility | 3,8 | 3,2 | 25,1 | 19,3 | 19,0 | 10,5 | 97,6 | 57,1 |
| np | 2,2 | 1,5 | 13,6 | 8,8 | 13,8 | 7,3 | 71,9 | 57,5 |
| load | 2,0 | 1,5 | 13,1 | 8,0 | 14,2 | 9,6 | 76,6 | 59,5 |
| best_power (Leistungskurve) | 3,4 | 2,9 | 22,6 | 15,1 | 18,8 | 13,9 | 99,6 | 88,0 |
| zones (alle Schemata) | 6,1 | 5,2 | 41,2 | 24,4 | 35,8 | 27,3 | 237,9 | 133,8 |
| pmc (Kern) | 0,001 | 0,001 | 0,005 | 0,004 | 0,021 | 0,025 | 0,160 | 0,126 |

Mediane in ms. Die Phasen np, load, best_power und zones rufen im Beispiel weiter je Aufruf `check_plausibility` auf und profitieren nur von dessen schnellerer Fassung; der größere Gewinn im Import kommt davon, dass der Import die Prüfung nur noch einmal je Aktivität ausführt.

## Hotspots

Aufteilung des Erstimports hist5 vor den Änderungen (callgrind und Teilzeiten):

1. FIT-Prüfsumme (CRC-16) im Parser: Halbbyte-Verfahren mit zwei abhängigen Tabellenschritten je Byte. Einzelmessung über 77 MB: rund 400 ms Halbbyte, rund 210 ms Bytetabelle, rund 55 ms mit acht Tabellen. Damit gut ein Fünftel des Imports.
2. `check_plausibility` lief je Aktivität bis zu achtmal (Parser, Kennzahlen, Belastung, je Zonenschema, Bestleistung) und kopierte jedes Mal alle Samples in einen schrittweise wachsenden Vektor. Etwa 30 Prozent der Instruktionen des Imports.
3. Vorbereiten der SQL-Anweisungen: `Connection::execute` und `query_row` bereiten jede Anweisung neu vor, je Aktivität rund ein Dutzend (Zonenzeilen und Bestleistungen sogar je Zeile). Nach den anderen Änderungen etwa ein Drittel der verbleibenden Importzeit.
4. Kein quadratisches Verfahren gefunden: NP nutzt bereits ein gleitendes 30-s-Mittel in O(n), die Bestleistung (Leistungskurve über 5, 60, 300, 1200, 3600 s) Präfixsummen in O(n) je Dauer, die Duplikatregel ein BTreeMap-Fenster, PMC eine Schleife je Tag. Der Import läuft in einer Transaktion, `source_file.rel_path` ist UNIQUE (Index), die Kindtabellen haben `activity_id` vorn im Primärschlüssel.

## Durchgeführte Änderungen

| Commit | Änderung | Gleichheitstest |
|---|---|---|
| fb555f6 | Messdaten (`bench`, `gen-bench`, optionales Kadenzfeld im Generator, aus für alle vorhandenen Fixtures), `bench_phases`, `scripts/bench.sh` | Tests in `bench.rs`, Fixtures unverändert |
| 76a1c94 | `check_plausibility`: erst Grenzen der Abschnitte suchen, dann je Abschnitt genau eine Zuteilung | `check_plausibility_matches_single_pass_reference` (alte Fassung im Test, LCG-Reihen mit Lücken, unplausiblen und nicht endlichen Werten, Fehlerfällen; Vergleich über Debug-Ausgabe, also bitgleich) |
| 7e1645e, 0e0e1f5 | CRC-16 erst mit Bytetabelle, dann mit acht Tabellen (slicing by 8), Tabellen zur Übersetzungszeit | `byte_table_matches_nibble_method` (Halbbyte-Verfahren als Referenz, alle Einzelbytes, alle Längen 0 bis 64, versetzte Anfänge, 4096 LCG-Byte) |
| 805de9f | Einfügen von Zonen und Bestleistung je Aktivität mit einer vorbereiteten Anweisung (in 429f739 aufgegangen) | Import-Tests, Tabellenvergleich |
| 956879b | Import prüft die Samples einmal: `compute_load_prepared`, `zone_seconds_prepared`, `best_power_prepared` in core; die alten Funktionen prüfen und rufen diese auf. Bei gescheiterter Prüfung nimmt der Speicher die alten Funktionen, damit Status und Meldungen gleich bleiben | `prepared_variants_match_self_checking_functions` (alle Sportarten, alle Zonenbezüge mit und ohne Schwellen, Bestleistung) |
| 429f739 | `statements::Db`: kleiner Zwischenspeicher vorbereiteter Anweisungen je Lauf (Import, Neuberechnung), ohne neue Abhängigkeit | Tests in `statements.rs`, Import- und Neuberechnungstests |

Zusätzlich wurden nach 956879b und 429f739 alle vier Datensätze mit dem alten und dem neuen Binary importiert und die Tabellen `source_file`, `activity`, `activity_load`, `activity_zone_time` und `activity_best_power` ohne Laufzeitstempel verglichen (Werte als `repr`, also bitgleich): alle gleich, zum Beispiel hist5 mit 1500 Aktivitäten, 20 630 Zonenzeilen und 3391 Bestleistungen.

`crates/core` bleibt rein: keine neue Abhängigkeit, kein Dateisystem, keine Zeit, kein Zufall. Zeitmessung steht nur im Beispiel unter crates/store/examples.

## Bewusst nicht optimiert

- Parser je Nachricht (`read_data`, `Builder::accept`): Drei Versuche (ein Durchlauf statt Einzelsuche je Feld in `record`, Wegfall des Zwischenvektors in `to_one_hz`, reine Zählung statt voller Plausibilitätsprüfung im Parser) brachten an einen Kern gebunden keinen Unterschied über dem Rauschen. Zurückgenommen.
- SHA-256: rund 0,9 ns je Byte (Hardwarebefehle über das Crate `sha2`), 69 ms für 77 MB, unter 10 Prozent des Imports.
- `threshold_lookup` läuft je Aktivität zweimal (Belastung, Zonen). Seit dem Zwischenspeicher kostet das nur noch die Ausführung der Abfrage; ein Zwischenspeicher der Ergebnisse bräuchte Regeln zur Gültigkeit und lohnt die Komplexität nicht.
- PMC (0,16 ms für 5 Jahre im Kern, 7 ms als CLI samt Start und Datenbank) und Planer (rund 10 ms als CLI) sind von Prozessstart und Öffnen der Datenbank bestimmt.
- Zweiter Import unveränderter Dateien: Lesen und Prüfsumme dominieren, rund 0,07 ms je Datei.
- Leistungskurve: Es gibt nur die fünf festen Dauern, das Verfahren ist linear. Eine vollständige Mean-Max-Kurve über alle Dauern existiert im Produkt nicht.

## Abhängigkeiten

Keine neue Abhängigkeit, auch keine Dev-Abhängigkeit. Erwogen und verworfen: rusqlite-Feature `cache` (zieht `hashlink` 0.12.2 nach, MIT OR Apache-2.0); stattdessen eigener Zwischenspeicher `crates/store/src/statements.rs`. Lockfiles unverändert, das Tauri-Projekt ist nicht berührt.

## Bestätigung: Referenzwerte unverändert

Keine Referenzkonstante, kein Snapshot und kein Test wurde geändert oder gelöscht (`git diff d25debd -- '*.snap'` leer). Ausgeführt auf 429f739:

```text
cargo fmt --all --check                                        # ok
cargo clippy --workspace --all-targets --locked -- -D warnings # ok
cargo test --workspace --locked                                # 540 bestanden, 0 fehlgeschlagen
```

Darin unter anderem die handgerechneten Referenzwerte T-NP-1 bis T-NP-3, T-TSS, T-rTSS, T-sTSS, T-sRPE, hrTSS (crates/core/src/load.rs), Zonen und Bestleistung (zones.rs, best_power.rs), die Fixture-Tests des Parsers (crates/fit/tests) und die Snapshots der CLI (crates/cli/src/snapshots).

## Nachmessung auf dem zusammengeführten Stand

Die Optimierungen wurden auf `main` übernommen, das inzwischen T33 (Leistungskurve über 22 Dauern) und T37 (Bestzeiten im Laufen) enthält. Beide rechnen im Import zusätzlich und prüften die Samples je Aufruf erneut; `best_efforts_prepared` (core) und `write_best_effort` (store) nutzen deshalb jetzt dieselbe einmalige Prüfung. Vorher: `c63ac70` (Stand ohne die Änderungen dieses Anhangs), nachher: `HEAD` des Zweigs. Gleiche Daten (`gen-bench`, fester Startwert), Release-Builds, `scripts/bench.sh` mit `RUNS=9`, 4 Kerne. Der Vorher-Lauf begann bei Last 4,1 (kurz nach dem Bau), der Nachher-Lauf bei 2,3; deshalb gelten die Minima als belastbarer als die Mediane.

Erstimport über die Kommandozeile, Median (Minimum) in ms:

| Satz | vorher | nachher | Änderung (Minimum) |
|---|---|---|---|
| long10 (10 Fahrten zu 5 h, 1 Hz) | 66,0 (60,9) | 40,9 (35,4) | -42 % |
| long50 (50 Fahrten zu 5 h) | 336,2 (300,4) | 140,2 (135,3) | -55 % |
| hist1 (1 Jahr, 300 Einheiten) | 387,7 (323,5) | 182,5 (154,3) | -52 % |
| hist5 (5 Jahre, 1500 Einheiten, 5,6 Mio. Messpunkte) | 1896,8 (1558,2) | 794,6 (760,8) | -51 % |

Unveränderte Pfade, beide Binaries abwechselnd in einem hyperfine-Aufruf, 30 Läufe nach 3 Aufwärmläufen auf derselben Datenbank (hist5), Median (Minimum) in ms:

| Messung | vorher | nachher |
|---|---|---|
| zweiter Import (alle Dateien unverändert) | 100,0 (82,4) | 98,8 (82,4) |
| `pmc` über 5 Jahre | 6,7 (6,1) | 6,4 (5,3) |
| `plan` über 12 Wochen | 10,2 (9,1) | 11,2 (8,8) |

Keine Verschlechterung über dem Rauschen. Der Planer und `pmc` werden vom Prozessstart und vom Öffnen der Datenbank bestimmt.

Gleichheit auf dem zusammengeführten Stand: Dieselben Daten (hist5 und long10) wurden mit dem alten und dem neuen Binary in je eine frische Datenbank mit gleichen Schwellen importiert. Verglichen wurden alle Spalten ohne Zeitstempel als `repr`, also bitgleich:

| Tabelle | Zeilen hist5 | Ergebnis |
|---|---|---|
| `source_file` | 1500 | gleich |
| `activity` | 1500 | gleich |
| `activity_load` | 1500 | gleich |
| `activity_zone_time` | 20 630 | gleich |
| `activity_best_power` | 14 738 | gleich |
| `activity_best_effort` | 1183 | gleich |

Die Pläne beider Binaries unterscheiden sich nur in `plan_id` und `created_at`. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` und `cargo test --workspace --locked` (809 Tests, 0 fehlgeschlagen) laufen auf dem zusammengeführten Stand grün. Clippy 1.99 verlangte in der CRC-Schleife `as_chunks` statt `chunks_exact`; die Prüfsumme blieb gleich (Test gegen das Halbbyte-Verfahren).
