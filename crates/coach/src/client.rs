//! Schnittstelle zum Modell und die beiden HTTP-Clients (SPEC 5.4, 9.2).
//!
//! `LlmClient` ist die einzige Stelle, an der Text zum Modell geht. Die Clients rufen nur die
//! geprüfte Basisadresse auf (`BaseUrl`), folgen keiner Weiterleitung, nutzen keinen Proxy aus
//! der Umgebung (ein Proxy würde lokalen Verkehr aus dem Rechner leiten) und halten ein
//! Zeitlimit je Aufruf ein. Sie senden `stream: false`, die Antwort kommt als ein Stück.
//!
//! Beide Anbieter bekommen das Schema der erwarteten Antwort (`format` bei Ollama, `response_format` bei
//! OpenAI-kompatiblen Servern). Weil die Unterstützung schwankt (SPEC 9.2), prüfen `refine_plan`
//! und `chat_turn` jede Antwort selbst; das Schema im Aufruf ist nur eine Hilfe für das Modell.

use std::io::ErrorKind;
use std::time::Duration;

use serde_json::{Value, json};
use thiserror::Error;

use crate::provider::BaseUrl;

/// Größte Antwort in Bytes, die ein Client liest (Konvention). Eine Änderung mit 14 Einträgen
/// und 1200 Zeichen Erklärung ist weit darunter, auch mit "Denkspur" mancher Modelle.
pub const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;

/// Rolle einer Nachricht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

/// Ein Aufruf an das Modell: die Nachrichten, das Schema der erwarteten Antwort und das
/// Zeitlimit. Der gesamte Text, der zum Modell geht, steht in `messages`.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequest {
    pub messages: Vec<Message>,
    /// Schema der erwarteten Antwort als JSON (`plan-edit.schema.json` oder
    /// `plan-chat.schema.json`).
    pub schema: Value,
    pub timeout_s: u32,
}

/// Antwort des Modells: der Text der Nachricht (soll JSON nach dem Schema sein, geprüft wird
/// später).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmResponse {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LlmError {
    #[error("the model did not answer in time")]
    Timeout,
    #[error("the model server is not reachable: {0}")]
    Transport(String),
    #[error("the model server answered with status {0}")]
    Status(u16),
    #[error("the answer of the model server has an unexpected shape: {0}")]
    BadResponse(String),
}

impl LlmError {
    /// Stabiler Fehlercode für Oberfläche und Protokolle.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Transport(_) => "transport",
            Self::Status(_) => "status",
            Self::BadResponse(_) => "bad_response",
        }
    }
}

/// Ein Modell, das auf Nachrichten antwortet (SPEC 5.4).
pub trait LlmClient {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse, LlmError>;
}

/// Schema ohne die Kopfschlüssel, die Server nicht brauchen und manche nicht verstehen
/// (`$schema`, `$id`). Der Rest bleibt unverändert.
pub fn server_schema(schema: &Value) -> Value {
    let mut out = schema.clone();
    if let Some(map) = out.as_object_mut() {
        map.remove("$schema");
        map.remove("$id");
    }
    out
}

fn messages_json(req: &LlmRequest) -> Value {
    Value::Array(
        req.messages
            .iter()
            .map(|m| json!({ "role": m.role.as_str(), "content": m.content }))
            .collect(),
    )
}

pub(crate) fn map_ureq_error(e: ureq::Error) -> LlmError {
    match e {
        ureq::Error::Timeout(_) => LlmError::Timeout,
        ureq::Error::Io(io) if matches!(io.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
            LlmError::Timeout
        }
        // Die Meldung von ureq nennt Adresse und Art des Fehlers, nie Inhalte der Anfrage.
        other => LlmError::Transport(other.to_string()),
    }
}

/// Agent für genau einen Aufruf: keine Weiterleitung, kein Proxy aus der Umgebung, Status
/// außerhalb von 2xx wird selbst geprüft, globales Zeitlimit. Gilt für jeden Aufruf an einen
/// Modellserver, auch für die Modellliste (`discover`).
pub(crate) fn agent(timeout_s: u32) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(u64::from(timeout_s.max(1)))))
        .build();
    ureq::Agent::new_with_config(config)
}

/// Prüft den Status und liest höchstens [`MAX_RESPONSE_BYTES`] als JSON.
fn read_json(mut response: ureq::http::Response<ureq::Body>) -> Result<Value, LlmError> {
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(LlmError::Status(status));
    }
    let raw = response
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE_BYTES)
        .read_to_string()
        .map_err(map_ureq_error)?;
    serde_json::from_str(&raw).map_err(|_| LlmError::BadResponse("the body is not json".into()))
}

/// Sendet `body` als JSON an `url` und liefert den Text der Antwort. Eine Weiterleitung, ein
/// Proxy aus der Umgebung und ein Status außerhalb von 200 bis 299 sind Fehler.
fn post_json(url: &str, body: &Value, timeout_s: u32) -> Result<Value, LlmError> {
    let text = serde_json::to_string(body)
        .map_err(|e| LlmError::Transport(format!("request cannot be encoded: {e}")))?;
    let response = agent(timeout_s)
        .post(url)
        .header("Content-Type", "application/json")
        .send(text)
        .map_err(map_ureq_error)?;
    read_json(response)
}

/// `GET` auf `url` mit denselben Regeln wie [`post_json`]. Nur für die Modellliste.
pub(crate) fn get_json(url: &str, timeout_s: u32) -> Result<Value, LlmError> {
    let response = agent(timeout_s).get(url).call().map_err(map_ureq_error)?;
    read_json(response)
}

/// Name des Schemas für `response_format` aus `$id`, zum Beispiel `plan_chat` für
/// `plan-chat.schema.json`. Ohne `$id` gilt `plan_edit` wie bisher.
fn schema_name(schema: &Value) -> String {
    schema
        .get("$id")
        .and_then(Value::as_str)
        .map(|id| id.trim_end_matches(".schema.json").replace(['-', '.'], "_"))
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or_else(|| "plan_edit".to_string())
}

/// Ollama: `POST /api/chat` mit `format` (Schema) und `stream: false`.
#[derive(Debug, Clone)]
pub struct OllamaClient {
    base_url: BaseUrl,
    model: String,
}

impl OllamaClient {
    pub fn new(base_url: BaseUrl, model: String) -> Self {
        Self { base_url, model }
    }

    fn body(&self, req: &LlmRequest) -> Value {
        json!({
            "model": self.model,
            "messages": messages_json(req),
            "stream": false,
            "format": server_schema(&req.schema),
            // Temperatur 0 macht die Antworten bei gleichem Eingang möglichst gleich
            // (Konvention, keine Garantie).
            "options": { "temperature": 0 },
        })
    }
}

impl LlmClient for OllamaClient {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse, LlmError> {
        let url = self.base_url.endpoint("/api/chat", false);
        let answer = post_json(&url, &self.body(req), req.timeout_s)?;
        let content = answer
            .pointer("/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| LlmError::BadResponse("message.content is missing".into()))?;
        Ok(LlmResponse {
            content: content.to_string(),
        })
    }
}

/// OpenAI-kompatibler lokaler Server: `POST /v1/chat/completions` mit `response_format`
/// (`json_schema`) und `stream: false`. Kein Schlüssel, keine Kopfzeile für Zugangsdaten.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClient {
    base_url: BaseUrl,
    model: String,
}

impl OpenAiCompatibleClient {
    pub fn new(base_url: BaseUrl, model: String) -> Self {
        Self { base_url, model }
    }

    fn body(&self, req: &LlmRequest) -> Value {
        json!({
            "model": self.model,
            "messages": messages_json(req),
            "stream": false,
            "temperature": 0,
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": schema_name(&req.schema),
                    "strict": true,
                    "schema": server_schema(&req.schema),
                },
            },
        })
    }
}

impl LlmClient for OpenAiCompatibleClient {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse, LlmError> {
        let url = self.base_url.endpoint("/v1/chat/completions", true);
        let answer = post_json(&url, &self.body(req), req.timeout_s)?;
        let content = answer
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| LlmError::BadResponse("choices[0].message.content is missing".into()))?;
        Ok(LlmResponse {
            content: content.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_name_comes_from_the_id() {
        assert_eq!(schema_name(&crate::edit::edit_schema()), "plan_edit");
        assert_eq!(schema_name(&crate::chat::chat_schema()), "plan_chat");
        assert_eq!(schema_name(&json!({})), "plan_edit");
        assert_eq!(schema_name(&json!({"$id": "a b"})), "plan_edit");
    }
}
