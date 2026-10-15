//! Gemeinsame Hilfen der Integrationstests: Dateien aus Bytefolgen bauen.
#![allow(dead_code)]

use lastwerk_testkit::encoder::{BaseType, FieldDef, FitWriter, Value, crc16};
use lastwerk_testkit::fixtures::ErrorCode;

/// Fehlercode des Parsers zu einer Erwartung der Fixtures.
pub fn code_of(e: ErrorCode) -> &'static str {
    match e {
        ErrorCode::CrcMismatch => "crc_mismatch",
        ErrorCode::Truncated => "truncated",
        ErrorCode::EmptyFile => "empty_file",
        ErrorCode::NoActivity => "no_activity",
    }
}

/// Fester Startzeitpunkt der handgebauten Dateien: derselbe wie in den Fixtures
/// (10. Mai 2026 06:00:00 UTC, FIT-Zeit).
pub const START_FIT: u32 = lastwerk_testkit::fixtures::START_FIT;

/// Schreibt `file_id` mit Typ `activity` (4) und Hersteller Garmin.
pub fn write_file_id(w: &mut FitWriter, file_type: u8) {
    w.define(
        0,
        0,
        &[
            FieldDef::new(0, BaseType::Enum),
            FieldDef::new(1, BaseType::Uint16),
            FieldDef::new(2, BaseType::Uint16),
            FieldDef::new(3, BaseType::Uint32z),
        ],
    );
    w.write(
        0,
        &[
            Value::U8(file_type),
            Value::U16(1),
            Value::U16(777),
            Value::U32(123_456_789),
        ],
    );
}

/// Baut aus rohem Datenteil eine vollständige Datei mit gewähltem Kopf.
pub fn wrap(header_size: u8, protocol: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![header_size, protocol, 0x34, 0x08];
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(b".FIT");
    if header_size == 14 {
        let crc = crc16(&out);
        out.extend_from_slice(&crc.to_le_bytes());
    }
    out.extend_from_slice(data);
    let crc = crc16(&out);
    out.extend_from_slice(&crc.to_le_bytes());
    out
}

/// Datenteil einer Datei, aus der der Writer Kopf und Prüfsummen schon gesetzt hat.
pub fn data_of(file: &[u8]) -> &[u8] {
    &file[14..file.len() - 2]
}

/// Datenteil von `file_id` mit Typ `activity`, als Bytes zum Anhängen weiterer Nachrichten.
pub fn file_id_bytes(file_type: u8) -> Vec<u8> {
    let mut w = FitWriter::new();
    write_file_id(&mut w, file_type);
    data_of(&w.finish()).to_vec()
}

/// Setzt Kopf- und Dateiprüfsumme nach einer Änderung neu, soweit die Datei dafür lang genug
/// ist. So erreichen Mutationen den Decoder statt an der Prüfsumme zu scheitern.
pub fn fix_crc(bytes: &mut [u8]) {
    if bytes.len() >= 14 && bytes[0] == 14 {
        let crc = crc16(&bytes[..12]);
        bytes[12..14].copy_from_slice(&crc.to_le_bytes());
    }
    if bytes.len() >= 2 {
        let n = bytes.len() - 2;
        let crc = crc16(&bytes[..n]);
        bytes[n..].copy_from_slice(&crc.to_le_bytes());
    }
}
