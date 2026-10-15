//! Lesender Client für `connectapi.garmin.com` (SPEC 5.6).
//!
//! Es gibt keine freie Pfadmethode. Jede Methode baut ihren Pfad aus getypten Eingaben, und vor
//! dem Senden prüft [`ensure_allowed`] den Pfad gegen eine feste Liste von Präfixen. Es werden
//! nur GET-Anfragen gesendet, die Methode ist im HTTP-Teil nicht anders ansprechbar. Pfade und
//! Parameter nach python-garminconnect 0.3.17 (Q27, MIT), `__init__.py`.
//!
//! Ablauf je Anfrage: Sperre prüfen, Token laden, bei weniger als 900 s Restlaufzeit erneuern
//! und speichern, anfragen, bei 401 genau einmal erneuern und wiederholen. Alle Anfragen laufen
//! nacheinander durch die Drossel, auch die zum Erneuern.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde_json::Value;

use crate::auth::{Ticket, TokenSet, needs_refresh};
use crate::clock::{Clock, Jitter, SystemClock, SystemJitter};
use crate::endpoints::Endpoints;
use crate::error::{GarminError, Result};
use crate::exchange::{TokenEndpoint, native_headers};
use crate::http::{Http, HttpResponse};
use crate::store::{StoreError, TokenStore};
use crate::throttle::{Throttle, ThrottleConfig};

/// Größtes zulässiges JSON einer Antwort (SPEC 5.6, Aufgabe T21).
pub const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
/// Größte zulässige FIT-Datei (ZIP) einer Antwort (SPEC 5.6: höchstens 256 MiB).
pub const MAX_FIT_BYTES: u64 = 256 * 1024 * 1024;
/// Größte Seite der Aktivitätsliste (Q27: `MAX_ACTIVITY_LIMIT`).
pub const MAX_ACTIVITY_LIMIT: u32 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientConfig {
    /// Zeitlimit je Datenabruf (Aufgabe T21: 15 s, wie die Bibliothek).
    pub api_timeout: Duration,
    /// Zeitlimit für Tausch und Erneuern (Aufgabe T21: 30 s, wie die Bibliothek).
    pub login_timeout: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_timeout: Duration::from_secs(15),
            login_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// Kennung des Kontos in Pfaden (`displayName`), meist eine UUID.
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivitySummary {
    pub activity_id: u64,
    /// Lokale Startzeit des Geräts, ohne Zeitzone, wie Garmin sie liefert.
    pub start_time_local: NaiveDateTime,
    /// `activityType.typeKey`, zum Beispiel `running`; `unknown`, wenn Garmin keinen nennt.
    pub activity_type: String,
}

/// Die erlaubten Ziele. Neue Ziele entstehen nur hier, mit Eintrag in [`ALLOWED_PREFIXES`].
enum Target<'a> {
    SocialProfile,
    Activities {
        start: u32,
        limit: u32,
    },
    ActivityFit {
        id: u64,
    },
    HrvRange {
        start: NaiveDate,
        end: NaiveDate,
    },
    HrvDay {
        date: NaiveDate,
    },
    SleepDay {
        display_name: &'a str,
        date: NaiveDate,
    },
    RhrRange {
        display_name: &'a str,
        start: NaiveDate,
        end: NaiveDate,
    },
    BodyBattery {
        start: NaiveDate,
        end: NaiveDate,
    },
    StressDay {
        date: NaiveDate,
    },
}

/// Pfadpräfixe, die der Client je anfragen darf. Ein Pfad außerhalb wird nie gesendet.
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

/// Ungeschützte Zeichen nach RFC 3986 bleiben, alles andere wird prozentkodiert. Damit kann ein
/// Name nie einen Pfadteil, eine Query oder ein Fragment einschleusen.
fn encode_path_segment(segment: &str) -> Result<String> {
    if segment.is_empty() || segment == "." || segment == ".." || segment.len() > 128 {
        return Err(GarminError::InvalidInput("display name"));
    }
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(out)
}

fn check_range(start: NaiveDate, end: NaiveDate) -> Result<()> {
    if start > end {
        return Err(GarminError::InvalidInput("date range"));
    }
    Ok(())
}

impl Target<'_> {
    /// Pfad samt Query, aus getypten Eingaben gebaut.
    fn path_and_query(&self) -> Result<String> {
        Ok(match self {
            Self::SocialProfile => "/userprofile-service/socialProfile".to_owned(),
            Self::Activities { start, limit } => {
                if *limit == 0 || *limit > MAX_ACTIVITY_LIMIT {
                    return Err(GarminError::InvalidInput("activity limit"));
                }
                format!(
                    "/activitylist-service/activities/search/activities?start={start}&limit={limit}"
                )
            }
            Self::ActivityFit { id } => {
                if *id == 0 {
                    return Err(GarminError::InvalidInput("activity id"));
                }
                format!("/download-service/files/activity/{id}")
            }
            Self::HrvRange { start, end } => {
                check_range(*start, *end)?;
                format!("/hrv-service/hrv/daily/{start}/{end}")
            }
            Self::HrvDay { date } => format!("/hrv-service/hrv/{date}"),
            Self::SleepDay { display_name, date } => format!(
                "/wellness-service/wellness/dailySleepData/{}?date={date}&nonSleepBufferMinutes=60",
                encode_path_segment(display_name)?
            ),
            Self::RhrRange {
                display_name,
                start,
                end,
            } => {
                check_range(*start, *end)?;
                format!(
                    "/userstats-service/wellness/daily/{}?fromDate={start}&untilDate={end}&metricId=60",
                    encode_path_segment(display_name)?
                )
            }
            Self::BodyBattery { start, end } => {
                check_range(*start, *end)?;
                format!(
                    "/wellness-service/wellness/bodyBattery/reports/daily?startDate={start}&endDate={end}"
                )
            }
            Self::StressDay { date } => format!("/wellness-service/wellness/dailyStress/{date}"),
        })
    }

    fn accept(&self) -> &'static str {
        match self {
            Self::ActivityFit { .. } => "*/*",
            _ => "application/json",
        }
    }

    fn max_body(&self) -> u64 {
        match self {
            Self::ActivityFit { .. } => MAX_FIT_BYTES,
            _ => MAX_JSON_BYTES,
        }
    }
}

/// Zweite Sicherung neben dem Aufbau aus getypten Eingaben: Der fertige Pfad muss mit einem
/// erlaubten Präfix beginnen und darf nur unverfängliche Zeichen enthalten. Fängt Fehler in
/// künftigen Änderungen ab, bevor etwas gesendet wird (Q27 prüft ebenso auf `..`, `?` im Pfad).
fn ensure_allowed(path_and_query: &str) -> Result<()> {
    let (path, query) = match path_and_query.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (path_and_query, None),
    };
    let prefix_ok = ALLOWED_PREFIXES.iter().any(|p| path.starts_with(p));
    let path_ok = path
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'.' | b'_' | b'~' | b'%'))
        && !path.split('/').any(|segment| segment == "..");
    let query_ok = query.is_none_or(|q| {
        q.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'=' | b'&' | b'-' | b'.' | b'_' | b'~' | b'%')
        })
    });
    if prefix_ok && path_ok && query_ok {
        Ok(())
    } else {
        Err(GarminError::InvalidInput("path not allowed"))
    }
}

pub struct ClientBuilder {
    endpoints: Endpoints,
    store: Arc<dyn TokenStore>,
    lockout_file: PathBuf,
    clock: Arc<dyn Clock>,
    jitter: Arc<dyn Jitter>,
    throttle: ThrottleConfig,
    config: ClientConfig,
}

impl ClientBuilder {
    /// Eigene Uhr und eigenes Schlafen, damit Tests nicht warten.
    pub fn clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn jitter(mut self, jitter: Arc<dyn Jitter>) -> Self {
        self.jitter = jitter;
        self
    }

    pub fn throttle_config(mut self, config: ThrottleConfig) -> Self {
        self.throttle = config;
        self
    }

    pub fn client_config(mut self, config: ClientConfig) -> Self {
        self.config = config;
        self
    }

    pub fn build(self) -> GarminClient {
        let throttle = Throttle::new(
            self.lockout_file,
            self.throttle,
            Arc::clone(&self.clock),
            self.jitter,
        );
        GarminClient {
            http: Http::new(self.endpoints.allow_http()),
            endpoints: self.endpoints,
            store: self.store,
            throttle,
            clock: self.clock,
            config: self.config,
            call_lock: Mutex::new(()),
        }
    }
}

pub struct GarminClient {
    endpoints: Endpoints,
    store: Arc<dyn TokenStore>,
    throttle: Throttle,
    clock: Arc<dyn Clock>,
    http: Http,
    config: ClientConfig,
    /// Hält Anfragen hintereinander (kein Parallelabruf, Abrufregel 4).
    call_lock: Mutex<()>,
}

impl std::fmt::Debug for GarminClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GarminClient { .. }")
    }
}

impl GarminClient {
    /// Client mit Systemuhr und Standarddrossel. `lockout_file` bestimmt der Aufrufer (Datenordner
    /// der App).
    pub fn new(
        endpoints: Endpoints,
        store: Arc<dyn TokenStore>,
        lockout_file: impl Into<PathBuf>,
    ) -> Self {
        Self::builder(endpoints, store, lockout_file).build()
    }

    pub fn builder(
        endpoints: Endpoints,
        store: Arc<dyn TokenStore>,
        lockout_file: impl Into<PathBuf>,
    ) -> ClientBuilder {
        ClientBuilder {
            endpoints,
            store,
            lockout_file: lockout_file.into(),
            clock: Arc::new(SystemClock),
            jitter: Arc::new(SystemJitter),
            throttle: ThrottleConfig::default(),
            config: ClientConfig::default(),
        }
    }

    // Verbindung

    /// Tauscht das Ticket der Anmeldung gegen Token und legt sie ab. Ersetzt einen vorhandenen
    /// Eintrag.
    pub fn connect(&self, ticket: &Ticket) -> Result<()> {
        let _guard = self.lock_calls();
        let tokens = self.token_endpoint().exchange_ticket(ticket)?;
        self.store.save(&tokens)?;
        Ok(())
    }

    /// Löscht die Token aus dem Schlüsselbund ("Trennen"). Widerruf bei Garmin bleibt Sache des
    /// Nutzers (SPEC 5.6).
    pub fn disconnect(&self) -> Result<()> {
        let _guard = self.lock_calls();
        self.store.delete()?;
        Ok(())
    }

    /// Ob ein lesbarer Token abgelegt ist. Sagt nichts darüber, ob Garmin ihn noch annimmt.
    pub fn is_connected(&self) -> Result<bool> {
        match self.store.load() {
            Ok(tokens) => Ok(tokens.is_some()),
            Err(StoreError::Corrupt) => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    /// Ende der Sperre, solange sie gilt.
    pub fn active_lockout(&self) -> Option<DateTime<Utc>> {
        self.throttle.active_lockout()
    }

    // Endpunkte (nur lesend)

    pub fn social_profile(&self) -> Result<Profile> {
        let value = self.get_json(&Target::SocialProfile)?;
        let display_name = value
            .get("displayName")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or(GarminError::InvalidResponse("no displayName"))?;
        Ok(Profile {
            display_name: display_name.to_owned(),
        })
    }

    /// Eine Seite der Aktivitätsliste, neueste zuerst. `start` ist der Versatz, `limit` zwischen
    /// 1 und 1000.
    pub fn activities(&self, start: u32, limit: u32) -> Result<Vec<ActivitySummary>> {
        let value = self.get_json(&Target::Activities { start, limit })?;
        let items = match &value {
            Value::Null => return Ok(Vec::new()),
            Value::Array(items) => items,
            _ => return Err(GarminError::InvalidResponse("activities not a list")),
        };
        items.iter().map(parse_activity).collect()
    }

    /// Rohbytes des Originals einer Aktivität (ZIP mit der FIT-Datei). Das Entpacken ist Sache von
    /// T22. Leer bei 204.
    pub fn activity_fit(&self, activity_id: u64) -> Result<Vec<u8>> {
        Ok(self
            .fetch(&Target::ActivityFit { id: activity_id })?
            .unwrap_or_default())
    }

    /// HRV für einen Zeitraum, unverändert als JSON (`Null` bei 204).
    pub fn hrv_range(&self, start: NaiveDate, end: NaiveDate) -> Result<Value> {
        self.get_json(&Target::HrvRange { start, end })
    }

    pub fn hrv_day(&self, date: NaiveDate) -> Result<Value> {
        self.get_json(&Target::HrvDay { date })
    }

    pub fn sleep_day(&self, display_name: &str, date: NaiveDate) -> Result<Value> {
        self.get_json(&Target::SleepDay { display_name, date })
    }

    /// Ruhepuls je Tag (`metricId=60`), unverändert als JSON.
    pub fn rhr_range(&self, display_name: &str, start: NaiveDate, end: NaiveDate) -> Result<Value> {
        self.get_json(&Target::RhrRange {
            display_name,
            start,
            end,
        })
    }

    pub fn body_battery(&self, start: NaiveDate, end: NaiveDate) -> Result<Value> {
        self.get_json(&Target::BodyBattery { start, end })
    }

    /// Stress eines Tages (SPEC 5.6 nennt Stress in der Allowlist).
    pub fn stress_day(&self, date: NaiveDate) -> Result<Value> {
        self.get_json(&Target::StressDay { date })
    }

    // Innenleben

    fn lock_calls(&self) -> std::sync::MutexGuard<'_, ()> {
        self.call_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn token_endpoint(&self) -> TokenEndpoint<'_> {
        TokenEndpoint {
            http: &self.http,
            endpoints: &self.endpoints,
            throttle: &self.throttle,
            timeout: self.config.login_timeout,
        }
    }

    fn now_s(&self) -> i64 {
        self.clock.now_ms().div_euclid(1000)
    }

    fn load_tokens(&self) -> Result<TokenSet> {
        match self.store.load() {
            Ok(Some(tokens)) => Ok(tokens),
            // Ein unlesbarer Eintrag hilft nicht weiter: neu verbinden überschreibt ihn.
            Ok(None) | Err(StoreError::Corrupt) => Err(GarminError::NotConnected),
            Err(err) => Err(err.into()),
        }
    }

    fn refresh_and_save(&self, tokens: &TokenSet) -> Result<TokenSet> {
        let fresh = self.token_endpoint().refresh(tokens)?;
        self.store.save(&fresh)?;
        Ok(fresh)
    }

    fn get_once(&self, tokens: &TokenSet, target: &Target<'_>, path: &str) -> Result<HttpResponse> {
        self.throttle.before_request()?;
        let authorization = format!("Bearer {}", tokens.access_token());
        let headers = native_headers(&[
            ("Authorization", authorization.as_str()),
            ("Accept", target.accept()),
        ]);
        let url = format!("{}{path}", self.endpoints.connectapi());
        let response = self
            .http
            .get(&url, &headers, self.config.api_timeout, target.max_body());
        self.throttle
            .after_response(response.as_ref().ok().map(|r| r.status))?;
        response
    }

    /// Eine Anfrage mit Erneuern und Wiederholung bei 401. `None` bei 204.
    fn fetch(&self, target: &Target<'_>) -> Result<Option<Vec<u8>>> {
        let path = target.path_and_query()?;
        ensure_allowed(&path)?;
        let _guard = self.lock_calls();

        let mut tokens = self.load_tokens()?;
        if needs_refresh(&tokens, self.now_s()) {
            tokens = self.refresh_and_save(&tokens)?;
        }
        let mut response = self.get_once(&tokens, target, &path)?;
        if response.status == 401 {
            // Genau einmal: Das Token kann vor der Zeit ungültig geworden sein (Q27: 401 erneuert
            // und wiederholt). Ein zweites 401 heißt, die Anmeldung ist hinfällig.
            tokens = self.refresh_and_save(&tokens)?;
            response = self.get_once(&tokens, target, &path)?;
            if response.status == 401 {
                return Err(GarminError::AuthFailed {
                    attempts: Vec::new(),
                });
            }
        }
        match response.status {
            204 => Ok(None),
            200..=299 => Ok(Some(response.body)),
            404 => Err(GarminError::NotAvailable),
            429 | 403 => Err(GarminError::Throttled),
            status @ 400..=599 => Err(GarminError::Http(status)),
            // 1xx und 3xx: Weiterleitungen werden nie befolgt, eine ist hier unerwartet.
            _ => Err(GarminError::InvalidResponse("unexpected status class")),
        }
    }

    fn get_json(&self, target: &Target<'_>) -> Result<Value> {
        match self.fetch(target)? {
            None => Ok(Value::Null),
            Some(body) if body.iter().all(u8::is_ascii_whitespace) => Ok(Value::Null),
            Some(body) => serde_json::from_slice(&body)
                .map_err(|_| GarminError::InvalidResponse("body is not json")),
        }
    }
}

fn parse_activity(item: &Value) -> Result<ActivitySummary> {
    let activity_id = item
        .get("activityId")
        .and_then(Value::as_u64)
        .ok_or(GarminError::InvalidResponse("activity without id"))?;
    let start = item
        .get("startTimeLocal")
        .and_then(Value::as_str)
        .ok_or(GarminError::InvalidResponse("activity without start time"))?;
    let start_time_local = NaiveDateTime::parse_from_str(start, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| GarminError::InvalidResponse("activity start time format"))?;
    let activity_type = item
        .pointer("/activityType/typeKey")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_owned();
    Ok(ActivitySummary {
        activity_id,
        start_time_local,
        activity_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn every_target_path_matches_the_library_and_passes_the_allowlist() {
        let id = "0a1b2c3d-0000-1111-2222-333344445555";
        let cases: Vec<(Target<'_>, String)> = vec![
            (Target::SocialProfile, "/userprofile-service/socialProfile".into()),
            (
                Target::Activities { start: 20, limit: 100 },
                "/activitylist-service/activities/search/activities?start=20&limit=100".into(),
            ),
            (Target::ActivityFit { id: 12345 }, "/download-service/files/activity/12345".into()),
            (
                Target::HrvRange { start: d("2026-09-01"), end: d("2026-09-28") },
                "/hrv-service/hrv/daily/2026-09-01/2026-09-28".into(),
            ),
            (Target::HrvDay { date: d("2026-09-01") }, "/hrv-service/hrv/2026-09-01".into()),
            (
                Target::SleepDay { display_name: id, date: d("2026-09-01") },
                format!("/wellness-service/wellness/dailySleepData/{id}?date=2026-09-01&nonSleepBufferMinutes=60"),
            ),
            (
                Target::RhrRange { display_name: id, start: d("2026-09-01"), end: d("2026-09-07") },
                format!("/userstats-service/wellness/daily/{id}?fromDate=2026-09-01&untilDate=2026-09-07&metricId=60"),
            ),
            (
                Target::BodyBattery { start: d("2026-09-01"), end: d("2026-09-07") },
                "/wellness-service/wellness/bodyBattery/reports/daily?startDate=2026-09-01&endDate=2026-09-07".into(),
            ),
            (
                Target::StressDay { date: d("2026-09-01") },
                "/wellness-service/wellness/dailyStress/2026-09-01".into(),
            ),
        ];
        for (target, expected) in cases {
            let path = target.path_and_query().unwrap();
            assert_eq!(path, expected);
            ensure_allowed(&path).unwrap();
        }
    }

    #[test]
    fn display_name_cannot_inject_path_or_query() {
        for hostile in [
            "../../x",
            "..",
            ".",
            "",
            "a/b",
            "a?b=c",
            "a#b",
            "a\\b",
            "a b",
            "%2e%2e",
            "ä",
            &"x".repeat(129),
        ] {
            let target = Target::SleepDay {
                display_name: hostile,
                date: d("2026-09-01"),
            };
            match target.path_and_query() {
                Err(_) => {}
                Ok(path) => {
                    // Prozentkodiert und damit harmlos: genau ein Segment, keine zweite Query.
                    ensure_allowed(&path).unwrap();
                    let segment = path
                        .strip_prefix("/wellness-service/wellness/dailySleepData/")
                        .unwrap()
                        .split('?')
                        .next()
                        .unwrap();
                    assert!(
                        !segment.contains('/') && segment != ".." && segment != ".",
                        "{path}"
                    );
                    assert_eq!(path.matches('?').count(), 1, "{path}");
                }
            }
        }
        // Zeichen aus UUIDs und üblichen Namen bleiben lesbar.
        assert_eq!(encode_path_segment("a-b_c.d~e").unwrap(), "a-b_c.d~e");
        assert_eq!(encode_path_segment("a/b").unwrap(), "a%2Fb");
    }

    #[test]
    fn ensure_allowed_rejects_foreign_paths() {
        for bad in [
            "/",
            "/userprofile-service/socialProfile/../../admin",
            "/userprofile-service/other",
            "/wellness-service/wellness/bodyBattery/events/2026-09-01",
            "/activity-service/activity/1",
            "https://evil.example/hrv-service/hrv/1",
            "//evil.example/hrv-service/hrv/1",
            "/hrv-service/hrv/2026-09-01#x",
            "/hrv-service/hrv/a b",
            "/hrv-service/hrv/..",
            "/hrv-service/hrv/1?a=b c",
            "/hrv-service/hrv/1?a=<",
        ] {
            assert!(ensure_allowed(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn input_ranges_are_checked() {
        let bad = [
            Target::Activities { start: 0, limit: 0 },
            Target::Activities {
                start: 0,
                limit: MAX_ACTIVITY_LIMIT + 1,
            },
            Target::ActivityFit { id: 0 },
            Target::HrvRange {
                start: d("2026-09-02"),
                end: d("2026-09-01"),
            },
            Target::BodyBattery {
                start: d("2026-09-02"),
                end: d("2026-09-01"),
            },
        ];
        for target in bad {
            assert!(matches!(
                target.path_and_query(),
                Err(GarminError::InvalidInput(_))
            ));
        }
        assert!(
            Target::Activities {
                start: 0,
                limit: MAX_ACTIVITY_LIMIT
            }
            .path_and_query()
            .is_ok()
        );
    }

    #[test]
    fn parse_activity_reads_fields_and_rejects_incomplete_items() {
        let ok = serde_json::json!({
            "activityId": 98765432101u64,
            "startTimeLocal": "2026-09-01 06:30:15",
            "activityType": {"typeKey": "running"}
        });
        let a = parse_activity(&ok).unwrap();
        assert_eq!(a.activity_id, 98_765_432_101);
        assert_eq!(a.start_time_local.to_string(), "2026-09-01 06:30:15");
        assert_eq!(a.activity_type, "running");

        let no_type = serde_json::json!({"activityId": 1, "startTimeLocal": "2026-09-01 06:30:15"});
        assert_eq!(parse_activity(&no_type).unwrap().activity_type, "unknown");

        for bad in [
            serde_json::json!({"startTimeLocal": "2026-09-01 06:30:15"}),
            serde_json::json!({"activityId": 1}),
            serde_json::json!({"activityId": 1, "startTimeLocal": "gestern"}),
            serde_json::json!({"activityId": "1", "startTimeLocal": "2026-09-01 06:30:15"}),
        ] {
            assert!(parse_activity(&bad).is_err(), "{bad}");
        }
    }
}
