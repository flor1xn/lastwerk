//! Fuzz-artige Tests (SPEC 10.2, 11.4): mutierte Dateien dürfen den Parser nie zum Absturz
//! bringen. Ob das Ergebnis `Ok` oder `Err` ist, spielt keine Rolle.

mod common;

use std::sync::OnceLock;

use common::{fix_crc, wrap};
use lastwerk_fit::parse_activity;
use lastwerk_testkit::fixtures::{self, Fixture};
use proptest::prelude::*;

fn all_fixtures() -> &'static [Fixture] {
    static ALL: OnceLock<Vec<Fixture>> = OnceLock::new();
    ALL.get_or_init(fixtures::all)
}

/// Flips: Position (als Bruchteil der Länge in Promille, damit sie zu jeder Fixture passt)
/// und XOR-Maske.
fn flips() -> impl Strategy<Value = Vec<(u16, u8)>> {
    prop::collection::vec((0u16..1000, 1u8..=255), 1..8)
}

fn apply(bytes: &mut [u8], flips: &[(u16, u8)]) {
    if bytes.is_empty() {
        return;
    }
    for &(permille, mask) in flips {
        let idx = (bytes.len() - 1) * usize::from(permille) / 1000;
        bytes[idx] ^= mask;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    /// Zufällige Bitflips mit alter Prüfsumme: meist `crc_mismatch`, nie Panik.
    #[test]
    fn bitflips_never_panic(idx in 0usize..64, flips in flips()) {
        let f = &all_fixtures()[idx % all_fixtures().len()];
        let mut bytes = f.bytes.clone();
        apply(&mut bytes, &flips);
        let _ = parse_activity(&bytes);
    }

    /// Bitflips mit neu berechneter Prüfsumme: der Decoder sieht die kaputten Nachrichten.
    #[test]
    fn bitflips_with_valid_crc_never_panic(idx in 0usize..64, flips in flips()) {
        let f = &all_fixtures()[idx % all_fixtures().len()];
        let mut bytes = f.bytes.clone();
        apply(&mut bytes, &flips);
        fix_crc(&mut bytes);
        let _ = parse_activity(&bytes);
    }

    /// Abschneiden an jeder Stelle, mit und ohne angepasste Prüfsumme.
    #[test]
    fn truncation_never_panics(idx in 0usize..64, permille in 0u32..=1000, fix in any::<bool>()) {
        let f = &all_fixtures()[idx % all_fixtures().len()];
        let keep = f.bytes.len() * permille as usize / 1000;
        let mut bytes = f.bytes[..keep].to_vec();
        if fix {
            fix_crc(&mut bytes);
        }
        let _ = parse_activity(&bytes);
    }

    /// Abgeschnittene Datei mit korrektem Kopf und Ende: `data_size` passt zur Länge.
    #[test]
    fn truncated_data_section_with_valid_framing_never_panics(
        idx in 0usize..64,
        permille in 0u32..=1000,
    ) {
        let f = &all_fixtures()[idx % all_fixtures().len()];
        if f.bytes.len() < 16 {
            return Ok(());
        }
        let data = &f.bytes[14..f.bytes.len() - 2];
        let keep = data.len() * permille as usize / 1000;
        let _ = parse_activity(&wrap(14, 0x10, &data[..keep]));
    }

    /// Zufällige Bytes in einem gültigen Rahmen: Definitionen und Daten sind frei erfunden.
    #[test]
    fn random_data_section_never_panics(
        data in prop::collection::vec(any::<u8>(), 0..2048),
        header_size in prop::sample::select(vec![12u8, 14]),
        protocol in prop::sample::select(vec![0x10u8, 0x20]),
    ) {
        let _ = parse_activity(&wrap(header_size, protocol, &data));
    }

    /// Zufällige Bytes ohne jeden Rahmen.
    #[test]
    fn random_bytes_never_panic(data in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = parse_activity(&data);
    }

    /// Zufällige Bytes hinter einem gültigen `file_id`, damit die Nachrichten nach ihm
    /// (Definitionen mit Entwicklerfeldern, komprimierte Köpfe) häufig erreicht werden.
    #[test]
    fn random_messages_after_file_id_never_panic(
        tail in prop::collection::vec(any::<u8>(), 0..1024),
    ) {
        let mut data = common::file_id_bytes(4);
        data.extend_from_slice(&tail);
        let _ = parse_activity(&wrap(14, 0x20, &data));
    }
}

/// Ohne Mutation: Grundprobe, damit ein Fehler im Hilfscode nicht als Erfolg der
/// Mutationstests durchgeht.
#[test]
fn unmutated_fixtures_still_parse() {
    let ok = all_fixtures()
        .iter()
        .filter(|f| parse_activity(&f.bytes).is_ok())
        .count();
    let expected = all_fixtures()
        .iter()
        .filter(|f| f.expect.error.is_none())
        .count();
    assert_eq!(ok, expected);
}

/// Jedes einzelne Byte jeder kleinen Fixture auf jeden Wert 0, 0xFF und Bitflip, mit neu
/// berechneter Prüfsumme. Vollständig statt zufällig; die Fixture ist klein genug dafür.
#[test]
fn every_byte_of_a_short_file_survives_mutation() {
    let base = lastwerk_testkit::generators::activity(
        &lastwerk_testkit::generators::ActivityParams::cycling(
            lastwerk_testkit::fixtures::START_FIT,
            7200,
            12,
        ),
    );
    for i in 0..base.len() {
        for mask in [0xFFu8, 0x01, 0x80, 0x55] {
            let mut bytes = base.clone();
            bytes[i] ^= mask;
            fix_crc(&mut bytes);
            let _ = parse_activity(&bytes);
        }
        let mut bytes = base.clone();
        bytes[i] = 0;
        fix_crc(&mut bytes);
        let _ = parse_activity(&bytes);
    }
}
