//! Minimaler Rücklese-Helfer für die Tests des Testkits.
//!
//! Kein Produktiv-Parser (das ist T03): prüft Kopf und Prüfsummen, folgt Definitionen und
//! zählt Datennachrichten je globaler Nummer. Felder werden roh als vorzeichenlose
//! Little-Endian-Zahl gelesen, ohne Skalierung und ohne Profilwissen.

use std::collections::HashMap;

use crate::encoder::crc16;
use crate::profile::{
    DATA_TYPE_TAG, HEADER_SIZE, RECORD_HEADER_DEFINITION, RECORD_HEADER_DEVELOPER_DATA,
    RECORD_HEADER_LOCAL_MASK,
};

/// Eine gelesene Datennachricht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub global: u16,
    /// Feldnummer, Basistyp, rohe Bytes.
    pub fields: Vec<(u8, u8, Vec<u8>)>,
}

impl Message {
    /// Feld als vorzeichenlose Little-Endian-Zahl, falls vorhanden.
    pub fn field(&self, num: u8) -> Option<u64> {
        self.fields
            .iter()
            .find(|(n, _, _)| *n == num)
            .map(|(_, _, b)| {
                b.iter()
                    .rev()
                    .fold(0u64, |acc, &byte| (acc << 8) | u64::from(byte))
            })
    }
}

/// Ergebnis des Rücklesens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub protocol_version: u8,
    pub data_size: u32,
    pub header_crc_ok: bool,
    pub file_crc_ok: bool,
    pub messages: Vec<Message>,
}

impl Decoded {
    /// Anzahl Datennachrichten mit dieser globalen Nummer.
    pub fn count(&self, global: u16) -> usize {
        self.messages.iter().filter(|m| m.global == global).count()
    }

    /// Erste Datennachricht mit dieser globalen Nummer.
    pub fn first(&self, global: u16) -> Option<&Message> {
        self.messages.iter().find(|m| m.global == global)
    }
}

#[derive(Debug, Clone)]
struct Def {
    global: u16,
    big_endian: bool,
    fields: Vec<(u8, u8, u8)>,
}

/// Prüft, ob ab `pos` noch `n` Byte vor `end` liegen.
fn need(pos: usize, n: usize, end: usize) -> Result<(), String> {
    (pos + n <= end)
        .then_some(())
        .ok_or_else(|| "Definition abgeschnitten".to_owned())
}

/// Liest eine FIT-Datei. Fehler bei zu kurzen oder strukturell kaputten Dateien, Prüfsummen
/// werden nur als Flags gemeldet.
pub fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    let header_size = usize::from(HEADER_SIZE);
    if bytes.len() < header_size {
        return Err(format!("Datei hat nur {} Byte", bytes.len()));
    }
    if bytes[0] != HEADER_SIZE {
        return Err(format!("Kopflänge {} statt 14", bytes[0]));
    }
    if bytes[8..12] != DATA_TYPE_TAG {
        return Err("Kennung .FIT fehlt".to_owned());
    }
    let data_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let header_crc = u16::from_le_bytes([bytes[12], bytes[13]]);
    let header_crc_ok = header_crc == crc16(&bytes[..12]);
    let end = header_size + data_size as usize;
    if bytes.len() < end + 2 {
        return Err(format!(
            "Datei ist abgeschnitten: {} Byte, erwartet {}",
            bytes.len(),
            end + 2
        ));
    }
    let file_crc = u16::from_le_bytes([bytes[end], bytes[end + 1]]);
    let file_crc_ok = file_crc == crc16(&bytes[..end]);

    let mut defs: HashMap<u8, Def> = HashMap::new();
    let mut messages = Vec::new();
    let mut pos = header_size;
    while pos < end {
        let head = bytes[pos];
        pos += 1;
        if head & 0x80 != 0 {
            return Err("komprimierter Zeitstempelkopf wird nicht unterstützt".to_owned());
        }
        let local = head & RECORD_HEADER_LOCAL_MASK;
        if head & RECORD_HEADER_DEFINITION != 0 {
            need(pos, 5, end)?;
            let big_endian = bytes[pos + 1] != 0;
            let global_bytes = [bytes[pos + 2], bytes[pos + 3]];
            let global = if big_endian {
                u16::from_be_bytes(global_bytes)
            } else {
                u16::from_le_bytes(global_bytes)
            };
            let n_fields = usize::from(bytes[pos + 4]);
            pos += 5;
            need(pos, n_fields * 3, end)?;
            let mut fields = Vec::with_capacity(n_fields);
            for i in 0..n_fields {
                let f = &bytes[pos + i * 3..pos + i * 3 + 3];
                fields.push((f[0], f[1], f[2]));
            }
            pos += n_fields * 3;
            if head & RECORD_HEADER_DEVELOPER_DATA != 0 {
                need(pos, 1, end)?;
                let n_dev = usize::from(bytes[pos]);
                pos += 1 + n_dev * 3;
            }
            defs.insert(
                local,
                Def {
                    global,
                    big_endian,
                    fields,
                },
            );
        } else {
            let def = defs
                .get(&local)
                .ok_or_else(|| format!("Daten ohne Definition für lokale Nummer {local}"))?;
            let mut fields = Vec::with_capacity(def.fields.len());
            for &(num, size, base) in &def.fields {
                let size = usize::from(size);
                if pos + size > end {
                    return Err("Datennachricht abgeschnitten".to_owned());
                }
                let mut raw = bytes[pos..pos + size].to_vec();
                if def.big_endian {
                    raw.reverse();
                }
                fields.push((num, base, raw));
                pos += size;
            }
            messages.push(Message {
                global: def.global,
                fields,
            });
        }
    }
    Ok(Decoded {
        protocol_version: bytes[1],
        data_size,
        header_crc_ok,
        file_crc_ok,
        messages,
    })
}
