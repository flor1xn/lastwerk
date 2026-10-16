# T40: Garmin-Vollabzug Teil 2 (Verläufe, Trainingsstatus, S3 je Bereich)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 |
| cloudfähig | ja, **Livetest manuell** (O30, O32) |
| Voraussetzung | T39 |
| Bereich | `crates/garmin` (Allowlist), `crates/store` (Migration 0008), `crates/cli` (Abgleich), `app/src-tauri`, `app/ui`, `catalog`, `docs` |
| SPEC | 5.4, 5.5, 5.6, 8.6, 10, 14 (O32), Anhang A (Q28 bis Q33) |

## Ziel

Alles, was T39 unter "Offen" ließ, kommt beim Abgleich mit: Verlauf von FTP, Laktatschwellen-Herzfrequenz und Schwellentempo, das Schwellentempo selbst, VO2max-Verlauf, Trainingsstatus mit akuter und chronischer Last, Belastungsfokus, Rennprognosen, Lauftoleranz, Etagen und mit Freigabe je Bereich Gewicht und Körperwerte, Blutdruck und Ernährung.

## Warum

Der Nutzer will alle Werte aus Garmin ohne eigene Eingabe (Auftrag vom 16. Oktober 2026). T39 ließ diese Endpunkte aus, weil das Antwortformat weder in der Bibliothek (Q27) noch im Bericht der Sonde belegt war. Inzwischen liegen Aufzeichnungen echter Antworten und Lesecode in Projekten Dritter vor (Q28 bis Q33). Das sind Sekundärquellen; der Livetest (O32) bestätigt sie.

## Entscheidungen des Nutzers (16. Oktober 2026)

- **E5** Werte ohne Datum (maximale Herzfrequenz, Ruhepuls): Die Konvention aus T39 gilt (erstes Auftreten ab Fensterbeginn, spätere Änderung ab dem Tag des Abgleichs).
- **E7** S3 wird je Bereich freigegeben: Körper und Gewicht, Blutdruck, Ernährung, Trinken. Standard aus, angeboten in der Einrichtung (T41) und in den Einstellungen, Löschen je Bereich.

## Umfang

1. Allowlist (`crates/garmin`): Endpunkte der Tabelle unten. S3 nur über `SensitiveConsent`.
2. Speicher (Migration 0008): neue Spalten in `wellness_day` und `training_day`, neue S3-Tabellen `body_day`, `bp_day`, `nutrition_day`, Einstellungen `garmin.share.body`, `garmin.share.blood_pressure`, `garmin.share.nutrition`.
3. Abgleich (`crates/cli`): Verläufe in `threshold` mit Herkunft `garmin` und Datum `from`; Schwellentempo; neue Reihenfolge Konto, Aktivitäten, Schwellen und Import, Erholung, Tageswerte, damit die Last früh da ist.
4. App: Seite Gesundheit mit Trainingsstatus, Rennprognosen, Lauftoleranz, Etagen und den S3-Bereichen; Schalter je Bereich in den Einstellungen.

## Nicht im Umfang

- Menstruations- und Schwangerschaftsdaten (nie, E3).
- Tageszusammenfassung `usersummary/daily`, Lebensstil-Protokoll, Mahlzeiten-Einstellungen.
- Garmins Last als Ersatz für die eigene (T39 E2).
- Schreibende Endpunkte.

## Belegte Endpunkte

Belege: `I:` Zeile in `garminconnect/__init__.py` 0.3.17 (Q27) für Pfad und Parameter, Q28 bis Q33 für die Felder.

| Daten | Pfad | Felder, die Lastwerk liest | Beleg |
|---|---|---|---|
| FTP-Verlauf | `/biometric-service/stats/functionalThresholdPower/range/{von}/{bis}?sport=CYCLING&aggregation=daily&aggregationStrategy=LATEST`, höchstens 366 Tage | je Zeile `from`, `value` (W) | I:1696, Q28, Q31 |
| LTHR-Verlauf | `.../lactateThresholdHeartRate/range/...?sport=RUNNING...` | `from`, `value` (1/min) | I:1845, Q28 |
| Schwellentempo-Verlauf | `.../lactateThresholdSpeed/range/...?sport=RUNNING...` | `from`, `value` × 10 = m/s | I:1840, Q28, Q30 |
| Schwellentempo jüngster Wert | `latestLactateThreshold` (seit T39 abgerufen) | `speed` × 10 = m/s, `calendarDate` | I:1786, Q28, Q30 |
| VO2max-Verlauf | `/metrics-service/metrics/maxmet/daily/{von}/{bis}` | `generic.calendarDate`, `generic.vo2MaxPreciseValue`, `cycling.vo2MaxPreciseValue` | I:1683, Q28 |
| Trainingsstatus | `/metrics-service/metrics/trainingstatus/daily/{datum}` | Gerät mit `primaryTrainingDevice`: `calendarDate`, `trainingStatusFeedbackPhrase` (Präfix), `acuteTrainingLoadDTO.dailyTrainingLoadAcute`, `dailyTrainingLoadChronic`, `dailyAcuteChronicWorkloadRatio` | I:2356, Q28, Q29 |
| Belastungsfokus | `/metrics-service/metrics/trainingloadbalance/latest/{datum}` | `monthlyLoadAerobicLow`, `monthlyLoadAerobicHigh`, `monthlyLoadAnaerobic`, je mit `TargetMin` und `TargetMax`, `trainingBalanceFeedbackPhrase` | I:2345, Q28 |
| Rennprognosen | `/metrics-service/metrics/racepredictions/latest/{displayName}` und `/daily/{displayName}?fromCalendarDate&toCalendarDate` | `calendarDate`, `time5K`, `time10K`, `timeHalfMarathon`, `timeMarathon` (s) | I:2452, I:2469, Q28 |
| Lauftoleranz | `/metrics-service/metrics/runningtolerance/stats?startDate&endDate&aggregation=daily`, höchstens 336 Tage | `calendarDate`, `acuteDistance` (m), `acuteImpactLoad`, `acuteTolerance` (m), `runningToleranceFeedBackPhrase` | I:2414, Q32 |
| Etagen | `/wellness-service/wellness/floorsChartData/daily/{datum}` | `floorsValueDescriptorDTOList` (Schlüssel `floorsAscended`, `floorsDescended`), Summe über `floorValuesArray` | I:1198, Q29 |
| Gewicht, Körper (S3) | `/weight-service/weight/range/{von}/{bis}?includeAll=true` | `dailyWeightSummaries[].allWeightMetrics[]`: `calendarDate`, `date`, `weight`, `boneMass`, `muscleMass` (g), `bmi`, `bodyFat`, `bodyWater` (%) | I:1522, Q28, Q29 |
| Blutdruck (S3) | `/bloodpressure-service/bloodpressure/range/{von}/{bis}?includeAll=true` | `measurementSummaries[].measurements[]`: `systolic`, `diastolic`, `pulse`, `measurementTimestampLocal` | I:1644, Q33 |
| Ernährung (S3) | `/nutrition-service/food/logs/{datum}` | `mealDate`, `dailyNutritionContent.calories`, `carbs`, `fat`, `protein`, `dailyNutritionGoals.calories` | I:4383, Q33 |

## Vorgaben

- Drosselung, Sperre und GET-only wie in T21, kein Parallelabruf.
- Tageswerte mit einem Aufruf je Tag (Trainingsstatus, Etagen, Ernährung) nur für die letzten 28 Tage (`DETAIL_DAYS`, Faustregel wie in T39). Bereiche über das ganze Fenster in Stücken.
- Der Trainingsstatus wird aus dem Präfix der Phrase gelesen (`PRODUCTIVE_1` wird `productive`), nicht aus dem Zahlencode, weil Projekte Dritter die Codes widersprüchlich deuten (nur 7 = produktiv ist doppelt belegt).
- Schwellentempo: Garmin liefert Dekameter je Sekunde, also × 10 für m/s (Q30, Sekundärquelle; Gegenprobe in Q28: 0,336 bei LTHR 166 und VO2max 50 ergibt 4:57 min/km, plausibel, der Kehrwert ergäbe 5:36 min/km). Plausibel sind 1,5 bis 7 m/s (Faustregel), sonst verworfen.
- Plausibilitätsgrenzen je Wert im Code mit "Faustregel".
- Rohantworten werden nicht gespeichert.

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo fmt --manifest-path app/src-tauri/Cargo.toml --check && cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings && cargo test --manifest-path app/src-tauri/Cargo.toml --locked
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
sh scripts/check-all.sh
```

Erfolgskriterien:

- Alle neuen Abrufe laufen gegen die Attrappe, nur GET, zu lange Spannen werden vor dem Senden abgelehnt.
- Test: Ein FTP-Verlauf gibt einer Aktivität vor dem jüngsten Wert die Last nach dem damals gültigen Wert.
- Test: Ein Schwellentempo aus Garmin gibt einer Laufaktivität eine Last (rTSS).
- Test: S3 je Bereich, ohne Freigabe kein Aufruf, mit Freigabe nur der freigegebene Bereich.
- Migrationstest 7 auf 8 mit Nutzerdaten, neue Tabellen im Export.

## Manuelle Schritte

- Livetest mit echtem Konto (O30, O32): Schwellentempo in den Einstellungen gegen Garmin Connect vergleichen (Faktor 10), Verlauf von FTP und LTHR, Trainingsstatus, Rennprognosen, und mit Freigabe Gewicht und Blutdruck.
- O18 (Nutzungsbedingungen) bleibt beim Nutzer.

## Neue Abhängigkeiten

Keine neue. Geändert am 16. Oktober 2026, weil der automatische Release an `cargo deny check advisories` scheiterte (alle Versionen von zip unter 9.0.3 sind zurückgezogen):

- `zip` 8.6 auf 9.0 (9.0.4, MIT) in `crates/cli`. Einzige Anpassung: `ZipFile::name()` liefert seit 9 ein `Result`; ein ungültig kodierter Name gilt als keine FIT-Datei.
- `tauri-plugin-updater` 2.13.3 auf 2.14.0 (MIT OR Apache-2.0), nur im Lockfile.
- `deny.toml`: Ausnahme für das zurückgezogene `zip@4.6.1`, das nur `tauri-plugin-updater` mitbringt (auch 2.14.0 verlangt zip 4). Kein Sicherheitshinweis, nur der Rückzug. Entfernen, sobald das Plugin zip 9 nutzt.

## Blocker oder Vermerke

- 16. Oktober 2026: Nur Sekundärquellen für die Felder (Q28 bis Q33). Bricht der Livetest eine Annahme, verwirft der Parser die Werte (sie fehlen dann, es entsteht kein falscher Wert), und O32 nennt die Abweichung.
