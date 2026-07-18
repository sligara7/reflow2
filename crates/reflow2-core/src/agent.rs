//! Ambient-agent [`LlmBackend`] adapter (surface-plan.md, SP-2).
//!
//! In the agent-native surface there is no external LLM provider: the **calling
//! coding agent is the model**. But [`LlmBackend::complete`] is synchronous and
//! the agent is the *outer* caller that triggered the op — it cannot be reached
//! mid-op to answer. This module bridges that gap with the **collect-then-serve**
//! handshake (the SP-2 decision), which leans on the core's determinism:
//!
//! 1. **Prepare pass** — run the deterministic op under a [`PromptCollector`],
//!    which records every [`AgentPrompt`] the op issues and returns a stub so the
//!    op runs to completion. The op's *result* in this pass is discarded; only
//!    the collected prompts matter. The surface hands those (prompt + JSON hint)
//!    to the agent.
//! 2. The agent fills each prompt in-context and returns [`AgentAnswer`]s.
//! 3. **Serve pass** — replay the *same* op under an [`AgentBackend`] built from
//!    those answers. Because the core is deterministic, the op issues the
//!    identical prompt sequence, so each `complete` finds its answer by id.
//!
//! [`LlmBackend`] itself is untouched — still sync and object-safe. No provider,
//! no external API, no bill (IS-6).
//!
//! ## Scope / limits
//!
//! Collect-then-serve assumes an op's prompt sequence does not depend on the
//! *content* of earlier answers — true for every op wired today (e.g.
//! [`GapCandidate::to_prompt`](crate::detect::GapCandidate::to_prompt), one call
//! per gap). An op whose prompt N+1 branches on answer N needs the round
//! repeated: prepare → answer → prepare again picks up the newly-reachable
//! prompts, until a prepare pass collects nothing. The surface (SP-3) drives that
//! loop; these primitives support it unchanged.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::detect::fnv1a;
use crate::llm::{LlmBackend, LlmError, LlmRequest, LlmResponse};

/// Stable id for a request's *semantic* content (system framing + prompt + the
/// JSON hint), as lowercase hex of an FNV-1a hash — the same deterministic-id
/// discipline used for gap/heal issue ids. Identical prompts share an id, so the
/// prepare and serve passes agree and an answer is reusable/cacheable. Decoding
/// params ([`LlmParams`](crate::llm::LlmParams)) are deliberately excluded: they
/// don't change *what* is being asked, and the ambient agent does not tune them.
pub fn prompt_id(request: &LlmRequest) -> String {
    // Unit-separator between fields so distinct (system, prompt) pairs can't
    // collide by concatenation.
    let key = format!(
        "{}\u{1f}{}\u{1f}{}",
        request.system.as_deref().unwrap_or(""),
        request.prompt,
        request.expect_json
    );
    format!("{:016x}", fnv1a(&key))
}

/// One thing the ambient agent must answer — the payload the prepare pass hands
/// out. Serializable because it crosses the tool boundary as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPrompt {
    /// Stable id ([`prompt_id`]); the agent echoes it back in its [`AgentAnswer`].
    pub id: String,
    /// System/role framing, if the op set one.
    pub system: Option<String>,
    /// The prompt text for the agent to answer.
    pub prompt: String,
    /// Whether the op will parse the answer as JSON (a schema hint for the agent).
    pub expect_json: bool,
}

/// The ambient agent's filled reply, matched back to its prompt by [`id`](Self::id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAnswer {
    /// The [`AgentPrompt::id`] this answers.
    pub id: String,
    /// The answer text (a JSON string when the prompt's `expect_json` was set).
    pub text: String,
}

/// Prepare-pass backend: records every request as an [`AgentPrompt`] and returns
/// a stub so the op runs to completion. Requests are deduplicated by id (an op
/// that issues the same prompt twice yields one [`AgentPrompt`]) and kept in
/// first-seen order. The op's result under this backend is not meaningful — only
/// [`collected`](Self::collected) is.
pub struct PromptCollector {
    stub: String,
    seen: RefCell<Vec<AgentPrompt>>,
    ids: RefCell<HashSet<String>>,
}

impl PromptCollector {
    /// A collector whose stub response is the empty string.
    ///
    /// The stub only exists to let the op finish so its later prompts are
    /// reached; its content is discarded. For an op that parses each answer as
    /// JSON *and* branches on it, override with [`with_stub`](Self::with_stub) —
    /// though a content-dependent chain is the repeated-round case (see the
    /// module docs), not a single prepare pass.
    pub fn new() -> Self {
        Self {
            stub: String::new(),
            seen: RefCell::new(Vec::new()),
            ids: RefCell::new(HashSet::new()),
        }
    }

    /// Set the stub response returned to the op during the prepare pass.
    #[must_use]
    pub fn with_stub(mut self, stub: impl Into<String>) -> Self {
        self.stub = stub.into();
        self
    }

    /// The prompts the op issued, in first-seen order, deduplicated by id.
    pub fn collected(&self) -> Vec<AgentPrompt> {
        self.seen.borrow().clone()
    }
}

impl Default for PromptCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmBackend for PromptCollector {
    fn name(&self) -> &str {
        "agent-collector"
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, LlmError> {
        let id = prompt_id(request);
        if self.ids.borrow_mut().insert(id.clone()) {
            self.seen.borrow_mut().push(AgentPrompt {
                id,
                system: request.system.clone(),
                prompt: request.prompt.clone(),
                expect_json: request.expect_json,
            });
        }
        Ok(LlmResponse {
            text: self.stub.clone(),
            model: Some("agent-collector".to_string()),
        })
    }
}

/// Serve-pass backend: answers each request from the agent-supplied set, matched
/// by [`prompt_id`]. Fail-loud — a request with no matching answer is a real
/// prepare/serve desync and is returned as [`LlmError::Backend`], never a silent
/// fallback (AGENTS.md rule 4). Tracks which answers were consumed so callers can
/// surface stale or unused ones ([`unused_answers`](Self::unused_answers)) rather
/// than dropping them silently.
pub struct AgentBackend {
    answers: HashMap<String, String>,
    used: RefCell<HashSet<String>>,
}

impl AgentBackend {
    /// Build from the agent's answers. Duplicate ids collapse (last wins); an
    /// answer whose id no op requests is reported by [`unused_answers`](Self::unused_answers).
    pub fn from_answers(answers: impl IntoIterator<Item = AgentAnswer>) -> Self {
        Self {
            answers: answers.into_iter().map(|a| (a.id, a.text)).collect(),
            used: RefCell::new(HashSet::new()),
        }
    }

    /// Ids supplied but never requested by the op — stale/leftover answers,
    /// sorted for a deterministic report. Empty means every answer was used.
    pub fn unused_answers(&self) -> Vec<String> {
        let used = self.used.borrow();
        let mut unused: Vec<String> = self
            .answers
            .keys()
            .filter(|id| !used.contains(*id))
            .cloned()
            .collect();
        unused.sort();
        unused
    }
}

impl LlmBackend for AgentBackend {
    fn name(&self) -> &str {
        "agent"
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, LlmError> {
        let id = prompt_id(request);
        match self.answers.get(&id) {
            Some(text) => {
                self.used.borrow_mut().insert(id);
                Ok(LlmResponse {
                    text: text.clone(),
                    model: Some("agent".to_string()),
                })
            }
            None => Err(LlmError::Backend(format!(
                "no ambient-agent answer for prompt id {id}: prepare/serve desync \
                 (the op issued a prompt the agent was not asked to fill)"
            ))),
        }
    }
}
