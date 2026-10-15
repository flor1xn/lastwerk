//! Tests der reinen Hilfsfunktionen. Die Befehle gegen eine Datenbank prüft
//! `tests/service.rs`.

use super::*;

fn sample(t_s: u32, power_w: Option<f32>, hr_bpm: Option<f32>) -> Sample {
    Sample {
        t_s,
        power_w,
        hr_bpm,
        speed_mps: None,
    }
}

/// Handrechnung: 6 Samples auf höchstens 3 Punkte gibt Blöcke von 2. Im ersten Block ergibt
/// Leistung 100 und 200 das Mittel (100 + 200) / 2 = 150. Im zweiten fehlt die Leistung bei
/// einem Sample, das Mittel zählt nur den vorhandenen Wert (300). Im dritten fehlt sie ganz,
/// das ergibt `None` und nicht 0. Herzfrequenz: (120 + 130) / 2 = 125, kein Wert im zweiten
/// Block, (150 + 151) / 2 = 150,5.
#[test]
fn downsample_averages_blocks_and_keeps_missing_values_missing() {
    let samples = [
        sample(0, Some(100.0), Some(120.0)),
        sample(1, Some(200.0), Some(130.0)),
        sample(2, None, None),
        sample(3, Some(300.0), None),
        sample(4, None, Some(150.0)),
        sample(5, None, Some(151.0)),
    ];
    let s = downsample(&samples, 3);
    assert_eq!(s.status, "ok");
    assert_eq!(s.t_s, [0, 2, 4]);
    assert_eq!(s.power_w, [Some(150.0), Some(300.0), None]);
    assert_eq!(s.hr_bpm, [Some(125.0), None, Some(150.5)]);
}

#[test]
fn downsample_never_exceeds_the_limit() {
    let samples: Vec<Sample> = (0..10_000).map(|t| sample(t, Some(200.0), None)).collect();
    for max in [1, 7, 800, 20_000] {
        let s = downsample(&samples, max);
        assert!(s.t_s.len() <= max.max(1), "max {max}: {}", s.t_s.len());
        assert_eq!(s.t_s.len(), s.power_w.len());
        assert_eq!(s.t_s.len(), s.hr_bpm.len());
    }
    assert!(downsample(&[], 800).t_s.is_empty());
}

#[test]
fn source_path_rejects_traversal_and_absolute_paths() {
    let dir = Path::new("/data/fit");
    assert_eq!(
        source_path(dir, "a/b.fit"),
        Some(PathBuf::from("/data/fit/a/b.fit"))
    );
    for bad in ["../x.fit", "a/../../x.fit", "/root/x.fit", "a//b.fit", ""] {
        assert_eq!(source_path(dir, bad), None, "{bad}");
    }
}

#[test]
fn known_sync_folders_are_recognised_in_any_case() {
    for p in [
        "/home/u/Dropbox/Lastwerk/lastwerk.db",
        "C:/Users/u/OneDrive - Firma/Lastwerk/lastwerk.db",
        "/Users/u/Library/Mobile Documents/com~apple~CloudDocs/x.db",
        "/home/u/Nextcloud/x.db",
    ] {
        assert!(in_sync_folder(Path::new(p)), "{p}");
    }
    for p in [
        "/home/u/.local/share/Lastwerk/lastwerk.db",
        "C:/Users/u/AppData/Local/Lastwerk/lastwerk.db",
        // Der Name muss mit dem Dienst beginnen, ein Ordner "mydropboxes" zählt nicht.
        "/home/u/mydropboxes/x.db",
    ] {
        assert!(!in_sync_folder(Path::new(p)), "{p}");
    }
}

#[test]
fn rtss_stss_and_session_rpe_are_flagged_as_estimated() {
    let row = |method: &str| LoadRow {
        activity_id: 1,
        method: method.to_owned(),
        value: Some(50.0),
        intensity_factor: None,
        threshold_id: None,
        status: LoadStatus::Ok,
        computed_at: DateTime::from_timestamp(0, 0).unwrap(),
    };
    for (method, estimated) in [
        ("tss", false),
        ("hr_tss", false),
        ("r_tss", true),
        ("s_tss", true),
        ("session_rpe", true),
    ] {
        let r = row(method);
        assert_eq!(load_info(&[&r]).estimated, estimated, "{method}");
    }
}

#[test]
fn a_failed_load_has_no_value_and_no_method() {
    let r = LoadRow {
        activity_id: 1,
        method: "none".to_owned(),
        value: None,
        intensity_factor: None,
        threshold_id: None,
        status: LoadStatus::MissingThreshold,
        computed_at: DateTime::from_timestamp(0, 0).unwrap(),
    };
    let info = load_info(&[&r]);
    assert_eq!(info.status, "missing_threshold");
    assert_eq!(info.value, None);
    assert_eq!(info.method, None);
    assert!(!info.estimated);
    // Gar keine Zeile ist ebenfalls kein Wert.
    let none = load_info(&[]);
    assert_eq!((none.status.as_str(), none.value), ("unknown", None));
}
