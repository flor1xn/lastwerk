# T03: FIT einlesen (lastwerk-fit)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M1 |
| Voraussetzung | T02 |
| cloudfähig | ja |
| Bereich | `crates/fit` |
| SPEC | 5.4 (lastwerk-fit), 6.5, 10.2, 14 (O2), 11.4 (Fuzz) |

## Ziel

`parse_activity` und `scan_dir` lesen FIT-Dateien robust ein. Fehlerhafte Dateien erzeugen Fehler pro Datei, nie einen Abbruch.

## Warum

Alle Kennzahlen starten hier. Der Parser ist die Angriffsfläche für manipulierte Dateien (SPEC 10.2).

## Umfang

1. **Entscheidung O2 zuerst.** Prüfe, unter welchen Bedingungen das FIT-Profil weitergegeben werden darf und ob `fitparser` (oder eine andere Bibliothek) Garmin-Profildaten enthält und mit MIT OR Apache-2.0 vereinbar ist. Trage das Ergebnis mit Datum und Quelle in SPEC 14 (O2) ein. Standard bei unklarer Lage: eigener minimaler Decoder auf Basis des Protokolls und des Encoders aus T02, ohne Garmin-Profildateien im Repository.
2. `RawActivity`, `FitError`, `parse_activity(&[u8])`, `scan_dir(&Path)` nach SPEC 5.4. `FitError` unterscheidet Prüfsummenfehler, unbekannte Version, keine Aktivität (zum Beispiel Monitoring), leere Datei.
3. Aufbereitung auf 1 Hz und Plausibilitätsregeln nach SPEC 6.5 (gemeinsame Typen mit `lastwerk-core`, kein Doppelcode).
4. Größenlimit je Datei, Fehlerliste statt Abbruch.
5. Fuzz-artige Tests: mutierte Fixtures (Bitflips, Abschneiden) dürfen nie paniken (proptest).

## Nicht im Umfang

Speicherung, Duplikatregel, Belastung.

## Vorgaben

- `lastwerk-fit` hängt von `lastwerk-core` nur für gemeinsame Typen ab, nicht umgekehrt.
- Keine echten Dateien in Tests, nur die synthetischen Fixtures aus `lastwerk-testkit` (T02, API im Speicher) und deren Erwartungen.
- Eine neue Abhängigkeit (falls O2 für `fitparser` ausgeht) mit Name, Version und Lizenz unter "Blocker oder Vermerke" dieser Datei eintragen.

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p lastwerk-fit
cargo test --workspace
```

Erfolg: Jede Fixture aus `lastwerk-testkit` liefert das erwartete Ergebnis (Aktivität oder erwarteter Fehlercode), die beschädigten Dateien erscheinen als Fehler, die Mutationstests laufen ohne Panik, O2 ist in SPEC 14 entschieden.

## Manuelle Schritte

Keine.

## Blocker oder Vermerke

Keine Blocker. Stand 13. Oktober 2026:

- **Entscheidung O2:** eigener minimaler Decoder, kein `fitparser`, keine Garmin-Profildateien. Begründung und Quellenlage stehen in SPEC 14 (O2), Q19 und 5.3. Kurz: `fitparser` enthält aus Garmins `Profile.xlsx` erzeugten Profilcode, die Lizenzseiten von Garmin waren aus der Cloud-Umgebung nicht erreichbar (DNS-Fehler), die Lage ist unklar, also gilt der Standard dieses Tasks. Neu bewerten, wenn die Lizenz im Original gelesen werden kann.
- Neue Abhängigkeiten in `crates/fit` (alle bereits im Workspace durch `core` oder `testkit` vorhanden, keine neuen Pakete in `Cargo.lock`): `lastwerk-core` (Pfad, gemeinsame Typen und `check_plausibility`), `chrono` 0.4.45 (MIT OR Apache-2.0, nur Datumstypen, `default-features = false`, Features `std` und `serde` wie in `core`), `thiserror` 2.0.21 (MIT OR Apache-2.0, `FitError`), `serde` 1.0.229 (MIT OR Apache-2.0, Ableitungen für `RawActivity` und Teiltypen, damit der Import sie speichern kann). Entwicklung: `lastwerk-testkit` (Pfad), `proptest` 1.12.0 (MIT OR Apache-2.0, Mutationstests). `scan_dir` nutzt nur `std::fs`.
- `crates/fit/src/profile.rs` führt die Nachrichten- und Feldnummern ein zweites Mal (das Testkit ist Entwicklungscrate). Ein Test gleicht beide Listen ab. Neu gegenüber dem Testkit: `sport`-Nachricht (12), `record.enhanced_speed` (73), die Basistypen als Typnummern (untere fünf Bit des Typbytes).
- **Abweichung von SPEC 5.4:** `scan_dir` liefert `Result<FitEntry, ScanError>`, wobei `ScanError` den relativen Pfad und den `FitError` trägt. Ohne Pfad ließe sich ein Fehler in der Importliste keiner Datei zuordnen. Eingetragen in SPEC 5.4 (Umsetzung in T03). Dort stehen auch die Konventionen: Ursprung von `t_s` ist der erste `record`, letzter Wert je Sekunde gewinnt, Grenzen (64 MiB, 2 000 000 `record`, 100 000 `lap`), lokaler Versatz, Sportzuordnung.
- **Änderung im Testkit** (`crates/testkit/src/fixtures.rs`): Die Erwartung `sport` der Fixture `no_session` war `Some(Cycling)`, obwohl die Datei weder `session` noch `sport`-Nachricht enthält. Jetzt `None`. Der Parser liefert dort `Sport::Other`. Das Manifest ändert sich für diese eine Fixture entsprechend.
- Bewusste Konventionen im Decoder: komprimierte Zeitstempelköpfe werden nach der Standardregel (untere fünf Bit des Bezugs, Überlauf plus 32 s) rekonstruiert, ein komprimierter Datensatz ohne vorherigen vollen Zeitstempel behält keinen Zeitstempel und fällt als `record` weg. Entwicklerfelder werden nur der Größe nach übersprungen. Felder vom Typ String, Float, Byte und 64-Bit werden nicht gelesen, das Profil braucht sie nicht. Header-Prüfsumme 0 gilt als "nicht berechnet".
- Ergebnis je Fixture (19): alle 15 lesbaren Dateien liefern die Aktivität mit Sportart, Dauer (Toleranz 1 s), Kanälen, Anzahl Messpunkte, Start, lokalem Versatz, Beckenlänge, Lückensekunden und Ausreißerzahlen wie in `Expectation`. `corrupt_crc` ergibt `crc_mismatch`, `corrupt_truncated` ergibt `truncated`, `monitoring` ergibt `no_activity`, `empty` ergibt `empty_file`.
- Mutationstests (proptest, 400 Fälle je Eigenschaft): Bitflips mit alter und mit neu berechneter Prüfsumme, Abschneiden, zufällige Datenteile im gültigen Rahmen, zufällige Bytes, zufällige Nachrichten hinter einem gültigen `file_id`, dazu alle Bytes einer kurzen Datei einzeln verändert. Kein Fund. Ein einmaliger Lauf mit 30 000 Fällen je Eigenschaft im Release-Profil war ebenfalls ohne Panik.
- Die Fixtures, die `scan_dir` im Test liest, werden unter `target/fit-scan-test/` geschrieben und bei jedem Lauf neu angelegt. `*.fit` und `target/` sind ignoriert.
