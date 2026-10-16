//! Werte aus den Antworten des Vollabzugs lesen (T39, SPEC 5.6, 8.6).
//!
//! Wie in [`super::parse`]: Die Antworten sind nicht vertrauenswürdig. Jede Funktion liest nur
//! die benannten Felder und übergeht alles Unerwartete. Die Grenzen der Tageswerte prüft der
//! Speicher (`DayRecord::sanitize`), die der Schwellen diese Datei (SPEC 6.5).
//!
//! Feldnamen nach dem Bericht der Sonde (`docs/garmin-feldabdeckung.md`, Abschnitt in Klammern)
//! und dem Quelltext von python-garminconnect 0.3.17 (Q27, Zeile `I:`). Die Werte selbst hat die
//! Sonde nicht erfasst; Annahmen zu Werten stehen beim Feld und in SPEC 14 O32.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use lastwerk_core::{Sex, Sport};
use lastwerk_garmin::ThresholdSeries;
use lastwerk_store::ThresholdKind;
use serde_json::Value;

use super::parse::{Dropped, date_of, number};

/// Höchste plausible Leistung (SPEC 6.5, Standard 2500 W).
const FTP_MAX_W: f64 = 2500.0;
/// Herzfrequenzgrenzen (SPEC 6.5).
const HR_MIN_BPM: f64 = 30.0;
const HR_MAX_BPM: f64 = 230.0;
/// Garmins Laktatschwellen-Geschwindigkeit mal diesem Faktor ergibt m/s (Q30, Sekundärquelle:
/// Garmin liefert Dekameter je Sekunde, die Web-App rechnet mit `CmPerMsToMps = 10`). Gegenprobe
/// an Q28: 0,336 bei LTHR 166 und VO2max 50 ergibt 3,36 m/s, also 4:57 min/km (SPEC 8.6, O32).
pub const LT_SPEED_TO_MPS: f64 = 10.0;
/// Plausible Schwellengeschwindigkeit im Laufen in m/s (Faustregel: 1,5 m/s sind 11:07 min/km,
/// 7 m/s sind 2:23 min/km). Ein Wert außerhalb zeigt eine falsche Einheit an und wird verworfen.
const RUN_SPEED_MIN_MPS: f64 = 1.5;
const RUN_SPEED_MAX_MPS: f64 = 7.0;

fn within(value: f64, low: f64, high: f64, dropped: &mut Dropped) -> Option<f64> {
    if (low..=high).contains(&value) {
        Some(value)
    } else {
        dropped.0 += 1;
        None
    }
}

/// Werte aus dem Profil (`profile`, I:3515). Gewicht und Größe stehen in derselben Antwort und
/// werden bewusst nicht gelesen (S3 ohne Freigabe, SPEC 5.5).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ProfileValues {
    pub sex: Option<Sex>,
    pub birth_year: Option<i32>,
    pub vo2max_running: Option<f64>,
    pub vo2max_cycling: Option<f64>,
}

/// `userData.gender`, `userData.birthDate`, `userData.vo2MaxRunning`, `userData.vo2MaxCycling`.
/// Für `gender` nimmt Lastwerk `MALE` und `FEMALE` an (Groß- und Kleinschreibung egal); jeder
/// andere Wert bleibt leer (Annahme, O32). Das Geburtsjahr gilt nur zwischen 1900 und `today`.
pub fn profile(v: &Value, today: NaiveDate) -> ProfileValues {
    let Some(user) = v.get("userData") else {
        return ProfileValues::default();
    };
    let sex = user.get("gender").and_then(Value::as_str).and_then(|g| {
        match g.to_ascii_lowercase().as_str() {
            "male" => Some(Sex::Male),
            "female" => Some(Sex::Female),
            _ => None,
        }
    });
    let birth_year = user
        .get("birthDate")
        .and_then(date_of)
        .map(|d| d.year())
        .filter(|y| (1900..=today.year()).contains(y));
    ProfileValues {
        sex,
        birth_year,
        vo2max_running: user.get("vo2MaxRunning").and_then(number),
        vo2max_cycling: user.get("vo2MaxCycling").and_then(number),
    }
}

/// Eine Schwelle aus Garmin: Sportart (`None` für alle), Art, Wert, Datum (falls Garmin eins
/// nennt).
#[derive(Debug, Clone, PartialEq)]
pub struct GarminThreshold {
    pub sport: Option<Sport>,
    pub kind: ThresholdKind,
    pub value: f64,
    pub date: Option<NaiveDate>,
}

/// FTP (`ftp`, I:3344): `functionalThresholdPower` in Watt mit `calendarDate`. Ohne Datum kein
/// Wert, weil Garmin hier eins führt.
pub fn ftp(v: &Value, dropped: &mut Dropped) -> Option<GarminThreshold> {
    let date = date_of(v.get("calendarDate")?)?;
    let watts = number(v.get("functionalThresholdPower")?)?;
    let watts = within(watts, 1.0, FTP_MAX_W, dropped)?;
    Some(GarminThreshold {
        sport: Some(Sport::Cycling),
        kind: ThresholdKind::FtpW,
        value: watts,
        date: Some(date),
    })
}

/// Laktatschwelle (`lactate_threshold`, I:1757 bis I:1800). Die Rohantwort ist eine Liste fast
/// gleicher Einträge; die Bibliothek liest je Eintrag `heartRate` (oder Garmins Tippfehler
/// `hearRate`) und `heartRateCycling`. Lastwerk nimmt je Art den Eintrag mit dem jüngsten
/// `calendarDate`; fehlt es dem Eintrag, gilt das jüngste Datum der Liste. Die Herzfrequenz
/// misst Garmin im Laufen und gilt deshalb nur für Lauf, `heartRateCycling` für Rad (SPEC 8.6).
/// `speed` wird nicht gelesen, weil die Einheit nicht belegt ist (O32).
pub fn lactate_threshold(v: &Value, dropped: &mut Dropped) -> Vec<GarminThreshold> {
    let Some(items) = v.as_array() else {
        return Vec::new();
    };
    let list_date = items
        .iter()
        .filter_map(|i| i.get("calendarDate").and_then(date_of))
        .max();
    let mut best: Vec<GarminThreshold> = Vec::new();
    for item in items {
        let date = item.get("calendarDate").and_then(date_of).or(list_date);
        let Some(date) = date else { continue };
        let running = item
            .get("heartRate")
            .and_then(number)
            .or_else(|| item.get("hearRate").and_then(number));
        let cycling = item.get("heartRateCycling").and_then(number);
        let speed = item.get("speed").and_then(number).and_then(|raw| {
            within(
                raw * LT_SPEED_TO_MPS,
                RUN_SPEED_MIN_MPS,
                RUN_SPEED_MAX_MPS,
                dropped,
            )
        });
        let hr = |bpm: Option<f64>, dropped: &mut Dropped| {
            bpm.and_then(|b| within(b, HR_MIN_BPM, HR_MAX_BPM, dropped))
        };
        let found = [
            (Sport::Running, ThresholdKind::LthrBpm, hr(running, dropped)),
            (Sport::Cycling, ThresholdKind::LthrBpm, hr(cycling, dropped)),
            (Sport::Running, ThresholdKind::RunSpeedMps, speed),
        ];
        for (sport, kind, value) in found {
            let Some(value) = value else {
                continue;
            };
            let candidate = GarminThreshold {
                sport: Some(sport),
                kind,
                value,
                date: Some(date),
            };
            match best
                .iter_mut()
                .find(|t| t.sport == Some(sport) && t.kind == kind)
            {
                Some(t) if t.date < candidate.date => *t = candidate,
                Some(_) => {}
                None => best.push(candidate),
            }
        }
    }
    best
}

/// Sportart eines Zonenprofils. Die Namen sind nicht belegt (O32): angenommen `DEFAULT` für
/// alle und die Namen der Sportarten in Großbuchstaben.
fn zone_sport(name: &str) -> Option<Option<Sport>> {
    match name.to_ascii_uppercase().as_str() {
        "DEFAULT" => Some(None),
        "RUNNING" => Some(Some(Sport::Running)),
        "CYCLING" => Some(Some(Sport::Cycling)),
        "SWIMMING" => Some(Some(Sport::Swimming)),
        _ => None,
    }
}

/// Maximale Herzfrequenz und Ruhepuls aus den Herzfrequenzzonen (`zones`, I:3352): je Profil
/// `sport`, `maxHeartRateUsed`, `restingHeartRateUsed`. Garmin nennt kein Datum. Nennen alle
/// Profile denselben Wert, gilt er für alle Sportarten, sonst je Profil mit bekanntem Namen
/// (SPEC 8.6).
pub fn heart_rate_zones(v: &Value, dropped: &mut Dropped) -> Vec<GarminThreshold> {
    let Some(items) = v.as_array() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (field, kind) in [
        ("maxHeartRateUsed", ThresholdKind::HrMaxBpm),
        ("restingHeartRateUsed", ThresholdKind::HrRestBpm),
    ] {
        let values: Vec<(Option<&str>, f64)> = items
            .iter()
            .filter_map(|i| {
                let bpm = number(i.get(field)?)?;
                let bpm = within(bpm, HR_MIN_BPM, HR_MAX_BPM, dropped)?;
                Some((i.get("sport").and_then(Value::as_str), bpm))
            })
            .collect();
        let Some(&(_, first)) = values.first() else {
            continue;
        };
        if values.iter().all(|(_, b)| (b - first).abs() < 1e-9) {
            out.push(GarminThreshold {
                sport: None,
                kind,
                value: first,
                date: None,
            });
            continue;
        }
        for (name, bpm) in values {
            let Some(sport) = name.and_then(zone_sport) else {
                continue;
            };
            if out
                .iter()
                .any(|t: &GarminThreshold| t.kind == kind && t.sport == sport)
            {
                continue;
            }
            out.push(GarminThreshold {
                sport,
                kind,
                value: bpm,
                date: None,
            });
        }
    }
    out
}

/// Schritte je Tag (`daily_steps`, I:1213): Liste mit `calendarDate`, `totalSteps`,
/// `totalDistance` (Meter laut Docstring der Wochenform, I:1281).
pub fn steps_days(v: &Value) -> Vec<(NaiveDate, Option<f64>, Option<f64>)> {
    let Some(items) = v.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|i| {
            let date = date_of(i.get("calendarDate")?)?;
            Some((
                date,
                i.get("totalSteps").and_then(number),
                i.get("totalDistance").and_then(number),
            ))
        })
        .collect()
}

/// Zahl eines Feldes, wenn die Antwort ein Objekt ist.
pub fn field(v: &Value, name: &str) -> Option<f64> {
    v.get(name).and_then(number)
}

/// Datum `calendarDate` einer Tagesantwort. Weicht es vom angefragten Tag ab, verwirft der
/// Abgleich die Antwort.
pub fn calendar_date(v: &Value) -> Option<NaiveDate> {
    v.get("calendarDate").and_then(date_of)
}

/// Trainingsbereitschaft (`training_readiness`, I:2290): Liste von Ständen des Tages. Wie
/// `get_morning_training_readiness` (I:2325) zählt der Eintrag mit `inputContext`
/// `AFTER_WAKEUP_RESET`, sonst der erste. Gelesen: `score`, `level`, `acuteLoad`.
pub fn training_readiness(v: &Value) -> Option<(Option<f64>, Option<String>, Option<f64>)> {
    let items = v.as_array()?;
    let entry = items
        .iter()
        .find(|i| i.get("inputContext").and_then(Value::as_str) == Some("AFTER_WAKEUP_RESET"))
        .or_else(|| items.first())?;
    Some((
        field(entry, "score"),
        entry
            .get("level")
            .and_then(Value::as_str)
            .map(str::to_owned),
        field(entry, "acuteLoad"),
    ))
}

/// Ausdauerwert (`endurance_score`, I:2368): `enduranceScoreDTO.calendarDate` und
/// `.overallScore`, der jüngste Stand der Spanne.
pub fn endurance_score(v: &Value) -> Option<(NaiveDate, f64)> {
    let dto = v.get("enduranceScoreDTO")?;
    Some((
        date_of(dto.get("calendarDate")?)?,
        number(dto.get("overallScore")?)?,
    ))
}

/// Bergwert je Tag (`hill_score`, I:2494, `aggregation=daily`): `periodAvgScore` mit dem Datum
/// als Schlüssel.
pub fn hill_scores(v: &Value) -> Vec<(NaiveDate, f64)> {
    let Some(map) = v.get("periodAvgScore").and_then(Value::as_object) else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(k, val)| {
            let date = NaiveDate::parse_from_str(k, "%Y-%m-%d").ok()?;
            Some((date, number(val)?))
        })
        .collect()
}

/// Garmins Last je Aktivität (`training_load_activities`, I:2980): Liste mit `activityId`,
/// `activityTrainingLoad`, `trainingEffectLabel`, `startLocal`.
pub fn training_loads(v: &Value) -> Vec<(u64, NaiveDate, Option<f64>, Option<String>)> {
    let Some(items) = v.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|i| {
            let id = i.get("activityId")?.as_u64().filter(|id| *id > 0)?;
            let date = date_of(i.get("startLocal")?)?;
            Some((
                id,
                date,
                field(i, "activityTrainingLoad"),
                i.get("trainingEffectLabel")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            ))
        })
        .collect()
}

/// Verlauf einer Schwelle (`/biometric-service/stats/{kennzahl}/range`, I:1696, I:1807). Jede
/// Zeile ist eine Änderung mit `from`, `until`, `series` und `value` (Q28, Sekundärquelle). Der
/// Wert gilt ab `from`. Eine Zeile mit einer anderen `series` als der angefragten Sportart wird
/// übergangen: Garmin führt etwa eine eigene Lauf-FTP, die nicht fürs Rad gilt.
pub fn threshold_series(
    v: &Value,
    series: ThresholdSeries,
    dropped: &mut Dropped,
) -> Vec<GarminThreshold> {
    let Some(items) = v.as_array() else {
        return Vec::new();
    };
    let (sport, kind, name) = match series {
        ThresholdSeries::FunctionalThresholdPower => {
            (Sport::Cycling, ThresholdKind::FtpW, "cycling")
        }
        ThresholdSeries::LactateThresholdHeartRate => {
            (Sport::Running, ThresholdKind::LthrBpm, "running")
        }
        ThresholdSeries::LactateThresholdSpeed => {
            (Sport::Running, ThresholdKind::RunSpeedMps, "running")
        }
    };
    let mut out: Vec<GarminThreshold> = Vec::new();
    for item in items {
        if item
            .get("series")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.eq_ignore_ascii_case(name))
        {
            continue;
        }
        let Some(date) = item.get("from").and_then(date_of) else {
            continue;
        };
        let Some(raw) = item.get("value").and_then(number) else {
            continue;
        };
        let value = match kind {
            ThresholdKind::FtpW => within(raw, 1.0, FTP_MAX_W, dropped),
            ThresholdKind::RunSpeedMps => within(
                raw * LT_SPEED_TO_MPS,
                RUN_SPEED_MIN_MPS,
                RUN_SPEED_MAX_MPS,
                dropped,
            ),
            _ => within(raw, HR_MIN_BPM, HR_MAX_BPM, dropped),
        };
        let Some(value) = value else { continue };
        // Je Tag zählt die letzte Zeile (Garmin liefert mit `aggregationStrategy=LATEST` ohnehin
        // höchstens eine).
        out.retain(|t| t.date != Some(date));
        out.push(GarminThreshold {
            sport: Some(sport),
            kind,
            value,
            date: Some(date),
        });
    }
    out
}

/// VO2max je Tag mit Messung (`maxmet/daily`, I:1683): Liste mit `generic` (Laufen) und
/// `cycling`, je `calendarDate` und `vo2MaxPreciseValue` (sonst `vo2MaxValue`), Q28.
pub fn vo2max_days(v: &Value) -> Vec<(NaiveDate, Option<f64>, Option<f64>)> {
    let Some(items) = v.as_array() else {
        return Vec::new();
    };
    let value = |o: Option<&Value>| {
        let o = o.filter(|o| o.is_object())?;
        field(o, "vo2MaxPreciseValue").or_else(|| field(o, "vo2MaxValue"))
    };
    items
        .iter()
        .filter_map(|i| {
            let generic = i.get("generic");
            let cycling = i.get("cycling");
            let date = generic
                .and_then(|g| g.get("calendarDate"))
                .or_else(|| cycling.and_then(|c| c.get("calendarDate")))
                .and_then(date_of)?;
            Some((date, value(generic), value(cycling)))
        })
        .collect()
}

/// Schlüssel aus Garmins Phrase: Kleinbuchstaben, nur `a-z`, `0-9` und `_`. Mit
/// `strip_number` fällt eine angehängte Nummer weg (`PRODUCTIVE_1` wird `productive`); die
/// Nummer wählt bei Garmin nur einen Hinweistext aus.
fn phrase_key(v: Option<&Value>, strip_number: bool) -> Option<String> {
    let text = v?.as_str()?.trim();
    let text = if strip_number {
        match text.rsplit_once('_') {
            Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => {
                head
            }
            _ => text,
        }
    } else {
        text
    };
    let key = text.to_ascii_lowercase();
    (!key.is_empty()
        && key.len() <= 48
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    .then_some(key)
}

/// Eintrag des Hauptgeräts aus einer Karte Gerätekennung zu Eintrag. Einträge können `null`
/// sein; ohne `primaryTrainingDevice` zählt der erste Eintrag (wie ha-garmin, Q33).
fn primary_device(map: Option<&Value>) -> Option<&Value> {
    let map = map?.as_object()?;
    let entries: Vec<&Value> = map.values().filter(|e| e.is_object()).collect();
    entries
        .iter()
        .find(|e| e.get("primaryTrainingDevice").and_then(Value::as_bool) == Some(true))
        .or_else(|| entries.first())
        .copied()
}

/// Trainingsstatus eines Tages.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TrainingStatus {
    pub date: Option<NaiveDate>,
    pub status: Option<String>,
    pub acute: Option<f64>,
    pub chronic: Option<f64>,
    pub acwr: Option<f64>,
}

/// Trainingsstatus (`trainingstatus/daily`, I:2356; Q28, Q29): `latestTrainingStatusData` je
/// Gerät mit `calendarDate`, `trainingStatusFeedbackPhrase` und `acuteTrainingLoadDTO`
/// (`dailyTrainingLoadAcute`, `dailyTrainingLoadChronic`, `dailyAcuteChronicWorkloadRatio`).
/// Der Status kommt aus der Phrase, nicht aus der Zahl `trainingStatus`: Projekte Dritter deuten
/// die Zahlen widersprüchlich, nur 7 = produktiv ist doppelt belegt. Die Form unter
/// `mostRecentTrainingStatus` (`trainingstatus/aggregated`) wird ebenso gelesen.
pub fn training_status(v: &Value) -> Option<TrainingStatus> {
    let map = v.get("latestTrainingStatusData").or_else(|| {
        v.get("mostRecentTrainingStatus")
            .and_then(|m| m.get("latestTrainingStatusData"))
    });
    let entry = primary_device(map)?;
    let acute = entry.get("acuteTrainingLoadDTO");
    let out = TrainingStatus {
        date: entry.get("calendarDate").and_then(date_of),
        status: phrase_key(entry.get("trainingStatusFeedbackPhrase"), true),
        acute: acute.and_then(|a| field(a, "dailyTrainingLoadAcute")),
        chronic: acute.and_then(|a| field(a, "dailyTrainingLoadChronic")),
        acwr: acute.and_then(|a| field(a, "dailyAcuteChronicWorkloadRatio")),
    };
    (out != TrainingStatus::default()).then_some(out)
}

/// Belastungsfokus der letzten vier Wochen.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LoadBalance {
    /// Aerob niedrig, aerob hoch, anaerob: je Last, Zielminimum, Zielmaximum.
    pub aerobic_low: [Option<f64>; 3],
    pub aerobic_high: [Option<f64>; 3],
    pub anaerobic: [Option<f64>; 3],
    pub phrase: Option<String>,
}

/// Belastungsfokus (`trainingloadbalance/latest`, I:2345; Q28): `metricsTrainingLoadBalanceDTOMap`
/// je Gerät mit `monthlyLoadAerobicLow`, `monthlyLoadAerobicHigh`, `monthlyLoadAnaerobic`, je mit
/// `TargetMin` und `TargetMax`, und `trainingBalanceFeedbackPhrase`.
pub fn load_balance(v: &Value) -> Option<LoadBalance> {
    let map = v.get("metricsTrainingLoadBalanceDTOMap").or_else(|| {
        v.get("mostRecentTrainingLoadBalance")
            .and_then(|m| m.get("metricsTrainingLoadBalanceDTOMap"))
    });
    let e = primary_device(map)?;
    let three = |name: &str| {
        [
            field(e, name),
            field(e, &format!("{name}TargetMin")),
            field(e, &format!("{name}TargetMax")),
        ]
    };
    let out = LoadBalance {
        aerobic_low: three("monthlyLoadAerobicLow"),
        aerobic_high: three("monthlyLoadAerobicHigh"),
        anaerobic: three("monthlyLoadAnaerobic"),
        phrase: phrase_key(e.get("trainingBalanceFeedbackPhrase"), false),
    };
    (out != LoadBalance::default()).then_some(out)
}

/// Rennprognosen eines Tages in Sekunden: 5 km, 10 km, Halbmarathon, Marathon.
pub type RacePrediction = (NaiveDate, [Option<f64>; 4]);

/// Rennprognosen (`racepredictions/latest` und `/daily`, I:2452, I:2469; Q28): ein Objekt oder
/// eine Liste von Objekten mit `calendarDate`, `time5K`, `time10K`, `timeHalfMarathon`,
/// `timeMarathon` in Sekunden.
pub fn race_predictions(v: &Value) -> Vec<RacePrediction> {
    let items: Vec<&Value> = match v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(_) => vec![v],
        _ => Vec::new(),
    };
    items
        .into_iter()
        .filter_map(|i| {
            let date = date_of(i.get("calendarDate")?)?;
            let times = [
                field(i, "time5K"),
                field(i, "time10K"),
                field(i, "timeHalfMarathon"),
                field(i, "timeMarathon"),
            ];
            times.iter().any(Option::is_some).then_some((date, times))
        })
        .collect()
}

/// Lauftoleranz eines Tages.
#[derive(Debug, Clone, PartialEq)]
pub struct RunningTolerance {
    pub date: NaiveDate,
    pub acute_distance_m: Option<f64>,
    pub impact_load: Option<f64>,
    pub tolerance_m: Option<f64>,
    pub level: Option<String>,
}

/// Lauftoleranz (`runningtolerance/stats` mit `aggregation=daily`, I:2394; Q32): Liste mit
/// `calendarDate`, `acuteDistance` und `acuteTolerance` (Meter, gleitend über 7 Tage),
/// `acuteImpactLoad`, `runningToleranceFeedBackPhrase`. Die Reihenfolge ist beliebig.
pub fn running_tolerance(v: &Value) -> Vec<RunningTolerance> {
    let Some(items) = v.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|i| {
            Some(RunningTolerance {
                date: date_of(i.get("calendarDate")?)?,
                acute_distance_m: field(i, "acuteDistance"),
                impact_load: field(i, "acuteImpactLoad"),
                tolerance_m: field(i, "acuteTolerance"),
                level: phrase_key(i.get("runningToleranceFeedBackPhrase"), false),
            })
        })
        .collect()
}

/// Etagen eines Tages (`floorsChartData/daily`, I:1198; Q29): `floorValuesArray` sind
/// Viertelstunden, die Spalten nennt `floorsValueDescriptorDTOList` (`key`, `index`). Gelesen
/// werden die Spalten `floorsAscended` und `floorsDescended` über ihren Index, nicht über eine
/// feste Stelle. Die Antwort hat keine Tagessumme, deshalb die Summe der Zeilen.
pub fn floors(v: &Value) -> Option<(f64, f64)> {
    let descriptors = v.get("floorsValueDescriptorDTOList")?.as_array()?;
    let index_of = |key: &str| {
        descriptors
            .iter()
            .find(|d| d.get("key").and_then(Value::as_str) == Some(key))
            .and_then(|d| d.get("index")?.as_u64())
            .and_then(|i| usize::try_from(i).ok())
    };
    let (up, down) = (index_of("floorsAscended")?, index_of("floorsDescended")?);
    let rows = v.get("floorValuesArray")?.as_array()?;
    let mut sums = (0.0, 0.0);
    let mut any = false;
    for row in rows {
        let Some(row) = row.as_array() else { continue };
        if let Some(x) = row.get(up).and_then(number).filter(|x| *x >= 0.0) {
            sums.0 += x;
            any = true;
        }
        if let Some(x) = row.get(down).and_then(number).filter(|x| *x >= 0.0) {
            sums.1 += x;
            any = true;
        }
    }
    any.then_some(sums)
}

/// Körperwerte eines Tages: Gewicht, BMI, Fett, Wasser, Knochen, Muskeln (kg und Prozent).
pub type BodyValues = [Option<f64>; 6];

/// Gewicht und Körperzusammensetzung (`weight/range`, I:1522; Q28, Q29):
/// `dailyWeightSummaries[].allWeightMetrics[]` (sonst `dateWeightList[]` wie bei
/// `weight/dateRange`) mit `calendarDate`, `date` (Millisekunden), `weight`, `boneMass`,
/// `muscleMass` in Gramm, `bmi`, `bodyFat` und `bodyWater` in Prozent. Je Tag zählt die
/// jüngste Messung.
pub fn body_days(v: &Value) -> Vec<(NaiveDate, BodyValues)> {
    let mut metrics: Vec<&Value> = Vec::new();
    if let Some(days) = v.get("dailyWeightSummaries").and_then(Value::as_array) {
        for day in days {
            if let Some(all) = day.get("allWeightMetrics").and_then(Value::as_array) {
                metrics.extend(all);
            }
        }
    }
    if let Some(list) = v.get("dateWeightList").and_then(Value::as_array) {
        metrics.extend(list);
    }
    let grams = |m: &Value, name: &str| field(m, name).map(|g| g / 1000.0);
    let mut by_day: BTreeMap<NaiveDate, (f64, BodyValues)> = BTreeMap::new();
    for m in metrics {
        let Some(date) = m.get("calendarDate").and_then(date_of) else {
            continue;
        };
        let at = field(m, "date").unwrap_or(0.0);
        let values = [
            grams(m, "weight"),
            field(m, "bmi"),
            field(m, "bodyFat"),
            field(m, "bodyWater"),
            grams(m, "boneMass"),
            grams(m, "muscleMass"),
        ];
        if values.iter().all(Option::is_none) {
            continue;
        }
        match by_day.get(&date) {
            Some((t, _)) if *t > at => {}
            _ => {
                by_day.insert(date, (at, values));
            }
        }
    }
    by_day.into_iter().map(|(d, (_, v))| (d, v)).collect()
}

/// Blutdruck eines Tages: systolisch, diastolisch, Puls.
pub type BpValues = [Option<f64>; 3];

/// Blutdruck (`bloodpressure/range`, I:1644; Q33): `measurementSummaries[].measurements[]` mit
/// `systolic`, `diastolic`, `pulse` und `measurementTimestampLocal`; ohne Zeitstempel gilt
/// `startDate` der Zusammenfassung. Je Tag zählt die jüngste Messung.
pub fn blood_pressure_days(v: &Value) -> Vec<(NaiveDate, BpValues)> {
    let Some(summaries) = v.get("measurementSummaries").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut by_day: BTreeMap<NaiveDate, (String, BpValues)> = BTreeMap::new();
    for summary in summaries {
        let fallback = summary.get("startDate").and_then(date_of);
        let Some(items) = summary.get("measurements").and_then(Value::as_array) else {
            continue;
        };
        for m in items {
            let stamp = m
                .get("measurementTimestampLocal")
                .and_then(Value::as_str)
                .unwrap_or("");
            let Some(date) = m
                .get("measurementTimestampLocal")
                .and_then(date_of)
                .or(fallback)
            else {
                continue;
            };
            let values = [
                field(m, "systolic"),
                field(m, "diastolic"),
                field(m, "pulse"),
            ];
            if values.iter().all(Option::is_none) {
                continue;
            }
            match by_day.get(&date) {
                Some((t, _)) if t.as_str() > stamp => {}
                _ => {
                    by_day.insert(date, (stamp.to_owned(), values));
                }
            }
        }
    }
    by_day.into_iter().map(|(d, (_, v))| (d, v)).collect()
}

/// Ernährung eines Tages: Kalorien, Kohlenhydrate, Fett, Eiweiß, Kalorienziel.
pub type NutritionValues = [Option<f64>; 5];

/// Ernährungstagebuch (`food/logs`, I:4383; Q33): `mealDate`, `dailyNutritionContent` mit
/// `calories`, `carbs`, `fat`, `protein` (Gramm) und `dailyNutritionGoals.calories`.
pub fn nutrition_day(v: &Value) -> Option<(Option<NaiveDate>, NutritionValues)> {
    let content = v.get("dailyNutritionContent");
    let goals = v.get("dailyNutritionGoals");
    let values = [
        content.and_then(|c| field(c, "calories")),
        content.and_then(|c| field(c, "carbs")),
        content.and_then(|c| field(c, "fat")),
        content.and_then(|c| field(c, "protein")),
        goals.and_then(|g| field(g, "calories")),
    ];
    values
        .iter()
        .any(Option::is_some)
        .then(|| (v.get("mealDate").and_then(date_of), values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn profile_reads_sex_year_and_vo2max_but_never_weight() {
        let v = json!({"id": 1, "userData": {
            "gender": "FEMALE", "weight": 61000.0, "height": 170.0,
            "birthDate": "1990-04-12", "vo2MaxRunning": 52.0, "vo2MaxCycling": null
        }});
        let p = profile(&v, d("2026-10-16"));
        assert_eq!(
            p,
            ProfileValues {
                sex: Some(Sex::Female),
                birth_year: Some(1990),
                vo2max_running: Some(52.0),
                vo2max_cycling: None,
            }
        );
        let odd = json!({"userData": {"gender": "UNKNOWN", "birthDate": "2099-01-01"}});
        assert_eq!(profile(&odd, d("2026-10-16")), ProfileValues::default());
        assert_eq!(
            profile(&json!([]), d("2026-10-16")),
            ProfileValues::default()
        );
    }

    #[test]
    fn ftp_needs_a_date_and_a_plausible_value() {
        let mut dropped = Dropped::default();
        let v = json!({"calendarDate": "2026-03-01", "functionalThresholdPower": 251, "sport": "CYCLING"});
        let t = ftp(&v, &mut dropped).unwrap();
        assert_eq!((t.value, t.date), (251.0, Some(d("2026-03-01"))));
        assert_eq!(t.sport, Some(Sport::Cycling));
        assert!(ftp(&json!({"functionalThresholdPower": 251}), &mut dropped).is_none());
        assert!(
            ftp(
                &json!({"calendarDate": "2026-03-01", "functionalThresholdPower": 9000}),
                &mut dropped
            )
            .is_none()
        );
        assert_eq!(dropped.0, 1);
    }

    #[test]
    fn lactate_threshold_takes_the_latest_entry_per_sport() {
        let mut dropped = Dropped::default();
        let v = json!([
            {"calendarDate": "2026-02-01", "speed": 0.35, "heartRate": null},
            {"calendarDate": "2026-02-01", "speed": null, "hearRate": 168},
            {"calendarDate": "2026-01-01", "heartRate": 165, "heartRateCycling": 158},
            {"heartRate": 500}
        ]);
        let t = lactate_threshold(&v, &mut dropped);
        assert_eq!(t.len(), 3);
        let find = |sport, kind| {
            t.iter()
                .find(|t| t.sport == Some(sport) && t.kind == kind)
                .unwrap()
        };
        let run = find(Sport::Running, ThresholdKind::LthrBpm);
        assert_eq!((run.value, run.date), (168.0, Some(d("2026-02-01"))));
        let bike = find(Sport::Cycling, ThresholdKind::LthrBpm);
        assert_eq!((bike.value, bike.date), (158.0, Some(d("2026-01-01"))));
        // 0,35 × 10 = 3,5 m/s (Q30).
        let speed = find(Sport::Running, ThresholdKind::RunSpeedMps);
        assert!((speed.value - 3.5).abs() < 1e-9);
        assert_eq!(speed.date, Some(d("2026-02-01")));
        assert_eq!(dropped.0, 1);

        // Ein Wert, der schon in m/s vorläge (3,36 × 10 = 33,6 m/s), ist unplausibel.
        let mut dropped = Dropped::default();
        let t = lactate_threshold(
            &json!([{"calendarDate": "2026-02-01", "speed": 3.36}]),
            &mut dropped,
        );
        assert!(t.is_empty());
        assert_eq!(dropped.0, 1);
    }

    #[test]
    fn threshold_series_reads_rows_from_their_start_and_skips_other_series() {
        let mut dropped = Dropped::default();
        // Zeilenformat nach Q28, Werte synthetisch.
        let ftp_rows = json!([
            {"from": "2026-03-01", "until": "2026-03-01", "series": "cycling", "value": 240.0, "updatedDate": "2026-03-01"},
            {"from": "2026-06-10", "until": "2026-06-10", "series": "cycling", "value": 255.0, "updatedDate": "2026-06-10"},
            {"from": "2026-06-11", "until": "2026-06-11", "series": "running", "value": 347.0},
            {"from": "2026-07-01", "series": "cycling", "value": 9000.0},
            {"until": "2026-07-02", "value": 250.0}
        ]);
        let t = threshold_series(
            &ftp_rows,
            ThresholdSeries::FunctionalThresholdPower,
            &mut dropped,
        );
        assert_eq!(
            t.iter()
                .map(|t| (t.value, t.date.unwrap()))
                .collect::<Vec<_>>(),
            [(240.0, d("2026-03-01")), (255.0, d("2026-06-10"))]
        );
        assert!(
            t.iter()
                .all(|t| t.kind == ThresholdKind::FtpW && t.sport == Some(Sport::Cycling))
        );
        assert_eq!(dropped.0, 1);

        // Handrechnung: 0,33611017 × 10 = 3,3611017 m/s.
        let speed = json!([{"from": "2026-01-26", "until": "2026-01-26", "series": "running", "value": 0.33611017}]);
        let t = threshold_series(&speed, ThresholdSeries::LactateThresholdSpeed, &mut dropped);
        assert_eq!(t.len(), 1);
        assert!((t[0].value - 3.361_101_7).abs() < 1e-9);
        assert_eq!(t[0].kind, ThresholdKind::RunSpeedMps);

        let hr = json!([{"from": "2026-01-26", "series": "running", "value": 166.0}]);
        let t = threshold_series(
            &hr,
            ThresholdSeries::LactateThresholdHeartRate,
            &mut dropped,
        );
        assert_eq!((t[0].value, t[0].kind), (166.0, ThresholdKind::LthrBpm));
        assert!(
            threshold_series(
                &json!({}),
                ThresholdSeries::LactateThresholdHeartRate,
                &mut dropped
            )
            .is_empty()
        );
    }

    #[test]
    fn vo2max_days_read_generic_and_cycling() {
        let v = json!([
            {"userId": 1, "generic": {"calendarDate": "2026-01-19", "vo2MaxPreciseValue": 50.4, "vo2MaxValue": 50.0}, "cycling": null},
            {"generic": null, "cycling": {"calendarDate": "2026-01-20", "vo2MaxValue": 55.0}},
            {"generic": {"vo2MaxPreciseValue": 49.0}}
        ]);
        assert_eq!(
            vo2max_days(&v),
            [
                (d("2026-01-19"), Some(50.4), None),
                (d("2026-01-20"), None, Some(55.0))
            ]
        );
    }

    #[test]
    fn training_status_uses_the_primary_device_and_the_phrase() {
        // Aufbau nach Q28 (Gerätekennungen synthetisch).
        let v = json!({"userId": 1, "latestTrainingStatusData": {
            "111": null,
            "222": {"calendarDate": "2026-01-27", "trainingStatus": 7, "trainingStatusFeedbackPhrase": "PRODUCTIVE_1",
                    "primaryTrainingDevice": true,
                    "acuteTrainingLoadDTO": {"acwrPercent": 47, "dailyTrainingLoadAcute": 249, "dailyTrainingLoadChronic": 219, "dailyAcuteChronicWorkloadRatio": 1.1}},
            "333": {"calendarDate": "2026-01-27", "trainingStatusFeedbackPhrase": "DETRAINING", "primaryTrainingDevice": false}
        }});
        assert_eq!(
            training_status(&v),
            Some(TrainingStatus {
                date: Some(d("2026-01-27")),
                status: Some("productive".into()),
                acute: Some(249.0),
                chronic: Some(219.0),
                acwr: Some(1.1),
            })
        );
        let aggregated = json!({"mostRecentTrainingStatus": {"latestTrainingStatusData": {
            "9": {"trainingStatusFeedbackPhrase": "MAINTAINING_3"}}}});
        assert_eq!(
            training_status(&aggregated).unwrap().status.as_deref(),
            Some("maintaining")
        );
        let hostile = json!({"latestTrainingStatusData": {"9": {"trainingStatusFeedbackPhrase": "<b>x</b>"}}});
        assert_eq!(training_status(&hostile), None);
        assert_eq!(training_status(&json!(null)), None);
    }

    #[test]
    fn load_balance_reads_three_loads_with_targets() {
        let v = json!({"metricsTrainingLoadBalanceDTOMap": {"222": {
            "monthlyLoadAerobicLow": 187.1, "monthlyLoadAerobicLowTargetMin": 186, "monthlyLoadAerobicLowTargetMax": 400,
            "monthlyLoadAerobicHigh": 209.7, "monthlyLoadAnaerobic": 0.0,
            "trainingBalanceFeedbackPhrase": "AEROBIC_HIGH_SHORTAGE", "primaryTrainingDevice": true}}});
        let b = load_balance(&v).unwrap();
        assert_eq!(b.aerobic_low, [Some(187.1), Some(186.0), Some(400.0)]);
        assert_eq!(b.aerobic_high, [Some(209.7), None, None]);
        assert_eq!(b.anaerobic[0], Some(0.0));
        assert_eq!(b.phrase.as_deref(), Some("aerobic_high_shortage"));
    }

    #[test]
    fn race_predictions_take_object_or_list() {
        let latest = json!({"calendarDate": "2026-01-27", "time5K": 1492, "time10K": 3222, "timeHalfMarathon": 7289, "timeMarathon": 16208});
        assert_eq!(
            race_predictions(&latest),
            [(
                d("2026-01-27"),
                [Some(1492.0), Some(3222.0), Some(7289.0), Some(16208.0)]
            )]
        );
        let daily = json!([{"calendarDate": "2026-01-26", "time5K": 1500}, {"calendarDate": "2026-01-25", "time5K": null}]);
        assert_eq!(race_predictions(&daily).len(), 1);
    }

    #[test]
    fn running_tolerance_reads_rows_in_any_order() {
        let v = json!([
            {"acuteDistance": 34206, "acuteImpactLoad": 36447, "acuteTolerance": 65225, "calendarDate": "2026-09-18", "runningToleranceFeedBackPhrase": "MEDIUM_LOAD"},
            {"calendarDate": "2026-09-17", "acuteDistance": 30000}
        ]);
        let t = running_tolerance(&v);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].tolerance_m, Some(65225.0));
        assert_eq!(t[0].level.as_deref(), Some("medium_load"));
        assert_eq!(t[1].date, d("2026-09-17"));
    }

    #[test]
    fn floors_sum_the_columns_named_by_the_descriptors() {
        // Spalten absichtlich vertauscht: gelesen wird über den Index des Schlüssels.
        let v = json!({
            "floorsValueDescriptorDTOList": [
                {"key": "startTimeGMT", "index": 0}, {"key": "endTimeGMT", "index": 1},
                {"key": "floorsDescended", "index": 2}, {"key": "floorsAscended", "index": 3}
            ],
            "floorValuesArray": [
                ["2026-07-01T06:00:00.0", "2026-07-01T06:15:00.0", 0, 0],
                ["2026-07-01T17:00:00.0", "2026-07-01T17:15:00.0", 1, 3],
                ["2026-07-01T18:00:00.0", "2026-07-01T18:15:00.0", 2, 4]
            ]
        });
        // Handrechnung: auf 0 + 3 + 4 = 7, ab 0 + 1 + 2 = 3.
        assert_eq!(floors(&v), Some((7.0, 3.0)));
        assert_eq!(floors(&json!({"floorValuesArray": []})), None);
    }

    #[test]
    fn body_days_convert_grams_and_keep_the_latest_per_day() {
        let v = json!({"dailyWeightSummaries": [{"summaryDate": "2026-01-27", "allWeightMetrics": [
            {"date": 1000, "calendarDate": "2026-01-27", "weight": 75000.0, "bmi": null},
            {"date": 2000, "calendarDate": "2026-01-27", "weight": 74200.0, "bmi": 22.8, "bodyFat": 19.3, "bodyWater": 58.9, "boneMass": 3539, "muscleMass": 26979}
        ]}]});
        let days = body_days(&v);
        assert_eq!(days.len(), 1);
        // Handrechnung: 74 200 g = 74,2 kg, 3539 g = 3,539 kg, 26 979 g = 26,979 kg.
        assert_eq!(
            days[0].1,
            [
                Some(74.2),
                Some(22.8),
                Some(19.3),
                Some(58.9),
                Some(3.539),
                Some(26.979)
            ]
        );
        let list = json!({"dateWeightList": [{"calendarDate": "2026-01-20", "weight": 70000.0}]});
        assert_eq!(body_days(&list)[0].1[0], Some(70.0));
    }

    #[test]
    fn blood_pressure_keeps_the_latest_measurement_per_day() {
        let v = json!({"measurementSummaries": [{"startDate": "2026-02-01", "measurements": [
            {"systolic": 130, "diastolic": 85, "pulse": 70, "measurementTimestampLocal": "2026-02-01T07:00:00.0"},
            {"systolic": 121, "diastolic": 79, "pulse": 62, "measurementTimestampLocal": "2026-02-01T21:00:00.0"},
            {"systolic": 118, "diastolic": 77}
        ]}]});
        assert_eq!(
            blood_pressure_days(&v),
            [(d("2026-02-01"), [Some(121.0), Some(79.0), Some(62.0)])]
        );
    }

    #[test]
    fn nutrition_reads_totals_and_goal() {
        let v = json!({"mealDate": "2026-02-01", "dailyNutritionContent": {"calories": 2400, "carbs": 300, "fat": 80, "protein": 120},
                       "dailyNutritionGoals": {"calories": 2600}, "mealDetails": []});
        assert_eq!(
            nutrition_day(&v),
            Some((
                Some(d("2026-02-01")),
                [
                    Some(2400.0),
                    Some(300.0),
                    Some(80.0),
                    Some(120.0),
                    Some(2600.0)
                ]
            ))
        );
        assert_eq!(nutrition_day(&json!({"mealDetails": []})), None);
    }

    #[test]
    fn zones_give_one_value_for_all_or_one_per_known_profile() {
        let mut dropped = Dropped::default();
        let same = json!([
            {"sport": "DEFAULT", "maxHeartRateUsed": 190, "restingHeartRateUsed": 48},
            {"sport": "RUNNING", "maxHeartRateUsed": 190, "restingHeartRateUsed": 48}
        ]);
        let t = heart_rate_zones(&same, &mut dropped);
        assert_eq!(t.len(), 2);
        assert!(t.iter().all(|t| t.sport.is_none() && t.date.is_none()));

        let differ = json!([
            {"sport": "DEFAULT", "maxHeartRateUsed": 190, "restingHeartRateUsed": 48},
            {"sport": "CYCLING", "maxHeartRateUsed": 182, "restingHeartRateUsed": 48},
            {"sport": "ROWING", "maxHeartRateUsed": 175, "restingHeartRateUsed": 48}
        ]);
        let t = heart_rate_zones(&differ, &mut dropped);
        let max: Vec<_> = t
            .iter()
            .filter(|t| t.kind == ThresholdKind::HrMaxBpm)
            .map(|t| (t.sport, t.value))
            .collect();
        assert_eq!(max, [(None, 190.0), (Some(Sport::Cycling), 182.0)]);
        assert!(heart_rate_zones(&json!({}), &mut dropped).is_empty());
    }

    #[test]
    fn readiness_prefers_the_morning_entry() {
        let v = json!([
            {"calendarDate": "2026-10-01", "score": 40, "level": "LOW", "acuteLoad": 300, "inputContext": "UPDATE_REALTIME_VARIABLES"},
            {"calendarDate": "2026-10-01", "score": 72, "level": "HIGH", "acuteLoad": 280, "inputContext": "AFTER_WAKEUP_RESET"}
        ]);
        assert_eq!(
            training_readiness(&v),
            Some((Some(72.0), Some("HIGH".into()), Some(280.0)))
        );
        assert_eq!(training_readiness(&json!([])), None);
    }

    #[test]
    fn scores_steps_and_loads_are_read_by_date() {
        let e = json!({"avg": 5000, "enduranceScoreDTO": {"calendarDate": "2026-10-07", "overallScore": 5321}});
        assert_eq!(endurance_score(&e), Some((d("2026-10-07"), 5321.0)));
        let h = json!({"periodAvgScore": {"2026-10-07": 61, "kaputt": 3, "2026-10-08": null}});
        assert_eq!(hill_scores(&h), vec![(d("2026-10-07"), 61.0)]);
        let s = json!([{"calendarDate": "2026-10-07", "totalSteps": 9000, "totalDistance": 7100, "stepGoal": 8000}]);
        assert_eq!(
            steps_days(&s),
            vec![(d("2026-10-07"), Some(9000.0), Some(7100.0))]
        );
        let l = json!([
            {"activityId": 1001, "activityTrainingLoad": 112.5, "trainingEffectLabel": "AEROBIC_BASE", "startLocal": "2026-10-07 06:30:00"},
            {"activityId": 0, "startLocal": "2026-10-07 06:30:00"}
        ]);
        assert_eq!(
            training_loads(&l),
            vec![(
                1001,
                d("2026-10-07"),
                Some(112.5),
                Some("AEROBIC_BASE".into())
            )]
        );
    }
}
