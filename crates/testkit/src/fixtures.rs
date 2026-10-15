//! Feste Fixture-Sammlung mit Erwartungen (SPEC 11.3, 12 M1).
//!
//! [`all`] liefert alle Fixtures im Speicher, in fester Reihenfolge und bei jedem Aufruf mit
//! denselben Bytes. [`manifest_json`] beschreibt dieselben Fixtures für den Dateiexport durch
//! `gen-fixtures`. Beides entsteht aus derselben Quelle, es gibt keine zweite Liste.

use serde::{Deserialize, Serialize};

use crate::generators::{
    ActivityParams, Anomaly, Gap, HrProfile, MonitoringParams, PowerProfile, Sport, WATCH,
    activity, corrupt_crc, monitoring, truncate,
};
use crate::profile::fit_to_unix;

/// Fester Startzeitpunkt aller Fixtures: 10. Mai 2026 06:00:00 UTC.
///
/// Rechenweg: 1970-01-01 bis 2026-01-01 sind 56 Jahre mit 14 Schaltjahren (1972 bis 2024),
/// also `56 * 365 + 14 = 20454` Tage; bis zum 10. Mai kommen `31 + 28 + 31 + 30 + 9 = 129`
/// Tage dazu, zusammen 20583 Tage. `20583 * 86400 + 6 * 3600 = 1 778 392 800` Unix-Sekunden,
/// abzüglich der FIT-Epoche 631 065 600 ergibt 1 147 327 200.
pub const START_FIT: u32 = 1_147_327_200;

/// Unix-Zeit des Startzeitpunkts, siehe [`START_FIT`].
pub const START_UNIX: i64 = 1_778_392_800;

/// Lokaler Zeitversatz aller Fixtures: Mitteleuropäische Sommerzeit, UTC+2.
pub const LOCAL_OFFSET_S: i32 = 7200;

/// Zeitversatz der Uhr gegenüber dem Radcomputer im Edge/Uhr-Paar in Sekunden.
pub const PAIR_OFFSET_S: u32 = 20;

/// Name des Edge/Uhr-Paars in [`Expectation::pair`].
pub const PAIR_RIDE: &str = "ride_pair";

/// Art der Datei.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    Activity,
    Monitoring,
    Empty,
    Corrupt,
}

/// Erwarteter Fehlercode des Parsers (SPEC 5.4, `FitError`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    CrcMismatch,
    Truncated,
    EmptyFile,
    NoActivity,
}

/// Sportart, als Zeichenkette im Manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SportName {
    Cycling,
    Running,
    Swimming,
    Strength,
}

impl From<Sport> for SportName {
    fn from(s: Sport) -> Self {
        match s {
            Sport::Cycling => SportName::Cycling,
            Sport::Running => SportName::Running,
            Sport::Swimming => SportName::Swimming,
            Sport::Strength => SportName::Strength,
        }
    }
}

/// Was T03 (Parser) und T04 (Import) für ein Fixture erwarten dürfen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expectation {
    pub kind: FixtureKind,
    /// Erwartete Anzahl Aktivitäten nach dem Einlesen, 0 oder 1.
    pub activities: u8,
    /// Erwarteter Fehlercode, `None` bei lesbaren Dateien.
    pub error: Option<ErrorCode>,
    /// Paarzugehörigkeit für die Duplikatregel (SPEC 6.4), zum Beispiel Edge und Uhr.
    pub pair: Option<String>,
    /// Sportart laut `session`. `None`, wenn die Datei sie nicht enthält (Fixture `no_session`).
    pub sport: Option<SportName>,
    /// Dauer laut `session.total_elapsed_time` in Sekunden.
    pub duration_s: Option<u32>,
    pub has_power: bool,
    pub has_heart_rate: bool,
    pub has_session: bool,
    /// Anzahl der `record`-Nachrichten in der Datei.
    pub record_count: u32,
    /// Startzeit in Unix-Sekunden.
    pub start_unix_s: Option<i64>,
    /// Lokaler Zeitversatz in Sekunden aus `activity.local_timestamp`.
    pub local_offset_s: Option<i32>,
    /// Beckenlänge in Metern, nur Schwimmen.
    pub pool_length_m: Option<f32>,
    /// Summe der Sekunden ohne `record`-Nachricht innerhalb der Dauer.
    pub gap_seconds: u32,
    /// Anzahl Samples mit Leistung über 2500 W (SPEC 6.5).
    pub power_outliers: u32,
    /// Anzahl Samples mit Herzfrequenz unter 30 oder über 230 bpm (SPEC 6.5).
    pub heart_rate_outliers: u32,
}

impl Expectation {
    /// Erwartung einer lesbaren Aktivitätsdatei aus ihren Parametern.
    fn for_activity(p: &ActivityParams, pair: Option<&str>) -> Self {
        let power_outliers = p
            .anomalies
            .iter()
            .filter(|a| matches!(a, Anomaly::Power { watts, .. } if *watts > 2500))
            .count() as u32;
        let heart_rate_outliers = p
            .anomalies
            .iter()
            .filter(|a| matches!(a, Anomaly::HeartRate { bpm, .. } if *bpm < 30 || *bpm > 230))
            .count() as u32;
        Self {
            kind: FixtureKind::Activity,
            activities: 1,
            error: None,
            pair: pair.map(str::to_owned),
            // Die Sportart steht nur in `session`. Ohne `session` (und ohne `sport`-Nachricht)
            // weiß kein Leser sie, die Erwartung ist dann `None` (Änderung in T03).
            sport: p.with_session.then(|| p.sport.into()),
            duration_s: Some(p.duration_s),
            has_power: p.power != PowerProfile::None,
            has_heart_rate: p.heart_rate != HrProfile::None,
            has_session: p.with_session,
            record_count: p.record_count(),
            start_unix_s: Some(fit_to_unix(p.start_fit)),
            local_offset_s: Some(p.local_offset_s),
            pool_length_m: p.pool_length_m,
            gap_seconds: p.duration_s - p.record_count(),
            power_outliers,
            heart_rate_outliers,
        }
    }

    /// Erwartung einer Datei ohne Aktivität (Monitoring, leer, beschädigt).
    fn failing(kind: FixtureKind, error: ErrorCode, record_count: u32) -> Self {
        Self {
            kind,
            activities: 0,
            error: Some(error),
            pair: None,
            sport: None,
            duration_s: None,
            has_power: false,
            has_heart_rate: false,
            has_session: false,
            record_count,
            start_unix_s: None,
            local_offset_s: None,
            pool_length_m: None,
            gap_seconds: 0,
            power_outliers: 0,
            heart_rate_outliers: 0,
        }
    }
}

/// Ein Fixture: Name ohne Endung, Bytes der Datei, Erwartung.
#[derive(Debug, Clone, PartialEq)]
pub struct Fixture {
    pub name: String,
    pub bytes: Vec<u8>,
    pub expect: Expectation,
}

impl Fixture {
    fn activity(name: &str, params: ActivityParams) -> Self {
        Self::activity_paired(name, params, None)
    }

    fn activity_paired(name: &str, params: ActivityParams, pair: Option<&str>) -> Self {
        Self {
            name: name.to_owned(),
            bytes: activity(&params),
            expect: Expectation::for_activity(&params, pair),
        }
    }

    /// Dateiname im Ausgabeordner von `gen-fixtures`.
    pub fn file_name(&self) -> String {
        format!("{}.fit", self.name)
    }
}

/// Grundfahrt mit dem Radcomputer: 1800 s, 200 W, 140 bpm, 8 m/s, am Starttag.
fn ride(duration_s: u32) -> ActivityParams {
    ride_on_day(0, duration_s)
}

/// Grundfahrt wie [`ride`], aber `day` Tage nach dem Starttag.
///
/// Jedes Fixture außer dem Edge/Uhr-Paar bekommt einen eigenen Tag. Sonst erkennt die
/// Duplikatregel (SPEC 6.4: gleiche Sportart, Start innerhalb 120 s, Dauer innerhalb 10
/// Prozent) die gleichzeitig startenden Fahrten untereinander als eine Fahrt, und ein
/// Import des ganzen Ordners ergäbe nicht je Fixture eine primäre Aktivität (T04).
fn ride_on_day(day: u32, duration_s: u32) -> ActivityParams {
    ActivityParams::cycling(
        START_FIT + day * SECONDS_PER_DAY,
        LOCAL_OFFSET_S,
        duration_s,
    )
}

const SECONDS_PER_DAY: u32 = 86_400;

/// Alle Fixtures in fester Reihenfolge. Zweimaliger Aufruf liefert identische Bytes.
pub fn all() -> Vec<Fixture> {
    // T-NP-1: 3600 s konstant 200 W
    let mut v = vec![Fixture::activity(
        "power_constant_200w_3600s",
        ride_on_day(1, 3600),
    )];

    // T-NP-2: Wechsel 30 s 100 W und 30 s 300 W, lange Dauer
    v.push(Fixture::activity(
        "power_intervals_100w_300w_30s",
        ActivityParams {
            power: PowerProfile::Intervals {
                low: 100,
                high: 300,
                period_s: 30,
            },
            ..ride_on_day(2, 3600)
        },
    ));

    v.push(Fixture::activity(
        "power_ramp_100w_300w",
        ActivityParams {
            power: PowerProfile::Ramp {
                start: 100,
                end: 300,
            },
            ..ride_on_day(3, 1800)
        },
    ));

    v.push(Fixture::activity(
        "hr_drift_130_160",
        ActivityParams {
            power: PowerProfile::Constant(180),
            heart_rate: HrProfile::Drift {
                start: 130,
                end: 160,
            },
            ..ride_on_day(4, 3600)
        },
    ));

    // Lücken bis 10 s: Werte halten (SPEC 6.5)
    v.push(Fixture::activity(
        "gaps_short",
        ActivityParams {
            gaps: vec![
                Gap {
                    at_s: 300,
                    len_s: 5,
                },
                Gap {
                    at_s: 900,
                    len_s: 10,
                },
            ],
            ..ride_on_day(5, 1800)
        },
    ));

    // Lücke über 10 s: Pause (SPEC 6.5)
    v.push(Fixture::activity(
        "gaps_long",
        ActivityParams {
            gaps: vec![Gap {
                at_s: 600,
                len_s: 60,
            }],
            ..ride(1800)
        },
    ));

    // Spitzen und Aussetzer über den Grenzen aus SPEC 6.5
    v.push(Fixture::activity(
        "spikes_dropouts",
        ActivityParams {
            anomalies: vec![
                Anomaly::Power {
                    at_s: 600,
                    watts: 3000,
                },
                Anomaly::HeartRate { at_s: 700, bpm: 0 },
                Anomaly::HeartRate {
                    at_s: 800,
                    bpm: 250,
                },
            ],
            ..ride_on_day(6, 1800)
        },
    ));

    // Edge/Uhr-Paar: Uhr startet 20 s später, dauert 30 s kürzer, ohne Leistung (SPEC 6.4)
    v.push(Fixture::activity_paired(
        "pair_edge",
        ride_on_day(7, 3600),
        Some(PAIR_RIDE),
    ));
    v.push(Fixture::activity_paired(
        "pair_watch",
        ActivityParams {
            start_fit: START_FIT + 7 * SECONDS_PER_DAY + PAIR_OFFSET_S,
            power: PowerProfile::None,
            device: WATCH,
            ..ride(3570)
        },
        Some(PAIR_RIDE),
    ));

    v.push(Fixture::activity(
        "no_power",
        ActivityParams {
            power: PowerProfile::None,
            device: WATCH,
            ..ride_on_day(8, 1800)
        },
    ));

    v.push(Fixture::activity(
        "no_heart_rate",
        ActivityParams {
            heart_rate: HrProfile::None,
            ..ride_on_day(9, 1800)
        },
    ));

    v.push(Fixture::activity(
        "no_session",
        ActivityParams {
            with_session: false,
            ..ride(1800)
        },
    ));

    v.push(Fixture::activity(
        "run_speed",
        ActivityParams {
            sport: Sport::Running,
            power: PowerProfile::None,
            heart_rate: HrProfile::Constant(150),
            speed_mps: Some(3.5),
            device: WATCH,
            ..ride(2400)
        },
    ));

    v.push(Fixture::activity(
        "swim_pool_25m",
        ActivityParams {
            sport: Sport::Swimming,
            power: PowerProfile::None,
            heart_rate: HrProfile::None,
            speed_mps: Some(1.2),
            pool_length_m: Some(25.0),
            device: WATCH,
            ..ride(1800)
        },
    ));

    v.push(Fixture::activity(
        "strength",
        ActivityParams {
            sport: Sport::Strength,
            power: PowerProfile::None,
            heart_rate: HrProfile::Constant(110),
            speed_mps: None,
            device: WATCH,
            ..ride(2700)
        },
    ));

    // Beschädigt: falsche Prüfsumme
    let base = ride(600);
    v.push(Fixture {
        name: "corrupt_crc".to_owned(),
        bytes: corrupt_crc(activity(&base)),
        expect: Expectation::failing(FixtureKind::Corrupt, ErrorCode::CrcMismatch, 600),
    });

    // Beschädigt: nach 60 Prozent der Länge abgeschnitten
    v.push(Fixture {
        name: "corrupt_truncated".to_owned(),
        bytes: truncate(activity(&base), 600),
        expect: Expectation::failing(FixtureKind::Corrupt, ErrorCode::Truncated, 0),
    });

    // Monitoring: Tagesdaten ohne Aktivität
    v.push(Fixture {
        name: "monitoring".to_owned(),
        bytes: monitoring(&MonitoringParams {
            start_fit: START_FIT,
            samples: 96,
            interval_s: 900,
            heart_rate_bpm: 55,
            device: WATCH,
        }),
        expect: Expectation::failing(FixtureKind::Monitoring, ErrorCode::NoActivity, 0),
    });

    // Leer: 0 Byte
    v.push(Fixture {
        name: "empty".to_owned(),
        bytes: Vec::new(),
        expect: Expectation::failing(FixtureKind::Empty, ErrorCode::EmptyFile, 0),
    });

    v
}

/// Liefert das Fixture mit diesem Namen.
pub fn by_name(name: &str) -> Option<Fixture> {
    all().into_iter().find(|f| f.name == name)
}

/// Eintrag im Manifest: Name, Dateiname, Größe und Erwartung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub name: String,
    pub file: String,
    pub size_bytes: usize,
    pub expect: Expectation,
}

/// Inhalt von `manifest.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Versionsnummer des Manifestformats, steigt bei unverträglichen Änderungen.
    pub version: u32,
    pub fixtures: Vec<ManifestEntry>,
}

/// Aktuelle Version des Manifestformats.
pub const MANIFEST_VERSION: u32 = 1;

impl Manifest {
    pub fn from_fixtures(fixtures: &[Fixture]) -> Self {
        Self {
            version: MANIFEST_VERSION,
            fixtures: fixtures
                .iter()
                .map(|f| ManifestEntry {
                    name: f.name.clone(),
                    file: f.file_name(),
                    size_bytes: f.bytes.len(),
                    expect: f.expect.clone(),
                })
                .collect(),
        }
    }
}

/// `manifest.json` als Text: eingerückt, Schlüssel alphabetisch sortiert, Fixtures in der
/// Reihenfolge von [`all`], abschließender Zeilenumbruch.
pub fn manifest_json(fixtures: &[Fixture]) -> String {
    let value = serde_json::to_value(Manifest::from_fixtures(fixtures))
        .expect("Manifest ist serialisierbar");
    let mut s = serde_json::to_string_pretty(&value).expect("JSON-Ausgabe");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::crc16;
    use crate::profile::{self, FIELD_TIMESTAMP};
    use crate::readback::{Decoded, decode};
    use std::collections::HashSet;

    fn decoded(name: &str) -> Decoded {
        let f = by_name(name).expect("Fixture vorhanden");
        decode(&f.bytes).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    #[test]
    fn start_constants_match_hand_calculation() {
        assert_eq!(fit_to_unix(START_FIT), START_UNIX);
        // 20583 Tage plus 6 Stunden
        assert_eq!(START_UNIX, 20583 * 86_400 + 6 * 3600);
    }

    #[test]
    fn names_are_unique_and_file_safe() {
        let names: Vec<String> = all().into_iter().map(|f| f.name).collect();
        let set: HashSet<&String> = names.iter().collect();
        assert_eq!(set.len(), names.len(), "doppelte Fixture-Namen");
        for n in &names {
            assert!(
                n.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "Name {n:?} enthält unzulässige Zeichen"
            );
        }
    }

    #[test]
    fn all_is_deterministic() {
        let a = all();
        let b = all();
        assert_eq!(a, b);
        assert_eq!(manifest_json(&a), manifest_json(&b));
    }

    #[test]
    fn readable_fixtures_pass_crc_and_match_record_count() {
        for f in all() {
            if f.expect.kind == FixtureKind::Corrupt || f.expect.kind == FixtureKind::Empty {
                continue;
            }
            let d = decode(&f.bytes).unwrap_or_else(|e| panic!("{}: {e}", f.name));
            assert!(d.header_crc_ok, "{}: Kopfprüfsumme", f.name);
            assert!(d.file_crc_ok, "{}: Dateiprüfsumme", f.name);
            assert_eq!(
                d.count(profile::MESG_RECORD),
                f.expect.record_count as usize,
                "{}: Anzahl record",
                f.name
            );
            assert_eq!(d.count(profile::MESG_FILE_ID), 1, "{}: file_id", f.name);
            assert_eq!(
                d.count(profile::MESG_DEVICE_INFO),
                1,
                "{}: device_info",
                f.name
            );
            let sessions = usize::from(f.expect.has_session);
            assert_eq!(
                d.count(profile::MESG_SESSION),
                sessions,
                "{}: session",
                f.name
            );
            assert_eq!(d.count(profile::MESG_LAP), sessions, "{}: lap", f.name);
            let activities = usize::from(f.expect.kind == FixtureKind::Activity);
            assert_eq!(
                d.count(profile::MESG_ACTIVITY),
                activities,
                "{}: activity",
                f.name
            );
        }
    }

    #[test]
    fn activity_fixtures_carry_expected_channels_and_times() {
        for f in all() {
            if f.expect.kind != FixtureKind::Activity {
                continue;
            }
            let d = decode(&f.bytes).unwrap();
            let first = d.first(profile::MESG_RECORD).expect("record vorhanden");
            assert_eq!(
                first.field(profile::record::POWER).is_some(),
                f.expect.has_power,
                "{}: Leistung",
                f.name
            );
            assert_eq!(
                first.field(profile::record::HEART_RATE).is_some(),
                f.expect.has_heart_rate,
                "{}: Herzfrequenz",
                f.name
            );
            let start_fit =
                u32::try_from(f.expect.start_unix_s.unwrap() - profile::FIT_EPOCH_OFFSET_S)
                    .unwrap();
            assert_eq!(first.field(FIELD_TIMESTAMP), Some(u64::from(start_fit)));

            let act = d.first(profile::MESG_ACTIVITY).expect("activity vorhanden");
            let ts = act.field(FIELD_TIMESTAMP).unwrap();
            let local = act.field(profile::activity::LOCAL_TIMESTAMP).unwrap();
            assert_eq!(
                local as i64 - ts as i64,
                i64::from(f.expect.local_offset_s.unwrap()),
                "{}: lokaler Versatz",
                f.name
            );

            if let Some(duration) = f.expect.duration_s.filter(|_| f.expect.has_session) {
                let s = d.first(profile::MESG_SESSION).unwrap();
                assert_eq!(
                    s.field(profile::session::TOTAL_ELAPSED_TIME),
                    Some(u64::from(duration) * 1000),
                    "{}: Dauer",
                    f.name
                );
                assert_eq!(
                    s.field(profile::session::START_TIME),
                    Some(u64::from(start_fit))
                );
            }
        }
    }

    #[test]
    fn constant_power_fixture_holds_200w_everywhere() {
        let d = decoded("power_constant_200w_3600s");
        let powers: Vec<u64> = d
            .messages
            .iter()
            .filter(|m| m.global == profile::MESG_RECORD)
            .map(|m| m.field(profile::record::POWER).unwrap())
            .collect();
        assert_eq!(powers.len(), 3600);
        assert!(powers.iter().all(|&p| p == 200));
    }

    #[test]
    fn intervals_fixture_alternates_every_30s() {
        let d = decoded("power_intervals_100w_300w_30s");
        let powers: Vec<u64> = d
            .messages
            .iter()
            .filter(|m| m.global == profile::MESG_RECORD)
            .map(|m| m.field(profile::record::POWER).unwrap())
            .collect();
        assert_eq!(powers[0], 100);
        assert_eq!(powers[29], 100);
        assert_eq!(powers[30], 300);
        assert_eq!(powers[59], 300);
        assert_eq!(powers[60], 100);
        // Mittelwert exakt 200 W, da gleich viele Intervalle beider Stufen
        let sum: u64 = powers.iter().sum();
        assert_eq!(sum, 200 * 3600);
    }

    #[test]
    fn spikes_fixture_contains_outliers_at_known_seconds() {
        let f = by_name("spikes_dropouts").unwrap();
        assert_eq!(f.expect.power_outliers, 1);
        assert_eq!(f.expect.heart_rate_outliers, 2);
        let d = decode(&f.bytes).unwrap();
        let records: Vec<_> = d
            .messages
            .iter()
            .filter(|m| m.global == profile::MESG_RECORD)
            .collect();
        assert_eq!(records[600].field(profile::record::POWER), Some(3000));
        assert_eq!(records[700].field(profile::record::HEART_RATE), Some(0));
        assert_eq!(records[800].field(profile::record::HEART_RATE), Some(250));
        assert_eq!(records[801].field(profile::record::HEART_RATE), Some(140));
    }

    #[test]
    fn gap_fixtures_skip_seconds() {
        let short = by_name("gaps_short").unwrap();
        assert_eq!(short.expect.gap_seconds, 15);
        assert_eq!(short.expect.record_count, 1785);
        let long = by_name("gaps_long").unwrap();
        assert_eq!(long.expect.gap_seconds, 60);
        let d = decode(&long.bytes).unwrap();
        let stamps: Vec<u64> = d
            .messages
            .iter()
            .filter(|m| m.global == profile::MESG_RECORD)
            .map(|m| m.field(FIELD_TIMESTAMP).unwrap())
            .collect();
        // Sekunde 599 ist da, danach springt der Zeitstempel um 61 auf Sekunde 660
        assert_eq!(stamps[599], u64::from(START_FIT) + 599);
        assert_eq!(stamps[600], u64::from(START_FIT) + 660);
    }

    #[test]
    fn pair_has_offset_and_similar_duration() {
        let edge = by_name("pair_edge").unwrap().expect;
        let watch = by_name("pair_watch").unwrap().expect;
        assert_eq!(edge.pair.as_deref(), Some(PAIR_RIDE));
        assert_eq!(watch.pair.as_deref(), Some(PAIR_RIDE));
        assert_eq!(
            watch.start_unix_s.unwrap() - edge.start_unix_s.unwrap(),
            i64::from(PAIR_OFFSET_S)
        );
        assert!(edge.has_power);
        assert!(!watch.has_power);
        // Dauer weicht um 30 s ab, unter 10 Prozent (SPEC 6.4)
        let (de, dw) = (edge.duration_s.unwrap(), watch.duration_s.unwrap());
        assert!(de.abs_diff(dw) * 10 < de);
    }

    #[test]
    fn swim_fixture_has_pool_length() {
        let d = decoded("swim_pool_25m");
        let s = d.first(profile::MESG_SESSION).unwrap();
        assert_eq!(s.field(profile::session::POOL_LENGTH), Some(2500));
        assert_eq!(
            s.field(profile::session::SPORT),
            Some(u64::from(profile::sport::SWIMMING))
        );
    }

    #[test]
    fn corrupt_crc_fails_checksum_but_keeps_length() {
        let f = by_name("corrupt_crc").unwrap();
        let d = decode(&f.bytes).unwrap();
        assert!(d.header_crc_ok);
        assert!(!d.file_crc_ok);
        let stored = u16::from_le_bytes([f.bytes[f.bytes.len() - 2], f.bytes[f.bytes.len() - 1]]);
        assert_ne!(stored, crc16(&f.bytes[..f.bytes.len() - 2]));
    }

    #[test]
    fn corrupt_truncated_is_shorter_than_data_size() {
        let f = by_name("corrupt_truncated").unwrap();
        let data_size = u32::from_le_bytes([f.bytes[4], f.bytes[5], f.bytes[6], f.bytes[7]]);
        assert!(f.bytes.len() < 14 + data_size as usize + 2);
        assert!(decode(&f.bytes).is_err());
    }

    #[test]
    fn monitoring_has_no_activity_messages() {
        let d = decoded("monitoring");
        assert_eq!(d.count(profile::MESG_MONITORING), 96);
        assert_eq!(d.count(profile::MESG_SESSION), 0);
        assert_eq!(d.count(profile::MESG_ACTIVITY), 0);
        assert_eq!(d.count(profile::MESG_RECORD), 0);
        let id = d.first(profile::MESG_FILE_ID).unwrap();
        assert_eq!(
            id.field(profile::file_id::TYPE),
            Some(u64::from(profile::file_id::FILE_TYPE_MONITORING_B))
        );
    }

    #[test]
    fn empty_fixture_has_no_bytes() {
        let f = by_name("empty").unwrap();
        assert!(f.bytes.is_empty());
        assert_eq!(f.expect.error, Some(ErrorCode::EmptyFile));
    }

    #[test]
    fn manifest_round_trips_and_sorts_keys() {
        let fixtures = all();
        let text = manifest_json(&fixtures);
        let parsed: Manifest = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, Manifest::from_fixtures(&fixtures));
        assert_eq!(parsed.fixtures.len(), fixtures.len());
        // Erste beiden Schlüssel der obersten Ebene sind alphabetisch sortiert
        let fixtures_pos = text.find("\"fixtures\"").unwrap();
        let version_pos = text.find("\"version\"").unwrap();
        assert!(fixtures_pos < version_pos);
        assert!(text.ends_with('\n'));
    }
}
