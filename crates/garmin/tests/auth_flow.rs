//! Tausch des Tickets, Erneuern und Wiederholung bei 401, gegen den Attrappen-Server auf Loopback.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use base64::Engine;
use common::{MARKER, Reply, Rig, START_S, closed_port_url, jwt, tokens};
use lastwerk_garmin::{
    AttemptReason, ClientConfig, Endpoints, GarminClient, GarminError, MemoryStore, NetworkKind,
    StoreError, TokenSet, TokenStore, code,
};
use serde_json::json;

const TOKEN_PATH: &str = "/di-oauth2-service/oauth/token";
const PROFILE_PATH: &str = "/userprofile-service/socialProfile";

fn basic(client_id: &str) -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{client_id}:"))
    )
}

fn token_reply(access: &str, refresh: Option<&str>) -> Reply {
    let mut body = json!({ "access_token": access, "expires_in": 3600 });
    if let Some(refresh) = refresh {
        body["refresh_token"] = json!(refresh);
    }
    Reply::json(200, &body)
}

#[test]
fn connect_posts_the_exchange_like_the_library_and_stores_the_tokens() {
    let access = jwt(
        START_S + 3600,
        "GARMIN_CONNECT_MOBILE_ANDROID_DI_2024Q4",
        MARKER,
    );
    let reply_access = access.clone();
    let rig = Rig::new(move |_| token_reply(&reply_access, Some("refresh-1")));

    rig.client.connect(&common::ticket()).unwrap();

    let posts = rig.posts();
    assert_eq!(posts.len(), 1);
    let post = &posts[0];
    assert_eq!(post.path(), TOKEN_PATH);
    assert_eq!(
        post.header("authorization"),
        Some(basic("GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2").as_str())
    );
    assert_eq!(
        post.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(post.header("user-agent"), Some("GCM-Android-5.23"));
    assert_eq!(post.header("x-garmin-client-platform"), Some("Android"));
    assert_eq!(post.header("x-gcexperience"), Some("GC5"));
    assert_eq!(post.header("accept"), Some("application/json"));
    assert_eq!(
        post.form("client_id").as_deref(),
        Some("GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2")
    );
    assert_eq!(
        post.form("service_ticket"),
        Some(format!("ST-{MARKER}-cas"))
    );
    assert_eq!(
        post.form("grant_type").as_deref(),
        Some("https://connectapi.garmin.com/di-oauth2-service/oauth/grant/service_ticket")
    );
    assert_eq!(
        post.form("service_url").as_deref(),
        Some("https://connect.garmin.com/app")
    );

    let stored = rig.store.load().unwrap().unwrap();
    assert_eq!(stored.access_token_for_tests(), access);
    assert_eq!(stored.refresh_token_for_tests(), Some("refresh-1"));
    // Die Kennung im Token gewinnt vor der versuchten.
    assert_eq!(
        stored.client_id(),
        "GARMIN_CONNECT_MOBILE_ANDROID_DI_2024Q4"
    );
    assert!(rig.client.is_connected().unwrap());
}

#[test]
fn connect_tries_client_ids_in_order_until_one_is_accepted() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let rig = Rig::new(move |_| match counter.fetch_add(1, Ordering::SeqCst) {
        0 => Reply::status(400),
        1 => Reply::status(401),
        // Undurchsichtiges Token ohne Nutzlast: Die Kennung bleibt die versuchte.
        _ => token_reply("opaque-access", Some("refresh-1")),
    });

    rig.client.connect(&common::ticket()).unwrap();

    let ids: Vec<String> = rig
        .posts()
        .iter()
        .map(|p| p.form("client_id").unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2",
            "GARMIN_CONNECT_MOBILE_ANDROID_DI_2024Q4",
            "GARMIN_CONNECT_MOBILE_ANDROID_DI",
        ]
    );
    for (post, id) in rig.posts().iter().zip(&ids) {
        assert_eq!(post.header("authorization"), Some(basic(id).as_str()));
    }
    assert_eq!(
        rig.store.load().unwrap().unwrap().client_id(),
        "GARMIN_CONNECT_MOBILE_ANDROID_DI"
    );
    // Auch zwischen den Versuchen gilt der Mindestabstand von 1,5 s.
    assert_eq!(
        rig.clock.sleeps(),
        [Duration::from_millis(1500), Duration::from_millis(1500)]
    );
}

#[test]
fn connect_collects_one_reason_per_client_id_when_all_fail() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let rig = Rig::new(move |_| match counter.fetch_add(1, Ordering::SeqCst) {
        0 => Reply::status(400),
        1 => Reply::status(401),
        2 => Reply::bytes(200, format!("<html>{MARKER}</html>").into_bytes()),
        _ => Reply::status(500),
    });

    let err = rig.client.connect(&common::ticket()).unwrap_err();

    let GarminError::AuthFailed { attempts } = &err else {
        panic!("{err:?}");
    };
    assert_eq!(attempts.len(), 4);
    assert_eq!(
        attempts[0].client_id,
        "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2"
    );
    assert_eq!(attempts[0].reason, AttemptReason::Status(400));
    assert_eq!(attempts[1].reason, AttemptReason::Status(401));
    assert_eq!(attempts[2].reason, AttemptReason::InvalidResponse);
    assert_eq!(attempts[3].client_id, "GARMIN_CONNECT_MOBILE_IOS_DI");
    assert_eq!(attempts[3].reason, AttemptReason::Status(500));
    assert_eq!(err.code(), code::AUTH_FAILED);
    assert!(!rig.client.is_connected().unwrap(), "nichts abgelegt");
}

#[test]
fn connect_stops_at_429_without_trying_the_other_ids() {
    let rig = Rig::new(|_| Reply::status(429));
    let err = rig.client.connect(&common::ticket()).unwrap_err();
    assert_eq!(err, GarminError::Throttled);
    assert_eq!(err.code(), code::THROTTLED);
    assert_eq!(rig.posts().len(), 1);
}

#[test]
fn connect_reports_network_failure_without_address() {
    let store = Arc::new(MemoryStore::new());
    let client = GarminClient::builder(
        Endpoints::for_tests(&closed_port_url()),
        store.clone(),
        common::scratch_dir("net").join("lockout.json"),
    )
    .clock(common::FakeClock::new())
    .build();

    let err = client.connect(&common::ticket()).unwrap_err();

    assert_eq!(err, GarminError::Network(NetworkKind::Connect));
    assert_eq!(err.code(), code::NETWORK);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn connect_reports_a_failing_store_as_store_failed() {
    struct BrokenStore;
    impl TokenStore for BrokenStore {
        fn load(&self) -> Result<Option<TokenSet>, StoreError> {
            Err(StoreError::Unavailable)
        }
        fn save(&self, _: &TokenSet) -> Result<(), StoreError> {
            Err(StoreError::Unavailable)
        }
        fn delete(&self) -> Result<(), StoreError> {
            Err(StoreError::Unavailable)
        }
    }
    let server = common::MockServer::start(|_| token_reply("opaque", Some("r")));
    let client = GarminClient::builder(
        server.endpoints(),
        Arc::new(BrokenStore),
        common::scratch_dir("broken").join("lockout.json"),
    )
    .clock(common::FakeClock::new())
    .build();

    let err = client.connect(&common::ticket()).unwrap_err();
    assert_eq!(err, GarminError::Store(StoreError::Unavailable));
    assert_eq!(err.code(), code::STORE_FAILED);
    assert_eq!(
        client.social_profile().unwrap_err().code(),
        code::STORE_FAILED
    );
}

#[test]
fn unreadable_store_entry_counts_as_not_connected() {
    struct CorruptStore;
    impl TokenStore for CorruptStore {
        fn load(&self) -> Result<Option<TokenSet>, StoreError> {
            Err(StoreError::Corrupt)
        }
        fn save(&self, _: &TokenSet) -> Result<(), StoreError> {
            Ok(())
        }
        fn delete(&self) -> Result<(), StoreError> {
            Ok(())
        }
    }
    let server = common::MockServer::start(|_| Reply::status(500));
    let client = GarminClient::builder(
        server.endpoints(),
        Arc::new(CorruptStore),
        common::scratch_dir("corrupt").join("lockout.json"),
    )
    .clock(common::FakeClock::new())
    .build();

    assert!(!client.is_connected().unwrap());
    let err = client.social_profile().unwrap_err();
    assert_eq!(err, GarminError::NotConnected);
    assert_eq!(err.code(), code::NOT_CONNECTED);
    assert_eq!(server.count(), 0);
}

#[test]
fn without_tokens_nothing_is_sent() {
    let rig = Rig::new(|_| Reply::status(500));
    assert_eq!(
        rig.client.social_profile().unwrap_err(),
        GarminError::NotConnected
    );
    assert_eq!(rig.server.count(), 0);
}

#[test]
fn expiring_token_is_refreshed_and_saved_before_the_request() {
    let old = tokens(100, "refresh-old"); // 100 s Rest: unter 900 s
    let old_bearer = format!("Bearer {}", old.access_token_for_tests());
    let new_access = jwt(
        START_S + 7200,
        "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2",
        "new",
    );
    let reply_access = new_access.clone();
    let rig = Rig::connected(
        move |seen| {
            if seen.method == "POST" {
                token_reply(&reply_access, Some("refresh-new"))
            } else {
                Reply::json(200, &json!({"displayName": "synthetic-user-1"}))
            }
        },
        old,
    );

    let profile = rig.client.social_profile().unwrap();
    assert_eq!(profile.display_name, "synthetic-user-1");

    let order: Vec<String> = rig.server.seen().iter().map(|s| s.method.clone()).collect();
    assert_eq!(order, ["POST", "GET"], "erst erneuern, dann anfragen");
    let post = &rig.posts()[0];
    assert_eq!(post.path(), TOKEN_PATH);
    assert_eq!(
        post.header("authorization"),
        Some(basic("GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2").as_str())
    );
    assert_eq!(post.form("grant_type").as_deref(), Some("refresh_token"));
    assert_eq!(
        post.form("client_id").as_deref(),
        Some("GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2")
    );
    assert_eq!(post.form("refresh_token").as_deref(), Some("refresh-old"));
    let get = &rig.gets()[0];
    assert_ne!(get.header("authorization"), Some(old_bearer.as_str()));
    assert_eq!(
        get.header("authorization"),
        Some(format!("Bearer {new_access}").as_str())
    );
    let stored = rig.store.load().unwrap().unwrap();
    assert_eq!(stored.access_token_for_tests(), new_access);
    assert_eq!(stored.refresh_token_for_tests(), Some("refresh-new"));
}

#[test]
fn refresh_keeps_the_old_refresh_token_when_garmin_sends_none() {
    let new_access = jwt(
        START_S + 7200,
        "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2",
        "n",
    );
    let rig = Rig::connected(
        move |seen| {
            if seen.method == "POST" {
                token_reply(&new_access, None)
            } else {
                Reply::json(200, &json!({"displayName": "u"}))
            }
        },
        tokens(10, "refresh-old"),
    );
    rig.client.social_profile().unwrap();
    assert_eq!(
        rig.store.load().unwrap().unwrap().refresh_token_for_tests(),
        Some("refresh-old")
    );
}

#[test]
fn token_with_more_than_900_seconds_left_is_not_refreshed() {
    let rig = Rig::connected(
        |_| Reply::json(200, &json!({"displayName": "u"})),
        tokens(1000, "r"),
    );
    rig.client.social_profile().unwrap();
    assert!(rig.posts().is_empty());
    assert_eq!(rig.gets().len(), 1);
}

#[test]
fn a_401_triggers_exactly_one_refresh_and_one_retry() {
    let old = tokens(3600, "refresh-old");
    let old_bearer = format!("Bearer {}", old.access_token_for_tests());
    let new_access = jwt(
        START_S + 7200,
        "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2",
        "n",
    );
    let (reply_access, expected_new) = (new_access.clone(), format!("Bearer {new_access}"));
    let rig = Rig::connected(
        move |seen| match seen.method.as_str() {
            "POST" => token_reply(&reply_access, Some("refresh-new")),
            _ if seen.header("authorization") == Some(old_bearer.as_str()) => Reply::status(401),
            _ => Reply::json(200, &json!({"displayName": "u"})),
        },
        old,
    );

    assert_eq!(rig.client.social_profile().unwrap().display_name, "u");

    let order: Vec<String> = rig.server.seen().iter().map(|s| s.method.clone()).collect();
    assert_eq!(order, ["GET", "POST", "GET"]);
    assert_eq!(
        rig.gets()[1].header("authorization"),
        Some(expected_new.as_str())
    );
    assert_eq!(
        rig.store.load().unwrap().unwrap().access_token_for_tests(),
        new_access
    );
}

#[test]
fn a_second_401_ends_with_auth_failed_and_no_further_retry() {
    let new_access = jwt(
        START_S + 7200,
        "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2",
        "n",
    );
    let rig = Rig::connected(
        move |seen| {
            if seen.method == "POST" {
                token_reply(&new_access, Some("r2"))
            } else {
                Reply::status(401)
            }
        },
        tokens(3600, "r1"),
    );

    let err = rig.client.social_profile().unwrap_err();

    assert!(matches!(err, GarminError::AuthFailed { .. }), "{err:?}");
    assert_eq!(err.code(), code::AUTH_FAILED);
    assert_eq!(rig.gets().len(), 2);
    assert_eq!(rig.posts().len(), 1);
}

#[test]
fn rejected_refresh_is_auth_failed_and_sends_no_request() {
    let rig = Rig::connected(|_| Reply::status(400), tokens(10, "r1"));
    let err = rig.client.social_profile().unwrap_err();
    assert!(matches!(err, GarminError::AuthFailed { .. }), "{err:?}");
    assert!(rig.gets().is_empty());
    // Der Eintrag bleibt: Nur "Trennen" löscht ihn.
    assert!(rig.client.is_connected().unwrap());
}

#[test]
fn expired_token_without_refresh_token_needs_a_new_login() {
    let access = jwt(START_S + 10, "cid", MARKER);
    let rig = Rig::connected(
        |_| Reply::status(500),
        TokenSet::for_tests(&access, None, "cid"),
    );
    let err = rig.client.social_profile().unwrap_err();
    assert!(matches!(err, GarminError::AuthFailed { .. }), "{err:?}");
    assert_eq!(rig.server.count(), 0);
}

#[test]
fn refresh_that_returns_no_token_is_an_invalid_response() {
    let rig = Rig::connected(|_| Reply::json(200, &json!({"nope": 1})), tokens(10, "r1"));
    let err = rig.client.social_profile().unwrap_err();
    assert!(matches!(err, GarminError::InvalidResponse(_)), "{err:?}");
    assert_eq!(err.code(), code::INVALID_RESPONSE);
}

#[test]
fn disconnect_deletes_the_tokens() {
    let rig = Rig::connected(|_| Reply::status(500), tokens(3600, "r"));
    assert!(rig.client.is_connected().unwrap());
    rig.client.disconnect().unwrap();
    assert!(!rig.client.is_connected().unwrap());
    assert!(rig.store.load().unwrap().is_none());
    // Zweites Trennen ist kein Fehler.
    rig.client.disconnect().unwrap();
    assert_eq!(rig.server.count(), 0);
}

#[test]
fn the_login_timeout_applies_to_the_exchange() {
    let config = ClientConfig {
        login_timeout: Duration::from_millis(300),
        ..ClientConfig::default()
    };
    let rig = Rig::with(|_| Reply::hang(Duration::from_secs(3)), None, config);
    let started = Instant::now();

    let err = rig.client.connect(&common::ticket()).unwrap_err();

    assert_eq!(err, GarminError::Network(NetworkKind::Timeout));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn profile_path_constant_is_the_allowlisted_one() {
    let rig = Rig::connected(
        |_| Reply::json(200, &json!({"displayName": "u"})),
        tokens(3600, "r"),
    );
    rig.client.social_profile().unwrap();
    assert_eq!(rig.gets()[0].path(), PROFILE_PATH);
}
