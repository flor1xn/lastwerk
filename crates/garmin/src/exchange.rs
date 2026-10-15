//! Tausch des Tickets gegen Token und Erneuern (SPEC 5.6).
//!
//! Protokoll nach python-garminconnect 0.3.17 (Q27, MIT), `client.py`: `_exchange_service_ticket`
//! und `_refresh_di_token`. Übernommen ist Wissen (Adresse, Formularfelder, Kopfzeilen,
//! Reihenfolge der Client-Kennungen), kein Code. Ob `diauth.garmin.com` Anfragen ohne die
//! TLS-Eigenheiten eines Browsers annimmt, zeigt erst der Livetest beim Nutzer (O30, T23).
//!
//! Jede Anfrage läuft durch die Drossel. Token und Ticket erscheinen in keinem Fehler.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use crate::auth::{Ticket, TokenSet, jwt_claims};
use crate::endpoints::{Endpoints, SERVICE_URL};
use crate::error::{AttemptFailure, AttemptReason, GarminError, Result};
use crate::http::Http;
use crate::throttle::Throttle;

/// Antworten des Token-Dienstes sind klein. Obergrenze gegen Fehlverhalten des Servers.
const MAX_TOKEN_RESPONSE_BYTES: u64 = 64 * 1024;

/// Art der Zuteilung beim Tausch eines Service-Tickets (Q27, `DI_GRANT_TYPE`).
const GRANT_SERVICE_TICKET: &str =
    "https://connectapi.garmin.com/di-oauth2-service/oauth/grant/service_ticket";

/// Client-Kennungen der Garmin-Connect-App, in dieser Reihenfolge versucht (Q27, `DI_CLIENT_IDS`).
/// Garmin nimmt je nach Zeitpunkt nur einige an; die erste angenommene gewinnt.
pub(crate) const DI_CLIENT_IDS: [&str; 4] = [
    "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2",
    "GARMIN_CONNECT_MOBILE_ANDROID_DI_2024Q4",
    "GARMIN_CONNECT_MOBILE_ANDROID_DI",
    "GARMIN_CONNECT_MOBILE_IOS_DI",
];

/// Kopfzeilen der mobilen Garmin-Connect-App (Q27, `_native_headers`). Die Schnittstelle erwartet
/// sie; ohne sie lehnt Garmin Anfragen häufiger ab.
const NATIVE_HEADERS: [(&str, &str); 8] = [
    ("User-Agent", "GCM-Android-5.23"),
    (
        "X-Garmin-User-Agent",
        "com.garmin.android.apps.connectmobile/5.23; ; Google/sdk_gphone64_arm64/google; Android/33; Dalvik/2.1.0",
    ),
    ("X-Garmin-Paired-App-Version", "10861"),
    ("X-Garmin-Client-Platform", "Android"),
    ("X-App-Ver", "10861"),
    ("X-Lang", "en"),
    ("X-GCExperience", "GC5"),
    ("Accept-Language", "en-US,en;q=0.9"),
];

/// Native Kopfzeilen plus die übergebenen, in der Reihenfolge der Bibliothek.
pub(crate) fn native_headers<'a>(extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    NATIVE_HEADERS
        .iter()
        .copied()
        .chain(extra.iter().copied())
        .collect()
}

/// `Basic <client_id>:` mit leerem Passwort (Q27, `_build_basic_auth`).
fn basic_auth(client_id: &str) -> String {
    format!("Basic {}", STANDARD.encode(format!("{client_id}:")))
}

/// Alles, was ein Aufruf zum Token-Dienst braucht.
pub(crate) struct TokenEndpoint<'a> {
    pub http: &'a Http,
    pub endpoints: &'a Endpoints,
    pub throttle: &'a Throttle,
    pub timeout: Duration,
}

impl TokenEndpoint<'_> {
    fn post(&self, client_id: &str, form: &[(&str, &str)]) -> Result<crate::http::HttpResponse> {
        self.throttle.before_request()?;
        let auth = basic_auth(client_id);
        let headers = native_headers(&[
            ("Authorization", auth.as_str()),
            ("Accept", "application/json"),
            ("Cache-Control", "no-cache"),
        ]);
        let response = self.http.post_form(
            &self.endpoints.token_url(),
            &headers,
            form,
            self.timeout,
            MAX_TOKEN_RESPONSE_BYTES,
        );
        // Ein Netzfehler (Err) zählt nicht für die Sperre; eine Antwort schon.
        self.throttle
            .after_response(response.as_ref().ok().map(|r| r.status))?;
        response
    }

    /// Tauscht das Ticket der Anmeldung gegen ein Token. Probiert die Client-Kennungen der Reihe
    /// nach und sammelt je Kennung den Grund (nur Statuszahl), ohne Inhalte der Antwort.
    /// 429 bricht sofort ab (Q27 tut dasselbe); ein Netzfehler ebenfalls, weil jede weitere
    /// Kennung am selben Fehler scheitern würde.
    pub fn exchange_ticket(&self, ticket: &Ticket) -> Result<TokenSet> {
        let mut attempts = Vec::new();
        for client_id in DI_CLIENT_IDS {
            let response = self.post(
                client_id,
                &[
                    ("client_id", client_id),
                    ("service_ticket", ticket.as_str()),
                    ("grant_type", GRANT_SERVICE_TICKET),
                    ("service_url", SERVICE_URL),
                ],
            )?;
            if response.status == 429 {
                return Err(GarminError::Throttled);
            }
            if !response.is_success() {
                attempts.push(AttemptFailure {
                    client_id,
                    reason: AttemptReason::Status(response.status),
                });
                continue;
            }
            match parse_token_response(&response.body) {
                Some(parsed) => {
                    let client_id = parsed.client_id.unwrap_or_else(|| client_id.to_owned());
                    return Ok(TokenSet::new(
                        parsed.access_token,
                        parsed.refresh_token,
                        client_id,
                    ));
                }
                None => attempts.push(AttemptFailure {
                    client_id,
                    reason: AttemptReason::InvalidResponse,
                }),
            }
        }
        Err(GarminError::AuthFailed { attempts })
    }

    /// Erneuert mit dem Erneuerungstoken (Q27, `_refresh_di_token`). Liefert Garmin kein neues
    /// Erneuerungstoken, bleibt das alte.
    pub fn refresh(&self, tokens: &TokenSet) -> Result<TokenSet> {
        let Some(refresh_token) = tokens.refresh_token() else {
            return Err(GarminError::AuthFailed {
                attempts: Vec::new(),
            });
        };
        let response = self.post(
            tokens.client_id(),
            &[
                ("grant_type", "refresh_token"),
                ("client_id", tokens.client_id()),
                ("refresh_token", refresh_token),
            ],
        )?;
        match response.status {
            429 | 403 => Err(GarminError::Throttled),
            400 | 401 => Err(GarminError::AuthFailed {
                attempts: Vec::new(),
            }),
            _ if !response.is_success() => Err(GarminError::Http(response.status)),
            _ => {
                let parsed = parse_token_response(&response.body)
                    .ok_or(GarminError::InvalidResponse("no access token in refresh"))?;
                Ok(TokenSet::new(
                    parsed.access_token,
                    parsed
                        .refresh_token
                        .or_else(|| tokens.refresh_token().map(str::to_owned)),
                    parsed
                        .client_id
                        .unwrap_or_else(|| tokens.client_id().to_owned()),
                ))
            }
        }
    }
}

struct ParsedToken {
    access_token: String,
    refresh_token: Option<String>,
    client_id: Option<String>,
}

fn parse_token_response(body: &[u8]) -> Option<ParsedToken> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?
        .to_owned();
    let refresh_token = value
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    // Die Kennung im Token gewinnt vor der versuchten (Q27): Garmin kann eine andere ausstellen.
    let client_id = jwt_claims(&access_token).and_then(|c| c.client_id);
    Some(ParsedToken {
        access_token,
        refresh_token,
        client_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // base64("GARMIN_CONNECT_MOBILE_IOS_DI:") von Hand:
    // "GARMIN_CONNECT_MOBILE_IOS_DI:" hat 29 Bytes, ergibt 40 Zeichen Base64 mit einem "=" am Ende.
    #[test]
    fn basic_auth_has_empty_password() {
        assert_eq!(basic_auth("a"), "Basic YTo="); // "a:" -> 0x61 0x3a -> "YTo="
        let encoded = basic_auth("GARMIN_CONNECT_MOBILE_IOS_DI");
        let raw = STANDARD
            .decode(encoded.strip_prefix("Basic ").unwrap())
            .unwrap();
        assert_eq!(raw, b"GARMIN_CONNECT_MOBILE_IOS_DI:");
    }

    #[test]
    fn parse_token_response_requires_access_token() {
        assert!(parse_token_response(br#"{"access_token":"a","refresh_token":"r"}"#).is_some());
        assert!(parse_token_response(br#"{"access_token":""}"#).is_none());
        assert!(parse_token_response(br#"{"refresh_token":"r"}"#).is_none());
        assert!(parse_token_response(br#"{"access_token":5}"#).is_none());
        assert!(parse_token_response(b"<html>").is_none());
        let only_access = parse_token_response(br#"{"access_token":"a"}"#).unwrap();
        assert_eq!(only_access.refresh_token, None);
    }

    #[test]
    fn client_ids_keep_the_order_of_the_library() {
        assert_eq!(DI_CLIENT_IDS[0], "GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2");
        assert_eq!(DI_CLIENT_IDS[3], "GARMIN_CONNECT_MOBILE_IOS_DI");
    }
}
