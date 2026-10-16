# T39: Garmin-Vollabzug und Schwellenwerte automatisch aus Garmin

| Feld | Wert |
|---|---|
| Status | in Arbeit |
| Meilenstein | nach 1.0 |
| cloudfähig | ja, **Livetest manuell** (O30, O32) |
| Voraussetzung | T22, T23, T28 |
| Bereich | `crates/garmin` (Allowlist), `crates/store` (Migration 0007), `crates/cli` (Abgleich), `app/src-tauri`, `app/ui`, `catalog/locales`, `catalog/changelog.json`, `docs` |
| SPEC | 2.1, 4.2, 5.4 (`lastwerk-garmin`, `lastwerk-cli`), 5.5, 5.6, 6.2, 6.3, 8.1, 8.6, 10, 14, Anhang A |

Arbeitstitel im Auftrag war T35. Die Nummer ist vergeben (Trainingskalender), ebenso T36 bis T38, deshalb T39.

## Ziel

Der Nutzer meldet sich einmal bei Garmin an. Danach sind alle Daten, die Garmin über die belegten Endpunkte liefert, und alle Schwellenwerte, die Garmin selbst führt, ohne weitere Konfiguration da. Manuelle Werte bleiben als Überschreibung möglich, sind aber nicht mehr der Normalweg.

## Warum

Bisher galten Schwellenwerte erst ab dem Tag, an dem der Nutzer sie eintrug. Frühere Aktivitäten zeigten "Schwellenwert fehlt", und vor der ersten Last musste der Nutzer FTP, LTHR, Ruhepuls, maximale Herzfrequenz, Geschlecht und Geburtsjahr selbst kennen und eintragen. Garmin kennt diese Werte, oft mit Datum.

## Entscheidungen des Nutzers (16. Oktober 2026, in SPEC 2.1 und 8.6 vermerkt)

- **E1** Schwellenwerte kommen standardmäßig automatisch aus Garmin, mit Herkunft `garmin` und dem Datum aus Garmin. Manuell geht vor Garmin, bei gleichem Datum gewinnt manuell. Ersetzt "Schätzung aus FIT nur als Vorschlag" (SPEC 2.1) für Werte, die Garmin liefert.
- **E2** Garmins eigene Last und Trainingseffekt werden nur zusätzlich angezeigt. Lastwerk rechnet die Last weiter selbst (SPEC 8.1).
- **E3** Nie abgerufen: Menstruations- und Schwangerschaftsdaten. S3 (Gewicht, Körperzusammensetzung, Blutdruck, Hydration, Ernährung) nur mit Freigabe in den Einstellungen, getrennt gespeichert.
- **E4** Kein Cloud-Modell, keine Telemetrie, kein Netz aus dem Frontend.

## Umfang

1. **Allowlist** (`crates/garmin`): neue lesende Endpunkte, nur solche, deren Pfad, Parameter und Antwortfelder im Quelltext von python-garminconnect 0.3.17 (Q27) und im Bericht der Sonde (`docs/garmin-feldabdeckung.md`) belegt sind. Liste und Belege siehe "Belegte Endpunkte". S3 nur über den Marker `SensitiveConsent`.
2. **Speicher** (`crates/store`, Migration 0007): `threshold.origin` kennt `garmin`, neue Nutzertabellen `wellness_day`, `training_day`, `garmin_activity_load`, `sensitive_day`, im Cache `activity_load.missing` (welcher Wert fehlt). Alles außer dem Cache im JSON-Export.
3. **Abgleich** (`crates/cli`): Phase "Konto" (Profil, Schwellen) vor dem Import, Tageswerte, Trainingswerte, Garmins Last je Aktivität, S3 nur mit Freigabe. Garmin-Schwellen gehen nach `threshold`, danach Neuberechnung der betroffenen Aktivitäten.
4. **App und Oberfläche**: Erststart nur Sprache und Garmin, Schwellenwerte mit Herkunft und Datum, neue Seite "Gesundheit" für Tages- und Trainingswerte, Freigabe der S3-Werte in den Einstellungen, Hinweis bei "Schwellenwert fehlt".

## Nicht im Umfang

- Endpunkte ohne belegtes Antwortformat (siehe "Offen").
- Garmins Last als Ersatz für die eigene (E2).
- Schreibende Endpunkte jeder Art.
- Änderungen an `bridge/` (eingefroren seit T20).

## Belegte Endpunkte

Belege: `I:` Zeile in `garminconnect/__init__.py` 0.3.17, `FD` Abschnitt in `docs/garmin-feldabdeckung.md`.

| Daten | Pfad | Felder, die Lastwerk liest | Beleg |
|---|---|---|---|
| Profil | `/userprofile-service/userprofile/user-settings` | `userData.gender`, `userData.birthDate`, `userData.vo2MaxRunning`, `userData.vo2MaxCycling` (Gewicht und Größe werden nicht gelesen, S3) | I:574, I:3515, FD profile |
| FTP | `/biometric-service/biometric/latestFunctionalThresholdPower/CYCLING` | `functionalThresholdPower`, `calendarDate` | I:3344, FD ftp |
| Laktatschwelle | `/biometric-service/biometric/latestLactateThreshold` | je Eintrag `heartRate` (Tippfehler `hearRate` ebenso), `heartRateCycling`, `calendarDate` | I:1757 bis I:1800, FD lactate_threshold |
| Herzfrequenzzonen | `/biometric-service/heartRateZones` | je Eintrag `sport`, `maxHeartRateUsed`, `restingHeartRateUsed` | I:3352, FD zones |
| Schritte | `/usersummary-service/stats/steps/daily/{von}/{bis}`, höchstens 28 Tage | `calendarDate`, `totalSteps`, `totalDistance` | I:1213, FD daily_steps |
| Tagesherzfrequenz | `/wellness-service/wellness/dailyHeartRate/{displayName}?date=` | `calendarDate`, `minHeartRate`, `maxHeartRate` | I:1341, FD heart_rates |
| Atmung | `/wellness-service/wellness/daily/respiration/{datum}` | `calendarDate`, `avgWakingRespirationValue`, `avgSleepRespirationValue` | I:1944, FD respiration |
| SpO2 | `/wellness-service/wellness/daily/spo2/{datum}` | `calendarDate`, `averageSpO2`, `lowestSpO2` | I:1952, FD spo2 |
| Intensitätsminuten | `/wellness-service/wellness/daily/im/{datum}` | `calendarDate`, `moderateMinutes`, `vigorousMinutes` | I:1970, FD intensity_minutes |
| Stress (schon in T21) | `/wellness-service/wellness/dailyStress/{datum}` | `calendarDate`, `avgStressLevel`, `maxStressLevel` | I:2170, FD stress |
| Trainingsbereitschaft | `/metrics-service/metrics/trainingreadiness/{datum}` | Eintrag mit `inputContext` `AFTER_WAKEUP_RESET`, sonst der erste (wie I:2325); `score`, `level`, `acuteLoad`, `calendarDate` | I:2290, I:2298, FD training_readiness |
| Fitnessalter | `/fitnessage-service/fitnessage/{datum}` | `fitnessAge` | I:2486, FD fitness_age |
| Ausdauerwert | `/metrics-service/metrics/endurancescore/stats?startDate&endDate&aggregation=weekly`, höchstens 392 Tage | `enduranceScoreDTO.calendarDate`, `enduranceScoreDTO.overallScore` | I:2368, FD endurance_score, Spanne aus `bridge/registry.yaml` |
| Bergwert | `/metrics-service/metrics/hillscore/stats?startDate&endDate&aggregation=daily`, höchstens 392 Tage | `periodAvgScore.<datum>` | I:2494, FD hill_score |
| Garmins Last je Aktivität | `/fitnessstats-service/activity/all?startDate&endDate&metric=activityTrainingLoad&metric=trainingEffectLabel&metric=trainingEffectLabelSrvrCalc` | `activityId`, `activityTrainingLoad`, `trainingEffectLabel`, `startLocal` | I:2980, FD training_load_activities |
| Hydration (S3, nur mit Freigabe) | `/usersummary-service/usersummary/hydration/daily/{datum}` | `calendarDate`, `valueInML` | I:1936, FD hydration |

Weiterhin aus T21 und T22: Aktivitäten samt FIT, HRV, Schlaf, Ruhepuls, Body Battery.

## Vorgaben

- Drosselung, Sperre und GET-only bleiben wie in T21. Kein Parallelabruf.
- Tageswerte, die je Tag einen Aufruf kosten (Tagesherzfrequenz, Atmung, SpO2, Intensitätsminuten, Stress, Trainingsbereitschaft, Hydration), holt der Abgleich nur für die letzten 28 Tage des Fensters (Faustregel, wie `SLEEP_DAYS` beim Schlaf: Beim ersten Abgleich wären es sonst über 2500 Aufrufe, rund 90 Minuten bei 2 s je Aufruf). Bereichsendpunkte (Schritte, Bergwert, Garmins Last) decken das ganze Fenster ab.
- Plausibilitätsgrenzen je Wert stehen im Code mit "Faustregel" (SPEC 6.5 für Herzfrequenz).
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

- Alle neuen Abrufe laufen in Tests gegen die Attrappe; der Allowlist-Test lehnt jeden nicht freigegebenen Pfad ab, darunter alle Menstruations-, Schwangerschafts- und nicht freigegebenen S3-Pfade.
- Test: Eine Aktivität vor heute bekommt eine Last, wenn Garmin eine Schwelle mit früherem Datum liefert, ohne manuelle Eingabe.
- Test: Ein manueller Wert überschreibt einen Garmin-Wert bei gleichem Datum.
- Test: Menstruations- und Schwangerschaftsdaten werden nie abgerufen, S3 nur mit Freigabe.
- Migrationstest 6 auf 7 mit Nutzerdaten.
- Status `erledigt` in Task-Datei und Tabelle, Eintrag in `catalog/changelog.json`.

## Manuelle Schritte

- Livetest mit echtem Konto (O30, O32): Abgleich laufen lassen, in den Einstellungen Herkunft und Datum der Schwellen mit Garmin Connect vergleichen, Seite "Gesundheit" prüfen.
- O18 (Nutzungsbedingungen) bleibt beim Nutzer.

## Neue Abhängigkeiten

Keine.

## Offen (nicht belegt, nicht in der Allowlist)

- **Verlauf von FTP und Laktatschwelle** (`/biometric-service/stats/functionalThresholdPower/range/...`, `lactateThresholdHeartRate/range`, `lactateThresholdSpeed/range`, I:1696, I:1807): Pfade belegt, Zeilenformat nicht (die Bibliothek liest nichts daraus, die Sonde rief sie nicht auf). Bis zum Livetest kommt je Abgleich nur der jüngste Wert mit seinem Datum; die Zeitreihe wächst mit jedem Abgleich, ältere Werte vor dem ersten Abgleich fehlen.
- **Schwellentempo** (`speed` der Laktatschwelle, `userData.lactateThresholdSpeed`): Einheit nicht belegt (FD nennt nur "number"). Nicht übernommen, das Schwellentempo bleibt manuell.
- **Etagen** (`floorsChartData`): Bedeutung der vier Werte je Eintrag nicht belegt.
- **Trainingsstatus, Belastungsfokus, akute und chronische Last aus dem Trainingsstatus** (`trainingstatus/aggregated`, `trainingstatus/daily`, `trainingloadbalance`): innere Feldnamen nicht belegt (FD nennt nur Schlüsselzahlen).
- **VO2max-Verlauf** (`maxmet/daily`): Felder in `generic` nicht belegt. Lastwerk nimmt den aktuellen Wert aus dem Profil.
- **Rennprognosen** (`racepredictions`): Sonde scheiterte, Felder nicht belegt.
- **Lauftoleranz** (`runningtolerance/stats`): Sonde leer, Felder nicht belegt.
- **Gewicht, Körperzusammensetzung, Blutdruck, Ernährung**: Listen in der Sonde leer (`dateWeightList`, `measurementSummaries`, `mealDetails`), Felder je Messung nicht belegt. Von S3 ist damit nur Hydration angebunden.
- **Tageszusammenfassung** (`usersummary/daily`): 94 Schlüssel ohne Namen im Bericht.

## Blocker oder Vermerke

- 16. Oktober 2026: Werte ohne Datum bei Garmin (maximale Herzfrequenz und Ruhepuls aus den Zonen) bekommen beim ersten Auftreten den Beginn des Abgleichfensters als Gültigkeitsdatum, bei einer späteren Änderung den Tag des Abgleichs (Konvention, SPEC 8.6). Sonst bekämen frühere Aktivitäten keine Last. Bitte bestätigen oder anders entscheiden.
