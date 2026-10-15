//! Aufbau der Datei: Kopf, Byte-Reihenfolge, Entwicklerfelder, komprimierte Zeitstempel,
//! unbekannte Nachrichten und die Fehlerfälle. Dateien entstehen aus handgeschriebenen
//! Bytefolgen oder über den Encoder des Testkits.

mod common;

use common::{START_FIT, data_of, file_id_bytes, wrap, write_file_id};
use lastwerk_core::load::Sport;
use lastwerk_fit::{FitError, MAX_FILE_BYTES, parse_activity};
use lastwerk_testkit::encoder::{BaseType, FieldDef, FitWriter, Value};

const T: u32 = START_FIT;

/// Definition (Little Endian) von `record` unter lokaler Nummer 2: Zeitstempel, Leistung,
/// Herzfrequenz.
fn record_def_le() -> Vec<u8> {
    vec![0x42, 0, 0, 20, 0, 3, 253, 4, 0x86, 7, 2, 0x84, 3, 1, 0x02]
}

fn record_le(ts: u32, power: u16, hr: u8) -> Vec<u8> {
    let mut v = vec![0x02];
    v.extend_from_slice(&ts.to_le_bytes());
    v.extend_from_slice(&power.to_le_bytes());
    v.push(hr);
    v
}

fn activity_file(tail: &[u8]) -> Vec<u8> {
    let mut data = file_id_bytes(4);
    data.extend_from_slice(tail);
    wrap(14, 0x10, &data)
}

fn err(bytes: &[u8]) -> FitError {
    parse_activity(bytes).expect_err("Fehler erwartet")
}

// Kopf

#[test]
fn twelve_byte_header_is_accepted() {
    let mut tail = record_def_le();
    tail.extend(record_le(T, 150, 120));
    let mut data = file_id_bytes(4);
    data.extend(tail);
    let a = parse_activity(&wrap(12, 0x10, &data)).unwrap();
    assert_eq!(a.samples.len(), 1);
    assert_eq!(a.samples[0].power_w, Some(150.0));
}

#[test]
fn protocol_major_one_and_two_are_accepted_others_not() {
    let mut tail = record_def_le();
    tail.extend(record_le(T, 1, 60));
    let mut data = file_id_bytes(4);
    data.extend(tail);
    assert!(parse_activity(&wrap(14, 0x10, &data)).is_ok());
    assert!(parse_activity(&wrap(14, 0x21, &data)).is_ok());
    assert_eq!(err(&wrap(14, 0x30, &data)), FitError::UnsupportedVersion(3));
    assert_eq!(err(&wrap(14, 0x00, &data)).code(), "unsupported_version");
}

#[test]
fn bad_header_is_invalid() {
    let good = activity_file(&[]);
    // Kopflänge 13
    let mut bytes = good.clone();
    bytes[0] = 13;
    assert_eq!(err(&bytes), FitError::InvalidHeader);
    // Kennung beschädigt
    let mut bytes = good.clone();
    bytes[8] = b'X';
    assert_eq!(err(&bytes), FitError::InvalidHeader);
}

#[test]
fn header_crc_is_checked_unless_zero() {
    let good = activity_file(&[]);
    let mut bad = good.clone();
    bad[12] ^= 0xFF;
    assert_eq!(err(&bad), FitError::CrcMismatch);
    // Prüfsumme 0 heißt "nicht berechnet": der Kopf wird nicht geprüft, die Datei-Prüfsumme
    // muss dann neu gesetzt sein, da sie den Kopf abdeckt.
    let mut zero = good.clone();
    zero[12] = 0;
    zero[13] = 0;
    let n = zero.len();
    let crc = lastwerk_testkit::encoder::crc16(&zero[..n - 2]);
    zero[n - 2..].copy_from_slice(&crc.to_le_bytes());
    // Fehler "keine Aktivität" statt Prüfsummenfehler: der Kopf wurde akzeptiert
    assert_eq!(err(&zero), FitError::NoActivity);
}

#[test]
fn short_and_oversized_inputs() {
    assert_eq!(err(&[]), FitError::EmptyFile);
    assert_eq!(err(&[14]), FitError::Truncated);
    assert_eq!(err(&[0; 5]), FitError::InvalidHeader);
    let too_large = vec![0u8; usize::try_from(MAX_FILE_BYTES).unwrap() + 1];
    let e = err(&too_large);
    assert_eq!(e.code(), "too_large");
    assert_eq!(
        e,
        FitError::TooLarge {
            size: MAX_FILE_BYTES + 1,
            limit: MAX_FILE_BYTES
        }
    );
}

#[test]
fn data_size_larger_than_file_is_truncated() {
    let mut bytes = activity_file(&record_def_le());
    // data_size um 100 erhöhen, Kopf-Prüfsumme neu setzen
    let size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) + 100;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    common::fix_crc(&mut bytes);
    assert_eq!(err(&bytes), FitError::Truncated);
    // data_size über 4 GiB minus Kopf: kein Überlauf
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    common::fix_crc(&mut bytes);
    assert_eq!(err(&bytes), FitError::Truncated);
}

#[test]
fn trailing_bytes_after_the_file_are_ignored() {
    let mut tail = record_def_le();
    tail.extend(record_le(T, 1, 60));
    let mut bytes = activity_file(&tail);
    let single = parse_activity(&bytes).unwrap();
    bytes.extend_from_slice(&[0xAA; 20]);
    assert_eq!(parse_activity(&bytes).unwrap(), single);
}

// Nachrichten

#[test]
fn big_endian_definitions_are_read() {
    // Definition von `record` in Big Endian unter lokaler Nummer 1
    let mut tail = vec![
        0x41, 0, 1, 0x00, 0x14, 3, 253, 4, 0x86, 7, 2, 0x84, 3, 1, 0x02,
    ];
    for (i, power) in [(0u32, 250u16), (1, 258), (2, 0x0102)] {
        tail.push(0x01);
        tail.extend_from_slice(&(T + i).to_be_bytes());
        tail.extend_from_slice(&power.to_be_bytes());
        tail.push(100 + i as u8);
    }
    let a = parse_activity(&activity_file(&tail)).unwrap();
    let power: Vec<_> = a.samples.iter().map(|s| s.power_w).collect();
    // 0x0102 = 258
    assert_eq!(power, vec![Some(250.0), Some(258.0), Some(258.0)]);
    let hr: Vec<_> = a.samples.iter().map(|s| s.hr_bpm).collect();
    assert_eq!(hr, vec![Some(100.0), Some(101.0), Some(102.0)]);
    assert_eq!(a.start_utc.timestamp(), i64::from(T) + 631_065_600);
}

#[test]
fn big_endian_global_number_is_swapped() {
    // Global 0x1400 in Big Endian wäre 20 gelesen als Little Endian 0x0014 verwechselt:
    // hier steht 20 als [0x00, 0x14] mit Architektur 1 und muss `record` sein.
    let tail = [
        vec![0x41, 0, 1, 0x00, 0x14, 1, 253, 4, 0x86],
        vec![0x01],
        T.to_be_bytes().to_vec(),
    ]
    .concat();
    let a = parse_activity(&activity_file(&tail)).unwrap();
    assert_eq!(a.samples.len(), 1);
}

#[test]
fn developer_fields_are_skipped() {
    // Kopf 0x62 = Definition (0x40) + Entwicklerdaten (0x20) + lokal 2. Ein Entwicklerfeld
    // von 3 Byte (Feld 0, Größe 3, Entwicklerindex 0) folgt in den Daten nach den Standardfeldern.
    let mut tail = vec![
        0x62, 0, 0, 20, 0, 3, 253, 4, 0x86, 7, 2, 0x84, 3, 1, 0x02, 1, 0, 3, 0,
    ];
    for i in 0..3u32 {
        tail.push(0x02);
        tail.extend_from_slice(&(T + i).to_le_bytes());
        tail.extend_from_slice(&(200 + i as u16).to_le_bytes());
        tail.push(90);
        tail.extend_from_slice(&[0xDE, 0xAD, 0xBE]); // Entwicklerdaten
    }
    let a = parse_activity(&activity_file(&tail)).unwrap();
    let power: Vec<_> = a.samples.iter().map(|s| s.power_w).collect();
    assert_eq!(power, vec![Some(200.0), Some(201.0), Some(202.0)]);
    assert!(a.samples.iter().all(|s| s.hr_bpm == Some(90.0)));
}

#[test]
fn unknown_messages_and_fields_are_skipped() {
    // Unbekannte globale Nummer 65280 (Bereich für Hersteller) unter lokaler Nummer 5 mit
    // einem Stringfeld und einem Float, dazwischen und danach gültige Records.
    let mut tail = record_def_le();
    tail.extend(record_le(T, 100, 100));
    tail.extend([0x45, 0, 0, 0x00, 0xFF, 2, 9, 4, 0x07, 10, 4, 0x88]);
    tail.extend([0x05, b'a', b'b', b'c', 0, 0, 0, 0xC0, 0x3F]);
    tail.extend(record_le(T + 1, 101, 101));
    // Zusätzlich ein Feld unbekannter Nummer in `record` (Feld 99, uint8) wird ignoriert.
    tail.extend([0x43, 0, 0, 20, 0, 2, 253, 4, 0x86, 99, 1, 0x02]);
    tail.extend([0x03]);
    tail.extend_from_slice(&(T + 2).to_le_bytes());
    tail.push(7);
    let a = parse_activity(&activity_file(&tail)).unwrap();
    assert_eq!(a.samples.len(), 3);
    assert_eq!(a.samples[1].power_w, Some(101.0));
    assert_eq!(a.samples[2].power_w, None);
}

#[test]
fn compressed_timestamp_headers_continue_the_clock() {
    // Normale Nachricht setzt den Bezug auf T. Danach 70 Records mit komprimiertem Kopf, deren
    // Versatz jede Sekunde um 1 steigt (5 Bit, Überlauf nach 31). Erwartung: lückenlos T..T+70.
    let mut tail = record_def_le();
    tail.extend(record_le(T, 100, 100));
    // Definition ohne Zeitstempel unter lokaler Nummer 1 (komprimierte Köpfe erreichen 0 bis 3)
    tail.extend([0x41, 0, 0, 20, 0, 1, 7, 2, 0x84]);
    for i in 1..=70u32 {
        let offset = ((T + i) & 0x1F) as u8;
        tail.push(0x80 | (1 << 5) | offset);
        tail.extend_from_slice(&(100 + i as u16).to_le_bytes());
    }
    let a = parse_activity(&activity_file(&tail)).unwrap();
    assert_eq!(a.samples.len(), 71);
    for (i, s) in a.samples.iter().enumerate() {
        assert_eq!(s.t_s as usize, i);
        assert_eq!(s.power_w, Some(100.0 + i as f32));
    }
    assert_eq!(a.plausibility.short_gaps + a.plausibility.long_gaps, 0);
}

#[test]
fn compressed_header_without_reference_is_skipped_not_fatal() {
    // Kein voller Zeitstempel vorher: der Record bekommt keinen Zeitpunkt und fällt weg.
    let mut tail = vec![0x41, 0, 0, 20, 0, 1, 7, 2, 0x84];
    tail.extend([0x80 | (1 << 5) | 3, 100, 0]);
    tail.extend(record_def_le());
    tail.extend(record_le(T, 5, 50));
    let a = parse_activity(&activity_file(&tail)).unwrap();
    assert_eq!(a.samples.len(), 1);
}

#[test]
fn redefinition_replaces_the_old_definition() {
    let mut tail = record_def_le();
    tail.extend(record_le(T, 100, 100));
    // lokale Nummer 2 neu: nur Zeitstempel
    tail.extend([0x42, 0, 0, 20, 0, 1, 253, 4, 0x86]);
    tail.push(0x02);
    tail.extend_from_slice(&(T + 1).to_le_bytes());
    let a = parse_activity(&activity_file(&tail)).unwrap();
    assert_eq!(a.samples.len(), 2);
    assert_eq!(a.samples[1].power_w, None);
}

#[test]
fn invalid_values_become_none() {
    let mut tail = record_def_le();
    tail.extend([0x02]);
    tail.extend_from_slice(&T.to_le_bytes());
    tail.extend_from_slice(&0xFFFFu16.to_le_bytes()); // Leistung ungültig
    tail.push(0xFF); // Herzfrequenz ungültig
    let a = parse_activity(&activity_file(&tail)).unwrap();
    assert_eq!(a.samples[0].power_w, None);
    assert_eq!(a.samples[0].hr_bpm, None);
}

// Aufbereitung

#[test]
fn same_second_last_record_wins_and_order_is_restored() {
    let mut tail = record_def_le();
    tail.extend(record_le(T + 2, 12, 60));
    tail.extend(record_le(T, 10, 60));
    tail.extend(record_le(T + 1, 11, 60));
    tail.extend(record_le(T + 1, 99, 60)); // zweiter Wert derselben Sekunde
    let a = parse_activity(&activity_file(&tail)).unwrap();
    let power: Vec<_> = a.samples.iter().map(|s| s.power_w).collect();
    assert_eq!(power, vec![Some(10.0), Some(99.0), Some(12.0)]);
    let t: Vec<_> = a.samples.iter().map(|s| s.t_s).collect();
    assert_eq!(t, vec![0, 1, 2]);
}

#[test]
fn enhanced_speed_wins_over_speed() {
    let mut tail = vec![0x42, 0, 0, 20, 0, 3, 253, 4, 0x86, 6, 2, 0x84, 73, 4, 0x86];
    tail.push(0x02);
    tail.extend_from_slice(&T.to_le_bytes());
    tail.extend_from_slice(&1000u16.to_le_bytes());
    tail.extend_from_slice(&5500u32.to_le_bytes());
    tail.push(0x02);
    tail.extend_from_slice(&(T + 1).to_le_bytes());
    tail.extend_from_slice(&2000u16.to_le_bytes());
    tail.extend_from_slice(&u32::MAX.to_le_bytes()); // erweitert ungültig, speed gilt
    let a = parse_activity(&activity_file(&tail)).unwrap();
    // 5500 / 1000 = 5,5 m/s und 2000 / 1000 = 2 m/s
    assert_eq!(a.samples[0].speed_mps, Some(5.5));
    assert_eq!(a.samples[1].speed_mps, Some(2.0));
}

#[test]
fn sport_message_is_the_fallback_without_session() {
    let mut tail = record_def_le();
    tail.extend(record_le(T, 1, 60));
    // sport: Feld 0 = 1 (Laufen), Feld 1 = 3 (Unterart)
    tail.extend([0x43, 0, 0, 12, 0, 2, 0, 1, 0x00, 1, 1, 0x00]);
    tail.extend([0x03, 1, 3]);
    let a = parse_activity(&activity_file(&tail)).unwrap();
    assert_eq!(a.sport, Sport::Running);
    assert_eq!(a.sub_sport.as_deref(), Some("sub_sport_3"));
}

#[test]
fn no_sport_anywhere_is_other() {
    let mut tail = record_def_le();
    tail.extend(record_le(T, 1, 60));
    let a = parse_activity(&activity_file(&tail)).unwrap();
    assert_eq!(a.sport, Sport::Other);
    assert_eq!(a.sub_sport, None);
}

#[test]
fn training_without_strength_subsport_is_other() {
    let mut tail = record_def_le();
    tail.extend(record_le(T, 1, 60));
    tail.extend([0x43, 0, 0, 12, 0, 2, 0, 1, 0x00, 1, 1, 0x00]);
    tail.extend([0x03, 10, 26]);
    assert_eq!(
        parse_activity(&activity_file(&tail)).unwrap().sport,
        Sport::Other
    );
}

#[test]
fn local_offset_comes_from_activity_message() {
    let mut tail = record_def_le();
    tail.extend(record_le(T, 1, 60));
    // activity: timestamp (253) und local_timestamp (5), Offset -3600 (UTC-1)
    tail.extend([0x43, 0, 0, 34, 0, 2, 253, 4, 0x86, 5, 4, 0x86]);
    tail.push(0x03);
    tail.extend_from_slice(&(T + 1).to_le_bytes());
    tail.extend_from_slice(&(T + 1 - 3600).to_le_bytes());
    let a = parse_activity(&activity_file(&tail)).unwrap();
    assert_eq!(a.local_offset_s, -3600);
}

#[test]
fn absurd_local_offset_is_malformed() {
    let mut tail = record_def_le();
    tail.extend(record_le(T, 1, 60));
    tail.extend([0x43, 0, 0, 34, 0, 2, 253, 4, 0x86, 5, 4, 0x86]);
    tail.push(0x03);
    tail.extend_from_slice(&(T + 1).to_le_bytes());
    tail.extend_from_slice(&(T + 1 + 200_000).to_le_bytes());
    assert_eq!(err(&activity_file(&tail)).code(), "malformed");
}

#[test]
fn relative_system_time_is_not_a_date() {
    let mut tail = record_def_le();
    tail.extend(record_le(1000, 1, 60));
    assert_eq!(err(&activity_file(&tail)).code(), "malformed");
}

// Fehlerfälle der Nachrichten

#[test]
fn data_without_definition_is_malformed() {
    let tail = [0x03, 1, 2, 3];
    assert_eq!(err(&activity_file(&tail)).code(), "malformed");
}

#[test]
fn message_beyond_data_section_is_malformed() {
    // Definition meldet 4 Byte, es folgen nur 2
    let mut tail = vec![0x42, 0, 0, 20, 0, 1, 253, 4, 0x86];
    tail.extend([0x02, 1, 2]);
    assert_eq!(err(&activity_file(&tail)).code(), "malformed");
}

#[test]
fn unknown_architecture_is_malformed() {
    let tail = [0x42, 0, 7, 20, 0, 0];
    assert_eq!(err(&activity_file(&tail)).code(), "malformed");
}

#[test]
fn missing_file_id_is_malformed() {
    let mut data = record_def_le();
    data.extend(record_le(T, 1, 60));
    assert_eq!(err(&wrap(14, 0x10, &data)).code(), "malformed");
}

#[test]
fn other_file_types_are_no_activity() {
    // 32 = monitoring_b, 34 = activity_summary, 1 = device
    for t in [32u8, 34, 1, 0] {
        let mut data = file_id_bytes(t);
        data.extend(record_def_le());
        data.extend(record_le(T, 1, 60));
        assert_eq!(err(&wrap(14, 0x10, &data)), FitError::NoActivity, "{t}");
    }
}

#[test]
fn activity_file_without_records_and_session_is_no_activity() {
    assert_eq!(err(&activity_file(&[])), FitError::NoActivity);
}

#[test]
fn session_without_records_is_readable_with_empty_samples() {
    let mut w = FitWriter::new();
    write_file_id(&mut w, 4);
    w.define(
        1,
        18,
        &[
            FieldDef::new(2, BaseType::Uint32),
            FieldDef::new(5, BaseType::Enum),
            FieldDef::new(7, BaseType::Uint32),
        ],
    );
    w.write(1, &[Value::U32(T), Value::U8(2), Value::U32(60_000)]);
    let a = parse_activity(&w.finish()).unwrap();
    assert!(a.samples.is_empty());
    assert_eq!(a.sport, Sport::Cycling);
    assert_eq!(a.start_utc.timestamp(), i64::from(T) + 631_065_600);
    assert_eq!(a.duration_s(), Some(60.0));
    assert!(!a.has_power());
}

#[test]
fn chained_data_of_encoder_matches_manual_framing() {
    // Der Encoder des Testkits und `wrap` erzeugen für denselben Datenteil dieselbe Datei,
    // abgesehen von der Profilversion im Kopf (hier 2100 gegen 0x0834 = 2100).
    let mut w = FitWriter::new();
    write_file_id(&mut w, 4);
    let file = w.finish();
    assert_eq!(wrap(14, 0x10, data_of(&file)), file);
}
