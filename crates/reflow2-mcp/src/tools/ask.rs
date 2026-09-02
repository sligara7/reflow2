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
                       refused rather than half-served. The questions are recorded all or none. \
                       THE REPLAYED GAP IS AN ECHO: only its `id` is read. The server resolves \
                       each gap afresh and ignores the text you send back, so trimming a \
                       description for readability or mangling a title CANNOT re-key your \
                       answers \u{2014} it used to, and two projects paid for it ten days apart \
                       before the guard was fixed on 2026-09-02. Send the row back as you got it \
                       and do not spend effort preserving it byte-for-byte. \
                       \u{26a0} A gap that has CLOSED since you took it is REFUSED rather than \
                       served from your stale copy, because recording a question against a \
                       finding nobody has is the worse outcome. Unmatched answers still come \
                       back in `unused_answers`, and `degrade_reason` says what went wrong.",
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
        let gaps = self.rehydrate_gaps(gaps).await?;

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
        let mut unused_per_gap = Vec::with_capacity(gaps.len());
        for (gap, supplied) in gaps.iter().zip(req.gaps.iter()) {
            let answers = supplied.answers.iter().map(|a| AgentAnswer {
                id: a.id.clone(),
                text: a.text.clone(),
            });
            let backend = AgentBackend::from_answers(answers);
            prompts.push(gap.to_prompt(&backend));
            // Per gap, because each gap is replayed against its own answers —
            // see `unused_answers` in the singular form for why this is the
            // cheapest desync signal available.
            unused_per_gap.push(backend.unused_answers());
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

        let mut g = self.write_lock().await?;
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
            .zip(unused_per_gap.iter())
            .map(|(((gap, prompt), question_id), unused)| {
                let mut row =
                    json!({ "gap_id": gap.id, "prompt": prompt, "question_id": question_id });
                if !unused.is_empty() {
                    row["unused_answers"] = json!(unused);
                }
                row
            })
            .collect();
        let stranded: usize = unused_per_gap.iter().map(Vec::len).sum();
        let mut reply = json!({ "status": "ok", "gaps": items });
        if stranded > 0 {
            reply["unused_answers_note"] = json!(format!(
                "{stranded} answer(s) across this batch were never requested. An answer is \
                 matched by an id hashed from the prompt text, and that text is built from each \
                 gap's own title and description — so an edited gap re-keys every answer it \
                 carries. Replay the gap objects from the prepare pass unchanged."
            ));
        }
        ok_json(reply)
    }

    #[tool(
        description = "Phrase a gap as a plain question via the ambient agent. \
                       Call with empty `answers` to get {status:needs_llm, prompts}; \
                       fill them and call again with `answers` to get {status:ok, prompt}. \
                       THE ANSWERS YOU FILL IN ARE WHERE THE TRANSLATION HAPPENS: the gap \
                       arrives in the detector's vocabulary (`unallocated_capability`, \
                       `unsatisfied_requirement`) and the question a person reads must be in \
                       THEIRS — what is actually missing and why it matters to their design. \
                       Read who you are talking to (their `Contributor` description) and match \
                       that domain; a `plain` question is not automatically one in their \
                       vocabulary, and swapping vocabulary is not simplifying — a systems \
                       engineer wants `interface` and `verification` kept. \
                       THE REPLAYED GAP IS AN ECHO: only its `id` is read, and the server \
                       resolves the gap afresh. Trimming the description or mangling the title \
                       CANNOT re-key your answers \u{2014} it used to, and that is what silently \
                       served raw detector jargon to two projects' users before the guard was \
                       fixed on 2026-09-02. \
                       \u{26a0} A gap that has CLOSED since you took it is REFUSED rather than \
                       served from your stale copy. Unmatched answers come back in \
                       `unused_answers`, and `degrade_reason` says what went wrong.",
        annotations(read_only_hint = false)
    )]
    pub async fn gap_to_prompt(
        &self,
        Parameters(req): Parameters<GapToPromptReq>,
    ) -> Result<CallToolResult, McpError> {
        let gap: GapCandidate = parse_struct_param(req.gap, "GapCandidate")?;
        let gap = self.rehydrate_gap(gap).await?;

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
        // AN ANSWER NOBODY ASKED FOR IS THE CHEAPEST DESYNC SIGNAL THERE IS,
        // and it was computed and thrown away. `unused_answers` is already
        // documented on the backend as the way to surface stale answers
        // "rather than dropping them silently" — it just had no caller. When a
        // replayed gap has been edited between the passes, EVERY answer lands
        // here, which is the one-read diagnosis dev_storyflow had to reach by
        // diffing two calls by hand on 2026-08-23.
        let unused = backend.unused_answers();

        // Record that this was asked, and in what words. Until BL-4 this tool
        // was the only one that never touched the graph: it phrased a question,
        // returned it, and forgot — so the next session re-derived the same gap
        // and asked again. Persisting here rather than in a separate call means
        // the record cannot be forgotten by an agent that does not know to make
        // it.
        let mut g = self.write_lock().await?;
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

        let mut reply = json!({
            "status": "ok", "prompt": prompt, "question_id": question_id,
        });
        if !unused.is_empty() {
            reply["unused_answers"] = json!(unused);
            reply["unused_answers_note"] = json!(format!(
                "{} answer(s) you supplied were never requested by this gap. An answer is \
                 matched by an id hashed from the prompt text, and that text is built from the \
                 gap's own title and description — so an edited gap re-keys every answer. \
                 Replay the gap object from the prepare pass unchanged.",
                unused.len()
            ));
        }
        ok_json(reply)
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
        let mut g = self.write_lock().await?;
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
        let mut g = self.write_lock().await?;
        let found = g.withdraw_question(&req.gap_id).map_err(dyno_err)?;
        ok_json(json!({ "withdrawn": found, "gap_id": req.gap_id }))
    }
}

impl ReflowService {
    /// Fill a COMPACTED gap back in from the graph before it is used.
    ///
    /// `detect_gaps` withholds `description`, `evidence` and `affected_ids`
    /// when the full reply would not fit (`ReplyDetail::TitlesOnly`), and an
    /// agent hands the row it was given straight back here. Used as-is, that
    /// row would phrase the question from a blank description and — worse —
    /// record it against an EMPTY anchor set, so `open_questions` would carry a
    /// question attached to nothing. The compaction is a fact about the reply,
    /// never about the gap, so it is undone here rather than pushed onto the
    /// caller.
    ///
    /// Only a row that is compacted is looked up: a real candidate always
    /// carries both prose fields, so `description` and `evidence` both empty is
    /// the discriminator, and the ordinary path pays nothing.
    ///
    /// A compacted row whose gap is no longer detected is REFUSED rather than
    /// asked about. It means the gap closed between the two calls, and phrasing
    /// a question about a gap that is gone — from a row with no content in it —
    /// is not something to do quietly.
    pub(crate) async fn rehydrate_gap(&self, gap: GapCandidate) -> Result<GapCandidate, McpError> {
        Ok(self.rehydrate_gaps(vec![gap]).await?.remove(0))
    }

    /// [`rehydrate_gap`](Self::rehydrate_gap) for a batch, detecting ONCE.
    ///
    /// `gaps_to_prompts` takes a whole ask at a time, and rehydrating each row
    /// on its own would re-run the detectors once per gap — the same answer,
    /// recomputed, for every row of one batch.
    pub(crate) async fn rehydrate_gaps(
        &self,
        gaps: Vec<GapCandidate>,
    ) -> Result<Vec<GapCandidate>, McpError> {
        // THE REPLAYED GAP IS AN ECHO, NOT AN INPUT
        // (dec:idea-is-a-replayed-gap-an-input-or-an-echo, 2026-09-02).
        //
        // ⭐ ONLY THE `id` IS READ. Everything else the caller sends back is
        // discarded in favour of the server's own copy, so mutilating the
        // payload between the passes CANNOT re-key the answers.
        //
        // 🛑 THIS GUARD USED TO RUN ONLY WHEN `description` AND `evidence` WERE
        // BOTH EMPTY — that is, only for a row from a BUDGETED reply — and so
        // trusted the caller's text in exactly the case where it can be wrong.
        // Two projects paid for that ten days apart, neither carelessly:
        // dev_storyflow TRIMMED the description for readability (2026-08-23, 4
        // of 5 questions silently degraded to raw detector jargon), and
        // proj:chama duplicated a fragment of the title while transcribing
        // (2026-09-02). The tool's own prose already said "REPLAY EACH GAP
        // OBJECT UNCHANGED" — the code just did not enforce it.
        //
        // COST: one `detect_gaps()` per call rather than per gap, which is why
        // the batch form exists. A closed gap is now REFUSED rather than served
        // from a stale copy; recording a question against a finding nobody has
        // is the worse outcome, and that trade is the settled part.
        if gaps.is_empty() {
            return Ok(gaps);
        }
        let open = {
            let g = self.graph.read().await;
            g.detect_gaps().map_err(dyno_err)?
        };
        gaps.into_iter()
            .map(|gap| {
                open.iter()
                    .find(|candidate| candidate.id == gap.id)
                    .cloned()
                    .ok_or_else(|| Self::gap_is_gone(&gap.id))
            })
            .collect()
    }

    fn gap_is_gone(gap_id: &str) -> McpError {
        {
            McpError::invalid_params(
                format!(
                    "no open gap has the id {gap_id}. The gap you replayed is resolved by ID \
                     against a fresh detect_gaps — the text you send back is an echo and is not \
                     read — so this means the gap itself was CLOSED or ACKNOWLEDGED since you \
                     took it, not that your payload was wrong. Re-run detect_gaps and ask about \
                     what is still open."
                ),
                None,
            )
        }
    }
}
