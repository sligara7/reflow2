//! `content_tools` tools — one slice of the MCP surface.
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
    AgentAnswer, AgentBackend, AskedQuestion, ChangeType, CorpusDocument, CorpusOptions,
    DEFAULT_SCOPE_DEPTH, DesignGraph, Dimension, DriftDisposition, DynoError, EpochType,
    GapCandidate, GenesisOptions, HealOptions, HealProposal, HealStrategy, IngestOptions,
    LinkArtifactOptions, LoopStatus, ObservedArtifact, ObservedPath, PromptCollector,
    PropagateOptions, ReadinessForecast, ReadinessGate, ReadinessKind, ReadinessObservation,
    ReconcileOptions, StoredNode, Value,
};

use crate::dto::{EdgeDto, NodeDto};
use crate::service::*;

#[tool_router(router = ingest_tools_router, vis = "pub")]
impl ReflowService {
    #[tool(
        description = "Extract a design from freeform text, with YOU as the model — no LLM \
                       provider is involved. Call it with no `answers`; it replies \
                       `status: needs_llm` and a list of prompts; answer each in context and call \
                       again with the SAME input and fragment_id plus EVERY answer so far, \
                       including earlier rounds. Repeat until `status: done`. Usually three \
                       rounds, because later passes are gated on the discovery classifier and \
                       threaded with the ids the earlier ones produced, so they cannot be asked \
                       up front. NOTHING IS WRITTEN until the final round: the earlier ones \
                       replay against a throwaway graph, so an abandoned handshake leaves no \
                       half-design behind. There is no server-side session — each call is \
                       self-contained, so it survives a restart and works across seats sharing \
                       one server. Prefer this over calling add_* yourself for anything \
                       document-shaped: it is what gives you provenance Fragments back to the \
                       source, snapshot-before-overwrite when a re-ingest changes something, the \
                       resolution bands that ask instead of guessing, and the structural pass \
                       that catches `Auth` versus `Authentication Service`.",
        annotations(read_only_hint = false)
    )]
    pub async fn ingest_step(
        &self,
        Parameters(req): Parameters<IngestStepReq>,
    ) -> Result<CallToolResult, McpError> {
        let answers: Vec<AgentAnswer> = req
            .answers
            .into_iter()
            .map(|a| serde_json::from_value(JsonValue::Object(a)))
            .collect::<Result<_, _>>()
            .map_err(|e| McpError::invalid_params(format!("invalid answer: {e}"), None))?;
        let options = IngestOptions {
            fragment_id: req.fragment_id.clone(),
            fragment_title: req
                .fragment_title
                .unwrap_or_else(|| req.fragment_id.clone()),
            provenance: req.provenance.unwrap_or_else(|| "authored".to_string()),
            epoch_id: req.epoch_id,
            ..IngestOptions::default()
        };
        let mut g = self.write_lock().await;
        ok_json(
            g.ingest_step(&req.input, &options, answers)
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "Turn a WHOLE FOLDER of documents into one design, with you as the model \
                       (cap:corpus-ingest). The corpus sibling of ingest_step, and the reason to \
                       prefer it over looping that one: it gathers the prompts for EVERY document \
                       into one round, so a hundred documents cost the same ~3 rounds a single \
                       document does instead of three hundred. YOU walk the directory and read the \
                       files — reflow2 performs no file I/O — and hand over `text` per document \
                       plus an opaque `source` locator it stores and never parses. Drive it like \
                       ingest_step: call with no `answers`, answer everything it returns, call \
                       again with the SAME documents plus every answer so far, until \
                       `status: done`. NOTHING IS WRITTEN until that final round, so an abandoned \
                       corpus leaves no half-design behind. WHAT IT DOES THAT A LOOP CANNOT: one \
                       epoch for the whole run rather than one per file; cross-document identity, \
                       so the same component named in forty specs converges on ONE node; and the \
                       ambiguous near-match band gathered across the corpus and DEDUPLICATED into \
                       one question instead of the same question forty times (dec:ask-not-repair). \
                       RE-RUNNING IS SAFE AND IS THE RESUME PATH: a document whose fragment_id \
                       already exists comes back `skipped`, not `failed`, so pointing it at a \
                       grown folder ingests only what is new. Read `failures` before you trust the \
                       result — it names every document that could not be taken, because a run \
                       that cannot say what it did not understand is worse than no run.",
        annotations(read_only_hint = false)
    )]
    pub async fn ingest_corpus_step(
        &self,
        Parameters(req): Parameters<IngestCorpusStepReq>,
    ) -> Result<CallToolResult, McpError> {
        let answers: Vec<AgentAnswer> = req
            .answers
            .into_iter()
            .map(|a| serde_json::from_value(JsonValue::Object(a)))
            .collect::<Result<_, _>>()
            .map_err(|e| McpError::invalid_params(format!("invalid answer: {e}"), None))?;
        let documents: Vec<CorpusDocument> = req
            .documents
            .into_iter()
            .map(|d| CorpusDocument {
                fragment_id: d.fragment_id,
                title: d.title,
                text: d.text,
                source: d.source,
            })
            .collect();
        if documents.is_empty() {
            return Err(McpError::invalid_params(
                "ingest_corpus_step needs at least one document — an empty corpus would report a \
                 clean run over nothing, which is the false-green this tool exists to avoid"
                    .to_string(),
                None,
            ));
        }
        let options = CorpusOptions {
            epoch_id: req
                .epoch_id
                .unwrap_or_else(|| "epoch:corpus-ingest".to_string()),
            provenance: req.provenance.unwrap_or_else(|| "imported".to_string()),
            ..CorpusOptions::default()
        };
        let mut g = self.write_lock().await;
        ok_json(
            g.ingest_corpus_step(&documents, &options, answers)
                .map_err(dyno_err)?,
        )
    }
}
