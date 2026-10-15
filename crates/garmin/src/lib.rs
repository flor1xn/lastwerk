//! Garmin-Zugang der App (SPEC 5.6, T21).
//!
//! Dieses Crate ist die einzige Stelle in Lastwerk, die mit Garmin spricht. Es
//!
//! - nimmt das Ticket der Anmeldung im Browserfenster entgegen (`ticket_from_redirect`, streng),
//! - tauscht es gegen DI-OAuth2-Token und erneuert sie (`GarminClient::connect`),
//! - legt die Token im Schlüsselbund des Betriebssystems ab (`KeyringStore`),
//! - ruft eine feste Liste lesender Endpunkte ab (`GarminClient::activities` und andere),
//! - drosselt und sperrt nach Abrufregel 4 und O24 (`Throttle`).
//!
//! Es kennt weder die Datenbank noch FIT-Dateien noch die Oberfläche. Passwort und MFA sieht es nie.
//! Token erscheinen in keinem `Debug`, `Display` und keiner Fehlermeldung. Der Zugang ist
//! inoffiziell (Q27, O18) und kann jederzeit brechen.
//!
//! Tests laufen nur gegen einen Attrappen-Server auf `127.0.0.1` (Funktion `test-endpoints`).

pub mod auth;
pub mod client;
pub mod clock;
pub mod endpoints;
pub mod error;
mod exchange;
mod http;
pub mod store;
pub mod throttle;

pub use auth::{Ticket, TokenSet, needs_refresh, ticket_from_redirect};
pub use client::{
    ActivitySummary, ClientBuilder, ClientConfig, GarminClient, MAX_ACTIVITY_LIMIT, MAX_FIT_BYTES,
    MAX_JSON_BYTES, Profile,
};
pub use clock::{Clock, Jitter, SystemClock, SystemJitter};
pub use endpoints::Endpoints;
pub use error::{AttemptFailure, AttemptReason, GarminError, NetworkKind, Result, code};
pub use store::{KeyringStore, MemoryStore, StoreError, TokenStore};
pub use throttle::{Throttle, ThrottleConfig};
