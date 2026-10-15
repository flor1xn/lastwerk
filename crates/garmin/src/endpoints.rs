//! Basisadressen der drei Garmin-Dienste (SPEC 5.6).
//!
//! Die Felder sind privat. Der einzige öffentliche Weg zu einer anderen Adresse als den echten
//! `https`-Adressen ist `for_tests`, und den gibt es nur mit der Funktion `test-endpoints` (die
//! Entwicklungsabhängigkeit des Crates schaltet sie für `cargo test` ein). Eine App, die das Crate
//! normal einbindet, kann also weder `http` noch einen fremden Host einstellen.

/// Dienstadresse, für die Garmin das Ticket ausstellt. Muss beim Tausch dieselbe sein wie bei der
/// Anmeldung (Q27: `service_url` muss zur SSO-Anmeldung passen).
pub const SERVICE_URL: &str = "https://connect.garmin.com/app";

/// Client-Kennung der Anmeldeseite im Browser (Q27, `PORTAL_SSO_CLIENT_ID`).
const PORTAL_CLIENT_ID: &str = "GarminConnect";

#[derive(Clone, PartialEq, Eq)]
pub struct Endpoints {
    sso: String,
    diauth: String,
    connectapi: String,
    allow_http: bool,
}

impl Default for Endpoints {
    /// Die echten Adressen, ausschließlich `https`.
    fn default() -> Self {
        Self {
            sso: "https://sso.garmin.com".to_owned(),
            diauth: "https://diauth.garmin.com".to_owned(),
            connectapi: "https://connectapi.garmin.com".to_owned(),
            allow_http: false,
        }
    }
}

impl std::fmt::Debug for Endpoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Endpoints")
            .field("sso", &self.sso)
            .field("diauth", &self.diauth)
            .field("connectapi", &self.connectapi)
            .finish()
    }
}

impl Endpoints {
    /// Alle drei Dienste unter einer Loopback-Adresse `http://127.0.0.1:<port>` (ein einfacher
    /// Attrappen-Server unterscheidet sie am Pfad). Andere Adressen sind ein Programmierfehler
    /// im Test und lösen eine Panik aus, damit ein Test nie versehentlich ins Netz geht.
    #[cfg(any(test, feature = "test-endpoints"))]
    pub fn for_tests(base: &str) -> Self {
        assert!(
            base.starts_with("http://127.0.0.1:") && !base.ends_with('/'),
            "for_tests nimmt nur http://127.0.0.1:<port> ohne Schrägstrich am Ende"
        );
        Self {
            sso: base.to_owned(),
            diauth: base.to_owned(),
            connectapi: base.to_owned(),
            allow_http: true,
        }
    }

    pub fn sso(&self) -> &str {
        &self.sso
    }

    pub fn diauth(&self) -> &str {
        &self.diauth
    }

    pub fn connectapi(&self) -> &str {
        &self.connectapi
    }

    /// Nur Tests setzen `http`. Der HTTP-Client erzwingt sonst `https`.
    pub(crate) fn allow_http(&self) -> bool {
        self.allow_http
    }

    /// Adresse des Token-Dienstes (Q27, `DI_TOKEN_URL`).
    pub(crate) fn token_url(&self) -> String {
        format!("{}/di-oauth2-service/oauth/token", self.diauth)
    }

    /// Adresse der Anmeldeseite von Garmin für das eigene Fenster (SPEC 5.6). `language` ist ein
    /// Sprachcode wie `de` oder `de-DE`; alles andere ergibt `None`, damit nichts in die Adresse
    /// gelangt, was dort nicht hingehört.
    pub fn sign_in_url(&self, language: &str) -> Option<String> {
        let valid = !language.is_empty()
            && language.len() <= 8
            && language
                .bytes()
                .all(|b| b.is_ascii_alphabetic() || b == b'-')
            && !language.starts_with('-')
            && !language.ends_with('-');
        valid.then(|| {
            format!(
                "{}/portal/sso/{language}/sign-in?clientId={PORTAL_CLIENT_ID}&service={SERVICE_URL}",
                self.sso
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_https_only() {
        let e = Endpoints::default();
        assert_eq!(e.sso(), "https://sso.garmin.com");
        assert_eq!(e.diauth(), "https://diauth.garmin.com");
        assert_eq!(e.connectapi(), "https://connectapi.garmin.com");
        assert!(!e.allow_http());
        assert_eq!(
            e.token_url(),
            "https://diauth.garmin.com/di-oauth2-service/oauth/token"
        );
    }

    #[test]
    fn sign_in_url_matches_spec() {
        let e = Endpoints::default();
        assert_eq!(
            e.sign_in_url("de").unwrap(),
            "https://sso.garmin.com/portal/sso/de/sign-in?clientId=GarminConnect&service=https://connect.garmin.com/app"
        );
        assert!(e.sign_in_url("de-DE").is_some());
        for bad in [
            "",
            "../x",
            "de/x",
            "de?x",
            "-de",
            "de-",
            "deutschland",
            "d e",
        ] {
            assert!(e.sign_in_url(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn for_tests_accepts_only_loopback() {
        let e = Endpoints::for_tests("http://127.0.0.1:4711");
        assert!(e.allow_http());
        assert_eq!(
            e.token_url(),
            "http://127.0.0.1:4711/di-oauth2-service/oauth/token"
        );
    }

    #[test]
    #[should_panic(expected = "for_tests")]
    fn for_tests_rejects_foreign_host() {
        let _ = Endpoints::for_tests("http://connectapi.garmin.com");
    }
}
