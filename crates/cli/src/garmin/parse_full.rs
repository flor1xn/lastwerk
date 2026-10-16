//! Werte aus den Antworten des Vollabzugs lesen (T39, SPEC 5.6, 8.6).
//!
//! Wie in [`super::parse`]: Die Antworten sind nicht vertrauenswürdig. Jede Funktion liest nur
//! die benannten Felder und übergeht alles Unerwartete. Die Grenzen der Tageswerte prüft der
//! Speicher (`DayRecord::sanitize`), die der Schwellen diese Datei (SPEC 6.5).
//!
//! Feldnamen nach dem Bericht der Sonde (`docs/garmin-feldabdeckung.md`, Abschnitt in Klammern)
//! und dem Quelltext von python-garminconnect 0.3.17 (Q27, Zeile `I:`). Die Werte selbst hat die
//! Sonde nicht erfasst; Annahmen zu Werten stehen beim Feld und in SPEC 14 O32.

use chrono::{Datelike, NaiveDate};
use lastwerk_core::{Sex, Sport};
use lastwerk_store::ThresholdKind;
use serde_json::Value;

use super::parse::{Dropped, date_of, number};

/// Höchste plausible Leistung (SPEC 6.5, Standard 2500 W).
const FTP_MAX_W: f64 = 2500.0;
/// Herzfrequenzgrenzen (SPEC 6.5).
const HR_MIN_BPM: f64 = 30.0;
const HR_MAX_BPM: f64 = 230.0;

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
        for (sport, bpm) in [(Sport::Running, running), (Sport::Cycling, cycling)] {
            let Some(bpm) = bpm.and_then(|b| within(b, HR_MIN_BPM, HR_MAX_BPM, dropped)) else {
                continue;
            };
            let candidate = GarminThreshold {
                sport: Some(sport),
                kind: ThresholdKind::LthrBpm,
                value: bpm,
                date: Some(date),
            };
            match best.iter_mut().find(|t| t.sport == Some(sport)) {
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
        assert_eq!(t.len(), 2);
        let run = t.iter().find(|t| t.sport == Some(Sport::Running)).unwrap();
        assert_eq!((run.value, run.date), (168.0, Some(d("2026-02-01"))));
        let bike = t.iter().find(|t| t.sport == Some(Sport::Cycling)).unwrap();
        assert_eq!((bike.value, bike.date), (158.0, Some(d("2026-01-01"))));
        assert_eq!(dropped.0, 1);
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
