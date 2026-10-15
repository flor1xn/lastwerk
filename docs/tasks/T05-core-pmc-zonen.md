# T05: Zonen, Fitness und Schwellenwerte (core, store, cli)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M2 |
| Voraussetzung | T04 |
| cloudfähig | ja |
| Bereich | `crates/core`, `crates/store`, `crates/cli`, `catalog/zones.json` |
| SPEC | 8.2, 8.5, 8.6, 6.2 (`threshold`, `activity_zone_time`, `activity_best_power`), 12 (M2) |

## Ziel

CTL, ATL und TSB als Verlauf, Zonen und Zeit in Zonen je Aktivität, Schwellenwerte mit Gültigkeitsdatum erfassbar und Schätzvorschläge, alles über CLI nutzbar.

## Umfang

- `pmc(&[DailyLoad], &PmcParams) -> Vec<PmcPoint>` nach SPEC 8.2. Standard tau 42 und 7 Tage (einstellbar), Startwerte einstellbar.
- Zonenschemata aus `catalog/zones.json` (SPEC 8.5), `zone_seconds`. Grenzen sind Parameter und als Sekundärquelle oder Faustregel gekennzeichnet.
- Speicher: `activity_zone_time` und `activity_best_power` beim Import und `rebuild` füllen (Bestwerte 5, 60, 300, 1200, 3600 s).
- Schwellenwerte: `lastwerk thresholds <set|list>` mit Gültigkeitsdatum. Eine Schwelle wirkt nur ab ihrem Datum (SPEC 3). Beim Setzen werden betroffene Lasten neu berechnet.
- Vorschläge (SPEC 8.6): FTP aus bester 20-Minuten-Leistung, LTHR, CSS. Die App schlägt vor und übernimmt nie selbst (Herkunft `suggested_accepted` nur nach ausdrücklicher Übernahme).
- `lastwerk pmc --db --from --to --format csv|json` über `daily_loads` (nur primäre Aktivitäten, SPEC 6.4).

## Nicht im Umfang

Oberfläche (T06), Planer, Vorausberechnung für Planwochen (T07).

## Vorgaben

- Referenzwerte T-PMC-1 und T-PMC-2 aus SPEC 11.2 und die Tabelle in SPEC 8.2 (Toleranz 1e-3).
- TSB mit den Werten des Vortags (Konvention, SPEC 8.2).
- Golden-Test (`insta`) für `pmc.csv` über feste synthetische Eingaben.
- Fehlende `HR_max` ersetzt durch `estimate_hr_max` aus T01 und ist in der Ausgabe als Schätzung markiert.

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p lastwerk-core pmc::
cargo run -p lastwerk-testkit --bin gen-fixtures -- --out target/fixtures
cargo run -p lastwerk-cli -- import --dir target/fixtures --db target/m1.db
cargo run -p lastwerk-cli -- pmc --db target/m1.db --from 2026-01-01 --to 2026-03-31 --format csv
cargo test --workspace
```

Erfolg: T-PMC-1 und T-PMC-2 grün, `pmc`-CSV entspricht dem Golden-Snapshot, Zeit in Zonen summiert sich je Aktivität auf die Dauer (abzüglich Lücken), Schwellenwert mit späterem Datum ändert frühere Lasten nicht.

## Manuelle Schritte

Keine.

## Blocker oder Vermerke

Offene Fragen O8 (Originalquellen der Zonengrenzen) und O14 (Session-RPE-Umrechnung) bleiben offen und stehen im Code als Faustregel.

Keine Blocker. Erledigt am 13. Oktober 2026.

Neue Abhängigkeit (nur Entwicklung, passt zu `deny.toml`, `cargo-deny` selbst lief in dieser Umgebung nicht, die Lizenzen wurden über `cargo metadata` geprüft):

| Crate | Name | Version | Lizenz | Grund |
|---|---|---|---|---|
| cli (dev) | insta, ohne Standardfeatures | 1.49.0 | Apache-2.0 | Golden-Test für `pmc.csv` (SPEC 11.4). Zieht `similar` 2.7.0 (Apache-2.0), `tempfile` 3.27.0 (MIT OR Apache-2.0), `fastrand` 2.5.0, `rustix` 1.1.5 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT), `linux-raw-sys` 0.12.1 und `errno` 0.3.14 (MIT OR Apache-2.0) |

`insta` steht unter Apache-2.0 und nicht unter MIT. Das passt zur Lizenzliste in `deny.toml`. `lastwerk-core` ist in `crates/cli` jetzt eine normale statt einer Entwicklungsabhängigkeit (Typen `PmcParams`, `Sport`, `Sex`). `cargo-insta` war nicht installiert. Die Snapshots entstanden mit `INSTA_UPDATE=always cargo test` und stehen unter `crates/cli/src/snapshots/`. Ihr Inhalt wurde geprüft, die ersten drei Zeilen zusätzlich von Hand gerechnet (Test `golden_input_matches_the_hand_calculation_for_the_first_days`).

Entscheidungen und Abweichungen von der SPEC (jeweils im betroffenen Abschnitt vermerkt):

- 5.4 core: `pmc(daily, p, from, to)` mit Zeitraum, weil `daily` nur Tage mit Last enthält. `zone_seconds(a, scheme, th, cfg)` mit Bezugswert und Plausibilitätsgrenzen und `Result`. `ZoneScheme` hat zusätzlich `sports` und `note`.
- 5.4 store: `DailyLoad` zieht nach `core` um (der Store gibt ihn weiter). Der Zonenkatalog ist per `include_str!` in `lastwerk-store` eingebettet, damit das Programm ohne Nebendateien läuft und `core` rein bleibt.
- 5.4 und 6.3 Neuberechnung nach `thresholds set`: `Store::recompute_loads` liest die FIT-Dateien aus `--dir` (der Cache hat keine Messpunkte), prüft die SHA-256 und rechnet nur die betroffenen Aktivitäten. Ohne `--dir` gibt die CLI einen Hinweis auf `rebuild` aus und endet mit 0.
- 6.2: Migration 0002 mit `activity_zone_time.estimated` (Kennzeichnung der aus dem Alter geschätzten maximalen Herzfrequenz), Einstellung `athlete.birth_year`.
- 5.4 cli: zusätzlich `thresholds list|suggest`, `athlete set`, Kurzformen `--ftp`, `--lthr`, `--hr-max`, `--hr-rest`, `--run-speed`, `--css`, `--from-suggestion` und die Optionen `--tau-ctl`, `--tau-atl`, `--ctl0`, `--atl0` bei `pmc`.
- 8.5: Zonenstatus nach SPEC. Nur `power7` ist `Sekundärquelle` (Coggan, Q16). `hr_lthr5` (Q22), `hr_karvonen` (Q14 nur für das Konzept), `run_speed5` und `swim_css5` (Q15 nur für den Begriff) sind `Faustregel`, weil die SPEC sie so führt. Alle Grenzen sind vor einer Freigabe am Original zu prüfen (O8). Die Grenzen von `run_speed5` und `swim_css5` hat das Projekt gewählt, sie haben keine Quelle.
- 8.6: Die LTHR ist aus dem Cache nicht ableitbar (keine Herzfrequenzreihe, ein 30-Minuten-Test ist in einer Datei nicht erkennbar). Die CLI weist darauf hin, `core::estimate` rechnet sie aus einer übergebenen Reihe.
- Die Abnahme `pmc --from 2026-01-01 --to 2026-03-31` liefert für die Fixtures nur Nullen, weil die Fixtures im Mai 2026 liegen und die Schwellen fehlen. Der Golden-Snapshot gilt deshalb einem festen synthetischen Verlauf, nicht der Ausgabe auf den Fixtures. Der CLI-Test `thresholds_set_recomputes_and_pmc_follows_the_formula` prüft die Kette Import, Schwelle, Neuberechnung, Verlauf auf den Fixtures.

Offene Punkte für spätere Tasks:

- T06: Zonen, Bestleistungen und `estimated` stehen in der Datenbank, die Oberfläche muss sie anzeigen (Texte unter `zones.*` in den Sprachdateien) und Startwerte und Zeitkonstanten des PMC als Einstellungen führen (SPEC 8.2 sagt, sie stehen im Backup). Die CLI-Texte stehen weiter als Zeichenketten im Code.
- T07: `pmc` ist für Planwochen ab einem Startwert nutzbar (`ctl0`, `atl0` aus dem letzten Tag der Historie).
- Die Berechnung der Zonen nimmt `rpe = None` wie die Belastung. Manuelle Krafteinheiten (`manual_session`) fließen noch in keine Last ein.
