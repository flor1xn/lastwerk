//! Generatoren für synthetische FIT-Dateien (SPEC 11.3).
//!
//! Jeder Generator ist eine reine Funktion seiner Parameter: keine Systemzeit, kein Zufall.
//! Startzeiten sind feste FIT-Zeitstempel, Anomalien stehen explizit mit Zeitpunkt und Wert in
//! den Parametern. Dieselben Parameter ergeben immer dieselben Bytes.

use crate::encoder::{BaseType, FieldDef, FitWriter, Value};
use crate::profile::{self, FIELD_TIMESTAMP};

/// Sportart der Fixtures. Spiegelt `Sport` aus SPEC 5.4 ohne `Other`, damit das Testkit
/// unabhängig von `lastwerk-core` bleibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sport {
    Cycling,
    Running,
    Swimming,
    Strength,
}

impl Sport {
    /// Werte der Profil-Enums `sport` und `sub_sport` für diese Sportart.
    pub fn codes(self) -> (u8, u8) {
        match self {
            Sport::Cycling => (profile::sport::CYCLING, profile::sub_sport::ROAD),
            Sport::Running => (profile::sport::RUNNING, profile::sub_sport::GENERIC),
            Sport::Swimming => (profile::sport::SWIMMING, profile::sub_sport::LAP_SWIMMING),
            Sport::Strength => (
                profile::sport::TRAINING,
                profile::sub_sport::STRENGTH_TRAINING,
            ),
        }
    }
}

/// Aufzeichnendes Gerät. Hersteller ist Garmin, die Produktnummern sind frei erfunden und
/// entsprechen keinem realen Gerät.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    pub manufacturer: u16,
    pub product: u16,
    pub serial: u32,
}

/// Fiktiver Radcomputer mit Leistungsmesser.
pub const EDGE: Device = Device {
    manufacturer: profile::manufacturer::GARMIN,
    product: 4242,
    serial: 3_900_000_001,
};

/// Fiktive Uhr ohne Leistungsmesser.
pub const WATCH: Device = Device {
    manufacturer: profile::manufacturer::GARMIN,
    product: 4343,
    serial: 3_900_000_002,
};

/// Leistungsverlauf über die Dauer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerProfile {
    /// Kein Leistungsfeld in den `record`-Nachrichten.
    None,
    /// Konstante Leistung in Watt.
    Constant(u16),
    /// Wechsel zwischen `low` und `high`, je `period_s` Sekunden, beginnend mit `low`.
    Intervals { low: u16, high: u16, period_s: u32 },
    /// Linear von `start` (erste Sekunde) bis `end` (letzte Sekunde).
    Ramp { start: u16, end: u16 },
}

/// Herzfrequenzverlauf über die Dauer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HrProfile {
    /// Kein Herzfrequenzfeld in den `record`-Nachrichten.
    None,
    /// Konstante Herzfrequenz in bpm.
    Constant(u8),
    /// Linear von `start` (erste Sekunde) bis `end` (letzte Sekunde), zum Beispiel Drift.
    Drift { start: u8, end: u8 },
}

/// Zeitlücke: ab Sekunde `at_s` fehlen `len_s` aufeinanderfolgende `record`-Nachrichten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    pub at_s: u32,
    pub len_s: u32,
}

/// Einzelne Störung an einem festen Zeitpunkt. Überschreibt den Wert aus dem Verlauf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anomaly {
    /// Leistungsspitze, zum Beispiel über der Obergrenze aus SPEC 6.5.
    Power { at_s: u32, watts: u16 },
    /// Aussetzer oder Ausreißer der Herzfrequenz (0 oder über 230, SPEC 6.5).
    HeartRate { at_s: u32, bpm: u8 },
}

/// Parameter einer Aktivitätsdatei. `record`-Nachrichten entstehen mit 1 Hz für die Sekunden
/// `0..duration_s`, abzüglich der Lücken.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityParams {
    pub sport: Sport,
    /// Startzeit als FIT-Zeitstempel (Sekunden seit 31. Dezember 1989 UTC).
    pub start_fit: u32,
    /// Lokaler Zeitversatz in Sekunden, landet in `activity.local_timestamp`.
    pub local_offset_s: i32,
    pub duration_s: u32,
    pub power: PowerProfile,
    pub heart_rate: HrProfile,
    /// Konstante Kadenz in 1/min. `None` schreibt kein Kadenzfeld, damit bleiben die Bytes
    /// der vorhandenen Fixtures unverändert.
    pub cadence_rpm: Option<u8>,
    /// Konstante Geschwindigkeit in m/s. Mit Geschwindigkeit entsteht auch `distance`.
    pub speed_mps: Option<f32>,
    /// Beckenlänge in Metern, nur Schwimmen, landet in `session.pool_length`.
    pub pool_length_m: Option<f32>,
    pub gaps: Vec<Gap>,
    pub anomalies: Vec<Anomaly>,
    pub device: Device,
    /// Ohne `session` fehlen auch `lap`, `activity` bleibt erhalten.
    pub with_session: bool,
}

impl ActivityParams {
    /// Grundfall: Radfahrt mit konstanter Leistung und Herzfrequenz, ohne Störungen.
    pub fn cycling(start_fit: u32, local_offset_s: i32, duration_s: u32) -> Self {
        Self {
            sport: Sport::Cycling,
            start_fit,
            local_offset_s,
            duration_s,
            power: PowerProfile::Constant(200),
            heart_rate: HrProfile::Constant(140),
            cadence_rpm: None,
            speed_mps: Some(8.0),
            pool_length_m: None,
            gaps: Vec::new(),
            anomalies: Vec::new(),
            device: EDGE,
            with_session: true,
        }
    }

    /// Ist die Sekunde `t` von einer Lücke abgedeckt?
    pub fn in_gap(&self, t: u32) -> bool {
        self.gaps
            .iter()
            .any(|g| t >= g.at_s && t < g.at_s.saturating_add(g.len_s))
    }

    /// Anzahl der `record`-Nachrichten, die der Generator schreibt.
    pub fn record_count(&self) -> u32 {
        (0..self.duration_s).filter(|&t| !self.in_gap(t)).count() as u32
    }

    /// Sekunden mit Aufzeichnung, Grundlage von `total_timer_time`.
    fn timer_seconds(&self) -> u32 {
        self.record_count()
    }

    /// Leistung in Sekunde `t`, mit Anomalien.
    pub fn power_at(&self, t: u32) -> Option<u16> {
        let spike = self.anomalies.iter().find_map(|a| match a {
            Anomaly::Power { at_s, watts } if *at_s == t => Some(*watts),
            _ => None,
        });
        if spike.is_some() {
            return spike;
        }
        match self.power {
            PowerProfile::None => None,
            PowerProfile::Constant(w) => Some(w),
            PowerProfile::Intervals {
                low,
                high,
                period_s,
            } => {
                let period = period_s.max(1);
                Some(if (t / period).is_multiple_of(2) {
                    low
                } else {
                    high
                })
            }
            PowerProfile::Ramp { start, end } => Some(lerp_u16(start, end, t, self.duration_s)),
        }
    }

    /// Herzfrequenz in Sekunde `t`, mit Anomalien.
    pub fn heart_rate_at(&self, t: u32) -> Option<u8> {
        let outlier = self.anomalies.iter().find_map(|a| match a {
            Anomaly::HeartRate { at_s, bpm } if *at_s == t => Some(*bpm),
            _ => None,
        });
        if outlier.is_some() {
            return outlier;
        }
        match self.heart_rate {
            HrProfile::None => None,
            HrProfile::Constant(bpm) => Some(bpm),
            HrProfile::Drift { start, end } => Some(
                u8::try_from(lerp_u16(
                    u16::from(start),
                    u16::from(end),
                    t,
                    self.duration_s,
                ))
                .expect("Drift bleibt im u8-Bereich"),
            ),
        }
    }

    fn has_power(&self) -> bool {
        self.power != PowerProfile::None
    }

    fn has_heart_rate(&self) -> bool {
        self.heart_rate != HrProfile::None
    }
}

/// Linear zwischen `start` (t = 0) und `end` (t = duration - 1), ganzzahlig gerundet.
fn lerp_u16(start: u16, end: u16, t: u32, duration_s: u32) -> u16 {
    if duration_s <= 1 {
        return start;
    }
    let span = f64::from(end) - f64::from(start);
    let frac = f64::from(t) / f64::from(duration_s - 1);
    let v = (f64::from(start) + span * frac).round();
    // Rundung bleibt zwischen start und end, also im u16-Bereich.
    v as u16
}

/// Lokale Nachrichtennummern der Aktivitätsdatei.
const LOCAL_FILE_ID: u8 = 0;
const LOCAL_DEVICE_INFO: u8 = 1;
const LOCAL_RECORD: u8 = 2;
const LOCAL_LAP: u8 = 3;
const LOCAL_SESSION: u8 = 4;
const LOCAL_ACTIVITY: u8 = 5;
const LOCAL_MONITORING: u8 = 2;

fn write_file_id(w: &mut FitWriter, file_type: u8, device: Device, time_created: u32) {
    w.define(
        LOCAL_FILE_ID,
        profile::MESG_FILE_ID,
        &[
            FieldDef::new(profile::file_id::TYPE, BaseType::Enum),
            FieldDef::new(profile::file_id::MANUFACTURER, BaseType::Uint16),
            FieldDef::new(profile::file_id::PRODUCT, BaseType::Uint16),
            FieldDef::new(profile::file_id::SERIAL_NUMBER, BaseType::Uint32z),
            FieldDef::new(profile::file_id::TIME_CREATED, BaseType::Uint32),
        ],
    );
    w.write(
        LOCAL_FILE_ID,
        &[
            Value::U8(file_type),
            Value::U16(device.manufacturer),
            Value::U16(device.product),
            Value::U32(device.serial),
            Value::U32(time_created),
        ],
    );
}

fn write_device_info(w: &mut FitWriter, device: Device, timestamp: u32) {
    w.define(
        LOCAL_DEVICE_INFO,
        profile::MESG_DEVICE_INFO,
        &[
            FieldDef::new(FIELD_TIMESTAMP, BaseType::Uint32),
            FieldDef::new(profile::device_info::DEVICE_INDEX, BaseType::Uint8),
            FieldDef::new(profile::device_info::MANUFACTURER, BaseType::Uint16),
            FieldDef::new(profile::device_info::SERIAL_NUMBER, BaseType::Uint32z),
            FieldDef::new(profile::device_info::PRODUCT, BaseType::Uint16),
        ],
    );
    w.write(
        LOCAL_DEVICE_INFO,
        &[
            Value::U32(timestamp),
            Value::U8(profile::device_info::DEVICE_INDEX_CREATOR),
            Value::U16(device.manufacturer),
            Value::U32(device.serial),
            Value::U16(device.product),
        ],
    );
}

/// Skaliert Meter auf das uint32-Feld (Meter mal 100).
fn distance_cm(m: f64) -> u32 {
    (m * profile::record::DISTANCE_SCALE).round() as u32
}

/// Erzeugt eine Aktivitätsdatei aus den Parametern.
pub fn activity(p: &ActivityParams) -> Vec<u8> {
    let mut w = FitWriter::new();
    let end_fit = p.start_fit + p.duration_s;
    write_file_id(
        &mut w,
        profile::file_id::FILE_TYPE_ACTIVITY,
        p.device,
        p.start_fit,
    );
    write_device_info(&mut w, p.device, p.start_fit);

    // record: nur die vorhandenen Kanäle werden definiert
    let mut record_fields = vec![FieldDef::new(FIELD_TIMESTAMP, BaseType::Uint32)];
    if p.has_heart_rate() {
        record_fields.push(FieldDef::new(profile::record::HEART_RATE, BaseType::Uint8));
    }
    if p.cadence_rpm.is_some() {
        record_fields.push(FieldDef::new(profile::record::CADENCE, BaseType::Uint8));
    }
    if p.speed_mps.is_some() {
        record_fields.push(FieldDef::new(profile::record::DISTANCE, BaseType::Uint32));
        record_fields.push(FieldDef::new(profile::record::SPEED, BaseType::Uint16));
    }
    if p.has_power() {
        record_fields.push(FieldDef::new(profile::record::POWER, BaseType::Uint16));
    }
    w.define(LOCAL_RECORD, profile::MESG_RECORD, &record_fields);

    for t in (0..p.duration_s).filter(|&t| !p.in_gap(t)) {
        let mut values = vec![Value::U32(p.start_fit + t)];
        if p.has_heart_rate() {
            values.push(p.heart_rate_at(t).map_or(Value::Invalid, Value::U8));
        }
        if let Some(rpm) = p.cadence_rpm {
            values.push(Value::U8(rpm));
        }
        if let Some(speed) = p.speed_mps {
            let speed = f64::from(speed);
            values.push(Value::U32(distance_cm(speed * f64::from(t))));
            values.push(Value::U16(
                (speed * profile::record::SPEED_SCALE).round() as u16
            ));
        }
        if p.has_power() {
            values.push(p.power_at(t).map_or(Value::Invalid, Value::U16));
        }
        w.write(LOCAL_RECORD, &values);
    }

    let elapsed_ms = p.duration_s * 1000;
    let timer_ms = p.timer_seconds() * 1000;
    let total_distance = p
        .speed_mps
        .map(|s| distance_cm(f64::from(s) * f64::from(p.duration_s)));

    if p.with_session {
        w.define(
            LOCAL_LAP,
            profile::MESG_LAP,
            &[
                FieldDef::new(FIELD_TIMESTAMP, BaseType::Uint32),
                FieldDef::new(profile::lap::START_TIME, BaseType::Uint32),
                FieldDef::new(profile::lap::TOTAL_ELAPSED_TIME, BaseType::Uint32),
                FieldDef::new(profile::lap::TOTAL_TIMER_TIME, BaseType::Uint32),
                FieldDef::new(profile::lap::TOTAL_DISTANCE, BaseType::Uint32),
            ],
        );
        w.write(
            LOCAL_LAP,
            &[
                Value::U32(end_fit),
                Value::U32(p.start_fit),
                Value::U32(elapsed_ms),
                Value::U32(timer_ms),
                total_distance.map_or(Value::Invalid, Value::U32),
            ],
        );

        let (sport, sub_sport) = p.sport.codes();
        let mut session_fields = vec![
            FieldDef::new(FIELD_TIMESTAMP, BaseType::Uint32),
            FieldDef::new(profile::session::START_TIME, BaseType::Uint32),
            FieldDef::new(profile::session::SPORT, BaseType::Enum),
            FieldDef::new(profile::session::SUB_SPORT, BaseType::Enum),
            FieldDef::new(profile::session::TOTAL_ELAPSED_TIME, BaseType::Uint32),
            FieldDef::new(profile::session::TOTAL_TIMER_TIME, BaseType::Uint32),
            FieldDef::new(profile::session::TOTAL_DISTANCE, BaseType::Uint32),
        ];
        let mut session_values = vec![
            Value::U32(end_fit),
            Value::U32(p.start_fit),
            Value::U8(sport),
            Value::U8(sub_sport),
            Value::U32(elapsed_ms),
            Value::U32(timer_ms),
            total_distance.map_or(Value::Invalid, Value::U32),
        ];
        if let Some(pool) = p.pool_length_m {
            session_fields.push(FieldDef::new(
                profile::session::POOL_LENGTH,
                BaseType::Uint16,
            ));
            session_values.push(Value::U16(
                (f64::from(pool) * profile::session::POOL_LENGTH_SCALE).round() as u16,
            ));
        }
        w.define(LOCAL_SESSION, profile::MESG_SESSION, &session_fields);
        w.write(LOCAL_SESSION, &session_values);
    }

    w.define(
        LOCAL_ACTIVITY,
        profile::MESG_ACTIVITY,
        &[
            FieldDef::new(FIELD_TIMESTAMP, BaseType::Uint32),
            FieldDef::new(profile::activity::TOTAL_TIMER_TIME, BaseType::Uint32),
            FieldDef::new(profile::activity::NUM_SESSIONS, BaseType::Uint16),
            FieldDef::new(profile::activity::LOCAL_TIMESTAMP, BaseType::Uint32),
        ],
    );
    let local_end = i64::from(end_fit) + i64::from(p.local_offset_s);
    w.write(
        LOCAL_ACTIVITY,
        &[
            Value::U32(end_fit),
            Value::U32(timer_ms),
            Value::U16(u16::from(p.with_session)),
            Value::U32(u32::try_from(local_end).expect("Ortszeit im FIT-Bereich")),
        ],
    );
    w.finish()
}

/// Parameter einer Monitoring-Datei (Tagesdaten ohne Aktivität).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitoringParams {
    pub start_fit: u32,
    /// Anzahl der `monitoring`-Nachrichten.
    pub samples: u32,
    /// Abstand der Nachrichten in Sekunden.
    pub interval_s: u32,
    /// Ruhepuls, konstant über alle Nachrichten.
    pub heart_rate_bpm: u8,
    pub device: Device,
}

/// Erzeugt eine Monitoring-Datei: `file_id` mit Typ `monitoring_b`, `device_info` und
/// `monitoring`-Nachrichten, keine `session`, keine `activity`.
pub fn monitoring(p: &MonitoringParams) -> Vec<u8> {
    let mut w = FitWriter::new();
    write_file_id(
        &mut w,
        profile::file_id::FILE_TYPE_MONITORING_B,
        p.device,
        p.start_fit,
    );
    write_device_info(&mut w, p.device, p.start_fit);
    w.define(
        LOCAL_MONITORING,
        profile::MESG_MONITORING,
        &[
            FieldDef::new(FIELD_TIMESTAMP, BaseType::Uint32),
            FieldDef::new(profile::monitoring::HEART_RATE, BaseType::Uint8),
        ],
    );
    for i in 0..p.samples {
        w.write(
            LOCAL_MONITORING,
            &[
                Value::U32(p.start_fit + i * p.interval_s),
                Value::U8(p.heart_rate_bpm),
            ],
        );
    }
    w.finish()
}

/// Verfälscht die Datei-Prüfsumme: das letzte Byte wird invertiert, der Rest bleibt gleich.
pub fn corrupt_crc(mut bytes: Vec<u8>) -> Vec<u8> {
    if let Some(last) = bytes.last_mut() {
        *last = !*last;
    }
    bytes
}

/// Schneidet die Datei nach `keep_permille` Promille ihrer Länge ab. Kopf und `data_size`
/// bleiben erhalten, die Datei ist danach kürzer als angekündigt und hat keine Prüfsumme.
pub fn truncate(mut bytes: Vec<u8>, keep_permille: u32) -> Vec<u8> {
    let keep = (bytes.len() as u64 * u64::from(keep_permille) / 1000) as usize;
    bytes.truncate(keep);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_interpolates_linearly_between_endpoints() {
        // 100 bis 300 W über 5 Sekunden: 100, 150, 200, 250, 300
        let p = ActivityParams {
            power: PowerProfile::Ramp {
                start: 100,
                end: 300,
            },
            ..ActivityParams::cycling(0, 0, 5)
        };
        let values: Vec<u16> = (0..5).map(|t| p.power_at(t).unwrap()).collect();
        assert_eq!(values, vec![100, 150, 200, 250, 300]);
    }

    #[test]
    fn intervals_alternate_with_period() {
        let p = ActivityParams {
            power: PowerProfile::Intervals {
                low: 100,
                high: 300,
                period_s: 30,
            },
            ..ActivityParams::cycling(0, 0, 120)
        };
        assert_eq!(p.power_at(0), Some(100));
        assert_eq!(p.power_at(29), Some(100));
        assert_eq!(p.power_at(30), Some(300));
        assert_eq!(p.power_at(59), Some(300));
        assert_eq!(p.power_at(60), Some(100));
    }

    #[test]
    fn gaps_reduce_record_count() {
        let p = ActivityParams {
            gaps: vec![
                Gap { at_s: 10, len_s: 5 },
                Gap {
                    at_s: 50,
                    len_s: 10,
                },
            ],
            ..ActivityParams::cycling(0, 0, 100)
        };
        assert_eq!(p.record_count(), 85);
        assert!(p.in_gap(10));
        assert!(p.in_gap(14));
        assert!(!p.in_gap(15));
    }

    #[test]
    fn anomalies_override_profile_values() {
        let p = ActivityParams {
            anomalies: vec![
                Anomaly::Power {
                    at_s: 3,
                    watts: 3000,
                },
                Anomaly::HeartRate { at_s: 4, bpm: 0 },
            ],
            ..ActivityParams::cycling(0, 0, 10)
        };
        assert_eq!(p.power_at(2), Some(200));
        assert_eq!(p.power_at(3), Some(3000));
        assert_eq!(p.heart_rate_at(4), Some(0));
        assert_eq!(p.heart_rate_at(5), Some(140));
    }

    #[test]
    fn drift_reaches_end_value_in_last_second() {
        let p = ActivityParams {
            heart_rate: HrProfile::Drift {
                start: 130,
                end: 160,
            },
            ..ActivityParams::cycling(0, 0, 3600)
        };
        assert_eq!(p.heart_rate_at(0), Some(130));
        assert_eq!(p.heart_rate_at(3599), Some(160));
    }

    #[test]
    fn corrupt_crc_changes_only_last_byte() {
        let original = activity(&ActivityParams::cycling(1000, 0, 10));
        let bad = corrupt_crc(original.clone());
        assert_eq!(bad.len(), original.len());
        assert_eq!(&bad[..bad.len() - 1], &original[..original.len() - 1]);
        assert_ne!(bad[bad.len() - 1], original[original.len() - 1]);
    }

    #[test]
    fn truncate_keeps_requested_share() {
        let bytes = vec![0u8; 1000];
        assert_eq!(truncate(bytes, 600).len(), 600);
    }
}
