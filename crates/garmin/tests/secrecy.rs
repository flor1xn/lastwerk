//! Markertest (SPEC 10.2): Ein Token-Marker darf in keinem `Debug`, `Display` und keiner
//! Fehlermeldung erscheinen, ebenso keine Adresse mit Query, kein Ticket und kein Teil einer
//! Antwort. Der Server wiederholt in seinen Fehlerantworten absichtlich alles, was er bekommen hat.

mod common;

use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDate;
use common::{MARKER, Reply, Rig, closed_port_url, jwt, scratch_dir, tokens};
use lastwerk_garmin::{
    AttemptFailure, AttemptReason, ClientConfig, Endpoints, GarminClient, GarminError,
    KeyringStore, MemoryStore, NetworkKind, StoreError, Throttle, ThrottleConfig, TokenSet,
    TokenStore, ticket_from_redirect,
};
use serde_json::json;

type Handler = Box<dyn Fn(&common::Seen) -> Reply + Send + Sync>;

fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

/// Token, dessen Zugangs- und Erneuerungstoken beide den Marker tragen.
fn marked_tokens(valid_for_s: i64) -> TokenSet {
    tokens(valid_for_s, &format!("refresh-{MARKER}"))
}

/// Alles, was in einer Meldung nicht stehen darf.
const FORBIDDEN: [&str; 11] = [
    MARKER,
    "Bearer",
    "Basic ",
    "127.0.0.1",
    "http://",
    "https://",
    "ticket=",
    "service_ticket",
    "start=",
    "date=",
    "<html>",
];

fn assert_clean(what: &str, error: &GarminError) {
    let renderings = [
        format!("{error}"),
        format!("{error:?}"),
        format!("{error:#?}"),
        error.code().to_owned(),
    ];
    for text in renderings {
        for forbidden in FORBIDDEN {
            assert!(
                !text.contains(forbidden),
                "{what}: Meldung enthält {forbidden:?}: {text}"
            );
        }
    }
}

/// Antwort, die alles zurückspielt: Marker, Anfragekopf und Rumpf.
fn echo(seen: &common::Seen, status: u16) -> Reply {
    Reply::bytes(
        status,
        format!(
            "<html>{MARKER} {} {} {:?} {}</html>",
            seen.method, seen.target, seen.headers, seen.body
        )
        .into_bytes(),
    )
    .header("X-Echo", MARKER)
    .header("Location", &format!("https://evil.example/?t={MARKER}"))
}

#[test]
fn secret_types_print_no_contents() {
    let tokens = marked_tokens(3600);
    let ticket = common::ticket();
    let store = MemoryStore::with_tokens(tokens.clone());
    let rig = Rig::connected(|_| Reply::status(500), tokens.clone());

    let renderings = [
        format!("{tokens:?}"),
        format!("{tokens:#?}"),
        format!("{:?}", Some(tokens.clone())),
        format!("{:?}", Ok::<_, GarminError>(tokens.clone())),
        format!("{ticket:?}"),
        format!(
            "{:?}",
            ticket_from_redirect(&format!(
                "https://connect.garmin.com/app?ticket=ST-{MARKER}"
            ))
        ),
        format!("{store:?}"),
        format!("{:?}", store.load()),
        format!("{:?}", rig.client),
        format!("{:#?}", rig.client),
    ];
    for text in renderings {
        assert!(!text.contains(MARKER), "{text}");
        assert!(!text.contains("Bearer"), "{text}");
    }
    assert_eq!(format!("{tokens:?}"), "TokenSet { .. }");
    assert_eq!(format!("{ticket:?}"), "Ticket { .. }");
}

#[test]
fn exchange_failures_print_no_response_content() {
    let scenarios: Vec<(&str, Handler)> = vec![
        ("400 mit Echo", Box::new(|s| echo(s, 400))),
        ("401 mit Echo", Box::new(|s| echo(s, 401))),
        ("429 mit Echo", Box::new(|s| echo(s, 429))),
        ("500 mit Echo", Box::new(|s| echo(s, 500))),
        ("200 mit Echo statt JSON", Box::new(|s| echo(s, 200))),
        (
            "200 mit Marker im JSON ohne Token",
            Box::new(|_| Reply::json(200, &json!({"error": MARKER, "access_token": ""}))),
        ),
        ("Weiterleitung", Box::new(|s| echo(s, 302))),
    ];
    for (name, handler) in scenarios {
        let rig = Rig::new(move |seen| handler(seen));
        let error = rig.client.connect(&common::ticket()).unwrap_err();
        assert_clean(name, &error);
        assert!(!rig.client.is_connected().unwrap(), "{name}");
    }
}

#[test]
fn data_request_failures_print_no_response_content() {
    let statuses = [200, 204, 302, 400, 401, 403, 404, 429, 500, 503];
    for status in statuses {
        let rig = Rig::connected(move |seen| echo(seen, status), marked_tokens(3600));
        let name = format!("Status {status}");
        let calls: Vec<Result<(), GarminError>> = vec![
            rig.client.social_profile().map(drop),
            rig.client.activities(0, 5).map(drop),
            rig.client.activity_fit(7).map(drop),
            rig.client.hrv_day(d("2026-09-01")).map(drop),
            rig.client.sleep_day("synthetic", d("2026-09-01")).map(drop),
            rig.client
                .rhr_range("synthetic", d("2026-09-01"), d("2026-09-02"))
                .map(drop),
        ];
        for result in calls {
            if let Err(error) = result {
                assert_clean(&name, &error);
            }
        }
        // Nach den Sperren und Pausen bleiben die Dateien frei von Markern.
        if rig.lockout_file.exists() {
            let content = std::fs::read_to_string(&rig.lockout_file).unwrap();
            assert!(
                !content.contains(MARKER) && !content.contains("127.0.0.1"),
                "{content}"
            );
        }
    }
}

#[test]
fn refresh_and_double_401_print_no_token_content() {
    // Erneuerung wird abgelehnt, Server wiederholt Erneuerungstoken und Formular.
    let rig = Rig::connected(|seen| echo(seen, 400), marked_tokens(10));
    assert_clean("Erneuerung 400", &rig.client.social_profile().unwrap_err());

    // 401, Erneuerung klappt, 401 noch einmal.
    let access = jwt(common::START_S + 7200, "cid", MARKER);
    let rig = Rig::connected(
        move |seen| {
            if seen.method == "POST" {
                Reply::json(
                    200,
                    &json!({"access_token": access, "refresh_token": MARKER}),
                )
            } else {
                echo(seen, 401)
            }
        },
        marked_tokens(3600),
    );
    let error = rig.client.social_profile().unwrap_err();
    assert!(matches!(error, GarminError::AuthFailed { .. }));
    assert_clean("doppeltes 401", &error);
}

#[test]
fn network_failures_print_no_address() {
    let tokens = marked_tokens(3600);
    let client = GarminClient::builder(
        Endpoints::for_tests(&closed_port_url()),
        Arc::new(MemoryStore::with_tokens(tokens)),
        scratch_dir("net-secrecy").join("lockout.json"),
    )
    .clock(common::FakeClock::new())
    .build();
    for error in [
        client.social_profile().unwrap_err(),
        client.connect(&common::ticket()).unwrap_err(),
        client.hrv_day(d("2026-09-01")).unwrap_err(),
    ] {
        assert_eq!(error, GarminError::Network(NetworkKind::Connect));
        assert_clean("Verbindung verweigert", &error);
    }

    let config = ClientConfig {
        api_timeout: Duration::from_millis(200),
        ..ClientConfig::default()
    };
    let rig = Rig::with(
        |_| Reply::hang(Duration::from_secs(2)),
        Some(marked_tokens(3600)),
        config,
    );
    assert_clean(
        "Zeitlimit",
        &rig.client.hrv_day(d("2026-09-01")).unwrap_err(),
    );
}

#[test]
fn lockout_and_input_errors_print_no_secrets() {
    let rig = Rig::connected(|seen| echo(seen, 429), marked_tokens(100_000));
    let mut last = None;
    for _ in 0..4 {
        last = rig.client.social_profile().err();
        assert_clean("Sperre", last.as_ref().unwrap());
    }
    assert!(matches!(last, Some(GarminError::Locked { .. })));

    let rig = Rig::connected(|seen| echo(seen, 200), marked_tokens(3600));
    let hostile = format!("{MARKER}/../x?y=1");
    let error = rig.client.sleep_day("..", d("2026-09-01")).unwrap_err();
    assert_clean("Eingabe", &error);
    // Ein Name mit Marker wird kodiert gesendet; die Fehlermeldungen der Gegenseite bleiben draußen.
    let _ = rig.client.sleep_day(&hostile, d("2026-09-01"));
    assert_clean(
        "Datumsbereich",
        &rig.client
            .hrv_range(d("2026-09-02"), d("2026-09-01"))
            .unwrap_err(),
    );
}

#[test]
fn store_and_attempt_types_print_no_contents() {
    for error in [
        StoreError::Unavailable,
        StoreError::Corrupt,
        StoreError::TooLarge,
        StoreError::Other,
    ] {
        let text = format!("{error} {error:?} {}", GarminError::Store(error));
        assert!(!text.contains(MARKER) && !text.contains("http"), "{text}");
    }
    let attempts = GarminError::AuthFailed {
        attempts: vec![
            AttemptFailure {
                client_id: "GARMIN_CONNECT_MOBILE_IOS_DI",
                reason: AttemptReason::Status(400),
            },
            AttemptFailure {
                client_id: "GARMIN_CONNECT_MOBILE_ANDROID_DI",
                reason: AttemptReason::InvalidResponse,
            },
        ],
    };
    assert_clean("Versuche", &attempts);
    let keyring = KeyringStore::new();
    assert!(!format!("{keyring:?}").contains(MARKER));
    let throttle = Throttle::new(
        scratch_dir("th-debug").join("l.json"),
        ThrottleConfig::default(),
        common::FakeClock::new(),
        Arc::new(common::FixedJitter(0.0)),
    );
    assert!(!format!("{throttle:?}").contains(MARKER));
}
