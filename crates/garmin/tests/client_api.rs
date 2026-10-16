//! Die lesenden Endpunkte: Pfade, Parameter, Kopfzeilen, Antwortbehandlung, Größen- und
//! Zeitgrenzen. Alle Antworten sind synthetisch.

mod common;

use std::time::{Duration, Instant};

use chrono::NaiveDate;
use common::{Reply, Rig, tokens};
use lastwerk_garmin::{
    ClientConfig, GarminError, MAX_ACTIVITY_LIMIT, MAX_FIT_BYTES, MAX_JSON_BYTES, NetworkKind,
    SensitiveConsent, code,
};
use serde_json::{Value, json};

fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

/// Prüfstand mit gültigem Token, der immer dieselbe JSON-Antwort liefert.
fn rig_json(value: Value) -> Rig {
    Rig::connected(move |_| Reply::json(200, &value), tokens(3600, "r"))
}

fn rig_status(status: u16) -> Rig {
    Rig::connected(move |_| Reply::status(status), tokens(3600, "r"))
}

#[test]
fn every_request_is_a_get_with_bearer_and_native_headers() {
    let rig = rig_json(json!({"displayName": "synthetic-user-1"}));
    let bearer = format!("Bearer {}", tokens(3600, "r").access_token_for_tests());

    rig.client.social_profile().unwrap();

    let get = &rig.gets()[0];
    assert_eq!(rig.server.count(), 1);
    assert_eq!(get.method, "GET");
    assert!(get.body.is_empty());
    assert_eq!(get.header("authorization"), Some(bearer.as_str()));
    assert_eq!(get.header("accept"), Some("application/json"));
    assert_eq!(get.header("user-agent"), Some("GCM-Android-5.23"));
    assert_eq!(
        get.header("x-garmin-user-agent"),
        Some(
            "com.garmin.android.apps.connectmobile/5.23; ; Google/sdk_gphone64_arm64/google; \
             Android/33; Dalvik/2.1.0"
        )
    );
    assert_eq!(get.header("x-garmin-paired-app-version"), Some("10861"));
    assert_eq!(get.header("x-app-ver"), Some("10861"));
    assert_eq!(get.header("x-lang"), Some("en"));
    assert_eq!(get.header("accept-language"), Some("en-US,en;q=0.9"));
    // Kein Cookie, keine Zugangsdaten außer dem Bearer-Token.
    assert!(get.header("cookie").is_none());
}

#[test]
fn social_profile_reads_the_display_name() {
    let rig = rig_json(json!({"displayName": "synthetic-user-1", "fullName": "ignored"}));
    assert_eq!(
        rig.client.social_profile().unwrap().display_name,
        "synthetic-user-1"
    );
    assert_eq!(rig.gets()[0].target, "/userprofile-service/socialProfile");

    for body in [
        json!({}),
        json!({"displayName": ""}),
        json!({"displayName": 5}),
        json!([]),
    ] {
        let rig = rig_json(body);
        let err = rig.client.social_profile().unwrap_err();
        assert!(matches!(err, GarminError::InvalidResponse(_)), "{err:?}");
    }
}

#[test]
fn activities_sends_paging_and_parses_the_list() {
    let rig = rig_json(json!([
        {"activityId": 1001, "startTimeLocal": "2026-09-02 06:30:00",
         "activityType": {"typeKey": "running", "typeId": 1}, "distance": 1.0},
        {"activityId": 1000, "startTimeLocal": "2026-09-01 17:45:10",
         "activityType": {"typeKey": "cycling"}}
    ]));

    let list = rig.client.activities(40, 2).unwrap();

    assert_eq!(
        rig.gets()[0].target,
        "/activitylist-service/activities/search/activities?start=40&limit=2"
    );
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].activity_id, 1001);
    assert_eq!(list[0].start_time_local.to_string(), "2026-09-02 06:30:00");
    assert_eq!(list[0].activity_type, "running");
    assert_eq!(list[1].activity_id, 1000);
    assert_eq!(list[1].activity_type, "cycling");
}

#[test]
fn activities_handles_empty_and_malformed_answers() {
    let rig = rig_status(204);
    assert!(rig.client.activities(0, 10).unwrap().is_empty());

    let rig = rig_json(json!([]));
    assert!(rig.client.activities(0, 10).unwrap().is_empty());

    for body in [
        json!({"activities": []}),
        json!([{"activityId": 1}]),
        json!("x"),
    ] {
        let rig = rig_json(body);
        let err = rig.client.activities(0, 10).unwrap_err();
        assert!(matches!(err, GarminError::InvalidResponse(_)), "{err:?}");
    }
}

#[test]
fn bad_input_is_rejected_before_anything_is_sent() {
    let rig = rig_json(json!({}));
    let range_err = rig
        .client
        .hrv_range(d("2026-09-02"), d("2026-09-01"))
        .unwrap_err();
    assert_eq!(range_err, GarminError::InvalidInput("date range"));
    assert_eq!(range_err.code(), code::INVALID_INPUT);
    assert!(rig.client.activities(0, 0).is_err());
    assert!(rig.client.activities(0, MAX_ACTIVITY_LIMIT + 1).is_err());
    assert!(rig.client.activity_fit(0).is_err());
    for hostile in ["", "..", "../../x", "a/b", "a?b=c", "a#b"] {
        let result = rig.client.sleep_day(hostile, d("2026-09-01"));
        // Entweder abgelehnt oder prozentkodiert in genau einem Segment gesendet.
        if result.is_ok() {
            let target = rig.gets().last().unwrap().target.clone();
            assert_eq!(target.matches('?').count(), 1, "{target}");
            assert!(!target.contains("/../"), "{target}");
        }
    }
    let sent_before = rig.server.count();
    assert!(rig.client.sleep_day("..", d("2026-09-01")).is_err());
    assert_eq!(rig.server.count(), sent_before);
}

#[test]
fn activity_fit_returns_the_raw_bytes_and_accepts_anything() {
    // Beginnt wie ein ZIP und enthält Nullen und Bytes über 127: nichts darf umkodiert werden.
    let zip: Vec<u8> = [b"PK\x03\x04".as_slice(), &[0, 1, 2, 0xFF, 0xFE, 0, 0x80]].concat();
    let reply_zip = zip.clone();
    let rig = Rig::connected(
        move |_| Reply::bytes(200, reply_zip.clone()),
        tokens(3600, "r"),
    );

    assert_eq!(rig.client.activity_fit(12_345).unwrap(), zip);

    let get = &rig.gets()[0];
    assert_eq!(get.target, "/download-service/files/activity/12345");
    assert_eq!(get.header("accept"), Some("*/*"));
}

#[test]
fn activity_fit_204_is_empty() {
    assert!(rig_status(204).client.activity_fit(1).unwrap().is_empty());
}

#[test]
fn recovery_endpoints_send_the_expected_paths_and_return_the_json_unchanged() {
    let payload = json!({"hrvSummaries": [{"calendarDate": "2026-09-01", "lastNightAvg": 55}]});
    let rig = rig_json(payload.clone());
    let name = "0a1b2c3d-0000-1111-2222-333344445555";

    assert_eq!(
        rig.client
            .hrv_range(d("2026-09-01"), d("2026-09-28"))
            .unwrap(),
        payload
    );
    assert_eq!(rig.client.hrv_day(d("2026-09-01")).unwrap(), payload);
    assert_eq!(
        rig.client.sleep_day(name, d("2026-09-01")).unwrap(),
        payload
    );
    assert_eq!(
        rig.client
            .rhr_range(name, d("2026-09-01"), d("2026-09-07"))
            .unwrap(),
        payload
    );
    assert_eq!(
        rig.client
            .body_battery(d("2026-09-01"), d("2026-09-07"))
            .unwrap(),
        payload
    );
    assert_eq!(rig.client.stress_day(d("2026-09-01")).unwrap(), payload);

    let targets: Vec<String> = rig.gets().into_iter().map(|s| s.target).collect();
    assert_eq!(
        targets,
        [
            "/hrv-service/hrv/daily/2026-09-01/2026-09-28".to_owned(),
            "/hrv-service/hrv/2026-09-01".to_owned(),
            format!(
                "/wellness-service/wellness/dailySleepData/{name}?date=2026-09-01&nonSleepBufferMinutes=60"
            ),
            format!(
                "/userstats-service/wellness/daily/{name}?fromDate=2026-09-01&untilDate=2026-09-07&metricId=60"
            ),
            "/wellness-service/wellness/bodyBattery/reports/daily?startDate=2026-09-01&endDate=2026-09-07"
                .to_owned(),
            "/wellness-service/wellness/dailyStress/2026-09-01".to_owned(),
        ]
    );
    assert!(rig.server.seen().iter().all(|s| s.method == "GET"));
}

#[test]
fn new_read_endpoints_send_the_expected_paths_with_get_only() {
    let payload = json!({"value": 1});
    let rig = rig_json(payload.clone());
    let name = "0a1b2c3d-0000-1111-2222-333344445555";
    let day = d("2026-09-01");

    assert_eq!(rig.client.cycling_ftp().unwrap(), payload);
    assert_eq!(
        rig.client
            .hydration_day(SensitiveConsent::granted_by_user(), day)
            .unwrap(),
        payload
    );
    rig.client.user_settings().unwrap();
    rig.client.lactate_threshold().unwrap();
    rig.client.heart_rate_zones().unwrap();
    rig.client.daily_steps(day, d("2026-09-28")).unwrap();
    rig.client.heart_rate_day(name, day).unwrap();
    rig.client.respiration_day(day).unwrap();
    rig.client.spo2_day(day).unwrap();
    rig.client.intensity_minutes_day(day).unwrap();
    rig.client.training_readiness(day).unwrap();
    rig.client.fitness_age(day).unwrap();
    rig.client.endurance_score(d("2026-01-01"), day).unwrap();
    rig.client.hill_score(d("2026-01-01"), day).unwrap();
    rig.client
        .training_load_activities(d("2026-01-01"), day)
        .unwrap();

    let targets: Vec<String> = rig.gets().into_iter().map(|s| s.target).collect();
    assert_eq!(
        targets,
        [
            "/biometric-service/biometric/latestFunctionalThresholdPower/CYCLING".to_owned(),
            "/usersummary-service/usersummary/hydration/daily/2026-09-01".to_owned(),
            "/userprofile-service/userprofile/user-settings".to_owned(),
            "/biometric-service/biometric/latestLactateThreshold".to_owned(),
            "/biometric-service/heartRateZones".to_owned(),
            "/usersummary-service/stats/steps/daily/2026-09-01/2026-09-28".to_owned(),
            format!("/wellness-service/wellness/dailyHeartRate/{name}?date=2026-09-01"),
            "/wellness-service/wellness/daily/respiration/2026-09-01".to_owned(),
            "/wellness-service/wellness/daily/spo2/2026-09-01".to_owned(),
            "/wellness-service/wellness/daily/im/2026-09-01".to_owned(),
            "/metrics-service/metrics/trainingreadiness/2026-09-01".to_owned(),
            "/fitnessage-service/fitnessage/2026-09-01".to_owned(),
            "/metrics-service/metrics/endurancescore/stats?startDate=2026-01-01&endDate=2026-09-01&aggregation=weekly".to_owned(),
            "/metrics-service/metrics/hillscore/stats?startDate=2026-01-01&endDate=2026-09-01&aggregation=daily".to_owned(),
            "/fitnessstats-service/activity/all?startDate=2026-01-01&endDate=2026-09-01&metric=activityTrainingLoad&metric=trainingEffectLabel&metric=trainingEffectLabelSrvrCalc".to_owned(),
        ]
    );
    assert!(rig.server.seen().iter().all(|s| s.method == "GET"));

    // Zu lange Spanne wird vor dem Senden abgelehnt.
    let sent = rig.server.count();
    assert_eq!(
        rig.client.daily_steps(day, d("2026-09-29")).unwrap_err(),
        GarminError::InvalidInput("date range")
    );
    assert_eq!(rig.server.count(), sent);
}

#[test]
fn status_codes_map_to_stable_errors() {
    let cases: [(u16, GarminError, &str); 7] = [
        (404, GarminError::NotAvailable, code::NOT_AVAILABLE),
        (429, GarminError::Throttled, code::THROTTLED),
        (403, GarminError::Throttled, code::THROTTLED),
        (400, GarminError::Http(400), code::NETWORK),
        (418, GarminError::Http(418), code::NETWORK),
        (500, GarminError::Http(500), code::NETWORK),
        (503, GarminError::Http(503), code::NETWORK),
    ];
    for (status, expected, expected_code) in cases {
        let rig = rig_status(status);
        let err = rig.client.hrv_day(d("2026-09-01")).unwrap_err();
        assert_eq!(err, expected, "Status {status}");
        assert_eq!(err.code(), expected_code, "Status {status}");
    }
}

#[test]
fn status_204_gives_null_json() {
    assert_eq!(
        rig_status(204).client.hrv_day(d("2026-09-01")).unwrap(),
        Value::Null
    );
}

#[test]
fn empty_or_broken_json_bodies_are_handled() {
    let rig = Rig::connected(|_| Reply::bytes(200, Vec::new()), tokens(3600, "r"));
    assert_eq!(rig.client.hrv_day(d("2026-09-01")).unwrap(), Value::Null);

    let rig = Rig::connected(
        |_| Reply::bytes(200, b"<html>nope</html>".to_vec()),
        tokens(3600, "r"),
    );
    let err = rig.client.hrv_day(d("2026-09-01")).unwrap_err();
    assert_eq!(err, GarminError::InvalidResponse("body is not json"));
    assert_eq!(err.code(), code::INVALID_RESPONSE);
}

#[test]
fn json_larger_than_8_mib_is_refused_and_exactly_8_mib_is_read() {
    assert_eq!(MAX_JSON_BYTES, 8 * 1024 * 1024);
    assert_eq!(MAX_FIT_BYTES, 256 * 1024 * 1024);

    let too_big = vec![b' '; MAX_JSON_BYTES as usize + 1];
    let rig = Rig::connected(
        move |_| Reply::bytes(200, too_big.clone()),
        tokens(3600, "r"),
    );
    let err = rig.client.hrv_day(d("2026-09-01")).unwrap_err();
    assert_eq!(err, GarminError::InvalidResponse("body too large"));

    let exact = vec![b' '; MAX_JSON_BYTES as usize];
    let rig = Rig::connected(move |_| Reply::bytes(200, exact.clone()), tokens(3600, "r"));
    assert_eq!(rig.client.hrv_day(d("2026-09-01")).unwrap(), Value::Null);
}

#[test]
fn redirects_are_never_followed() {
    let target = common::MockServer::start(|_| Reply::json(200, &json!({"displayName": "evil"})));
    let location = format!("{}/userprofile-service/socialProfile", target.base_url());
    let rig = Rig::connected(
        move |_| Reply::status(302).header("Location", &location),
        tokens(3600, "r"),
    );

    let err = rig.client.social_profile().unwrap_err();

    assert!(matches!(err, GarminError::InvalidResponse(_)), "{err:?}");
    assert_eq!(rig.gets().len(), 1);
    assert_eq!(
        target.count(),
        0,
        "das Bearer-Token darf nicht an die Zieladresse gehen"
    );
}

#[test]
fn a_hanging_server_hits_the_api_timeout() {
    let config = ClientConfig {
        api_timeout: Duration::from_millis(300),
        ..ClientConfig::default()
    };
    let rig = Rig::with(
        |_| Reply::hang(Duration::from_secs(3)),
        Some(tokens(3600, "r")),
        config,
    );
    let started = Instant::now();

    let err = rig.client.social_profile().unwrap_err();

    assert_eq!(err, GarminError::Network(NetworkKind::Timeout));
    assert_eq!(err.code(), code::NETWORK);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_dropped_connection_is_a_network_error() {
    // Der Server schließt, ohne zu antworten (hang_for = 0).
    let rig = Rig::connected(|_| Reply::hang(Duration::ZERO), tokens(3600, "r"));
    let err = rig.client.social_profile().unwrap_err();
    assert!(matches!(err, GarminError::Network(_)), "{err:?}");
}

#[test]
fn the_client_has_no_free_path_method() {
    // Übersetzungszeitprüfung per Konvention: Die öffentliche Fläche ist die Liste unten. Eine
    // neue Methode mit freiem Pfad müsste hier auffallen. Der Test ruft jede Methode einmal auf.
    let rig = rig_json(json!({"displayName": "n"}));
    let c = &rig.client;
    let _ = c.social_profile();
    let _ = c.activities(0, 1);
    let _ = c.activity_fit(1);
    let _ = c.hrv_range(d("2026-09-01"), d("2026-09-01"));
    let _ = c.hrv_day(d("2026-09-01"));
    let _ = c.sleep_day("n", d("2026-09-01"));
    let _ = c.rhr_range("n", d("2026-09-01"), d("2026-09-01"));
    let _ = c.body_battery(d("2026-09-01"), d("2026-09-01"));
    let _ = c.stress_day(d("2026-09-01"));
    const ALLOWED_PREFIXES: [&str; 8] = [
        "/userprofile-service/socialProfile",
        "/activitylist-service/activities/search/activities",
        "/download-service/files/activity/",
        "/hrv-service/hrv/",
        "/wellness-service/wellness/dailySleepData/",
        "/userstats-service/wellness/daily/",
        "/wellness-service/wellness/bodyBattery/reports/daily",
        "/wellness-service/wellness/dailyStress/",
    ];
    let seen = rig.server.seen();
    assert_eq!(seen.len(), 9);
    for request in seen {
        assert_eq!(request.method, "GET");
        assert!(
            ALLOWED_PREFIXES
                .iter()
                .any(|p| request.path().starts_with(p)),
            "{}",
            request.target
        );
    }
}
