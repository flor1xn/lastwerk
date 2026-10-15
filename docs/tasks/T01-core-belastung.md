# T01: Belastung je Aktivität (lastwerk-core)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M1 |
| Voraussetzung | T00 |
| cloudfähig | ja |
| Bereich | `crates/core` |
| SPEC | 5.4 (lastwerk-core), 6.5, 8.1, 11.1, 11.2 |

## Ziel

`lastwerk-core` berechnet die Belastung einer Aktivität je Sportart nach SPEC 8.1, rein und ohne Ein- und Ausgabe, und besteht alle handgerechneten Referenzwerte.

## Warum

Jede spätere Kennzahl (PMC, Plan, Oberfläche) baut auf der Belastung je Aktivität auf. Ein Fehler hier verfälscht alles.

## Umfang

- Typen aus SPEC 5.4: `Sport`, `Sample`, `ActivityInput`, `Thresholds`, `Sex`, `LoadMethod`, `Load`, `LoadConfig`, `LoadError` (Fehlerarten `missing_threshold`, `missing_channel`, `implausible`).
- `normalized_power(&[f32]) -> Option<f32>`: 30-Sekunden-Gleitmittel auf 1-Hz-Daten, vierte Potenz, Mittel, vierte Wurzel. Festlegung zu Anfangsfenstern (nur volle Fenster, Konvention) im Code-Kommentar dokumentieren.
- `compute_load(&ActivityInput, &Thresholds, &LoadConfig) -> Result<Load, LoadError>` mit Methodenwahl: Rad mit Leistung TSS, Rad ohne Leistung und Lauf mit Herzfrequenz hrTSS (TRIMP, Konstanten in `LoadConfig`), Lauf mit Geschwindigkeit rTSS, Schwimmen sTSS, Kraft Session-RPE. Reihenfolge der Methodenwahl beim Lauf (Geschwindigkeit vor Herzfrequenz) als Konvention festhalten.
- Plausibilitätsregeln aus SPEC 6.5 als Funktion vor der Berechnung (verworfene Samples und Lücken werden gezählt und im Ergebnis ausgewiesen).
- `estimate_hr_max(age) -> f32` nach `208 - 0,7 * Alter` (Q7), als Schätzung gekennzeichnet.
- Fehlende Daten erzeugen nie eine Null, sondern einen `LoadError` (SPEC 5.4).

## Nicht im Umfang

PMC, Zonen, Planer, Speicherung, FIT-Einlesen. Keine Abhängigkeit außer `serde`, `chrono` (nur Datumstypen) und `thiserror`. Dev-Abhängigkeit `proptest` ist erlaubt.

## Vorgaben

- Referenzwerte T-NP-1, T-NP-2, T-NP-3, T-TSS-1, T-TSS-2, T-RTSS-1, T-STSS-1, T-SRPE-1, T-HRMAX-1 aus SPEC 11.2 als Konstanten mit Rechenweg im Kommentar.
- TRIMP-Konstanten sind Parameter in `LoadConfig`, kein fest verdrahteter Code. Fehlt die Angabe zum Geschlecht, ist das Ergebnis `missing_threshold`, kein Mittelwert (SPEC 8.1).
- rTSS und sTSS sind "Faustregel" und im Ergebnis als geschätzt gekennzeichnet.
- Modul `load`, damit `cargo test -p lastwerk-core load::` die Tests trifft.

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p lastwerk-core load::
cargo test --workspace
```

Erfolg: Alle genannten Referenzwerte grün, Negativtests für fehlende Schwelle, fehlenden Kanal und unplausible Daten, `crates/core/Cargo.toml` enthält nur erlaubte Abhängigkeiten.

## Manuelle Schritte

Keine.

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026.

Abhängigkeiten in `crates/core/Cargo.toml` (alle MIT OR Apache-2.0, passend zu `deny.toml`):

| Name | Version | Lizenz | Grund |
|---|---|---|---|
| serde (derive) | 1.0.229 | MIT OR Apache-2.0 | Typen aus SPEC 5.4 serialisierbar für Speicher und Oberfläche |
| chrono | 0.4.45, ohne Standardfeatures, Features `std`, `serde` | MIT OR Apache-2.0 | `DateTime<Utc>` und `NaiveDate`. Ohne `clock`, damit keine Systemzeit ins Crate kommt |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | `LoadError` mit Anzeigetext und stabilem Fehlercode |
| proptest (nur Dev) | 1.12.0, ohne Standardfeatures, Feature `std` | MIT OR Apache-2.0 | Eigenschaft T-NP-3 |

Konventionen, die die SPEC offen ließ und die jetzt in SPEC 5.4, 8.1 und 11.2 vermerkt sind:

- `Load` trägt zusätzlich `estimated`, `duration_s` und `plausibility`. Verworfene Samples und Lücken stehen in `Plausibility`, das `compute_load` im Ergebnis mitliefert. `check_plausibility` ist öffentlich und liefert die in Abschnitte getrennten Samples.
- Methodenwahl: Leistung vor Herzfrequenz (Rad), Geschwindigkeit vor Herzfrequenz (Lauf). Fehlt die Schwelle der so bestimmten Methode, gibt es `missing_threshold` ohne stillen Wechsel. `Sport::Other` rechnet hrTSS, wenn Herzfrequenz vorliegt.
- NP nur über volle 30-Sekunden-Fenster. Innerhalb eines Abschnitts werden fehlende oder verworfene Leistungswerte übersprungen, nicht mit Null gefüllt. Lange Lücken (über 10 s) trennen Abschnitte, Fenster laufen nicht darüber. Die Dauer umfasst kurze Lücken und schließt Pausen aus.
- hrTSS: kurze Lücken halten die letzte gültige Herzfrequenz, Herzfrequenz unter dem Ruhepuls zählt Null. Normierer `60 * x_LTHR * a * exp(b * x_LTHR)`, dazu ein Referenztest: eine Stunde an der LTHR ergibt 100 unabhängig von den Konstanten.
- T-NP-3: die untere Schranke ist das Mittel der Fenstermittel, siehe Vermerk in SPEC 11.2.
- Kraft ohne RPE meldet `missing_channel`, RPE außerhalb 1 bis 10 `implausible`. `pool_length_m` wird mitgeführt, aber nicht verwendet.
- Offene Fragen O8 (Originalquellen der TRIMP-Konstanten) und O14 (Session-RPE-Umrechnung) bleiben offen. Beide Werte sind Parameter in `LoadConfig`.
