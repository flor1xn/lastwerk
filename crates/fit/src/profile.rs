//! Konstanten des FIT-Protokolls und des FIT-Profils, soweit der Decoder sie braucht.
//!
//! Alle Werte nach öffentlicher FIT-Protokoll- und Profilbeschreibung. Es stehen nur die
//! tatsächlich benötigten Nachrichten- und Feldnummern hier, keine Profildateien und keine
//! SDK-Quellen (SPEC 14, O2). Die Werte stimmen mit `lastwerk_testkit::profile` überein; das
//! Testkit ist eine Entwicklungsabhängigkeit und darf nicht im Produktivcode benutzt werden,
//! deshalb stehen sie hier ein zweites Mal. Ein Test gleicht beide Listen ab.

/// Offset der FIT-Epoche (31. Dezember 1989 00:00:00 UTC) zur Unix-Epoche in Sekunden.
///
/// Rechenweg: 1990-01-01 liegt 7305 Tage nach 1970-01-01 (20 Jahre, davon fünf Schaltjahre
/// 1972, 1976, 1980, 1984, 1988: `20 * 365 + 5 = 7305`). Die FIT-Epoche ist der Tag davor,
/// also `7304 * 86400 = 631 065 600`.
pub const FIT_EPOCH_OFFSET_S: i64 = 631_065_600;

/// Kleinster Zeitstempel, der eine Uhrzeit darstellt. Darunter zählt das Gerät Sekunden seit
/// dem Einschalten ("relative Systemzeit"), das ist kein Datum.
/// Rechenweg: `0x1000_0000 = 16^7 = 268 435 456`, etwa 8,5 Jahre nach der FIT-Epoche.
pub const MIN_DATE_TIME: i64 = 0x1000_0000;

// Dateikopf

/// Kopflänge ohne Prüfsumme.
pub const HEADER_SIZE_SHORT: usize = 12;
/// Kopflänge mit Prüfsumme.
pub const HEADER_SIZE_LONG: usize = 14;
/// Kennung ".FIT" im Dateikopf (Byte 8 bis 11).
pub const DATA_TYPE_TAG: [u8; 4] = *b".FIT";

// Rekordkopf

/// Bit 7: komprimierter Zeitstempelkopf (nur Datennachrichten).
pub const RECORD_HEADER_COMPRESSED: u8 = 0x80;
/// Bit 6: Definitionsnachricht (1) oder Datennachricht (0).
pub const RECORD_HEADER_DEFINITION: u8 = 0x40;
/// Bit 5: Definitionsnachricht enthält Entwicklerfelder.
pub const RECORD_HEADER_DEVELOPER_DATA: u8 = 0x20;
/// Bits 0 bis 3: lokale Nachrichtennummer im normalen Kopf.
pub const RECORD_HEADER_LOCAL_MASK: u8 = 0x0F;
/// Komprimierter Kopf: Bits 5 und 6 tragen die lokale Nachrichtennummer (0 bis 3).
pub const COMPRESSED_LOCAL_SHIFT: u8 = 5;
pub const COMPRESSED_LOCAL_MASK: u8 = 0x03;
/// Komprimierter Kopf: Bits 0 bis 4 tragen den Zeitversatz in Sekunden.
pub const COMPRESSED_OFFSET_MASK: u8 = 0x1F;
/// Architektur in der Definitionsnachricht: 0 = Little Endian, 1 = Big Endian.
pub const ARCH_LITTLE_ENDIAN: u8 = 0;
pub const ARCH_BIG_ENDIAN: u8 = 1;

// Basistypen (untere fünf Bit des Typbytes in der Felddefinition)

pub const BASE_MASK: u8 = 0x1F;
pub const BASE_ENUM: u8 = 0;
pub const BASE_SINT8: u8 = 1;
pub const BASE_UINT8: u8 = 2;
pub const BASE_SINT16: u8 = 3;
pub const BASE_UINT16: u8 = 4;
pub const BASE_SINT32: u8 = 5;
pub const BASE_UINT32: u8 = 6;
pub const BASE_UINT8Z: u8 = 10;
pub const BASE_UINT16Z: u8 = 11;
pub const BASE_UINT32Z: u8 = 12;

// Globale Nachrichtennummern

pub const MESG_FILE_ID: u16 = 0;
pub const MESG_SPORT: u16 = 12;
pub const MESG_SESSION: u16 = 18;
pub const MESG_LAP: u16 = 19;
pub const MESG_RECORD: u16 = 20;
pub const MESG_DEVICE_INFO: u16 = 23;
pub const MESG_ACTIVITY: u16 = 34;

/// Feldnummer `timestamp`, in allen Nachrichten gleich (uint32, Sekunden seit FIT-Epoche).
pub const FIELD_TIMESTAMP: u8 = 253;

/// Felder der Nachricht `file_id`.
pub mod file_id {
    pub const TYPE: u8 = 0;
    pub const MANUFACTURER: u8 = 1;
    pub const PRODUCT: u8 = 2;
    pub const SERIAL_NUMBER: u8 = 3;
    pub const TIME_CREATED: u8 = 4;

    /// Wert des Enums `file` für Aktivitätsdateien.
    pub const FILE_TYPE_ACTIVITY: i64 = 4;
}

/// Felder der Nachricht `device_info`.
pub mod device_info {
    pub const DEVICE_INDEX: u8 = 0;
    pub const MANUFACTURER: u8 = 2;
    pub const SERIAL_NUMBER: u8 = 3;
    pub const PRODUCT: u8 = 4;

    /// `device_index` des aufzeichnenden Geräts selbst.
    pub const DEVICE_INDEX_CREATOR: i64 = 0;
}

/// Felder der Nachricht `sport`.
pub mod sport_msg {
    pub const SPORT: u8 = 0;
    pub const SUB_SPORT: u8 = 1;
}

/// Felder der Nachricht `record`.
pub mod record {
    /// uint8, bpm.
    pub const HEART_RATE: u8 = 3;
    /// uint16, Meter je Sekunde mal 1000.
    pub const SPEED: u8 = 6;
    /// uint16, Watt.
    pub const POWER: u8 = 7;
    /// uint32, Meter je Sekunde mal 1000. Neuere Geräte schreiben beide Geschwindigkeitsfelder.
    pub const ENHANCED_SPEED: u8 = 73;

    pub const SPEED_SCALE: f32 = 1000.0;
}

/// Felder der Nachricht `lap`. Die Summenfelder teilen sich Nummer und Skalierung mit `session`.
pub mod lap {
    pub const START_TIME: u8 = 2;
    pub const TOTAL_ELAPSED_TIME: u8 = 7;
    pub const TOTAL_TIMER_TIME: u8 = 8;
    pub const TOTAL_DISTANCE: u8 = 9;
}

/// Felder der Nachricht `session`.
pub mod session {
    pub const START_TIME: u8 = 2;
    pub const SPORT: u8 = 5;
    pub const SUB_SPORT: u8 = 6;
    /// uint32, Sekunden mal 1000.
    pub const TOTAL_ELAPSED_TIME: u8 = 7;
    /// uint32, Sekunden mal 1000.
    pub const TOTAL_TIMER_TIME: u8 = 8;
    /// uint32, Meter mal 100.
    pub const TOTAL_DISTANCE: u8 = 9;
    /// uint16, Meter mal 100.
    pub const POOL_LENGTH: u8 = 44;

    pub const TIME_SCALE: f64 = 1000.0;
    pub const DISTANCE_SCALE: f64 = 100.0;
    pub const POOL_LENGTH_SCALE: f32 = 100.0;
}

/// Felder der Nachricht `activity`.
pub mod activity {
    /// uint32, Ortszeit als FIT-Zeitstempel; Differenz zu `timestamp` ist der lokale Offset.
    pub const LOCAL_TIMESTAMP: u8 = 5;
}

/// Werte des Enums `sport`.
pub mod sport {
    pub const RUNNING: i64 = 1;
    pub const CYCLING: i64 = 2;
    pub const SWIMMING: i64 = 5;
    pub const TRAINING: i64 = 10;
}

/// Werte des Enums `sub_sport`.
pub mod sub_sport {
    pub const GENERIC: i64 = 0;
    pub const ROAD: i64 = 7;
    pub const LAP_SWIMMING: i64 = 17;
    pub const STRENGTH_TRAINING: i64 = 20;
}

/// Werte des Enums `manufacturer`.
pub mod manufacturer {
    pub const GARMIN: i64 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use lastwerk_testkit::profile as kit;

    /// Die zwei Konstantenlisten dürfen nicht auseinanderlaufen, sonst läse der Decoder andere
    /// Felder, als der Encoder schreibt.
    #[test]
    fn constants_match_testkit() {
        assert_eq!(FIT_EPOCH_OFFSET_S, kit::FIT_EPOCH_OFFSET_S);
        assert_eq!(DATA_TYPE_TAG, kit::DATA_TYPE_TAG);
        assert_eq!(HEADER_SIZE_LONG, usize::from(kit::HEADER_SIZE));
        assert_eq!(RECORD_HEADER_DEFINITION, kit::RECORD_HEADER_DEFINITION);
        assert_eq!(
            RECORD_HEADER_DEVELOPER_DATA,
            kit::RECORD_HEADER_DEVELOPER_DATA
        );
        assert_eq!(RECORD_HEADER_LOCAL_MASK, kit::RECORD_HEADER_LOCAL_MASK);
        assert_eq!(ARCH_LITTLE_ENDIAN, kit::ARCH_LITTLE_ENDIAN);
        assert_eq!(BASE_ENUM, kit::BASE_ENUM & BASE_MASK);
        assert_eq!(BASE_SINT8, kit::BASE_SINT8 & BASE_MASK);
        assert_eq!(BASE_UINT8, kit::BASE_UINT8 & BASE_MASK);
        assert_eq!(BASE_SINT16, kit::BASE_SINT16 & BASE_MASK);
        assert_eq!(BASE_UINT16, kit::BASE_UINT16 & BASE_MASK);
        assert_eq!(BASE_SINT32, kit::BASE_SINT32 & BASE_MASK);
        assert_eq!(BASE_UINT32, kit::BASE_UINT32 & BASE_MASK);
        assert_eq!(BASE_UINT8Z, kit::BASE_UINT8Z & BASE_MASK);
        assert_eq!(BASE_UINT16Z, kit::BASE_UINT16Z & BASE_MASK);
        assert_eq!(BASE_UINT32Z, kit::BASE_UINT32Z & BASE_MASK);
        assert_eq!(MESG_FILE_ID, kit::MESG_FILE_ID);
        assert_eq!(MESG_SESSION, kit::MESG_SESSION);
        assert_eq!(MESG_LAP, kit::MESG_LAP);
        assert_eq!(MESG_RECORD, kit::MESG_RECORD);
        assert_eq!(MESG_DEVICE_INFO, kit::MESG_DEVICE_INFO);
        assert_eq!(MESG_ACTIVITY, kit::MESG_ACTIVITY);
        assert_eq!(FIELD_TIMESTAMP, kit::FIELD_TIMESTAMP);
        assert_eq!(file_id::TYPE, kit::file_id::TYPE);
        assert_eq!(file_id::MANUFACTURER, kit::file_id::MANUFACTURER);
        assert_eq!(file_id::PRODUCT, kit::file_id::PRODUCT);
        assert_eq!(file_id::SERIAL_NUMBER, kit::file_id::SERIAL_NUMBER);
        assert_eq!(file_id::TIME_CREATED, kit::file_id::TIME_CREATED);
        assert_eq!(
            file_id::FILE_TYPE_ACTIVITY,
            i64::from(kit::file_id::FILE_TYPE_ACTIVITY)
        );
        assert_eq!(device_info::DEVICE_INDEX, kit::device_info::DEVICE_INDEX);
        assert_eq!(device_info::MANUFACTURER, kit::device_info::MANUFACTURER);
        assert_eq!(device_info::SERIAL_NUMBER, kit::device_info::SERIAL_NUMBER);
        assert_eq!(device_info::PRODUCT, kit::device_info::PRODUCT);
        assert_eq!(record::HEART_RATE, kit::record::HEART_RATE);
        assert_eq!(record::SPEED, kit::record::SPEED);
        assert_eq!(record::POWER, kit::record::POWER);
        assert_eq!(lap::START_TIME, kit::lap::START_TIME);
        assert_eq!(lap::TOTAL_ELAPSED_TIME, kit::lap::TOTAL_ELAPSED_TIME);
        assert_eq!(lap::TOTAL_TIMER_TIME, kit::lap::TOTAL_TIMER_TIME);
        assert_eq!(lap::TOTAL_DISTANCE, kit::lap::TOTAL_DISTANCE);
        assert_eq!(session::START_TIME, kit::session::START_TIME);
        assert_eq!(session::SPORT, kit::session::SPORT);
        assert_eq!(session::SUB_SPORT, kit::session::SUB_SPORT);
        assert_eq!(
            session::TOTAL_ELAPSED_TIME,
            kit::session::TOTAL_ELAPSED_TIME
        );
        assert_eq!(session::TOTAL_TIMER_TIME, kit::session::TOTAL_TIMER_TIME);
        assert_eq!(session::TOTAL_DISTANCE, kit::session::TOTAL_DISTANCE);
        assert_eq!(session::POOL_LENGTH, kit::session::POOL_LENGTH);
        assert_eq!(activity::LOCAL_TIMESTAMP, kit::activity::LOCAL_TIMESTAMP);
        assert_eq!(sport::RUNNING, i64::from(kit::sport::RUNNING));
        assert_eq!(sport::CYCLING, i64::from(kit::sport::CYCLING));
        assert_eq!(sport::SWIMMING, i64::from(kit::sport::SWIMMING));
        assert_eq!(sport::TRAINING, i64::from(kit::sport::TRAINING));
        assert_eq!(sub_sport::GENERIC, i64::from(kit::sub_sport::GENERIC));
        assert_eq!(sub_sport::ROAD, i64::from(kit::sub_sport::ROAD));
        assert_eq!(
            sub_sport::LAP_SWIMMING,
            i64::from(kit::sub_sport::LAP_SWIMMING)
        );
        assert_eq!(
            sub_sport::STRENGTH_TRAINING,
            i64::from(kit::sub_sport::STRENGTH_TRAINING)
        );
        assert_eq!(manufacturer::GARMIN, i64::from(kit::manufacturer::GARMIN));
    }

    /// 0x1000_0000 = 16^7 = 268 435 456.
    #[test]
    fn min_date_time_matches_hand_calculation() {
        assert_eq!(MIN_DATE_TIME, 268_435_456);
    }
}
