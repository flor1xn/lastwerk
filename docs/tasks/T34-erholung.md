# T34: Erholung über Monate

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 (Auswertung) |
| cloudfähig | ja |
| Voraussetzung | T22, T25 |
| Bereich | `crates/core` (Basislinie als Reihe), `app/src-tauri` (Befehl `get_recovery`), `app/ui` (Ansicht "Erholung"), `catalog/locales` |
| SPEC | 4.2 (F20, F41), 8.4 (R12), Anhang A (Q6) |

## Ziel

Eine Ansicht "Erholung" zeigt HRV, Ruhepuls, Schlafdauer, Schlafwert und Body Battery über 30, 90 oder 365 Tage, jeweils mit der eigenen Basislinie aus R12 und dem Band, ab dem R12 ein Absinken erkennt, und darunter die Tageslast auf derselben Zeitachse.

## Warum

Garmin Connect zeigt HRV-Status und Schlaf ohne Bezug zur Trainingslast, das Dashboard nur 60 Tage als Mini-Verlauf. Die Seite verbindet beides.

## Umfang

1. `core`: Basislinie je Tag als Reihe über `recovery_drops` bzw. dieselbe Rechnung wie R12 (Mittel der `baseline_days` Tage davor, mindestens `min_baseline_days`), kein Duplikat der Logik.
2. Befehl `get_recovery(today, days)`: Tageswerte, Basislinie, untere Grenze für HRV und Schlaf (`hrv_drop_fraction`, `sleep_deficit_fraction` aus R12), Tageslast, Mittel der letzten 7 Tage gegen das Mittel des Zeitraums, Code aus `readiness_state`.
3. Ansicht "Erholung" in der Navigation mit Zeitraumwahl, je Größe ein Verlauf (uPlot) mit Band und Wert im Zeiger, Body Battery als Spanne Tief bis Hoch, Tageslast als Balken.

## Nicht im Umfang

Medizinische Deutung (SPEC 2.3), Korrelationsrechnung zwischen Last und HRV (keine Quelle).

## Vorgaben

- Texte beschreiben Trainingsdaten, keine Gesundheit; der Test aus T25 gegen Gesundheitsbegriffe gilt auch für die neuen Schlüssel.

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo fmt --manifest-path app/src-tauri/Cargo.toml --check && cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings && cargo test --manifest-path app/src-tauri/Cargo.toml --locked
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
sh scripts/check-all.sh
```

Dazu: Attrappe mit synthetischen Werten für jede neue Ansicht (auch leer und extrem), Vitest für Deutung und Formatierung, Playwright-Rauchtest Deutsch und Englisch, Sichtprüfung hell und dunkel, schmal und breit (`break-ui`).

## Manuelle Schritte

Sichtprüfung im echten Tauri-Fenster mit eigener Datenbank durch den Nutzer.

## Vermerke

### Geliefert (15. Oktober 2026)

- `core`: Die Basisrechnung von R12 steht jetzt einmal in `plan::recovery::baseline_mean`; `relative_drop` (R12, Dashboard) und die neue `baselines` rufen sie. `metrics::readiness::baseline_series` liefert je Tag Basis, Untergrenze (Basis mal 1 minus `hrv_drop_fraction` bzw. `sleep_deficit_fraction`) und `low` mit derselben Grenze "ab" wie R12. Test mit Handrechnung und Abgleich mit `readiness_state`.
- Befehl `get_recovery(today, days)` an allen vier Stellen, `days` ist 30, 90 oder 365 (sonst `invalid_value`). Liest die Erholungstage ab Zeitraumbeginn minus `baseline_days`, damit auch der erste Tag eine Basis hat, dazu die Tageslast. Mittel der letzten sieben Tage gegen das Mittel des Zeitraums (beschreibend), Zahl der Tage unter der Untergrenze, Deutung des jüngsten Tages wie im Dashboard. Integrationstest mit synthetischen Erholungstagen.
- Seite "Erholung" in der Navigation nach "Leistung": Zeitraum, Kennzahlen (Ø 7 Tage mit Abweichung zum Ø des Zeitraums), Verläufe für HRV, Ruhepuls und Schlaf mit Tageswert, eigener Basis und Untergrenze (Ruhepuls ohne, R12 kennt ihn nicht), Tage unter der Untergrenze als eigene Punkte, Body Battery als Spanne Hoch bis Tief, Tageslast als Balken. Alle Diagramme teilen den Zeiger (`cursor.sync`), die Werte des Tages stehen als Text darüber.
- Abweichung: keine Korrelationsrechnung, wie in der Task-Datei ausgeschlossen. Lücken bleiben Lücken.
