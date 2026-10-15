//! Fehler des Crates mit stabilen Codes für die App (SPEC 5.6, 10.2).
//!
//! Kein Fehler trägt eine URL (schon gar keine mit Query), ein Token, ein Ticket oder Teile
//! einer Antwort. Die Texte von `Display` sind feste englische Kurztexte, die Oberfläche
//! übersetzt über den Code (`catalog/locales`). Was Garmin geantwortet hat, steht höchstens
//! als Statuszahl darin.

use chrono::{DateTime, Utc};

use crate::store::StoreError;

/// Schlüssel für die App. Stabil: Die Oberfläche hängt Texte daran.
pub mod code {
    pub const NOT_CONNECTED: &str = "not_connected";
    pub const AUTH_FAILED: &str = "auth_failed";
    pub const THROTTLED: &str = "throttled";
    pub const LOCKED: &str = "locked";
    pub const NOT_AVAILABLE: &str = "not_available";
    pub const NETWORK: &str = "network";
    pub const INVALID_RESPONSE: &str = "invalid_response";
    /// Zusatz zur Liste der Aufgabe: Der Schlüsselbund ist nicht erreichbar oder voll.
    pub const STORE_FAILED: &str = "store_failed";
    /// Zusatz zur Liste der Aufgabe: Fehler des Aufrufers (Datum, Kennung, Name).
    pub const INVALID_INPUT: &str = "invalid_input";
}

/// Art eines Netzfehlers. Bewusst ohne die Meldung der HTTP-Bibliothek, weil sie Host und
/// Pfad enthalten kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkKind {
    /// Zeitlimit überschritten (Verbindung, Senden oder Antwort).
    Timeout,
    /// Name nicht aufgelöst oder Verbindung verweigert.
    Connect,
    /// TLS-Handshake oder Zertifikatsprüfung fehlgeschlagen.
    Tls,
    /// Alles andere (abgebrochene Verbindung, Protokollfehler).
    Other,
}

/// Warum ein Versuch beim Tausch des Tickets scheiterte. Die Client-Kennung ist kein
/// Geheimnis (sie steht in jeder App-Version), die Statuszahl ebenso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptFailure {
    pub client_id: &'static str,
    pub reason: AttemptReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptReason {
    /// Garmin lehnte mit diesem Status ab.
    Status(u16),
    /// Antwort ohne verwertbares Token.
    InvalidResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GarminError {
    /// Kein Token im Schlüsselbund (oder der Eintrag ist unlesbar): neu verbinden.
    #[error("not connected to Garmin")]
    NotConnected,
    /// Anmeldung abgelehnt: Ticket ungültig, Token abgelaufen und nicht erneuerbar.
    /// `attempts` ist beim Tausch des Tickets je Client-Kennung gefüllt, sonst leer.
    #[error("Garmin rejected the login")]
    AuthFailed { attempts: Vec<AttemptFailure> },
    /// 429 oder 403 von Garmin.
    #[error("Garmin is throttling requests")]
    Throttled,
    /// Sperre nach drei 429 oder 403 in Folge, aktiv bis `until`.
    #[error("requests to Garmin are locked until {until}")]
    Locked { until: DateTime<Utc> },
    /// 404: Garmin hat zu diesem Tag oder dieser Aktivität nichts.
    #[error("not available at Garmin")]
    NotAvailable,
    #[error("network error ({0:?})")]
    Network(NetworkKind),
    /// Anderer 4xx oder 5xx.
    #[error("unexpected HTTP status {0}")]
    Http(u16),
    /// Antwort zu groß, kein JSON oder ohne erwartetes Feld. Der Text nennt nur den Grund.
    #[error("invalid response from Garmin ({0})")]
    InvalidResponse(&'static str),
    #[error("token store failed ({0})")]
    Store(StoreError),
    /// Eingabe des Aufrufers abgelehnt, bevor etwas gesendet wurde.
    #[error("invalid input ({0})")]
    InvalidInput(&'static str),
}

impl GarminError {
    /// Stabiler Code für die Oberfläche. Mehrere Varianten teilen sich einen Code, wenn die
    /// Oberfläche sie gleich behandelt: `Http` zählt als `network` (Server oder Weg gestört).
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConnected => code::NOT_CONNECTED,
            Self::AuthFailed { .. } => code::AUTH_FAILED,
            Self::Throttled => code::THROTTLED,
            Self::Locked { .. } => code::LOCKED,
            Self::NotAvailable => code::NOT_AVAILABLE,
            Self::Network(_) | Self::Http(_) => code::NETWORK,
            Self::InvalidResponse(_) => code::INVALID_RESPONSE,
            Self::Store(_) => code::STORE_FAILED,
            Self::InvalidInput(_) => code::INVALID_INPUT,
        }
    }
}

impl From<StoreError> for GarminError {
    fn from(err: StoreError) -> Self {
        Self::Store(err)
    }
}

pub type Result<T> = std::result::Result<T, GarminError>;
