//! Niedrige Ebene: Dateikopf, Prüfsummen und Nachrichten eines FIT-Containers.
//!
//! Der Decoder kennt kein Profil. Er liefert zu jeder Datennachricht die globale Nummer und
//! die gültigen Ganzzahlfelder, den Rest erledigt `activity`. Alle Zugriffe auf Eingabebytes
//! laufen über `get`, Größen über `checked_*`, jede Schleife schreitet je Durchlauf um
//! mindestens ein Byte fort. Damit gibt es auf manipulierten Dateien keine Panik und keine
//! Endlosschleife (SPEC 10.2).
//!
//! Aufbau nach öffentlicher FIT-Protokollbeschreibung: Kopf (12 oder 14 Byte), Datenteil aus
//! Definitions- und Datennachrichten, zwei Byte Prüfsumme über Kopf und Datenteil.

use crate::error::FitError;
use crate::profile::{
    ARCH_BIG_ENDIAN, ARCH_LITTLE_ENDIAN, BASE_ENUM, BASE_MASK, BASE_SINT8, BASE_SINT16,
    BASE_SINT32, BASE_UINT8, BASE_UINT8Z, BASE_UINT16, BASE_UINT16Z, BASE_UINT32, BASE_UINT32Z,
    COMPRESSED_LOCAL_MASK, COMPRESSED_LOCAL_SHIFT, COMPRESSED_OFFSET_MASK, DATA_TYPE_TAG,
    FIELD_TIMESTAMP, HEADER_SIZE_LONG, HEADER_SIZE_SHORT, RECORD_HEADER_COMPRESSED,
    RECORD_HEADER_DEFINITION, RECORD_HEADER_DEVELOPER_DATA, RECORD_HEADER_LOCAL_MASK,
};

/// Reflektiertes Polynom der FIT-Prüfsumme (CRC-16/ARC, Startwert 0).
const CRC_POLY_REFLECTED: u16 = 0xA001;

/// Tabelle für ein ganzes Byte je Schritt, beim Übersetzen aus dem Polynom berechnet.
///
/// Die FIT-Protokollbeschreibung zeigt ein Halbbyte-Verfahren mit 16 Einträgen. Beide
/// Verfahren rechnen dieselbe CRC-16/ARC. Der Test `byte_table_matches_nibble_method`
/// vergleicht sie.
const CRC_TABLE: [u16; 256] = {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u16;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ CRC_POLY_REFLECTED
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Tabellen für acht Byte je Schritt ("slicing by 8"). `CRC_SLICES[k][b]` ist die Wirkung
/// des Bytes `b`, auf das noch `k` Nullbytes folgen; `CRC_SLICES[0]` ist [`CRC_TABLE`].
///
/// Grund: Die Prüfsumme läuft über jede Datei ganz, bei einer Kette von Tabellenschritten
/// je Byte war sie der größte Einzelposten des Parsers (T38, Anhang Messung: 77 MB in rund
/// 400 ms mit dem Halbbyte-Verfahren, rund 210 ms mit einer Bytetabelle, rund 55 ms mit
/// acht Tabellen). Die acht Nachschläge eines Schritts hängen nicht voneinander ab.
const CRC_SLICES: [[u16; 256]; 8] = {
    let mut tables = [[0u16; 256]; 8];
    tables[0] = CRC_TABLE;
    let mut k = 1;
    while k < 8 {
        let mut i = 0;
        while i < 256 {
            let prev = tables[k - 1][i];
            tables[k][i] = (prev >> 8) ^ CRC_TABLE[(prev & 0xFF) as usize];
            i += 1;
        }
        k += 1;
    }
    tables
};

/// Ein Byte weiter, Grundschritt der reflektierten CRC.
fn crc16_byte(crc: u16, byte: u8) -> u16 {
    (crc >> 8) ^ CRC_TABLE[usize::from((crc ^ u16::from(byte)) as u8)]
}

/// FIT-Prüfsumme über einen Puffer, Startwert 0.
///
/// Je acht Byte ein Schritt: Die 16 Bit der bisherigen Prüfsumme wirken nur auf die ersten
/// zwei Byte des Blocks, jedes Byte geht über die Tabelle seines Abstands zum Blockende ein.
/// Der Rest unter acht Byte läuft byteweise.
pub(crate) fn crc16(bytes: &[u8]) -> u16 {
    let t = &CRC_SLICES;
    let (blocks, rest) = bytes.as_chunks::<8>();
    let mut crc = 0u16;
    for b in blocks {
        let head = crc ^ u16::from_le_bytes([b[0], b[1]]);
        crc = t[7][usize::from(head as u8)]
            ^ t[6][usize::from(head >> 8)]
            ^ t[5][usize::from(b[2])]
            ^ t[4][usize::from(b[3])]
            ^ t[3][usize::from(b[4])]
            ^ t[2][usize::from(b[5])]
            ^ t[1][usize::from(b[6])]
            ^ t[0][usize::from(b[7])];
    }
    rest.iter().fold(crc, |crc, &b| crc16_byte(crc, b))
}

/// Liest zwei Byte als Little-Endian-Zahl.
fn le16(bytes: &[u8], at: usize) -> Option<u16> {
    let raw: [u8; 2] = bytes.get(at..at.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_le_bytes(raw))
}

/// Prüft Kopf und Prüfsummen und liefert den Datenteil.
///
/// Reihenfolge der Prüfungen, jede mit eigenem Fehler:
///
/// 1. Kopflänge 12 oder 14 und Kennung `.FIT`: sonst `InvalidHeader`. Zu kurz für den Kopf:
///    `Truncated`.
/// 2. Hauptversion des Protokolls 1 oder 2: sonst `UnsupportedVersion`.
/// 3. Kopf-Prüfsumme, falls der Kopf 14 Byte hat und die Prüfsumme nicht 0 ist (0 bedeutet
///    "nicht berechnet"): sonst `CrcMismatch`.
/// 4. Länge: Kopf plus `data_size` plus 2 Byte Prüfsumme muss in die Datei passen, sonst
///    `Truncated`. Überstehende Bytes danach werden ignoriert (aneinandergehängte Dateien,
///    Konvention: nur die erste Datei zählt).
/// 5. Datei-Prüfsumme über Kopf und Datenteil: sonst `CrcMismatch`.
pub(crate) fn data_section(bytes: &[u8]) -> Result<&[u8], FitError> {
    let header_size = usize::from(*bytes.first().ok_or(FitError::EmptyFile)?);
    if header_size != HEADER_SIZE_SHORT && header_size != HEADER_SIZE_LONG {
        return Err(FitError::InvalidHeader);
    }
    let header = bytes.get(..header_size).ok_or(FitError::Truncated)?;
    if header.get(8..12) != Some(&DATA_TYPE_TAG[..]) {
        return Err(FitError::InvalidHeader);
    }
    let protocol = *header.get(1).ok_or(FitError::InvalidHeader)?;
    let major = protocol >> 4;
    if major != 1 && major != 2 {
        return Err(FitError::UnsupportedVersion(major));
    }
    if header_size == HEADER_SIZE_LONG {
        let stored = le16(header, 12).ok_or(FitError::InvalidHeader)?;
        if stored != 0 && stored != crc16(header.get(..12).ok_or(FitError::InvalidHeader)?) {
            return Err(FitError::CrcMismatch);
        }
    }
    let raw_size: [u8; 4] = header
        .get(4..8)
        .and_then(|s| s.try_into().ok())
        .ok_or(FitError::InvalidHeader)?;
    let data_size =
        usize::try_from(u32::from_le_bytes(raw_size)).map_err(|_| FitError::Truncated)?;
    let data_end = header_size
        .checked_add(data_size)
        .ok_or(FitError::Truncated)?;
    let file_end = data_end.checked_add(2).ok_or(FitError::Truncated)?;
    if bytes.len() < file_end {
        return Err(FitError::Truncated);
    }
    let covered = bytes.get(..data_end).ok_or(FitError::Truncated)?;
    let stored = le16(bytes, data_end).ok_or(FitError::Truncated)?;
    if stored != crc16(covered) {
        return Err(FitError::CrcMismatch);
    }
    bytes.get(header_size..data_end).ok_or(FitError::Truncated)
}

/// Eine Datennachricht: globale Nummer und die gültigen Ganzzahlfelder (Feldnummer, Wert).
///
/// Felder mit ungültigem Wert (Basistyp-Wert "invalid") fehlen. Strings, Gleitkommazahlen,
/// Byte-Felder und 64-Bit-Typen werden nicht gelesen, das Profil von Lastwerk braucht sie
/// nicht. Bei Feldern mit mehreren Elementen (Arrays) zählt das erste Element.
#[derive(Debug, Default)]
pub(crate) struct Message {
    pub global: u16,
    pub fields: Vec<(u8, i64)>,
}

impl Message {
    /// Wert des Feldes, falls vorhanden und gültig. Bei doppelter Feldnummer gewinnt das letzte.
    pub fn get(&self, num: u8) -> Option<i64> {
        self.fields
            .iter()
            .rev()
            .find(|(n, _)| *n == num)
            .map(|(_, v)| *v)
    }
}

#[derive(Debug, Clone, Copy)]
struct FieldDef {
    num: u8,
    size: u8,
    base: u8,
}

#[derive(Debug)]
struct Definition {
    global: u16,
    big_endian: bool,
    fields: Vec<FieldDef>,
    /// Summe aller Feldgrößen samt Entwicklerfeldern: so viele Byte folgen dem Rekordkopf.
    body_len: usize,
}

/// Nimmt `n` Byte ab `pos` und rückt vor.
fn take<'a>(data: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], FitError> {
    let end = pos
        .checked_add(n)
        .ok_or_else(|| malformed("Länge läuft über"))?;
    let out = data
        .get(*pos..end)
        .ok_or_else(|| malformed("Nachricht reicht über den Datenteil hinaus"))?;
    *pos = end;
    Ok(out)
}

fn malformed(why: &str) -> FitError {
    FitError::Malformed(why.to_owned())
}

/// Liest `N` Byte als Zahl in der Byte-Reihenfolge der Definition.
fn read_int<const N: usize>(raw: &[u8], big_endian: bool) -> Option<[u8; N]> {
    let mut a: [u8; N] = raw.get(..N)?.try_into().ok()?;
    if big_endian {
        a.reverse();
    }
    Some(a)
}

/// Liest den Wert eines Feldes. `None` bei ungültigem Wert (Ungültigwert des Basistyps), bei
/// nicht gelesenen Typen und bei zu kurzem Feld.
///
/// Ungültigwerte nach FIT-Protokollbeschreibung: Ganzzahlen ohne `z` haben den größten
/// Wert des Typs (vorzeichenlos alle Bits 1, `sint*` `0x7F..`), `z`-Typen die 0.
fn read_value(raw: &[u8], base: u8, big_endian: bool) -> Option<i64> {
    match base & BASE_MASK {
        BASE_ENUM | BASE_UINT8 => {
            let v = *raw.first()?;
            (v != 0xFF).then_some(i64::from(v))
        }
        BASE_UINT8Z => {
            let v = *raw.first()?;
            (v != 0).then_some(i64::from(v))
        }
        BASE_SINT8 => {
            let v = *raw.first()?;
            (v != 0x7F).then_some(i64::from(i8::from_le_bytes([v])))
        }
        BASE_UINT16 => {
            let v = u16::from_le_bytes(read_int::<2>(raw, big_endian)?);
            (v != u16::MAX).then_some(i64::from(v))
        }
        BASE_UINT16Z => {
            let v = u16::from_le_bytes(read_int::<2>(raw, big_endian)?);
            (v != 0).then_some(i64::from(v))
        }
        BASE_SINT16 => {
            let v = i16::from_le_bytes(read_int::<2>(raw, big_endian)?);
            (v != i16::MAX).then_some(i64::from(v))
        }
        BASE_UINT32 => {
            let v = u32::from_le_bytes(read_int::<4>(raw, big_endian)?);
            (v != u32::MAX).then_some(i64::from(v))
        }
        BASE_UINT32Z => {
            let v = u32::from_le_bytes(read_int::<4>(raw, big_endian)?);
            (v != 0).then_some(i64::from(v))
        }
        BASE_SINT32 => {
            let v = i32::from_le_bytes(read_int::<4>(raw, big_endian)?);
            (v != i32::MAX).then_some(i64::from(v))
        }
        _ => None,
    }
}

/// Rekonstruiert den Zeitstempel einer Nachricht mit komprimiertem Kopf.
///
/// Konvention nach FIT-Protokollbeschreibung: Die unteren fünf Bit des letzten vollen
/// Zeitstempels werden durch den Versatz aus dem Kopf ersetzt. Ist der Versatz kleiner als
/// diese fünf Bit des Bezugs, ist der Zähler übergelaufen und es kommen 32 Sekunden dazu.
/// Rechenbeispiel: Bezug 1000 (`...0 0011 1110 1000`, untere fünf Bit 8), Versatz 12 ergibt
/// 1004, Versatz 3 ergibt `1000 - 8 + 3 + 32 = 1027`.
fn expand_compressed_timestamp(reference: u32, offset: u8) -> Option<u32> {
    let mask = u32::from(COMPRESSED_OFFSET_MASK);
    let offset = u32::from(offset) & mask;
    let base = reference & !mask;
    let mut ts = base.checked_add(offset)?;
    if offset < (reference & mask) {
        ts = ts.checked_add(mask + 1)?;
    }
    Some(ts)
}

/// Läuft über den Datenteil und ruft `on_message` für jede Datennachricht auf.
///
/// Definitionen werden je lokaler Nummer (0 bis 15) gemerkt und ersetzt, wenn eine neue
/// Definition unter derselben Nummer folgt. Unbekannte globale Nachrichten und Felder
/// kosten nur ihre Länge. Entwicklerfelder werden übersprungen (Header-Bit 5). Der Callback
/// kann den Lauf mit einem Fehler abbrechen.
pub(crate) fn walk<F>(data: &[u8], mut on_message: F) -> Result<(), FitError>
where
    F: FnMut(&Message) -> Result<(), FitError>,
{
    let mut defs: [Option<Definition>; 16] = std::array::from_fn(|_| None);
    let mut last_timestamp: Option<u32> = None;
    let mut msg = Message::default();
    let mut pos = 0usize;

    while pos < data.len() {
        let head = *data.get(pos).ok_or_else(|| malformed("Kopf fehlt"))?;
        pos += 1;

        if head & RECORD_HEADER_COMPRESSED != 0 {
            let local = (head >> COMPRESSED_LOCAL_SHIFT) & COMPRESSED_LOCAL_MASK;
            let offset = head & COMPRESSED_OFFSET_MASK;
            let def = defs
                .get(usize::from(local))
                .and_then(Option::as_ref)
                .ok_or_else(|| malformed("Datennachricht ohne Definition"))?;
            read_data(data, &mut pos, def, &mut msg)?;
            if let Some(reference) = last_timestamp {
                let ts = expand_compressed_timestamp(reference, offset)
                    .ok_or_else(|| malformed("komprimierter Zeitstempel läuft über"))?;
                msg.fields.push((FIELD_TIMESTAMP, i64::from(ts)));
                last_timestamp = Some(ts);
            }
            on_message(&msg)?;
        } else if head & RECORD_HEADER_DEFINITION != 0 {
            let local = usize::from(head & RECORD_HEADER_LOCAL_MASK);
            let def = read_definition(data, &mut pos, head & RECORD_HEADER_DEVELOPER_DATA != 0)?;
            if let Some(slot) = defs.get_mut(local) {
                *slot = Some(def);
            }
        } else {
            let local = usize::from(head & RECORD_HEADER_LOCAL_MASK);
            let def = defs
                .get(local)
                .and_then(Option::as_ref)
                .ok_or_else(|| malformed("Datennachricht ohne Definition"))?;
            read_data(data, &mut pos, def, &mut msg)?;
            if let Some(ts) = msg.get(FIELD_TIMESTAMP)
                && let Ok(ts) = u32::try_from(ts)
            {
                last_timestamp = Some(ts);
            }
            on_message(&msg)?;
        }
    }
    Ok(())
}

fn read_definition(
    data: &[u8],
    pos: &mut usize,
    has_developer_fields: bool,
) -> Result<Definition, FitError> {
    let fixed = take(data, pos, 5)?;
    // Byte 0 ist reserviert, Byte 1 die Architektur, dann die globale Nummer, dann die Feldzahl.
    let arch = *fixed
        .get(1)
        .ok_or_else(|| malformed("Definition zu kurz"))?;
    let big_endian = match arch {
        ARCH_LITTLE_ENDIAN => false,
        ARCH_BIG_ENDIAN => true,
        _ => return Err(malformed("unbekannte Architektur")),
    };
    let global_raw = [
        *fixed
            .get(2)
            .ok_or_else(|| malformed("Definition zu kurz"))?,
        *fixed
            .get(3)
            .ok_or_else(|| malformed("Definition zu kurz"))?,
    ];
    let global = if big_endian {
        u16::from_be_bytes(global_raw)
    } else {
        u16::from_le_bytes(global_raw)
    };
    let n_fields = usize::from(
        *fixed
            .get(4)
            .ok_or_else(|| malformed("Definition zu kurz"))?,
    );

    let mut fields = Vec::with_capacity(n_fields);
    let mut body_len = 0usize;
    for &[num, size, base] in take(data, pos, n_fields * 3)?.as_chunks::<3>().0 {
        body_len = body_len
            .checked_add(usize::from(size))
            .ok_or_else(|| malformed("Länge läuft über"))?;
        fields.push(FieldDef { num, size, base });
    }

    if has_developer_fields {
        let n_dev = usize::from(
            *take(data, pos, 1)?
                .first()
                .ok_or_else(|| malformed("Definition zu kurz"))?,
        );
        // Entwicklerfelder: Feldnummer, Größe, Index der Entwicklerdaten. Nur die Größe zählt,
        // die Werte werden übersprungen.
        for &[_num, size, _index] in take(data, pos, n_dev * 3)?.as_chunks::<3>().0 {
            body_len = body_len
                .checked_add(usize::from(size))
                .ok_or_else(|| malformed("Länge läuft über"))?;
        }
    }

    Ok(Definition {
        global,
        big_endian,
        fields,
        body_len,
    })
}

/// Liest den Rumpf einer Datennachricht nach ihrer Definition in `msg`.
fn read_data(
    data: &[u8],
    pos: &mut usize,
    def: &Definition,
    msg: &mut Message,
) -> Result<(), FitError> {
    let body = take(data, pos, def.body_len)?;
    msg.global = def.global;
    msg.fields.clear();
    let mut offset = 0usize;
    for f in &def.fields {
        let end = offset
            .checked_add(usize::from(f.size))
            .ok_or_else(|| malformed("Länge läuft über"))?;
        let raw = body
            .get(offset..end)
            .ok_or_else(|| malformed("Feld reicht über die Nachricht hinaus"))?;
        if let Some(v) = read_value(raw, f.base, def.big_endian) {
            msg.fields.push((f.num, v));
        }
        offset = end;
    }
    Ok(())
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

    /// Halbbyte-Verfahren der FIT-Protokollbeschreibung, bis T38 die Implementierung,
    /// jetzt nur Referenz für den Gleichheitstest.
    fn crc16_nibble(bytes: &[u8]) -> u16 {
        const NIBBLE_TABLE: [u16; 16] = [
            0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401, 0xA001, 0x6C00, 0x7800,
            0xB401, 0x5000, 0x9C01, 0x8801, 0x4400,
        ];
        bytes.iter().fold(0, |crc, &byte| {
            let tmp = NIBBLE_TABLE[usize::from(crc & 0x0F)];
            let crc = ((crc >> 4) & 0x0FFF) ^ tmp ^ NIBBLE_TABLE[usize::from(byte & 0x0F)];
            let tmp = NIBBLE_TABLE[usize::from(crc & 0x0F)];
            ((crc >> 4) & 0x0FFF) ^ tmp ^ NIBBLE_TABLE[usize::from((byte >> 4) & 0x0F)]
        })
    }

    /// Bytetabelle, acht Tabellen und Halbbyte-Verfahren liefern dieselbe Prüfsumme: für
    /// jedes einzelne Byte, für jede Länge bis 64 (alle Reste unter acht Byte, auch mit
    /// versetztem Anfang) und für Puffer aus einem linearen Kongruenzgenerator (Konstanten
    /// nach Knuth, MMIX) mit fester Saat.
    #[test]
    fn byte_table_matches_nibble_method() {
        for b in 0..=u8::MAX {
            assert_eq!(crc16(&[b]), crc16_nibble(&[b]), "Byte {b}");
            assert_eq!(crc16_byte(0, b), crc16_nibble(&[b]), "Byte {b}");
            assert_eq!(crc16(&[b, 0x5A, b]), crc16_nibble(&[b, 0x5A, b]));
            let block = [b, !b, 0x5A, b, 0xA5, b, 0x00, 0xFF, b];
            assert_eq!(crc16(&block), crc16_nibble(&block), "Block mit {b}");
        }
        let mut state: u64 = 0x5EED_0038;
        let mut bytes = Vec::with_capacity(4096);
        for _ in 0..4096 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            bytes.push((state >> 56) as u8);
        }
        for len in (0..=64).chain([100, 4095, 4096]) {
            assert_eq!(crc16(&bytes[..len]), crc16_nibble(&bytes[..len]), "{len}");
        }
        for start in 1..8 {
            let part = &bytes[start..start + 61];
            assert_eq!(crc16(part), crc16_nibble(part), "Anfang {start}");
        }
        assert_eq!(crc16_nibble(b"123456789"), CRC_CHECK_123456789);
    }

    #[test]
    fn compressed_timestamp_follows_convention() {
        // Bezug 1000: untere fünf Bit = 1000 mod 32 = 8, Basis = 992.
        assert_eq!(expand_compressed_timestamp(1000, 12), Some(992 + 12));
        // Versatz 3 < 8: Überlauf, 992 + 3 + 32 = 1027.
        assert_eq!(expand_compressed_timestamp(1000, 3), Some(1027));
        // Gleicher Versatz wie der Bezug: keine Zeit vergangen.
        assert_eq!(expand_compressed_timestamp(1000, 8), Some(1000));
        assert_eq!(expand_compressed_timestamp(u32::MAX, 0), None);
    }

    #[test]
    fn invalid_values_are_dropped() {
        assert_eq!(read_value(&[0xFF], 0x02, false), None);
        assert_eq!(read_value(&[0xFE], 0x02, false), Some(254));
        assert_eq!(read_value(&[0x00], 0x0A, false), None);
        assert_eq!(read_value(&[0xFF, 0xFF], 0x84, false), None);
        assert_eq!(read_value(&[0xFF, 0x7F], 0x83, false), None);
        assert_eq!(read_value(&[0xFF, 0xFF, 0xFF, 0xFF], 0x86, true), None);
        assert_eq!(read_value(&[0, 0, 0, 0], 0x8C, false), None);
    }

    #[test]
    fn byte_order_follows_definition() {
        // 0x0102 = 258
        assert_eq!(read_value(&[0x02, 0x01], 0x84, false), Some(258));
        assert_eq!(read_value(&[0x01, 0x02], 0x84, true), Some(258));
        // sint16 -2 = 0xFFFE
        assert_eq!(read_value(&[0xFE, 0xFF], 0x83, false), Some(-2));
        assert_eq!(read_value(&[0xFF, 0xFE], 0x83, true), Some(-2));
        // zu kurzes Feld
        assert_eq!(read_value(&[0x01], 0x84, false), None);
        // Array: erstes Element zählt
        assert_eq!(read_value(&[0x07, 0x00, 0x09, 0x00], 0x84, false), Some(7));
    }

    #[test]
    fn short_inputs_never_panic() {
        for len in 0..20 {
            let bytes = vec![0u8; len];
            let _ = data_section(&bytes);
        }
    }
}
