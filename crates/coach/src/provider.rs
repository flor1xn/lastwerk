//! Anbieter und Adressprüfung (SPEC 5.4, 9.2).
//!
//! Lastwerk ruft nur lokale Modelle auf. Die Adresse (`BaseUrl`) wird beim Anlegen geprüft und
//! kann danach nicht mehr verändert werden: Schema `http`, Host Loopback (`127.0.0.1`, `::1`,
//! `localhost`) oder, mit ausdrücklicher Bestätigung (`allow_lan`), eine private Adresse
//! (RFC 1918, Unique Local `fc00::/7`, Link-Local). Alles andere, auch ein öffentlicher Host
//! mit Bestätigung, ist `ProviderError::HostNotAllowed`. Es gibt kein Feld für Zugangsdaten
//! (kein `user@` in der Adresse) und keinen Standardwert außer Ollama auf Loopback.
//!
//! Hostnamen außer `localhost` werden abgelehnt, auch wenn sie im lokalen Netz liegen: Ohne
//! Namensauflösung lässt sich ihr Ziel nicht prüfen, und eine Auflösung vor der Prüfung wäre
//! ein Netzwerkzugriff auf eine vom Nutzer gelieferte Adresse. Im lokalen Netz zählt die
//! IP-Adresse. Kurzformen wie `2130706433` oder `127.1` erkennt die Prüfung nicht als Adresse
//! und lehnt sie deshalb ab.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use thiserror::Error;
use ureq::http::Uri;

use crate::client::{LlmClient, OllamaClient, OpenAiCompatibleClient};

/// Standardadresse von Ollama (SPEC 9.2). Der einzige Standardwert.
pub const OLLAMA_DEFAULT_URL: &str = "http://127.0.0.1:11434";

/// Längster zulässiger Modellname in Zeichen (Konvention).
const MAX_MODEL_NAME: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderError {
    #[error("the url is invalid: {0}")]
    InvalidUrl(String),
    #[error("scheme {0:?} is not allowed, only http")]
    SchemeNotAllowed(String),
    #[error("https is not supported in this version")]
    TlsNotSupported,
    #[error("host {host:?} is not allowed")]
    HostNotAllowed {
        host: String,
        /// Die Adresse liegt im lokalen Netz, aber der Nutzer hat es nicht bestätigt.
        lan_confirmation_missing: bool,
    },
    #[error("the model name is invalid")]
    InvalidModel,
}

impl ProviderError {
    /// Stabiler Fehlercode für Oberfläche und Protokolle.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidUrl(_) => "invalid_url",
            Self::SchemeNotAllowed(_) => "scheme_not_allowed",
            Self::TlsNotSupported => "tls_not_supported",
            Self::HostNotAllowed { .. } => "host_not_allowed",
            Self::InvalidModel => "invalid_model",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostClass {
    Loopback,
    Private,
    Other,
}

fn classify_v4(ip: Ipv4Addr) -> HostClass {
    if ip.is_loopback() {
        HostClass::Loopback
    } else if ip.is_private() || ip.is_link_local() {
        HostClass::Private
    } else {
        HostClass::Other
    }
}

fn classify_v6(ip: Ipv6Addr) -> HostClass {
    if ip.is_loopback() {
        return HostClass::Loopback;
    }
    if let Some(v4) = ip.to_ipv4_mapped() {
        return classify_v4(v4);
    }
    let first = ip.segments()[0];
    // fc00::/7 (Unique Local) und fe80::/10 (Link-Local).
    if first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80 {
        HostClass::Private
    } else {
        HostClass::Other
    }
}

/// Ordnet einen Host (ohne Klammern bei IPv6) einer Klasse zu.
fn classify_host(host: &str) -> HostClass {
    if host == "localhost" {
        return HostClass::Loopback;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => classify_v4(ip),
        Ok(IpAddr::V6(ip)) => classify_v6(ip),
        Err(_) => HostClass::Other,
    }
}

/// Geprüfte Basisadresse eines lokalen Modellservers. Nur über [`BaseUrl::parse`] zu bekommen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseUrl {
    /// `host` oder `host:port`, IPv6 in Klammern.
    authority: String,
    /// Pfadpräfix ohne abschließenden Schrägstrich, meist leer.
    prefix: String,
}

impl BaseUrl {
    /// Prüft `text` und legt die Adresse an. `allow_lan` ist die ausdrückliche Bestätigung des
    /// Nutzers für Hosts im lokalen Netz.
    pub fn parse(text: &str, allow_lan: bool) -> Result<Self, ProviderError> {
        let invalid = |what: &str| ProviderError::InvalidUrl(what.to_string());
        let uri: Uri = text.trim().parse().map_err(|_| invalid("syntax"))?;
        let scheme = uri.scheme_str().ok_or_else(|| invalid("scheme missing"))?;
        let authority = uri.authority().ok_or_else(|| invalid("host missing"))?;
        if authority.as_str().contains('@') {
            // Zugangsdaten in der Adresse gibt es nicht (SPEC 9.2).
            return Err(invalid("credentials"));
        }
        if uri.query().is_some() {
            return Err(invalid("query"));
        }
        let raw_host = authority.host();
        let host = raw_host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(raw_host)
            .to_ascii_lowercase();
        if host.is_empty() {
            return Err(invalid("host missing"));
        }
        match classify_host(&host) {
            HostClass::Loopback => {}
            HostClass::Private if allow_lan => {}
            HostClass::Private => {
                return Err(ProviderError::HostNotAllowed {
                    host,
                    lan_confirmation_missing: true,
                });
            }
            HostClass::Other => {
                return Err(ProviderError::HostNotAllowed {
                    host,
                    lan_confirmation_missing: false,
                });
            }
        }
        match scheme {
            "http" => {}
            "https" => return Err(ProviderError::TlsNotSupported),
            other => return Err(ProviderError::SchemeNotAllowed(other.to_string())),
        }
        let shown_host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host
        };
        let authority = match authority.port_u16() {
            Some(port) => format!("{shown_host}:{port}"),
            None => shown_host,
        };
        let prefix = uri.path().trim_end_matches('/').to_string();
        Ok(Self { authority, prefix })
    }

    /// Adresse eines Pfads auf dem Server, zum Beispiel `/api/chat`. Ein Pfadpräfix der
    /// Basisadresse bleibt erhalten; endet es auf `/v1`, wird es für OpenAI-kompatible Server
    /// nicht doppelt angehängt (`strip_v1`).
    pub fn endpoint(&self, path: &str, strip_v1: bool) -> String {
        let mut prefix = self.prefix.as_str();
        if strip_v1 {
            prefix = prefix.strip_suffix("/v1").unwrap_or(prefix);
        }
        format!("http://{}{}{}", self.authority, prefix, path)
    }

    /// Basisadresse als Text (ohne abschließenden Schrägstrich).
    pub fn as_text(&self) -> String {
        format!("http://{}{}", self.authority, self.prefix)
    }
}

impl std::fmt::Display for BaseUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_text())
    }
}

fn check_model(model: &str) -> Result<String, ProviderError> {
    let model = model.trim();
    if model.is_empty()
        || model.chars().count() > MAX_MODEL_NAME
        || model.chars().any(char::is_control)
    {
        return Err(ProviderError::InvalidModel);
    }
    Ok(model.to_string())
}

/// Lokaler Modellanbieter (SPEC 5.4). Kein Cloud-Anbieter, kein Schlüsselfeld.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    Ollama {
        base_url: BaseUrl,
        model: String,
    },
    /// LM Studio, llama.cpp-Server und ähnliche. Die Adresse setzt der Nutzer, es gibt keinen
    /// Standardwert.
    OpenAiCompatible {
        base_url: BaseUrl,
        model: String,
    },
}

impl Provider {
    /// Ollama. Ohne Adresse gilt `http://127.0.0.1:11434`.
    pub fn ollama(
        base_url: Option<&str>,
        model: &str,
        allow_lan: bool,
    ) -> Result<Self, ProviderError> {
        Ok(Self::Ollama {
            base_url: BaseUrl::parse(base_url.unwrap_or(OLLAMA_DEFAULT_URL), allow_lan)?,
            model: check_model(model)?,
        })
    }

    /// OpenAI-kompatibler lokaler Server. Die Adresse ist Pflicht.
    pub fn openai_compatible(
        base_url: &str,
        model: &str,
        allow_lan: bool,
    ) -> Result<Self, ProviderError> {
        Ok(Self::OpenAiCompatible {
            base_url: BaseUrl::parse(base_url, allow_lan)?,
            model: check_model(model)?,
        })
    }

    pub fn base_url(&self) -> &BaseUrl {
        match self {
            Self::Ollama { base_url, .. } | Self::OpenAiCompatible { base_url, .. } => base_url,
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::Ollama { model, .. } | Self::OpenAiCompatible { model, .. } => model,
        }
    }

    /// Kurzname für Ausgaben.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ollama { .. } => "ollama",
            Self::OpenAiCompatible { .. } => "openai-compatible",
        }
    }

    /// Der Client zu diesem Anbieter. Er ruft nur die geprüfte Adresse auf.
    pub fn client(&self) -> Box<dyn LlmClient> {
        match self {
            Self::Ollama { base_url, model } => {
                Box::new(OllamaClient::new(base_url.clone(), model.clone()))
            }
            Self::OpenAiCompatible { base_url, model } => {
                Box::new(OpenAiCompatibleClient::new(base_url.clone(), model.clone()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_error(text: &str, allow_lan: bool) -> Option<(String, bool)> {
        match BaseUrl::parse(text, allow_lan) {
            Err(ProviderError::HostNotAllowed {
                host,
                lan_confirmation_missing,
            }) => Some((host, lan_confirmation_missing)),
            _ => None,
        }
    }

    #[test]
    fn loopback_hosts_are_accepted_without_confirmation() {
        for text in [
            "http://127.0.0.1:11434",
            "http://127.0.0.1",
            "http://localhost:1234",
            "http://LOCALHOST:1234/",
            "http://[::1]:8080",
            "http://127.1.2.3:80",
            "http://[::ffff:127.0.0.1]:80",
        ] {
            assert!(BaseUrl::parse(text, false).is_ok(), "{text}");
        }
    }

    #[test]
    fn default_ollama_url_is_loopback() {
        let p = Provider::ollama(None, "llama3.2", false).unwrap();
        assert_eq!(p.base_url().as_text(), "http://127.0.0.1:11434");
        assert_eq!(p.model(), "llama3.2");
        assert_eq!(
            p.base_url().endpoint("/api/chat", false),
            "http://127.0.0.1:11434/api/chat"
        );
    }

    #[test]
    fn public_hosts_are_rejected_even_with_confirmation() {
        for text in [
            "https://api.openai.com",
            "https://api.anthropic.com",
            "http://api.openai.com",
            "http://8.8.8.8:11434",
            "https://1.1.1.1",
            "http://[2001:4860:4860::8888]",
            "http://[::ffff:8.8.8.8]",
            "http://0.0.0.0:11434",
            "http://224.0.0.1",
            "http://example.com",
            "http://localhost.evil.example",
            "http://127.0.0.1.evil.example",
            // Kurzformen von Adressen sind keine Adressen für die Prüfung und fallen durch.
            "http://2130706433",
            "http://127.1",
            "http://0x7f.0.0.1",
        ] {
            for allow_lan in [false, true] {
                assert!(
                    host_error(text, allow_lan).is_some(),
                    "{text} allow_lan={allow_lan}"
                );
            }
        }
    }

    #[test]
    fn private_hosts_need_the_confirmation() {
        for text in [
            "http://192.168.1.20:1234",
            "http://10.0.0.5",
            "http://172.16.0.1",
            "http://172.31.255.254",
            "http://169.254.10.10",
            "http://[fd12:3456::1]:8080",
            "http://[fe80::1]",
        ] {
            let (_, missing) = host_error(text, false).expect(text);
            assert!(missing, "{text}");
            assert!(BaseUrl::parse(text, true).is_ok(), "{text}");
        }
        // Die Grenzen der Bereiche liegen auf der öffentlichen Seite.
        for text in [
            "http://172.15.0.1",
            "http://172.32.0.1",
            "http://192.169.0.1",
        ] {
            assert_eq!(host_error(text, true), Some((text[7..].to_string(), false)));
        }
    }

    #[test]
    fn credentials_queries_and_odd_schemes_are_rejected() {
        assert!(matches!(
            BaseUrl::parse("http://user:pw@127.0.0.1:11434", false),
            Err(ProviderError::InvalidUrl(_))
        ));
        // Der Host nach dem @ ist der echte: kein Umweg über einen Loopback-Namen davor.
        assert!(matches!(
            BaseUrl::parse("http://127.0.0.1@evil.example/", true),
            Err(ProviderError::InvalidUrl(_))
        ));
        assert!(matches!(
            BaseUrl::parse("http://127.0.0.1:11434/?key=abc", false),
            Err(ProviderError::InvalidUrl(_))
        ));
        assert!(matches!(
            BaseUrl::parse("ftp://127.0.0.1", false),
            Err(ProviderError::SchemeNotAllowed(_))
        ));
        assert!(matches!(
            BaseUrl::parse("127.0.0.1:11434", false),
            Err(ProviderError::InvalidUrl(_))
        ));
        assert!(matches!(
            BaseUrl::parse("", false),
            Err(ProviderError::InvalidUrl(_))
        ));
        assert_eq!(
            BaseUrl::parse("https://127.0.0.1:11434", false),
            Err(ProviderError::TlsNotSupported)
        );
    }

    #[test]
    fn path_prefix_is_kept_and_v1_is_not_doubled() {
        let u = BaseUrl::parse("http://127.0.0.1:1234/v1/", false).unwrap();
        assert_eq!(
            u.endpoint("/v1/chat/completions", true),
            "http://127.0.0.1:1234/v1/chat/completions"
        );
        let u = BaseUrl::parse("http://127.0.0.1:1234/proxy", false).unwrap();
        assert_eq!(
            u.endpoint("/v1/chat/completions", true),
            "http://127.0.0.1:1234/proxy/v1/chat/completions"
        );
        let u = BaseUrl::parse("http://[::1]:1234", false).unwrap();
        assert_eq!(u.endpoint("/api/chat", false), "http://[::1]:1234/api/chat");
    }

    #[test]
    fn model_names_are_checked() {
        for bad in ["", "   ", "a\nb", &"x".repeat(201)] {
            assert_eq!(
                Provider::ollama(None, bad, false),
                Err(ProviderError::InvalidModel)
            );
        }
        assert!(Provider::ollama(None, "qwen3:8b", false).is_ok());
    }

    #[test]
    fn openai_compatible_has_no_default_host() {
        assert!(Provider::openai_compatible("", "m", false).is_err());
        assert!(Provider::openai_compatible("http://127.0.0.1:1234", "m", false).is_ok());
        assert_eq!(
            Provider::openai_compatible("https://api.openai.com", "gpt", true),
            Err(ProviderError::HostNotAllowed {
                host: "api.openai.com".into(),
                lan_confirmation_missing: false
            })
        );
    }
}
