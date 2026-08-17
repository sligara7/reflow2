//! `ask` tools — one slice of the MCP surface.
//!
//! Split out of `service.rs` under BL-181, which had grown to 6,356 lines and
//! 139 tools in one file: the design distinguished the systems these tools
//! serve and the build did not separate them at all. That mismatch is what
//! `granularity_report` reported, and this is the answer to it.
//!
//! **Function is unchanged by construction.** Every item here moved verbatim;
//! nothing was rewritten. `rmcp` composes routers, so this module declares its
//! own and `ReflowService::new` sums them — the surface a client sees is
//! byte-identical, which `tools/toolsnap.py` is what proves rather than claims.

#![allow(unused_imports)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities,
        ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use tokio::sync::RwLock;

use reflow2_core::bulk::{
    AskedRecord as BulkAskedRecord, ChecksumAccept as BulkChecksumAccept, EdgeSpec as BulkEdgeSpec,
    GapAck as BulkGapAck, NodeSpec as BulkNodeSpec,
};
use reflow2_core::temporal::ChangeRecord;
use reflow2_core::{
    AgentAnswer, AgentBackend, AskedQuestion, ChangeType, DEFAULT_SCOPE_DEPTH, DesignGraph,
    Dimension, DriftDisposition, DynoError, EpochType, GapCandidate, GenesisOptions, HealOptions,
    HealProposal, HealStrategy, IngestOptions, LinkArtifactOptions, LoopStatus, ObservedArtifact,
    ObservedPath, PromptCollector, PropagateOptions, ReadinessForecast, ReadinessGate,
    ReadinessKind, ReadinessObservation, ReconcileOptions, StoredNode, Value,
};

use crate::dto::{EdgeDto, NodeDto};
use crate::service::*;

#[tool_router(router = ask_router, vis = "pub")]
impl ReflowService {
    // ---- LLM handshake (SP-2 collect-then-serve) ----

    #[tool(
        description = "Phrase MANY gaps as plain questions in one handshake — the bulk form of \
                       gap_to_prompt, and the read half of the detect→ask→acknowledge round \
                       trip. Same two passes: call with every `answers` empty to get \
                       {status:needs_llm, gaps:[{gap_id, prompts}]}, fill them in and call again \
                       to get one prompt per gap. ANSWERS ARE GROUPED PER GAP, so prompt ids \
                       cannot collide across gaps — each gap is replayed against its own answers \
                       and never sees another's. A MIXED call (some gaps answered, some not) is \
                       refused rather than half-served. The questions are recorded all or none.",
        annotations(read_only_hint = false)
    )]
    pub async fn gaps_to_prompts(
        &self,
        Parameters(req): Parameters<GapsToPromptsReq>,
    ) -> Result<CallToolResult, McpError> {
        if req.gaps.is_empty() {
            return Err(McpError::invalid_params(
                "no gaps were passed — an empty ask is a mistake, not a no-op",
                None,
            ));
        }
        let mut gaps = Vec::with_capacity(req.gaps.len());
        for g in &req.gaps {
            gaps.push(parse_struct_param::<GapCandidate>(
                g.gap.clone(),
                "GapCandidate",
            )?);
        }

        let answered = req.gaps.iter().filter(|g| !g.answers.is_empty()).count();
        if answered != 0 && answered != req.gaps.len() {
            return Err(McpError::invalid_params(
                format!(
                    "{answered} of {} gaps carry answers. A batch is either the prepare pass \
                     (every `answers` empty) or the serve pass (every gap answered) — serving \
                     half of them would record some questions and silently drop the rest",
                    req.gaps.len()
                ),
                None,
            ));
        }

        // Prepare pass: harvest each gap's prompts, grouped by gap.
        if answered == 0 {
            let collected: Vec<JsonValue> = gaps
                .iter()
                .map(|gap| {
                    let collector = PromptCollector::new();
                    let _discarded = gap.to_prompt(&collector);
                    json!({ "gap_id": gap.id, "prompts": collector.collected() })
                })
                .collect();
            return ok_json(json!({ "status": "needs_llm", "gaps": collected }));
        }

        // Serve pass. Each gap gets a backend built from ITS OWN answers.
        let mut prompts = Vec::with_capacity(gaps.len());
        for (gap, supplied) in gaps.iter().zip(req.gaps.iter()) {
            let answers = supplied.answers.iter().map(|a| AgentAnswer {
                id: a.id.clone(),
                text: a.text.clone(),
            });
            let backend = AgentBackend::from_answers(answers);
            prompts.push(gap.to_prompt(&backend));
        }

        // Record all of them or none — the same bar the other bulk forms hold.
        let records: Vec<BulkAskedRecord> = gaps
            .iter()
            .zip(prompts.iter())
            .map(|(gap, prompt)| BulkAskedRecord {
                gap_id: gap.id.clone(),
                affected_ids: gap.affected_ids.clone(),
                question: prompt.question.clone(),
                context_setter: Some(prompt.context_setter.clone()),
                rephrase_degraded: prompt.rephrase_degraded,
            })
            .collect();

        let mut g = self.write_lock().await;
        let recorded = g
            .record_asked_questions(&records, req.asked_at.as_deref())
            .map_err(dyno_err)?;
        if !recorded.applied {
            return bulk_result(recorded, |q| q);
        }

        let items: Vec<JsonValue> = gaps
            .iter()
            .zip(prompts.iter())
            .zip(recorded.written.iter())
            .map(|((gap, prompt), question_id)| {
                json!({ "gap_id": gap.id, "prompt": prompt, "question_id": question_id })
            })
            .collect();
        ok_json(json!({ "status": "ok", "gaps": items }))
    }

    #[tool(
        description = "Phrase a gap as a plain question via the ambient agent. \
                       Call with empty `answers` to get {status:needs_llm, prompts}; \
                       fill them and call again with `answers` to get {status:ok, prompt}.",
        annotations(read_only_hint = false)
    )]
    pub async fn gap_to_prompt(
        &self,
        Parameters(req): Parameters<GapToPromptReq>,
    ) -> Result<CallToolResult, McpError> {
        let gap: GapCandidate = parse_struct_param(req.gap, "GapCandidate")?;

        if req.answers.is_empty() {
            // Prepare pass: harvest the prompt the op would issue.
            let collector = PromptCollector::new();
            let _discarded = gap.to_prompt(&collector);
            return ok_json(json!({
                "status": "needs_llm",
                "prompts": collector.collected(),
            }));
        }

        // Serve pass: replay the op with the agent's answers.
        let answers = req.answers.into_iter().map(|a| AgentAnswer {
            id: a.id,
            text: a.text,
        });
        let backend = AgentBackend::from_answers(answers);
        let prompt = gap.to_prompt(&backend);

        // Record that this was asked, and in what words. Until BL-4 this tool
        // was the only one that never touched the graph: it phrased a question,
        // returned it, and forgot — so the next session re-derived the same gap
        // and asked again. Persisting here rather than in a separate call means
        // the record cannot be forgotten by an agent that does not know to make
        // it.
        let mut g = self.write_lock().await;
        let question_id = g
            .record_asked_question(
                &gap.id,
                &gap.affected_ids,
                &prompt.question,
                AskedQuestion {
                    prompt_id: None,
                    context_setter: Some(&prompt.context_setter),
                    asked_at: req.asked_at.as_deref(),
                    rephrase_degraded: prompt.rephrase_degraded,
                },
            )
            .map_err(dyno_err)?;

        ok_json(json!({ "status": "ok", "prompt": prompt, "question_id": question_id }))
    }

    #[tool(
        description = "Questions already put to the user that still bear on something open, with the wording they saw. `status: asked` means they have not replied \u{2014} follow it up, do not ask again. `status: answered` means they replied but the gap is still open, so their answer needs writing into the design or the gap needs acknowledging; their reply comes back with it. Read this at the start of a session, before detect_gaps. AN EMPTY ANSWER IS NEVER BARE: because this is the orientation call a session runs FIRST, a zero here was being read as an all-clear while the loop was owed dozens of other things, so `count: 0` always arrives with a `loop_hint` saying WHICH it is \u{2014} the other non-zero debt named, or an explicit statement that nothing else is owed either. No questions open is not the same fact as nothing to do.",
        annotations(read_only_hint = true)
    )]
    pub async fn open_questions(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let records = g.open_questions().map_err(dyno_err)?;
        // An empty answer here is the one that gets read as permission, so it
        // is the one that must say what it is (BL-91's hint, un-throttled).
        let empty = records.is_empty();
        self.ok_read_empty_speaks(&g, records, empty)
    }

    #[tool(
        description = "Record what the user said in reply to a question, closing it. Write the \
                       design nodes their answer implies separately — this is the record that \
                       it was settled, not a substitute for the design. Takes EITHER `gap_id` \
                       or `question_id`; `open_questions` publishes both and either is accepted, \
                       because a Question this graph did not derive from a gap is reachable only \
                       by its own id. Answering one that was never asked is refused, not \
                       silently accepted — distinct from the withdraw_* tools, which no-op on an \
                       absent record — and the refusal names the ids that DO exist.",
        annotations(read_only_hint = false)
    )]
    pub async fn answer_question(
        &self,
        Parameters(req): Parameters<AnswerQuestionReq>,
    ) -> Result<CallToolResult, McpError> {
        let id = match (req.question_id.as_deref(), req.gap_id.as_deref()) {
            (Some(q), _) => q,
            (None, Some(g)) => g,
            (None, None) => {
                return Err(McpError::invalid_params(
                    "answer_question needs `gap_id` or `question_id` — open_questions returns \
                     both on every item; pass either."
                        .to_string(),
                    None,
                ));
            }
        };
        let mut g = self.write_lock().await;
        let found = g.answer_question(id, &req.answer).map_err(dyno_err)?;
        if !found {
            // Naming what EXISTS is the whole point: the old message said only
            // that the lookup missed, which cost a round trip the caller had no
            // way to shortcut — and both ids they held came from open_questions.
            let known = g.known_question_ids().map_err(dyno_err)?;
            return Err(McpError::invalid_params(
                if known.is_empty() {
                    format!("no recorded question for {id}; this design has no questions at all")
                } else {
                    format!(
                        "no recorded question for {id}; known question id(s): {}",
                        known.join(", ")
                    )
                },
                None,
            ));
        }
        ok_json(json!({ "answered": true, "question": id }))
    }

    #[tool(
        description = "Withdraw a question asked in error or overtaken by events. Kept in the                        graph, not deleted.",
        annotations(read_only_hint = false)
    )]
    pub async fn withdraw_question(
        &self,
        Parameters(req): Parameters<WithdrawQuestionReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let found = g.withdraw_question(&req.gap_id).map_err(dyno_err)?;
        ok_json(json!({ "withdrawn": found, "gap_id": req.gap_id }))
    }
}
