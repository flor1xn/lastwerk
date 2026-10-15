//! Drosselung und Sperre. Die Uhr ist falsch und das Schlafen kostet keine Zeit; geprüft wird,
//! wie lange die Drossel *schlafen würde*.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use chrono::NaiveDate;
use common::{FakeClock, FixedJitter, MARKER, Reply, Rig, START_S, scratch_dir, tokens};
use lastwerk_garmin::{Clock, GarminError, Throttle, ThrottleConfig, code};
use serde_json::json;

const LOCK_S: i64 = 6 * 3600;

fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn throttle(jitter: f64) -> (Throttle, Arc<FakeClock>) {
    let clock = FakeClock::new();
    let throttle = Throttle::new(
        scratch_dir("th").join("lockout.json"),
        ThrottleConfig::default(),
        clock.clone(),
        Arc::new(FixedJitter(jitter)),
    );
    (throttle, clock)
}

/// Server, der die Statuszahlen der Reihe nach liefert und danach immer 200.
fn scripted(statuses: Vec<u16>) -> Rig {
    let next = Arc::new(AtomicUsize::new(0));
    Rig::connected(
        move |_| {
            let i = next.fetch_add(1, Ordering::SeqCst);
            match statuses.get(i).copied().unwrap_or(200) {
                200 => Reply::json(200, &json!({"displayName": "u"})),
                status => Reply::status(status),
            }
        },
        // 27 Stunden gültig, damit auch Tests über die Sperrfrist von 6 Stunden nicht erneuern.
        tokens(100_000, "r"),
    )
}

// Mindestabstand

// Referenz: Mindestabstand 1500 ms, Zufallsanteil 500 ms * Bruchteil.
// Bruchteil 0: 1500 ms. Bruchteil 0,999: 1500 + 499 (abgerundet) = 1999 ms.
#[test]
fn spacing_is_1500_ms_plus_jitter_up_to_500_ms() {
    let (t, clock) = throttle(0.0);
    t.before_request().unwrap();
    assert!(clock.sleeps().is_empty(), "der erste Aufruf wartet nicht");
    t.before_request().unwrap();
    t.before_request().unwrap();
    assert_eq!(clock.sleeps(), [ms(1500), ms(1500)]);

    let (t, clock) = throttle(0.999);
    t.before_request().unwrap();
    t.before_request().unwrap();
    assert_eq!(clock.sleeps(), [ms(1999)]);
}

#[test]
fn time_that_already_passed_counts_towards_the_spacing() {
    let (t, clock) = throttle(0.0);
    t.before_request().unwrap();
    clock.advance(ms(1000));
    t.before_request().unwrap();
    assert_eq!(
        clock.sleeps(),
        [ms(500)],
        "1500 ms Abstand minus 1000 ms vergangen"
    );
    clock.advance(Duration::from_secs(10));
    t.before_request().unwrap();
    assert_eq!(clock.sleeps().len(), 1, "nach 10 s kein Warten");
}

#[test]
fn consecutive_client_calls_are_spaced_without_real_waiting() {
    let rig = scripted(vec![]);
    let started = std::time::Instant::now();
    for _ in 0..4 {
        rig.client.social_profile().unwrap();
    }
    assert_eq!(rig.clock.sleeps(), [ms(1500), ms(1500), ms(1500)]);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "die Tests dürfen nicht wirklich warten"
    );
}

#[test]
fn a_backwards_jumping_clock_cannot_make_the_throttle_sleep_for_hours() {
    let (t, clock) = throttle(0.0);
    t.before_request().unwrap();
    // Die Wanduhr springt eine Stunde zurück: Der letzte Aufruf liegt scheinbar in der Zukunft.
    clock.set_s(START_S - 3600);
    t.before_request().unwrap();
    assert_eq!(
        clock.sleeps(),
        [Duration::from_secs(30)],
        "Obergrenze statt 3601,5 s"
    );
}

// Pause nach 429 oder 403

#[test]
fn a_429_adds_a_30_second_pause_before_the_next_call() {
    let (t, clock) = throttle(0.0);
    t.before_request().unwrap();
    t.after_response(Some(429)).unwrap();
    t.before_request().unwrap();
    assert_eq!(clock.sleeps(), [Duration::from_secs(30)]);
}

// Sperre

// Referenz: Drei Antworten 429 in Folge. Die dritte löst die Sperre aus, Ende = Zeitpunkt der
// dritten Antwort + 6 h = + 21 600 s.
#[test]
fn third_block_in_a_row_locks_for_six_hours_and_survives_a_restart() {
    let rig = scripted(vec![429, 429, 429]);

    assert_eq!(
        rig.client.social_profile().unwrap_err(),
        GarminError::Throttled
    );
    assert_eq!(
        rig.client.social_profile().unwrap_err(),
        GarminError::Throttled
    );
    let third = rig.client.social_profile().unwrap_err();

    let now_s = rig.clock.now_ms() / 1000;
    let GarminError::Locked { until } = third else {
        panic!("{third:?}");
    };
    assert_eq!(until.timestamp(), now_s + LOCK_S);
    assert_eq!(third.code(), code::LOCKED);
    assert_eq!(rig.gets().len(), 3);
    // Zwischen den Antworten lagen 30 s Pause (zusätzlich zu den 1,5 s Abstand, die darin liegen).
    assert_eq!(
        rig.clock.sleeps(),
        [Duration::from_secs(30), Duration::from_secs(30)]
    );

    // Sperrdatei: Ende, Status, Zähler. Kein Token, keine Adresse.
    let file = std::fs::read_to_string(&rig.lockout_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&file).unwrap();
    assert_eq!(json["until_epoch_s"], now_s + LOCK_S);
    assert_eq!(json["http_status"], 429);
    assert_eq!(json["consecutive"], 3);
    assert!(
        !file.contains(MARKER) && !file.contains("127.0.0.1"),
        "{file}"
    );

    // Solange sie gilt, geht nichts mehr raus, auch nicht für Tausch und Daten, auch nach Neustart.
    let slept = rig.clock.total_slept();
    let restarted = rig.restart();
    for client in [&rig.client, &restarted] {
        assert!(matches!(
            client.social_profile(),
            Err(GarminError::Locked { .. })
        ));
        assert!(matches!(
            client.connect(&common::ticket()),
            Err(GarminError::Locked { .. })
        ));
        assert!(matches!(
            client.activity_fit(1),
            Err(GarminError::Locked { .. })
        ));
        assert_eq!(client.active_lockout(), Some(until));
    }
    assert_eq!(
        rig.server.count(),
        3,
        "kein weiterer Aufruf während der Sperre"
    );
    assert_eq!(
        rig.clock.total_slept(),
        slept,
        "gesperrte Aufrufe schlafen nicht"
    );

    // Kurz vor dem Ende noch gesperrt, danach frei.
    rig.clock.advance(Duration::from_secs(LOCK_S as u64 - 1));
    assert!(matches!(
        rig.client.social_profile(),
        Err(GarminError::Locked { .. })
    ));
    rig.clock.advance(Duration::from_secs(2));
    assert_eq!(rig.client.active_lockout(), None);
    assert_eq!(rig.client.social_profile().unwrap().display_name, "u");
    assert_eq!(rig.server.count(), 4);
}

#[test]
fn mixed_429_and_403_count_together() {
    let rig = scripted(vec![429, 403, 403]);
    assert_eq!(
        rig.client.social_profile().unwrap_err(),
        GarminError::Throttled
    );
    assert_eq!(
        rig.client.social_profile().unwrap_err(),
        GarminError::Throttled
    );
    assert!(matches!(
        rig.client.social_profile(),
        Err(GarminError::Locked { .. })
    ));
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&rig.lockout_file).unwrap()).unwrap();
    assert_eq!(json["http_status"], 403);
}

#[test]
fn any_other_response_resets_the_count() {
    // 429, 429, dann Erfolg, dann wieder 429, 429: noch keine Sperre.
    let rig = scripted(vec![429, 429, 200, 429, 429]);
    for _ in 0..2 {
        assert_eq!(
            rig.client.social_profile().unwrap_err(),
            GarminError::Throttled
        );
    }
    assert!(rig.client.social_profile().is_ok());
    for _ in 0..2 {
        assert_eq!(
            rig.client.social_profile().unwrap_err(),
            GarminError::Throttled
        );
    }
    assert!(!rig.lockout_file.exists());

    // Auch 404 ist eine echte Antwort und setzt zurück; 500 ebenso.
    for reset in [404, 500] {
        let rig = scripted(vec![429, 429, reset, 429]);
        let results: Vec<_> = (0..4)
            .map(|_| rig.client.social_profile().unwrap_err())
            .collect();
        assert!(
            results
                .iter()
                .all(|e| !matches!(e, GarminError::Locked { .. })),
            "{reset}: {results:?}"
        );
        assert!(!rig.lockout_file.exists());
    }
}

#[test]
fn network_errors_neither_count_nor_reset() {
    let (t, _clock) = throttle(0.0);
    t.after_response(Some(429)).unwrap();
    t.after_response(Some(429)).unwrap();
    t.after_response(None).unwrap();
    assert!(matches!(
        t.after_response(Some(429)),
        Err(GarminError::Locked { .. })
    ));
}

#[test]
fn a_429_while_refreshing_counts_like_any_other() {
    // Token läuft ab, die Erneuerung bekommt 429: Throttled, und es wird nichts angefragt.
    let rig = Rig::connected(|_| Reply::status(429), tokens(10, "r"));
    assert_eq!(
        rig.client.social_profile().unwrap_err(),
        GarminError::Throttled
    );
    assert!(rig.gets().is_empty());
}

#[test]
fn lockout_is_kept_in_memory_when_the_file_cannot_be_written() {
    // Elternpfad ist eine Datei: Verzeichnis und Datei lassen sich nicht anlegen.
    let blocker = scratch_dir("blocked").join("not-a-dir");
    std::fs::write(&blocker, "x").unwrap();
    let clock = FakeClock::new();
    let t = Throttle::new(
        blocker.join("lockout.json"),
        ThrottleConfig::default(),
        clock.clone(),
        Arc::new(FixedJitter(0.0)),
    );
    t.after_response(Some(429)).unwrap();
    t.after_response(Some(429)).unwrap();
    let err = t.after_response(Some(403)).unwrap_err();
    assert!(matches!(err, GarminError::Locked { .. }));
    assert!(matches!(
        t.before_request(),
        Err(GarminError::Locked { .. })
    ));
    assert!(t.active_lockout().is_some());
}

#[test]
fn a_damaged_or_expired_lockout_file_does_not_lock() {
    let dir = scratch_dir("damaged");
    let file = dir.join("lockout.json");
    let clock = FakeClock::new();
    let t = Throttle::new(
        &file,
        ThrottleConfig::default(),
        clock.clone(),
        Arc::new(FixedJitter(0.0)),
    );

    std::fs::write(&file, "kein json").unwrap();
    assert!(t.before_request().is_ok());

    std::fs::write(
        &file,
        format!(
            r#"{{"since_epoch_s":{},"until_epoch_s":{},"http_status":429,"consecutive":3}}"#,
            START_S - 100,
            START_S - 1
        ),
    )
    .unwrap();
    assert!(t.before_request().is_ok());
    assert_eq!(t.active_lockout(), None);

    std::fs::write(
        &file,
        format!(
            r#"{{"since_epoch_s":{START_S},"until_epoch_s":{},"http_status":403,"consecutive":3}}"#,
            START_S + 5
        ),
    )
    .unwrap();
    assert!(matches!(
        t.before_request(),
        Err(GarminError::Locked { .. })
    ));
}

#[test]
fn calls_never_run_in_parallel() {
    // Zwei Threads rufen gleichzeitig; der Server darf nie zwei offene Anfragen sehen.
    let open = Arc::new(AtomicUsize::new(0));
    let max_open = Arc::new(AtomicUsize::new(0));
    let (o, m) = (Arc::clone(&open), Arc::clone(&max_open));
    let rig = Rig::connected(
        move |_| {
            let now = o.fetch_add(1, Ordering::SeqCst) + 1;
            m.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(80));
            o.fetch_sub(1, Ordering::SeqCst);
            Reply::json(200, &json!({"displayName": "u"}))
        },
        tokens(3600, "r"),
    );
    let client = Arc::new(rig.client);
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let client = Arc::clone(&client);
            std::thread::spawn(move || client.hrv_day(d("2026-09-01")).unwrap())
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(max_open.load(Ordering::SeqCst), 1);
    assert_eq!(rig.server.count(), 3);
}
