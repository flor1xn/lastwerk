//! Encoder für FIT-Dateien: Dateikopf, Definitions- und Datennachrichten, Prüfsummen.
//!
//! Der Encoder kennt kein Profil. Er schreibt, was ihm die Aufrufer als Felddefinitionen und
//! Werte geben, und sorgt für die korrekte Byte-Form nach der öffentlichen
//! FIT-Protokollbeschreibung: Little Endian, normaler Rekordkopf, 14-Byte-Kopf mit eigener
//! Prüfsumme, Datei-Prüfsumme am Ende.

use crate::profile::{
    ARCH_LITTLE_ENDIAN, BASE_BYTE, BASE_ENUM, BASE_FLOAT32, BASE_SINT8, BASE_SINT16, BASE_SINT32,
    BASE_STRING, BASE_UINT8, BASE_UINT8Z, BASE_UINT16, BASE_UINT16Z, BASE_UINT32, BASE_UINT32Z,
    DATA_TYPE_TAG, HEADER_SIZE, PROTOCOL_VERSION_1_0, RECORD_HEADER_DEFINITION,
    RECORD_HEADER_LOCAL_MASK,
};

/// Profilversion im Dateikopf. Beispielwert ohne Bindung an eine bestimmte Profilausgabe,
/// das Feld ist für Leser rein informativ.
pub const PROFILE_VERSION: u16 = 2100;

/// Tabelle der CRC-16-Berechnung nach FIT-Protokollbeschreibung (Halbbyte-Verfahren,
/// reflektiertes Polynom 0xA001, Startwert 0). Entspricht CRC-16/ARC.
const CRC_TABLE: [u16; 16] = [
    0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401, 0xA001, 0x6C00, 0x7800, 0xB401,
    0x5000, 0x9C01, 0x8801, 0x4400,
];

/// Führt die FIT-Prüfsumme über ein Byte fort.
pub fn crc16_update(mut crc: u16, byte: u8) -> u16 {
    // Unteres Halbbyte
    let tmp = CRC_TABLE[usize::from(crc & 0x0F)];
    crc = (crc >> 4) & 0x0FFF;
    crc = crc ^ tmp ^ CRC_TABLE[usize::from(byte & 0x0F)];
    // Oberes Halbbyte
    let tmp = CRC_TABLE[usize::from(crc & 0x0F)];
    crc = (crc >> 4) & 0x0FFF;
    crc ^ tmp ^ CRC_TABLE[usize::from((byte >> 4) & 0x0F)]
}

/// FIT-Prüfsumme über einen Puffer, Startwert 0.
pub fn crc16(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0, |crc, &b| crc16_update(crc, b))
}

/// Basistyp eines Feldes. Strings und Bytefelder tragen ihre feste Länge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseType {
    Enum,
    Sint8,
    Uint8,
    Sint16,
    Uint16,
    Sint32,
    Uint32,
    /// Zeichenkette fester Länge, mit Nullbytes aufgefüllt.
    String(u8),
    Float32,
    Uint8z,
    Uint16z,
    Uint32z,
    /// Rohe Bytes fester Länge.
    Byte(u8),
}

impl BaseType {
    /// Kennbyte in der Felddefinition.
    pub fn code(self) -> u8 {
        match self {
            BaseType::Enum => BASE_ENUM,
            BaseType::Sint8 => BASE_SINT8,
            BaseType::Uint8 => BASE_UINT8,
            BaseType::Sint16 => BASE_SINT16,
            BaseType::Uint16 => BASE_UINT16,
            BaseType::Sint32 => BASE_SINT32,
            BaseType::Uint32 => BASE_UINT32,
            BaseType::String(_) => BASE_STRING,
            BaseType::Float32 => BASE_FLOAT32,
            BaseType::Uint8z => BASE_UINT8Z,
            BaseType::Uint16z => BASE_UINT16Z,
            BaseType::Uint32z => BASE_UINT32Z,
            BaseType::Byte(_) => BASE_BYTE,
        }
    }

    /// Feldlänge in Byte.
    pub fn size(self) -> u8 {
        match self {
            BaseType::Enum | BaseType::Sint8 | BaseType::Uint8 | BaseType::Uint8z => 1,
            BaseType::Sint16 | BaseType::Uint16 | BaseType::Uint16z => 2,
            BaseType::Sint32 | BaseType::Uint32 | BaseType::Uint32z | BaseType::Float32 => 4,
            BaseType::String(n) | BaseType::Byte(n) => n,
        }
    }

    /// Ungültiger Wert des Basistyps in Byte-Form (Little Endian).
    fn invalid_bytes(self) -> Vec<u8> {
        match self {
            BaseType::Enum | BaseType::Uint8 => vec![0xFF],
            BaseType::Sint8 => vec![0x7F],
            BaseType::Uint8z => vec![0x00],
            BaseType::Sint16 => 0x7FFFu16.to_le_bytes().to_vec(),
            BaseType::Uint16 => vec![0xFF, 0xFF],
            BaseType::Uint16z => vec![0x00, 0x00],
            BaseType::Sint32 => 0x7FFF_FFFFu32.to_le_bytes().to_vec(),
            BaseType::Uint32 | BaseType::Float32 => vec![0xFF; 4],
            BaseType::Uint32z => vec![0x00; 4],
            BaseType::String(n) | BaseType::Byte(n) => vec![0x00; usize::from(n)],
        }
    }
}

/// Felddefinition: Feldnummer aus dem Profil und Basistyp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDef {
    pub num: u8,
    pub base: BaseType,
}

impl FieldDef {
    pub const fn new(num: u8, base: BaseType) -> Self {
        Self { num, base }
    }
}

/// Wert eines Feldes. `Invalid` schreibt den ungültigen Wert des Basistyps.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    Str(String),
    Bytes(Vec<u8>),
    Invalid,
}

impl Value {
    /// Byte-Form des Werts für den gegebenen Basistyp. Typfehler sind Programmierfehler im
    /// Testkit und brechen ab, damit kein stiller Datenmüll entsteht.
    fn encode(&self, base: BaseType) -> Vec<u8> {
        match (self, base) {
            (Value::Invalid, b) => b.invalid_bytes(),
            (Value::U8(v), BaseType::Enum | BaseType::Uint8 | BaseType::Uint8z) => vec![*v],
            (Value::I8(v), BaseType::Sint8) => vec![v.to_le_bytes()[0]],
            (Value::U16(v), BaseType::Uint16 | BaseType::Uint16z) => v.to_le_bytes().to_vec(),
            (Value::I16(v), BaseType::Sint16) => v.to_le_bytes().to_vec(),
            (Value::U32(v), BaseType::Uint32 | BaseType::Uint32z) => v.to_le_bytes().to_vec(),
            (Value::I32(v), BaseType::Sint32) => v.to_le_bytes().to_vec(),
            (Value::F32(v), BaseType::Float32) => v.to_le_bytes().to_vec(),
            (Value::Str(s), BaseType::String(n)) => {
                let mut out = s.as_bytes().to_vec();
                let n = usize::from(n);
                assert!(
                    out.len() < n,
                    "Zeichenkette {s:?} passt nicht in {n} Byte samt Nullbyte"
                );
                out.resize(n, 0);
                out
            }
            (Value::Bytes(b), BaseType::Byte(n)) => {
                assert_eq!(b.len(), usize::from(n), "Bytefeld hat falsche Länge");
                b.clone()
            }
            (v, b) => panic!("Wert {v:?} passt nicht zu Basistyp {b:?}"),
        }
    }
}

/// Definition einer Nachricht unter einer lokalen Nummer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDef {
    pub global_num: u16,
    pub fields: Vec<FieldDef>,
}

/// Schreibt eine FIT-Datei in den Speicher.
///
/// Ablauf: `define` legt eine Nachrichtendefinition unter einer lokalen Nummer (0 bis 15) ab
/// und schreibt sie sofort als Definitionsnachricht. `write` schreibt eine Datennachricht zu
/// dieser lokalen Nummer. `finish` setzt Kopf und Prüfsummen und liefert die Bytes.
#[derive(Debug, Clone)]
pub struct FitWriter {
    protocol_version: u8,
    data: Vec<u8>,
    defs: Vec<Option<MessageDef>>,
}

impl Default for FitWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl FitWriter {
    /// Neuer Writer mit Protokollversion 1.0.
    pub fn new() -> Self {
        Self::with_protocol_version(PROTOCOL_VERSION_1_0)
    }

    /// Neuer Writer mit gegebener Protokollversion im Kopf (0x10 oder 0x20).
    pub fn with_protocol_version(protocol_version: u8) -> Self {
        Self {
            protocol_version,
            data: Vec::new(),
            defs: vec![None; 16],
        }
    }

    /// Legt eine Definition ab und schreibt die Definitionsnachricht.
    pub fn define(&mut self, local: u8, global_num: u16, fields: &[FieldDef]) {
        assert!(local <= RECORD_HEADER_LOCAL_MASK, "lokale Nummer über 15");
        assert!(fields.len() <= 255, "zu viele Felder");
        self.data.push(RECORD_HEADER_DEFINITION | local);
        self.data.push(0); // reserved
        self.data.push(ARCH_LITTLE_ENDIAN);
        self.data.extend_from_slice(&global_num.to_le_bytes());
        self.data.push(fields.len() as u8);
        for f in fields {
            self.data.push(f.num);
            self.data.push(f.base.size());
            self.data.push(f.base.code());
        }
        self.defs[usize::from(local)] = Some(MessageDef {
            global_num,
            fields: fields.to_vec(),
        });
    }

    /// Schreibt eine Datennachricht. Die Werte stehen in der Reihenfolge der Definition.
    pub fn write(&mut self, local: u8, values: &[Value]) {
        let def = self.defs[usize::from(local)]
            .as_ref()
            .unwrap_or_else(|| panic!("lokale Nummer {local} ist nicht definiert"));
        assert_eq!(
            values.len(),
            def.fields.len(),
            "Anzahl Werte passt nicht zur Definition von Nachricht {}",
            def.global_num
        );
        let mut body = Vec::new();
        for (value, field) in values.iter().zip(&def.fields) {
            body.extend(value.encode(field.base));
        }
        self.data.push(local & RECORD_HEADER_LOCAL_MASK);
        self.data.extend(body);
    }

    /// Fügt rohe Bytes in den Datenteil ein, zum Beispiel für bewusst kaputte Nachrichten.
    pub fn write_raw(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    /// Bisherige Länge des Datenteils in Byte.
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    /// Setzt Kopf und Prüfsummen und liefert die fertige Datei.
    pub fn finish(self) -> Vec<u8> {
        let data_size = u32::try_from(self.data.len()).expect("Datenteil über 4 GiB");
        let mut out = Vec::with_capacity(usize::from(HEADER_SIZE) + self.data.len() + 2);
        out.push(HEADER_SIZE);
        out.push(self.protocol_version);
        out.extend_from_slice(&PROFILE_VERSION.to_le_bytes());
        out.extend_from_slice(&data_size.to_le_bytes());
        out.extend_from_slice(&DATA_TYPE_TAG);
        let header_crc = crc16(&out);
        out.extend_from_slice(&header_crc.to_le_bytes());
        out.extend(self.data);
        let file_crc = crc16(&out);
        out.extend_from_slice(&file_crc.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prüfwert von CRC-16/ARC für die Zeichenkette "123456789" (allgemein veröffentlichter
    /// Referenzwert der CRC-Kataloge).
    const CRC_CHECK_123456789: u16 = 0xBB3D;

    #[test]
    fn crc_matches_published_check_value() {
        assert_eq!(crc16(b"123456789"), CRC_CHECK_123456789);
        assert_eq!(crc16(b""), 0);
    }

    #[test]
    fn empty_writer_produces_header_and_crc_only() {
        let bytes = FitWriter::new().finish();
        assert_eq!(bytes.len(), 16);
        assert_eq!(bytes[0], 14);
        assert_eq!(bytes[1], 0x10);
        assert_eq!(&bytes[4..8], &[0, 0, 0, 0]);
        assert_eq!(&bytes[8..12], b".FIT");
        let header_crc = u16::from_le_bytes([bytes[12], bytes[13]]);
        assert_eq!(header_crc, crc16(&bytes[..12]));
        let file_crc = u16::from_le_bytes([bytes[14], bytes[15]]);
        assert_eq!(file_crc, crc16(&bytes[..14]));
    }

    #[test]
    fn definition_and_data_have_expected_layout() {
        let mut w = FitWriter::new();
        w.define(
            3,
            20,
            &[
                FieldDef::new(253, BaseType::Uint32),
                FieldDef::new(7, BaseType::Uint16),
                FieldDef::new(3, BaseType::Uint8),
            ],
        );
        w.write(
            3,
            &[Value::U32(0x0102_0304), Value::U16(200), Value::Invalid],
        );
        let bytes = w.finish();
        let data = &bytes[14..bytes.len() - 2];
        // Definition: Kopf 0x43, reserved, arch, global 20 LE, 3 Felder, je 3 Byte
        assert_eq!(
            &data[..15],
            &[0x43, 0, 0, 20, 0, 3, 253, 4, 0x86, 7, 2, 0x84, 3, 1, 0x02]
        );
        // Daten: Kopf 0x03, Zeitstempel LE, Leistung LE, ungültige HF
        assert_eq!(&data[15..], &[0x03, 0x04, 0x03, 0x02, 0x01, 200, 0, 0xFF]);
        let data_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(data_size as usize, data.len());
    }

    #[test]
    #[should_panic(expected = "passt nicht zu Basistyp")]
    fn mismatched_value_type_panics() {
        let mut w = FitWriter::new();
        w.define(0, 0, &[FieldDef::new(0, BaseType::Uint8)]);
        w.write(0, &[Value::U32(1)]);
    }

    #[test]
    fn string_is_padded_with_zero_bytes() {
        assert_eq!(
            Value::Str("ab".into()).encode(BaseType::String(4)),
            vec![b'a', b'b', 0, 0]
        );
    }
}
