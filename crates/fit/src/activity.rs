//! Aktivität aus den Nachrichten einer FIT-Datei (SPEC 5.4, 6.5).

use chrono::{DateTime, Utc};
use lastwerk_core::load::{LoadConfig, Plausibility, Sample, Sport, check_plausibility};
use serde::{Deserialize, Serialize};

use crate::decode::{Message, data_section, walk};
use crate::error::FitError;
use crate::profile::{
    FIELD_TIMESTAMP, FIT_EPOCH_OFFSET_S, MESG_ACTIVITY, MESG_DEVICE_INFO, MESG_FILE_ID, MESG_LAP,
    MESG_RECORD, MESG_SESSION, MESG_SPORT, MIN_DATE_TIME, activity, device_info, file_id, lap,
    manufacturer, record, session, sport, sport_msg, sub_sport,
};

/// Größtes zulässiges Dateivolumen in Byte (64 MiB). Konvention nach SPEC 10.2: Eine
/// Aktivität mit 1 Hz und mehreren Kanälen braucht etwa 15 Byte je Sekunde, 64 MiB reichen
/// damit für über 40 Tage. Größere Dateien sind kein Trainingsdatensatz, sondern ein
/// Manipulationsversuch oder ein Irrläufer.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Höchste Zahl `record`-Nachrichten je Datei. Konvention (SPEC 10.2): Ein Rekord kann mit
/// komprimiertem Kopf ein Byte belegen, ohne Grenze wäre der Speicherbedarf das Mehrfache
/// der Dateigröße. 2 Millionen entsprechen über 23 Tagen bei 1 Hz.
pub const MAX_RECORDS: usize = 2_000_000;

/// Höchste Zahl `lap`-Nachrichten je Datei. Konvention aus demselben Grund wie [`MAX_RECORDS`].
pub const MAX_LAPS: usize = 100_000;

/// Größter zulässiger Betrag des lokalen Zeitversatzes in Sekunden (24 Stunden). Konvention:
/// Zeitzonen liegen zwischen UTC-12 und UTC+14, alles darüber ist ein beschädigtes Feld.
const MAX_LOCAL_OFFSET_S: i64 = 86_400;

/// Eine Runde (`lap`). Alles, was die Datei weglassen kann, ist `Option`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lap {
    pub start_time: Option<DateTime<Utc>>,
    pub total_elapsed_time_s: Option<f64>,
    pub total_timer_time_s: Option<f64>,
    pub total_distance_m: Option<f64>,
}

/// Zusammenfassung der `session`-Nachricht. Fehlt die Nachricht, ist alles `None`.
///
/// Bei mehreren Sessions (Mehrkampf) zählt die erste. Konvention: SPEC 5.4 kennt nur eine
/// Session je Aktivität, Mehrkampf ist nicht Teil von Version 1.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub start_time: Option<DateTime<Utc>>,
    /// Gesamtdauer inklusive Pausen in Sekunden.
    pub total_elapsed_time_s: Option<f64>,
    /// Dauer bei laufendem Timer in Sekunden.
    pub total_timer_time_s: Option<f64>,
    pub total_distance_m: Option<f64>,
    /// Beckenlänge in Metern, nur Schwimmen.
    pub pool_length_m: Option<f32>,
}

/// Eingelesene Aktivität (SPEC 5.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawActivity {
    pub sport: Sport,
    /// Name der Unterart, falls die Datei eine angibt.
    pub sub_sport: Option<String>,
    /// Zeitpunkt des ersten `record` (Ursprung von `Sample::t_s`). Ohne `record` der Start
    /// der Session oder, wenn auch der fehlt, der Erstellzeitpunkt aus `file_id`.
    pub start_utc: DateTime<Utc>,
    /// Ortszeit minus UTC in Sekunden aus `activity.local_timestamp`. Fehlt das Feld, 0.
    pub local_offset_s: i32,
    pub device_serial: Option<u32>,
    /// Hersteller und Produktnummer als Text, zum Beispiel `garmin:4242`.
    pub device_product: Option<String>,
    /// Messpunkte im 1-Hz-Raster, wie in der Datei: Lücken bleiben Lücken, unplausible Werte
    /// stehen noch drin. `plausibility` zählt sie. Wer rechnet, ruft
    /// `lastwerk_core::load::check_plausibility` selbst auf (kein Doppelcode der Regeln).
    pub samples: Vec<Sample>,
    pub laps: Vec<Lap>,
    pub session: SessionSummary,
    /// Zählung nach den Regeln aus SPEC 6.5 mit `LoadConfig::default()`: verworfene Werte und
    /// Lücken, nur zur Anzeige und zum Abgleich.
    pub plausibility: Plausibility,
}

impl RawActivity {
    /// Gesamtdauer in Sekunden: `session.total_elapsed_time`, sonst aus den Zeitstempeln der
    /// Samples (letzter Zeitpunkt plus eine Sekunde, da jedes Sample eine Sekunde belegt).
    pub fn duration_s(&self) -> Option<f64> {
        self.session
            .total_elapsed_time_s
            .or_else(|| self.samples.last().map(|s| f64::from(s.t_s) + 1.0))
    }

    pub fn has_power(&self) -> bool {
        self.samples.iter().any(|s| s.power_w.is_some())
    }

    pub fn has_heart_rate(&self) -> bool {
        self.samples.iter().any(|s| s.hr_bpm.is_some())
    }
}

/// Liest eine FIT-Datei aus dem Speicher.
///
/// Ablauf: Größe und Kopf prüfen, Prüfsummen prüfen, Nachrichten lesen, Aktivität bilden.
/// Fehler siehe [`FitError`]; jede Datei steht für sich, ein Fehler hat keine Nebenwirkung.
pub fn parse_activity(bytes: &[u8]) -> Result<RawActivity, FitError> {
    if bytes.is_empty() {
        return Err(FitError::EmptyFile);
    }
    let size = bytes.len() as u64;
    if size > MAX_FILE_BYTES {
        return Err(FitError::TooLarge {
            size,
            limit: MAX_FILE_BYTES,
        });
    }
    let data = data_section(bytes)?;
    let mut builder = Builder::default();
    walk(data, |m| builder.accept(m))?;
    builder.finish()
}

/// Ein `record` in der Reihenfolge der Datei, vor der Aufbereitung auf 1 Hz.
#[derive(Debug, Clone, Copy)]
struct RecordRow {
    timestamp: u32,
    power_w: Option<f32>,
    hr_bpm: Option<f32>,
    speed_mps: Option<f32>,
}

#[derive(Debug, Default)]
struct Builder {
    // file_id
    seen_file_id: bool,
    file_type: Option<i64>,
    manufacturer: Option<i64>,
    product: Option<i64>,
    serial: Option<i64>,
    time_created: Option<i64>,
    // device_info des aufzeichnenden Geräts, nur Rückfall
    seen_device_info: bool,
    dev_manufacturer: Option<i64>,
    dev_product: Option<i64>,
    dev_serial: Option<i64>,
    // sport (Rückfall für die Sportart)
    sport_msg: Option<(Option<i64>, Option<i64>)>,
    // session (erste)
    session: Option<SessionFields>,
    laps: Vec<Lap>,
    records: Vec<RecordRow>,
    // activity (erste)
    activity_timestamp: Option<i64>,
    activity_local_timestamp: Option<i64>,
}

#[derive(Debug, Default)]
struct SessionFields {
    summary: SessionSummary,
    sport: Option<i64>,
    sub_sport: Option<i64>,
}

/// FIT-Zeitstempel als UTC. Werte unter [`MIN_DATE_TIME`] sind keine Uhrzeit.
fn fit_time(ts: i64) -> Option<DateTime<Utc>> {
    if ts < MIN_DATE_TIME {
        return None;
    }
    DateTime::from_timestamp(ts.checked_add(FIT_EPOCH_OFFSET_S)?, 0)
}

fn scaled(v: Option<i64>, scale: f64) -> Option<f64> {
    v.map(|v| v as f64 / scale)
}

impl Builder {
    fn accept(&mut self, m: &Message) -> Result<(), FitError> {
        match m.global {
            MESG_FILE_ID if !self.seen_file_id => {
                self.seen_file_id = true;
                self.file_type = m.get(file_id::TYPE);
                self.manufacturer = m.get(file_id::MANUFACTURER);
                self.product = m.get(file_id::PRODUCT);
                self.serial = m.get(file_id::SERIAL_NUMBER);
                self.time_created = m.get(file_id::TIME_CREATED);
            }
            MESG_DEVICE_INFO if !self.seen_device_info => {
                // Ohne device_index gilt das Gerät als Ersteller (Index 0 ist der Standard).
                let index = m
                    .get(device_info::DEVICE_INDEX)
                    .unwrap_or(device_info::DEVICE_INDEX_CREATOR);
                if index == device_info::DEVICE_INDEX_CREATOR {
                    self.seen_device_info = true;
                    self.dev_manufacturer = m.get(device_info::MANUFACTURER);
                    self.dev_product = m.get(device_info::PRODUCT);
                    self.dev_serial = m.get(device_info::SERIAL_NUMBER);
                }
            }
            MESG_SPORT if self.sport_msg.is_none() => {
                self.sport_msg = Some((m.get(sport_msg::SPORT), m.get(sport_msg::SUB_SPORT)));
            }
            MESG_SESSION if self.session.is_none() => {
                self.session = Some(SessionFields {
                    summary: SessionSummary {
                        start_time: m.get(session::START_TIME).and_then(fit_time),
                        total_elapsed_time_s: scaled(
                            m.get(session::TOTAL_ELAPSED_TIME),
                            session::TIME_SCALE,
                        ),
                        total_timer_time_s: scaled(
                            m.get(session::TOTAL_TIMER_TIME),
                            session::TIME_SCALE,
                        ),
                        total_distance_m: scaled(
                            m.get(session::TOTAL_DISTANCE),
                            session::DISTANCE_SCALE,
                        ),
                        pool_length_m: m
                            .get(session::POOL_LENGTH)
                            .map(|v| v as f32 / session::POOL_LENGTH_SCALE),
                    },
                    sport: m.get(session::SPORT),
                    sub_sport: m.get(session::SUB_SPORT),
                });
            }
            MESG_LAP => {
                if self.laps.len() >= MAX_LAPS {
                    return Err(FitError::Malformed(format!("mehr als {MAX_LAPS} Runden")));
                }
                self.laps.push(Lap {
                    start_time: m.get(lap::START_TIME).and_then(fit_time),
                    total_elapsed_time_s: scaled(
                        m.get(lap::TOTAL_ELAPSED_TIME),
                        session::TIME_SCALE,
                    ),
                    total_timer_time_s: scaled(m.get(lap::TOTAL_TIMER_TIME), session::TIME_SCALE),
                    total_distance_m: scaled(m.get(lap::TOTAL_DISTANCE), session::DISTANCE_SCALE),
                });
            }
            MESG_RECORD => {
                // Ohne gültigen Zeitstempel lässt sich der Messpunkt nicht einordnen.
                if let Some(timestamp) = m.get(FIELD_TIMESTAMP).and_then(|t| u32::try_from(t).ok())
                {
                    if self.records.len() >= MAX_RECORDS {
                        return Err(FitError::Malformed(format!(
                            "mehr als {MAX_RECORDS} Messpunkte"
                        )));
                    }
                    // Neuere Geräte schreiben speed und enhanced_speed, dann hat das
                    // erweiterte Feld die höhere Auflösung des Wertebereichs.
                    let speed = m
                        .get(record::ENHANCED_SPEED)
                        .or_else(|| m.get(record::SPEED));
                    self.records.push(RecordRow {
                        timestamp,
                        power_w: m.get(record::POWER).map(|v| v as f32),
                        hr_bpm: m.get(record::HEART_RATE).map(|v| v as f32),
                        speed_mps: speed.map(|v| v as f32 / record::SPEED_SCALE),
                    });
                }
            }
            MESG_ACTIVITY if self.activity_timestamp.is_none() => {
                self.activity_timestamp = m.get(FIELD_TIMESTAMP);
                self.activity_local_timestamp = m.get(activity::LOCAL_TIMESTAMP);
            }
            _ => {}
        }
        Ok(())
    }

    fn finish(self) -> Result<RawActivity, FitError> {
        if !self.seen_file_id {
            return Err(FitError::Malformed("file_id fehlt".to_owned()));
        }
        if self.file_type != Some(file_id::FILE_TYPE_ACTIVITY) {
            return Err(FitError::NoActivity);
        }
        if self.records.is_empty() && self.session.is_none() {
            return Err(FitError::NoActivity);
        }

        let samples = to_one_hz(self.records);

        // Ursprung von t_s ist der erste record. Konvention: Dann stimmen start_utc und t_s = 0
        // überein, auch wenn die Session etwas früher beginnt (dann steht deren Start in
        // `session.start_time`).
        let origin_ts = samples.first().map(|(ts, _)| *ts);
        let session_start = self.session.as_ref().and_then(|s| s.summary.start_time);
        let start_utc = match origin_ts {
            Some(ts) => fit_time(i64::from(ts))
                .ok_or_else(|| FitError::Malformed("Zeitstempel ist kein Datum".to_owned()))?,
            None => session_start
                .or_else(|| self.time_created.and_then(fit_time))
                .ok_or_else(|| FitError::Malformed("kein Startzeitpunkt".to_owned()))?,
        };
        let samples: Vec<Sample> = samples.into_iter().map(|(_, s)| s).collect();

        let local_offset_s = match (self.activity_timestamp, self.activity_local_timestamp) {
            (Some(ts), Some(local)) => {
                let diff = local - ts;
                if diff.abs() > MAX_LOCAL_OFFSET_S {
                    return Err(FitError::Malformed(
                        "lokaler Zeitversatz außerhalb von 24 Stunden".to_owned(),
                    ));
                }
                i32::try_from(diff).unwrap_or(0)
            }
            _ => 0,
        };

        let (session_sport, session_sub) = self
            .session
            .as_ref()
            .map_or((None, None), |s| (s.sport, s.sub_sport));
        let (msg_sport, msg_sub) = self.sport_msg.unwrap_or((None, None));
        let sport_code = session_sport.or(msg_sport);
        let sub_sport_code = session_sub.or(msg_sub);

        let manufacturer_code = self.manufacturer.or(self.dev_manufacturer);
        let product_code = self.product.or(self.dev_product);
        let serial = self
            .serial
            .or(self.dev_serial)
            .and_then(|v| u32::try_from(v).ok());

        let plausibility = check_plausibility(&samples, &LoadConfig::default())
            .map_err(|e| FitError::Malformed(e.to_string()))?
            .plausibility;

        Ok(RawActivity {
            sport: map_sport(sport_code, sub_sport_code),
            sub_sport: sub_sport_code.map(sub_sport_name),
            start_utc,
            local_offset_s,
            device_serial: serial,
            device_product: product_code.map(|p| product_text(manufacturer_code, p)),
            samples,
            laps: self.laps,
            session: self.session.map(|s| s.summary).unwrap_or_default(),
            plausibility,
        })
    }
}

/// Bringt die `record`-Nachrichten auf ein 1-Hz-Raster relativ zum ersten Messpunkt.
///
/// - Sortierung stabil nach Zeitstempel (Geräte schreiben aufsteigend, die Sortierung schützt
///   nur vor beschädigten Dateien und vor der Fehlermeldung `t_s not strictly increasing`).
/// - Mehrere `record` in derselben Sekunde: der letzte in Dateireihenfolge gewinnt komplett.
///   Konvention, Geräte mit Mehrfachaufzeichnung pro Sekunde sind selten.
/// - Lücken bleiben Lücken: es wird nichts aufgefüllt oder interpoliert. Das ist Sache von
///   `check_plausibility` (SPEC 6.5).
///
/// Jedes Sample trägt seinen absoluten Zeitstempel mit, der erste davon ist der Ursprung von
/// `t_s` und wird für `start_utc` gebraucht.
fn to_one_hz(mut rows: Vec<RecordRow>) -> Vec<(u32, Sample)> {
    rows.sort_by_key(|r| r.timestamp);
    let Some(origin) = rows.first().map(|r| r.timestamp) else {
        return Vec::new();
    };
    let mut out: Vec<(u32, Sample)> = Vec::with_capacity(rows.len());
    for r in rows {
        let sample = Sample {
            // sortiert, also nie negativ
            t_s: r.timestamp - origin,
            power_w: r.power_w,
            hr_bpm: r.hr_bpm,
            speed_mps: r.speed_mps,
        };
        match out.last_mut() {
            Some((ts, last)) if *ts == r.timestamp => *last = sample,
            _ => out.push((r.timestamp, sample)),
        }
    }
    out
}

/// Sportart aus den Codes des Profils. Kraft ist `training` mit Unterart
/// `strength_training`; andere Trainingsarten und unbekannte Sportarten sind `Other`.
fn map_sport(sport_code: Option<i64>, sub_sport_code: Option<i64>) -> Sport {
    match sport_code {
        Some(sport::RUNNING) => Sport::Running,
        Some(sport::CYCLING) => Sport::Cycling,
        Some(sport::SWIMMING) => Sport::Swimming,
        Some(sport::TRAINING) if sub_sport_code == Some(sub_sport::STRENGTH_TRAINING) => {
            Sport::Strength
        }
        _ => Sport::Other,
    }
}

/// Name der Unterart; unbekannte Nummern erscheinen als `sub_sport_<n>`.
fn sub_sport_name(code: i64) -> String {
    match code {
        sub_sport::GENERIC => "generic".to_owned(),
        sub_sport::ROAD => "road".to_owned(),
        sub_sport::LAP_SWIMMING => "lap_swimming".to_owned(),
        sub_sport::STRENGTH_TRAINING => "strength_training".to_owned(),
        n => format!("sub_sport_{n}"),
    }
}

/// Gerät als Text: bekannter Hersteller als Name, sonst als Nummer, dann die Produktnummer.
fn product_text(manufacturer_code: Option<i64>, product: i64) -> String {
    match manufacturer_code {
        Some(manufacturer::GARMIN) => format!("garmin:{product}"),
        Some(n) => format!("manufacturer_{n}:{product}"),
        None => product.to_string(),
    }
}
