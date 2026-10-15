//! Dünne Hülle um `ureq`. Hier liegen alle Entscheidungen zum Transport:
//!
//! - TLS über rustls mit den Zertifikaten des Betriebssystems (`rustls-platform-verifier`), damit
//!   ein Firmenproxy mit eigener Zertifizierungsstelle funktioniert und keine Liste mitgelieferter
//!   Wurzelzertifikate gepflegt werden muss (SPEC 5.3).
//! - `https` ist Pflicht, außer die Adressen kommen aus `Endpoints::for_tests`.
//! - Keine Weiterleitungen: Eine 3xx-Antwort wird gemeldet, nie befolgt. Der Token-Dienst und die
//!   Daten-Schnittstelle leiten nicht um, und ein Bearer-Token soll nie an einen anderen Host gehen.
//! - Ein Proxy aus den Umgebungsvariablen (`HTTPS_PROXY` usw.) bleibt erlaubt (Nutzer hinter einem
//!   Firmenproxy), nur die Tests schalten ihn ab.
//! - Antwortgröße begrenzt, Zeitlimit je Aufruf, Fehler ohne Adresse und ohne Meldung der
//!   Bibliothek.

use std::sync::Arc;
use std::time::Duration;

use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

use crate::error::{GarminError, NetworkKind, Result};

#[derive(Debug)]
pub(crate) struct HttpResponse {
    pub status: u16,
    /// Nur bei Status 2xx gefüllt. Der Rumpf einer Fehlerantwort wird nie gelesen, damit er nirgends
    /// landen kann.
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

pub(crate) struct Http {
    agent: ureq::Agent,
}

impl Http {
    /// `allow_http` setzt nur `Endpoints::for_tests`; dann gilt auch kein Umgebungsproxy.
    pub fn new(allow_http: bool) -> Self {
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .root_certs(RootCerts::PlatformVerifier)
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        let mut builder = ureq::Agent::config_builder()
            .tls_config(tls)
            .https_only(!allow_http)
            .http_status_as_error(false)
            .max_redirects(0)
            .max_redirects_will_error(false);
        if allow_http {
            builder = builder.proxy(None);
        }
        Self {
            agent: builder.build().new_agent(),
        }
    }

    pub fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        timeout: Duration,
        max_body: u64,
    ) -> Result<HttpResponse> {
        let mut request = self.agent.get(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request
            .config()
            .timeout_global(Some(timeout))
            .build()
            .call()
            .map_err(classify)?;
        finish(response, max_body)
    }

    pub fn post_form(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        form: &[(&str, &str)],
        timeout: Duration,
        max_body: u64,
    ) -> Result<HttpResponse> {
        let mut request = self.agent.post(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request
            .config()
            .timeout_global(Some(timeout))
            .build()
            .send_form(form.iter().copied())
            .map_err(classify)?;
        finish(response, max_body)
    }
}

fn finish(mut response: ureq::http::Response<ureq::Body>, max_body: u64) -> Result<HttpResponse> {
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Ok(HttpResponse {
            status,
            body: Vec::new(),
        });
    }
    let body = response
        .body_mut()
        .with_config()
        // ureq meldet `BodyExceedsLimit` schon, wenn genau `limit` Bytes gelesen sind und noch
        // ein Lesezugriff folgt (Prüfung vor dem Ende-Test). Mit `max_body + 1` ist ein Rumpf von
        // genau `max_body` Bytes noch erlaubt und `max_body + 1` Bytes schon zu viel.
        .limit(max_body.saturating_add(1))
        .read_to_vec()
        .map_err(|err| match err {
            ureq::Error::BodyExceedsLimit(_) => GarminError::InvalidResponse("body too large"),
            other => classify(other),
        })?;
    Ok(HttpResponse { status, body })
}

/// Ordnet einen Fehler der Bibliothek ein. Die Meldung wird verworfen, weil sie Host und Pfad
/// nennen kann.
fn classify(err: ureq::Error) -> GarminError {
    use std::io::ErrorKind;
    let kind = match err {
        ureq::Error::Timeout(_) => NetworkKind::Timeout,
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => NetworkKind::Connect,
        ureq::Error::Io(ref io) => match io.kind() {
            ErrorKind::TimedOut | ErrorKind::WouldBlock => NetworkKind::Timeout,
            ErrorKind::ConnectionRefused | ErrorKind::NotFound | ErrorKind::AddrNotAvailable => {
                NetworkKind::Connect
            }
            _ => NetworkKind::Other,
        },
        ureq::Error::Tls(_) | ureq::Error::Pem(_) | ureq::Error::Rustls(_) => NetworkKind::Tls,
        _ => NetworkKind::Other,
    };
    GarminError::Network(kind)
}
