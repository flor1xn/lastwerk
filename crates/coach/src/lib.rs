//! lastwerk-coach: Optionale Anbindung eines lokalen Sprachmodells
//!
//! Das Modell wählt aus erlaubten Vorlagen und Dauerfenstern und erklärt in Worten, es rechnet
//! nie (SPEC 9.1). Jede Antwort wird geprüft, bei jedem Fehler bleibt der Regelplan (9.4).
//!
//! - [`provider`]: Anbieter (Ollama, OpenAI-kompatibler lokaler Server) und die Prüfung der
//!   Adresse (nur Loopback oder ein bestätigter Host im lokalen Netz).
//! - [`client`]: `LlmClient` und die HTTP-Clients (`ureq`, blockierend, mit Zeitlimit).
//! - [`fake`]: Attrappen-Client für Tests.
//! - [`input`]: der Eingang (SPEC 9.3), nur aggregierte Werte.
//! - [`edit`]: Schema und Anwendung der Änderungen (SPEC 7.2).
//! - [`refine`]: `refine_plan` mit Korrekturaufrufen und Fallback.
//! - [`chat`]: Gespräch mit dem Coach (SPEC 9.7), Antwort mit geprüftem Vorschlag.
//! - [`discover`]: Modelle eines lokalen Ollama-Servers finden und eines auswählen.
//! - [`catalog`], [`recommend`], [`pull`]: Auswahl, Empfehlung nach Hardware und Laden eines
//!   Modells (T29).
//!
//! Kein Cloud-Anbieter, kein Feld für Schlüssel (SPEC 9.2).

pub mod catalog;
pub mod chat;
pub mod client;
pub mod discover;
pub mod edit;
pub mod fake;
pub mod input;
pub mod provider;
pub mod pull;
pub mod recommend;
pub mod refine;

pub use catalog::{
    CatalogError, CatalogModel, embedded_catalog, is_valid_model_name, parse_catalog,
};
pub use chat::{
    ChatError, ChatFocus, ChatHistory, ChatInput, ChatMessage, ChatOutcome, ChatRole, ChatTopic,
    ChatView, CheckedProposal, MetricValue, PlanningChange, chat_messages, chat_system_prompt,
    chat_turn,
};
pub use client::{
    LlmClient, LlmError, LlmRequest, LlmResponse, Message, OllamaClient, OpenAiCompatibleClient,
    Role,
};
pub use discover::{ollama_models, pick_model};
pub use edit::{Edit, Issue, PlanEdit};
pub use fake::FakeClient;
pub use input::RefineInput;
pub use provider::{BaseUrl, OLLAMA_DEFAULT_URL, Provider, ProviderError};
pub use pull::{PullError, PullPhase, PullProgress, pull_model};
pub use recommend::{Fit, Hardware, ModelFit, Recommendation, recommend};
pub use refine::{
    DEFAULT_TIMEOUT_S, FallbackReason, MAX_CORRECTIONS, RefineConfig, RefineOutcome, refine_plan,
    system_prompt,
};

/// Name des Crates, nur zum Prüfen des Gerüsts.
pub const CRATE_NAME: &str = "lastwerk-coach";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }
}
