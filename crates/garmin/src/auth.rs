//! Ticket, Token und die Regeln dazu (SPEC 5.6). Der Netzwerkteil (Tausch, Erneuern) steht in
//! `exchange.rs`, damit dieses Modul ohne HTTP prüfbar bleibt.
//!
//! Quelle der Protokolldetails: python-garminconnect 0.3.17 (Q27, MIT), `client.py`. Übernommen
//! ist Wissen, kein Code.

use base64::Engine;
use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::endpoints::SERVICE_URL;

/// Ein Token läuft in weniger als dieser Zeit ab: vorher erneuern (Q27, `_token_expires_soon`).
pub const REFRESH_MARGIN_S: i64 = 900;

/// Obergrenze für die Länge eines Tickets. Echte Tickets haben rund 60 bis 120 Zeichen
/// (`ST-<Nummer>-<Zufall>-cas`, Beobachtung aus der Bibliothek, keine Garantie von Garmin).
/// Die Grenze verhindert, dass beliebig lange Eingaben in eine Anfrage gelangen.
const MAX_TICKET_LEN: usize = 256;

/// URL-sicheres Base64, mit und ohne Auffüllung lesbar (JWT nutzt keine Auffüllung, die
/// Bibliothek füllt vor dem Lesen auf).
const JWT_BASE64: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// Einmal-Ticket der Garmin-Anmeldung (`ST-…`). Wirkt wie ein Passwort für einen Tausch, deshalb
/// ohne Inhalt in `Debug`. Entsteht nur über [`ticket_from_redirect`].
#[derive(Clone, PartialEq, Eq)]
pub struct Ticket(String);

impl Ticket {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Ticket { .. }")
    }
}

/// Liest das Ticket aus der Weiterleitung nach der Anmeldung. Streng: Nur
/// `https://connect.garmin.com/app` (auch mit `/` am Ende), danach genau `?ticket=ST-…` mit
/// Zeichen `[A-Za-z0-9-]` und begrenzter Länge. Alles andere ergibt `None`: fremde Hosts,
/// Unterdomänen und Anhängsel (`connect.garmin.com.evil`), `http`, Port, Zugangsdaten in der
/// Adresse, Fragment, weitere oder doppelte Parameter, Prozentkodierung, Leerzeichen.
///
/// Der Navigationsfilter des Anmeldefensters (T23) ruft das für jede Navigation auf. Je enger
/// die Annahme, desto kleiner die Fläche, über die eine Seite ein Ticket erschleichen oder
/// unterschieben könnte (SPEC 10.2).
pub fn ticket_from_redirect(url: &str) -> Option<Ticket> {
    let rest = url.strip_prefix(SERVICE_URL)?;
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    let value = rest.strip_prefix("?ticket=")?;
    let valid = value.len() > "ST-".len()
        && value.len() <= MAX_TICKET_LEN
        && value.starts_with("ST-")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-');
    valid.then(|| Ticket(value.to_owned()))
}

/// Ausgewertete Angaben aus der Nutzlast eines JWT.
#[derive(Debug, PartialEq)]
pub(crate) struct JwtClaims {
    pub exp: Option<f64>,
    pub client_id: Option<String>,
}

/// Liest `exp` und `client_id` aus der Nutzlast eines JWT, ohne die Signatur zu prüfen. Das ist
/// hier richtig: Der Client besitzt den Schlüssel von Garmin nicht und braucht die Angaben nur,
/// um den Zeitpunkt der Erneuerung zu wählen und die Client-Kennung zu merken. Ob Garmin das
/// Token annimmt, entscheidet Garmin bei der Anfrage. Wie in der Bibliothek (Q27) gilt ein Token
/// mit `alg: none` als unlesbar.
pub(crate) fn jwt_claims(token: &str) -> Option<JwtClaims> {
    let mut parts = token.split('.');
    let header = parts.next()?;
    let payload = parts.next()?;
    let header: Value = serde_json::from_slice(&JWT_BASE64.decode(header).ok()?).ok()?;
    if header
        .get("alg")
        .and_then(Value::as_str)
        .is_some_and(|alg| alg.eq_ignore_ascii_case("none"))
    {
        return None;
    }
    let payload: Value = serde_json::from_slice(&JWT_BASE64.decode(payload).ok()?).ok()?;
    let object = payload.as_object()?;
    // `exp` stammt aus einer ungeprüften Nutzlast: nur eine endliche Zahl zählt (kein Wahrheitswert,
    // keine Zeichenkette), sonst bräche ein Fehlwert jede Anfrage ab.
    let exp = object
        .get("exp")
        .and_then(Value::as_f64)
        .filter(|e| e.is_finite());
    let client_id = object
        .get("client_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    Some(JwtClaims { exp, client_id })
}

/// Zugangs- und Erneuerungstoken von Garmin (DI-OAuth2). `Debug` zeigt keinen Inhalt. Es gibt
/// bewusst weder `Serialize` noch `Display` und keinen öffentlichen Zugriff auf die Zeichenketten:
/// Token sollen den Weg Garmin, Speicher, Anfrage nie verlassen.
///
/// Die Doctests belegen, dass kein `Display` und kein `Serialize` existiert und kein Feld
/// öffentlich ist:
///
/// ```compile_fail
/// fn needs_display<T: std::fmt::Display>() {}
/// needs_display::<lastwerk_garmin::TokenSet>();
/// ```
///
/// ```compile_fail
/// fn needs_serialize<T: serde::Serialize>() {}
/// needs_serialize::<lastwerk_garmin::TokenSet>();
/// ```
///
/// ```compile_fail
/// fn read(t: &lastwerk_garmin::TokenSet) -> &str { t.access_token() }
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct TokenSet {
    /// Leer, wenn der Schlüsselbund den Zugangstoken aus Platzgründen nicht hält (siehe
    /// `store.rs`); dann erneuert der Client vor der ersten Anfrage.
    access_token: String,
    refresh_token: Option<String>,
    client_id: String,
}

impl std::fmt::Debug for TokenSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenSet { .. }")
    }
}

/// Ablageformat im Schlüsselbund. Eigener Typ, damit `TokenSet` selbst nicht serialisierbar ist.
#[derive(Serialize, Deserialize)]
pub(crate) struct StoredTokens {
    pub v: u8,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub client_id: String,
}

impl TokenSet {
    pub(crate) fn new(
        access_token: String,
        refresh_token: Option<String>,
        client_id: String,
    ) -> Self {
        Self {
            access_token,
            refresh_token,
            client_id,
        }
    }

    pub(crate) fn access_token(&self) -> &str {
        &self.access_token
    }

    pub(crate) fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    /// Kennung der Garmin-Client-Anwendung, mit der das Token ausgestellt wurde. Kein Geheimnis.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Ablaufzeit des Zugangstokens (Unix-Sekunden, abgerundet), soweit die Nutzlast sie nennt.
    pub fn expires_at(&self) -> Option<i64> {
        jwt_claims(&self.access_token)
            .and_then(|c| c.exp)
            .map(|e| e.floor() as i64)
    }

    /// Ob es ein Erneuerungstoken gibt. Ohne ist nach Ablauf nur eine neue Anmeldung möglich.
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }

    pub(crate) fn to_stored(&self, with_access_token: bool) -> StoredTokens {
        StoredTokens {
            v: 1,
            access_token: if with_access_token {
                self.access_token.clone()
            } else {
                String::new()
            },
            refresh_token: self.refresh_token.clone(),
            client_id: self.client_id.clone(),
        }
    }

    pub(crate) fn from_stored(stored: StoredTokens) -> Option<Self> {
        // Ohne Erneuerungstoken und ohne Zugangstoken ist der Eintrag wertlos.
        if stored.v != 1
            || stored.client_id.is_empty()
            || (stored.access_token.is_empty() && stored.refresh_token.is_none())
        {
            return None;
        }
        Some(Self::new(
            stored.access_token,
            stored.refresh_token,
            stored.client_id,
        ))
    }

    /// Nur für Tests: erzeugt ein Token aus Teilen.
    #[cfg(any(test, feature = "test-endpoints"))]
    pub fn for_tests(access_token: &str, refresh_token: Option<&str>, client_id: &str) -> Self {
        Self::new(
            access_token.to_owned(),
            refresh_token.map(str::to_owned),
            client_id.to_owned(),
        )
    }

    /// Nur für Tests: liest den Zugangstoken, um Ablage und Erneuerung zu prüfen.
    #[cfg(any(test, feature = "test-endpoints"))]
    pub fn access_token_for_tests(&self) -> &str {
        &self.access_token
    }

    /// Nur für Tests: liest das Erneuerungstoken.
    #[cfg(any(test, feature = "test-endpoints"))]
    pub fn refresh_token_for_tests(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }
}

/// Ob das Token erneuert werden muss: weniger als 900 s bis `exp` (Q27) oder gar kein
/// Zugangstoken. Ist `exp` nicht lesbar, gilt das Token als gültig und der 401-Weg fängt es
/// ab (wie in der Bibliothek, sonst würde jede Anfrage erneuern und Garmin belasten).
/// `now_s` sind Unix-Sekunden.
pub fn needs_refresh(tokens: &TokenSet, now_s: i64) -> bool {
    if tokens.access_token.is_empty() {
        return true;
    }
    match jwt_claims(&tokens.access_token).and_then(|c| c.exp) {
        Some(exp) => (now_s as f64) > exp - REFRESH_MARGIN_S as f64,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "https://connect.garmin.com/app?ticket=ST-0123456-AbCdEf-cas";

    fn jwt(header: &str, payload: &str) -> String {
        let enc = |s: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s);
        format!("{}.{}.sig", enc(header), enc(payload))
    }

    #[test]
    fn accepts_the_exact_redirect() {
        let t = ticket_from_redirect(VALID).unwrap();
        assert_eq!(t.as_str(), "ST-0123456-AbCdEf-cas");
        // Schrägstrich vor der Query ist erlaubt.
        let t = ticket_from_redirect("https://connect.garmin.com/app/?ticket=ST-1-a").unwrap();
        assert_eq!(t.as_str(), "ST-1-a");
    }

    #[test]
    fn rejects_hostile_and_malformed_urls() {
        let hostile = [
            // Host und Schema
            "http://connect.garmin.com/app?ticket=ST-1-a",
            "HTTPS://connect.garmin.com/app?ticket=ST-1-a",
            "https://Connect.garmin.com/app?ticket=ST-1-a",
            "https://connect.garmin.com.evil/app?ticket=ST-1-a",
            "https://connect.garmin.com.evil.example/app?ticket=ST-1-a",
            "https://evil.example/https://connect.garmin.com/app?ticket=ST-1-a",
            "https://connect.garmin.com@evil.example/app?ticket=ST-1-a",
            "https://evil.example@connect.garmin.com/app?ticket=ST-1-a",
            "https://user:pw@connect.garmin.com/app?ticket=ST-1-a",
            "https://connect.garmin.com:443/app?ticket=ST-1-a",
            "https://connect.garmin.com:8443/app?ticket=ST-1-a",
            "https://sso.garmin.com/app?ticket=ST-1-a",
            "https://xconnect.garmin.com/app?ticket=ST-1-a",
            "//connect.garmin.com/app?ticket=ST-1-a",
            "javascript:alert(1)//https://connect.garmin.com/app?ticket=ST-1-a",
            " https://connect.garmin.com/app?ticket=ST-1-a",
            "https://connect.garmin.com/app?ticket=ST-1-a ",
            "https://connect.garmin.com/app?ticket=ST-1-a\n",
            // Pfad
            "https://connect.garmin.com/",
            "https://connect.garmin.com?ticket=ST-1-a",
            "https://connect.garmin.com/application?ticket=ST-1-a",
            "https://connect.garmin.com/apple?ticket=ST-1-a",
            "https://connect.garmin.com/app//?ticket=ST-1-a",
            "https://connect.garmin.com/app/x?ticket=ST-1-a",
            "https://connect.garmin.com/app/../app?ticket=ST-1-a",
            "https://connect.garmin.com/modern/?ticket=ST-1-a",
            "https://connect.garmin.com/App?ticket=ST-1-a",
            // Query und Fragment
            "https://connect.garmin.com/app",
            "https://connect.garmin.com/app?",
            "https://connect.garmin.com/app?ticket=",
            "https://connect.garmin.com/app?ticket=ST-",
            "https://connect.garmin.com/app?ticket",
            "https://connect.garmin.com/app?Ticket=ST-1-a",
            "https://connect.garmin.com/app?foo=1&ticket=ST-1-a",
            "https://connect.garmin.com/app?ticket=ST-1-a&foo=1",
            "https://connect.garmin.com/app?ticket=ST-1-a&ticket=ST-2-b",
            "https://connect.garmin.com/app?ticket=ST-1-a#frag",
            "https://connect.garmin.com/app#?ticket=ST-1-a",
            "https://connect.garmin.com/app?ticket=ST-1-a?ticket=ST-2-b",
            "https://connect.garmin.com/app?ticket=ST-1-a;x",
            // Wert des Tickets
            "https://connect.garmin.com/app?ticket=XX-1-a",
            "https://connect.garmin.com/app?ticket=st-1-a",
            "https://connect.garmin.com/app?ticket=ST-1_a",
            "https://connect.garmin.com/app?ticket=ST-1.a",
            "https://connect.garmin.com/app?ticket=ST%2D1-a",
            "https://connect.garmin.com/app?ticket=ST-1+a",
            "https://connect.garmin.com/app?ticket=ST-1%20a",
            "https://connect.garmin.com/app?ticket=ST-1-ä",
            "https://connect.garmin.com/app?ticket=ST-1-a%0d%0aX: y",
            "https://connect.garmin.com/app?ticket=ST-<script>",
            "",
        ];
        for url in hostile {
            assert!(ticket_from_redirect(url).is_none(), "angenommen: {url:?}");
        }
    }

    #[test]
    fn ticket_length_is_bounded() {
        let ok = format!(
            "https://connect.garmin.com/app?ticket=ST-{}",
            "a".repeat(253)
        );
        assert!(ticket_from_redirect(&ok).is_some()); // genau 256 Zeichen
        let too_long = format!(
            "https://connect.garmin.com/app?ticket=ST-{}",
            "a".repeat(254)
        );
        assert!(ticket_from_redirect(&too_long).is_none());
    }

    #[test]
    fn jwt_claims_reads_exp_and_client_id() {
        let t = jwt(
            r#"{"alg":"RS256"}"#,
            r#"{"exp":1760000000,"client_id":"GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2"}"#,
        );
        let claims = jwt_claims(&t).unwrap();
        assert_eq!(claims.exp, Some(1_760_000_000.0));
        assert_eq!(
            claims.client_id.as_deref(),
            Some("GARMIN_CONNECT_MOBILE_ANDROID_DI_2025Q2")
        );
    }

    #[test]
    fn jwt_claims_tolerates_padding_and_rejects_garbage() {
        // Mit Auffüllung (`=`) wie von der Bibliothek akzeptiert.
        let padded = format!(
            "{}.{}.s",
            base64::engine::general_purpose::URL_SAFE.encode(r#"{"alg":"RS256"}"#),
            base64::engine::general_purpose::URL_SAFE.encode(r#"{"exp":5}"#)
        );
        assert_eq!(jwt_claims(&padded).unwrap().exp, Some(5.0));

        assert!(jwt_claims("").is_none());
        assert!(jwt_claims("nur-ein-teil").is_none());
        assert!(jwt_claims("a.b.c").is_none());
        // alg none gilt als unlesbar
        assert!(jwt_claims(&jwt(r#"{"alg":"none"}"#, r#"{"exp":5}"#)).is_none());
        assert!(jwt_claims(&jwt(r#"{"alg":"None"}"#, r#"{"exp":5}"#)).is_none());
        // Nutzlast kein Objekt
        assert!(jwt_claims(&jwt(r#"{"alg":"RS256"}"#, "[1,2]")).is_none());
    }

    #[test]
    fn jwt_exp_must_be_a_finite_number() {
        for payload in [
            r#"{"exp":"1760000000"}"#,
            r#"{"exp":true}"#,
            r#"{"exp":[1]}"#,
            r#"{"exp":null}"#,
            r#"{}"#,
        ] {
            let t = jwt(r#"{"alg":"RS256"}"#, payload);
            assert_eq!(jwt_claims(&t).unwrap().exp, None, "{payload}");
        }
    }

    // Referenz: exp = 10_000, Rand 900 s. Erneuern, wenn now > exp - 900 = 9_100, also ab
    // 899 s Restlaufzeit. Bei genau 900 s Restlaufzeit (now = 9_100) noch nicht.
    #[test]
    fn needs_refresh_boundary_is_900_seconds() {
        let t = jwt(r#"{"alg":"RS256"}"#, r#"{"exp":10000}"#);
        let tokens = TokenSet::for_tests(&t, Some("r"), "cid");
        assert!(!needs_refresh(&tokens, 0));
        assert!(!needs_refresh(&tokens, 9_099)); // 901 s Rest
        assert!(!needs_refresh(&tokens, 9_100)); // 900 s Rest
        assert!(needs_refresh(&tokens, 9_101)); // 899 s Rest
        assert!(needs_refresh(&tokens, 10_000));
        assert!(needs_refresh(&tokens, 20_000));
        assert_eq!(tokens.expires_at(), Some(10_000));
    }

    #[test]
    fn needs_refresh_without_readable_exp_is_false_but_empty_token_is_true() {
        let opaque = TokenSet::for_tests("opaque-token", Some("r"), "cid");
        assert!(!needs_refresh(&opaque, i64::MAX));
        let empty = TokenSet::for_tests("", Some("r"), "cid");
        assert!(needs_refresh(&empty, 0));
    }

    #[test]
    fn stored_format_round_trips_and_validates() {
        let tokens = TokenSet::for_tests("a", Some("r"), "cid");
        let json = serde_json::to_string(&tokens.to_stored(true)).unwrap();
        let back = TokenSet::from_stored(serde_json::from_str(&json).unwrap()).unwrap();
        assert_eq!(back, tokens);

        // Ohne Zugangstoken bleibt das Erneuerungstoken.
        let json = serde_json::to_string(&tokens.to_stored(false)).unwrap();
        assert!(!json.contains("access_token"));
        let back = TokenSet::from_stored(serde_json::from_str(&json).unwrap()).unwrap();
        assert_eq!(back.access_token_for_tests(), "");
        assert_eq!(back.refresh_token_for_tests(), Some("r"));

        // Wertlose Einträge
        for bad in [
            r#"{"v":2,"access_token":"a","client_id":"c"}"#,
            r#"{"v":1,"access_token":"a","client_id":""}"#,
            r#"{"v":1,"client_id":"c"}"#,
        ] {
            let stored: StoredTokens = serde_json::from_str(bad).unwrap();
            assert!(TokenSet::from_stored(stored).is_none(), "{bad}");
        }
    }
}
