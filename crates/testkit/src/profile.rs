//! Konstanten des FIT-Protokolls und des FIT-Profils, soweit das Testkit sie braucht.
//!
//! Alle Werte nach öffentlicher FIT-Protokoll- und Profilbeschreibung. Es stehen nur die
//! tatsächlich benötigten Nachrichten- und Feldnummern hier, keine Profildateien und keine
//! SDK-Quellen (SPEC 14, O2). Produktnummern der Geräte sind frei erfunden, siehe
//! [`generators::Device`](crate::generators::Device).

/// Offset der FIT-Epoche (31. Dezember 1989 00:00:00 UTC) zur Unix-Epoche in Sekunden.
///
/// Rechenweg: 1990-01-01 liegt 7305 Tage nach 1970-01-01 (20 Jahre, davon fünf Schaltjahre
/// 1972, 1976, 1980, 1984, 1988: `20 * 365 + 5 = 7305`). Die FIT-Epoche ist der Tag davor,
/// also `7304 * 86400 = 631 065 600`.
pub const FIT_EPOCH_OFFSET_S: i64 = 631_065_600;

/// Rechnet einen FIT-Zeitstempel in Unix-Sekunden um.
pub const fn fit_to_unix(fit_timestamp: u32) -> i64 {
    fit_timestamp as i64 + FIT_EPOCH_OFFSET_S
}

// Dateikopf

/// Länge des Dateikopfs in Byte (Variante mit Prüfsumme).
pub const HEADER_SIZE: u8 = 14;
/// Protokollversion 1.0 (hohe Halbbyte = Hauptversion).
pub const PROTOCOL_VERSION_1_0: u8 = 0x10;
/// Protokollversion 2.0, ab der Entwicklerfelder zulässig sind.
pub const PROTOCOL_VERSION_2_0: u8 = 0x20;
/// Kennung ".FIT" im Dateikopf.
pub const DATA_TYPE_TAG: [u8; 4] = *b".FIT";

// Rekordkopf (normaler Kopf, Bit 7 = 0)

/// Bit 6: Definitionsnachricht (1) oder Datennachricht (0).
pub const RECORD_HEADER_DEFINITION: u8 = 0x40;
/// Bit 5: Definitionsnachricht enthält Entwicklerfelder.
pub const RECORD_HEADER_DEVELOPER_DATA: u8 = 0x20;
/// Bits 0 bis 3: lokale Nachrichtennummer.
pub const RECORD_HEADER_LOCAL_MASK: u8 = 0x0F;
/// Architektur in der Definitionsnachricht: 0 = Little Endian.
pub const ARCH_LITTLE_ENDIAN: u8 = 0;

// Basistypen (Byte in der Felddefinition)

pub const BASE_ENUM: u8 = 0x00;
pub const BASE_SINT8: u8 = 0x01;
pub const BASE_UINT8: u8 = 0x02;
pub const BASE_SINT16: u8 = 0x83;
pub const BASE_UINT16: u8 = 0x84;
pub const BASE_SINT32: u8 = 0x85;
pub const BASE_UINT32: u8 = 0x86;
pub const BASE_STRING: u8 = 0x07;
pub const BASE_FLOAT32: u8 = 0x88;
pub const BASE_UINT8Z: u8 = 0x0A;
pub const BASE_UINT16Z: u8 = 0x8B;
pub const BASE_UINT32Z: u8 = 0x8C;
pub const BASE_BYTE: u8 = 0x0D;

// Ungültige Werte je Basistyp

pub const INVALID_UINT8: u8 = 0xFF;
pub const INVALID_UINT16: u16 = 0xFFFF;
pub const INVALID_UINT32: u32 = 0xFFFF_FFFF;

// Globale Nachrichtennummern

pub const MESG_FILE_ID: u16 = 0;
pub const MESG_SESSION: u16 = 18;
pub const MESG_LAP: u16 = 19;
pub const MESG_RECORD: u16 = 20;
pub const MESG_DEVICE_INFO: u16 = 23;
pub const MESG_ACTIVITY: u16 = 34;
pub const MESG_MONITORING: u16 = 55;

/// Feldnummer `timestamp`, in allen Nachrichten gleich (uint32, Sekunden seit FIT-Epoche).
pub const FIELD_TIMESTAMP: u8 = 253;

/// Felder der Nachricht `file_id`.
pub mod file_id {
    pub const TYPE: u8 = 0;
    pub const MANUFACTURER: u8 = 1;
    pub const PRODUCT: u8 = 2;
    pub const SERIAL_NUMBER: u8 = 3;
    pub const TIME_CREATED: u8 = 4;

    /// Werte des Enums `file`.
    pub const FILE_TYPE_ACTIVITY: u8 = 4;
    pub const FILE_TYPE_MONITORING_B: u8 = 32;
}

/// Felder der Nachricht `device_info`.
pub mod device_info {
    pub const DEVICE_INDEX: u8 = 0;
    pub const MANUFACTURER: u8 = 2;
    pub const SERIAL_NUMBER: u8 = 3;
    pub const PRODUCT: u8 = 4;

    /// `device_index` des aufzeichnenden Geräts selbst.
    pub const DEVICE_INDEX_CREATOR: u8 = 0;
}

/// Felder der Nachricht `record`. Skalierungen stehen bei den Konstanten.
pub mod record {
    /// uint8, bpm.
    pub const HEART_RATE: u8 = 3;
    /// uint8, Umdrehungen je Minute. Der Parser liest das Feld nicht, es kostet ihn nur
    /// seine Länge; die Messdaten (T38) brauchen es für eine realistische Rekordgröße.
    pub const CADENCE: u8 = 4;
    /// uint32, Meter mal 100.
    pub const DISTANCE: u8 = 5;
    /// uint16, Meter je Sekunde mal 1000.
    pub const SPEED: u8 = 6;
    /// uint16, Watt.
    pub const POWER: u8 = 7;

    pub const SPEED_SCALE: f64 = 1000.0;
    pub const DISTANCE_SCALE: f64 = 100.0;
}

/// Felder der Nachricht `lap`. Die Summenfelder teilen sich Nummer und Skalierung mit `session`.
pub mod lap {
    pub const START_TIME: u8 = 2;
    /// uint32, Sekunden mal 1000.
    pub const TOTAL_ELAPSED_TIME: u8 = 7;
    /// uint32, Sekunden mal 1000.
    pub const TOTAL_TIMER_TIME: u8 = 8;
    /// uint32, Meter mal 100.
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
    pub const POOL_LENGTH_SCALE: f64 = 100.0;
}

/// Felder der Nachricht `activity`.
pub mod activity {
    /// uint32, Sekunden mal 1000.
    pub const TOTAL_TIMER_TIME: u8 = 0;
    pub const NUM_SESSIONS: u8 = 1;
    /// uint32, Ortszeit als FIT-Zeitstempel; Differenz zu `timestamp` ist der lokale Offset.
    pub const LOCAL_TIMESTAMP: u8 = 5;
}

/// Felder der Nachricht `monitoring`.
pub mod monitoring {
    /// uint8, bpm.
    pub const HEART_RATE: u8 = 27;
}

/// Werte des Enums `sport`.
pub mod sport {
    pub const RUNNING: u8 = 1;
    pub const CYCLING: u8 = 2;
    pub const SWIMMING: u8 = 5;
    pub const TRAINING: u8 = 10;
}

/// Werte des Enums `sub_sport`.
pub mod sub_sport {
    pub const GENERIC: u8 = 0;
    pub const ROAD: u8 = 7;
    pub const LAP_SWIMMING: u8 = 17;
    pub const STRENGTH_TRAINING: u8 = 20;
}

/// Werte des Enums `manufacturer`.
pub mod manufacturer {
    pub const GARMIN: u16 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1990-01-01 00:00:00 UTC ist Unix 631 152 000 (`7305 * 86400`), ein Tag davor
    /// ergibt 631 065 600.
    const EXPECTED_EPOCH_OFFSET_S: i64 = 7304 * 86_400;

    #[test]
    fn epoch_offset_matches_hand_calculation() {
        assert_eq!(FIT_EPOCH_OFFSET_S, EXPECTED_EPOCH_OFFSET_S);
        assert_eq!(fit_to_unix(0), FIT_EPOCH_OFFSET_S);
    }
}
