//! `LlmBackend` — the one seam between the deterministic core and any LLM.
//!
//! Every **LLM-reasoning op** (docs/interaction-surfaces.md §"deterministic vs.
//! LLM-reasoning ops") — phrasing a gap question, extraction, SME augmentation,
//! resolution adjudication, generative heal content — goes through this trait.
//! The deterministic core never names a provider; it holds a `&dyn LlmBackend`.
//! Swapping the model (an external API, a local model, the ambient coding agent,
//! or the test [`MockLlmBackend`]) touches only the wiring, never the core.
//! This is what makes the interaction-surface / LLM-provider decision safe to
//! defer (IS-4).
//!
//! ## Design (from first principles, not inherited)
//!
//! - **Synchronous.** The core is sync and dependency-light; a sync boundary
//!   serves the mock and the *agent-native* route (the ambient agent supplies
//!   the answer in-context) with no async runtime. A future hosted-HTTP surface
//!   can bridge to async inside its own backend impl — the core need not change.
//! - **Object-safe.** The trait has one core method so callers can store
//!   `Box<dyn LlmBackend>`; typed-JSON parsing lives in the free function
//!   [`complete_json`] rather than a generic trait method (which would break
//!   object safety).
//! - **Fail-loud.** A backend that can't answer returns [`LlmError`]; callers
//!   that must not block the loop degrade gracefully and *flag* it (never ship a
//!   silent fallback — the same discipline as the rest of the core).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;

/// Sampling / decoding parameters. All optional so a backend applies its own
/// defaults for anything unset.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LlmParams {
    /// Sampling temperature (0.0 = deterministic-ish).
    pub temperature: Option<f32>,
    /// Cap on generated tokens.
    pub max_tokens: Option<u32>,
}

/// One completion request. Provider-neutral: a system framing, the prompt, and
/// a hint that the caller wants machine-readable JSON back.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmRequest {
    /// Optional system/role framing.
    pub system: Option<String>,
    /// The user prompt.
    pub prompt: String,
    /// Decoding parameters.
    pub params: LlmParams,
    /// Hint that the caller will parse the response as JSON — a backend may use
    /// it to enable a JSON/structured-output mode.
    pub expect_json: bool,
}

impl LlmRequest {
    /// A plain prompt with no system framing.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            system: None,
            prompt: prompt.into(),
            params: LlmParams::default(),
            expect_json: false,
        }
    }

    /// Set the system framing.
    #[must_use]
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Set decoding parameters.
    #[must_use]
    pub fn with_params(mut self, params: LlmParams) -> Self {
        self.params = params;
        self
    }

    /// Mark that the caller expects JSON back.
    #[must_use]
    pub fn expecting_json(mut self) -> Self {
        self.expect_json = true;
        self
    }
}

/// A completion.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmResponse {
    /// The generated text (JSON string when the request expected JSON).
    pub text: String,
    /// The model that produced it, if the backend reports one.
    pub model: Option<String>,
}

impl LlmResponse {
    /// A bare text response with no model attribution.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            model: None,
        }
    }
}

/// Why an LLM call failed.
#[derive(Debug, Clone, PartialEq)]
pub enum LlmError {
    /// The backend failed to produce a completion (network, provider, quota…).
    Backend(String),
    /// The completion could not be parsed as the caller expected (e.g. JSON).
    Parse(String),
    /// No response was available (e.g. a mock ran dry with no default).
    NoResponse,
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmError::Backend(m) => write!(f, "LLM backend error: {m}"),
            LlmError::Parse(m) => write!(f, "LLM response parse error: {m}"),
            LlmError::NoResponse => write!(f, "LLM backend produced no response"),
        }
    }
}

impl std::error::Error for LlmError {}

/// The pluggable LLM boundary. Implementors: an HTTP provider, a local model,
/// an adapter that delegates to the ambient coding agent, or [`MockLlmBackend`].
///
/// Kept object-safe on purpose so the core can hold `&dyn LlmBackend` /
/// `Box<dyn LlmBackend>` and stay provider-neutral at runtime.
pub trait LlmBackend {
    /// A short, stable identifier for logs/provenance (e.g. `"mock"`, `"openai"`).
    fn name(&self) -> &str;

    /// Produce a completion for `request`.
    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, LlmError>;
}

/// Request a completion and parse it as `T`. A free function (not a trait
/// method) so [`LlmBackend`] stays object-safe. Sets the JSON hint on the
/// request for backends that support a structured-output mode.
pub fn complete_json<T: serde::de::DeserializeOwned>(
    backend: &dyn LlmBackend,
    request: &LlmRequest,
) -> Result<T, LlmError> {
    let mut req = request.clone();
    req.expect_json = true;
    let response = backend.complete(&req)?;
    serde_json::from_str(&response.text)
        .map_err(|e| LlmError::Parse(format!("{e} — body was: {}", response.text)))
}

/// A scriptable, deterministic backend for tests (and for a dry-run / offline
/// mode). Resolution order per call:
///
/// 1. the first substring **rule** whose key appears in the prompt,
/// 2. else the next **queued** response (FIFO),
/// 3. else the **default** response,
/// 4. else [`LlmError::NoResponse`].
///
/// Every request is recorded for assertions.
pub struct MockLlmBackend {
    name: String,
    default: Option<String>,
    rules: Vec<(String, String)>,
    queue: RefCell<VecDeque<String>>,
    calls: RefCell<Vec<LlmRequest>>,
}

impl MockLlmBackend {
    /// An empty mock (no default → runs dry with [`LlmError::NoResponse`]).
    pub fn new() -> Self {
        Self {
            name: "mock".to_string(),
            default: None,
            rules: Vec::new(),
            queue: RefCell::new(VecDeque::new()),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// Set the fallback response returned when no rule/queue entry matches.
    #[must_use]
    pub fn with_default(mut self, text: impl Into<String>) -> Self {
        self.default = Some(text.into());
        self
    }

    /// Enqueue a response (returned FIFO, consumed once).
    #[must_use]
    pub fn push(self, text: impl Into<String>) -> Self {
        self.queue.borrow_mut().push_back(text.into());
        self
    }

    /// Return `text` for any prompt containing `substring`.
    #[must_use]
    pub fn on_contains(mut self, substring: impl Into<String>, text: impl Into<String>) -> Self {
        self.rules.push((substring.into(), text.into()));
        self
    }

    /// The requests seen so far, in order.
    pub fn calls(&self) -> Vec<LlmRequest> {
        self.calls.borrow().clone()
    }

    /// How many completions have been requested.
    pub fn call_count(&self) -> usize {
        self.calls.borrow().len()
    }
}

impl Default for MockLlmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmBackend for MockLlmBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, LlmError> {
        self.calls.borrow_mut().push(request.clone());

        for (substring, text) in &self.rules {
            if request.prompt.contains(substring) {
                return Ok(LlmResponse {
                    text: text.clone(),
                    model: Some(self.name.clone()),
                });
            }
        }
        if let Some(text) = self.queue.borrow_mut().pop_front() {
            return Ok(LlmResponse {
                text,
                model: Some(self.name.clone()),
            });
        }
        match &self.default {
            Some(text) => Ok(LlmResponse {
                text: text.clone(),
                model: Some(self.name.clone()),
            }),
            None => Err(LlmError::NoResponse),
        }
    }
}
