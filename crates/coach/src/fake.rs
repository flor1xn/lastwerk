//! Attrappen-Client für Tests (SPEC 11.4): spielt ein Skript von Antworten ab und merkt sich
//! jeden Aufruf. Kein Netzwerk. Öffentlich, damit auch die Tests der CLI ihn nutzen können.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::client::{LlmClient, LlmError, LlmRequest, LlmResponse};

/// Skript: Jeder Aufruf nimmt die nächste Antwort. Ist das Skript leer, endet der Aufruf mit
/// `LlmError::Transport`, damit ein Test, der zu oft ruft, laut scheitert.
#[derive(Debug, Default)]
pub struct FakeClient {
    script: Mutex<VecDeque<Result<String, LlmError>>>,
    requests: Mutex<Vec<LlmRequest>>,
}

impl FakeClient {
    pub fn new(script: Vec<Result<String, LlmError>>) -> Self {
        Self {
            script: Mutex::new(script.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Antworten als Text, alle erfolgreich.
    pub fn with_replies<S: Into<String>>(replies: impl IntoIterator<Item = S>) -> Self {
        Self::new(replies.into_iter().map(|s| Ok(s.into())).collect())
    }

    /// Alle bisherigen Aufrufe in der Reihenfolge.
    pub fn requests(&self) -> Vec<LlmRequest> {
        self.requests.lock().expect("fake lock").clone()
    }

    /// Zahl der bisherigen Aufrufe.
    pub fn calls(&self) -> usize {
        self.requests.lock().expect("fake lock").len()
    }

    /// Aller Text, der zum Modell ging (Rolle und Inhalt aller Nachrichten aller Aufrufe).
    pub fn all_prompt_text(&self) -> String {
        let mut out = String::new();
        for r in self.requests() {
            for m in &r.messages {
                out.push_str(m.role.as_str());
                out.push('\n');
                out.push_str(&m.content);
                out.push('\n');
            }
        }
        out
    }
}

impl LlmClient for FakeClient {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse, LlmError> {
        self.requests.lock().expect("fake lock").push(req.clone());
        match self.script.lock().expect("fake lock").pop_front() {
            Some(Ok(content)) => Ok(LlmResponse { content }),
            Some(Err(e)) => Err(e),
            None => Err(LlmError::Transport("fake script is empty".into())),
        }
    }
}
