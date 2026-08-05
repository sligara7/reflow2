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

#[tool_router(router = content_tools_router, vis = "pub")]
impl ReflowService {
    #[tool(
        description = "Store bytes in this project's content store and get back the CONTENT HASH \
                       the design will point at (cap:content-store). For what the graph cannot hold \
                       inline: a user's design document, a session transcript, a mermaid diagram, \
                       an HTML mockup, a photograph of a whiteboard. Pass `text` for text or \
                       `base64` for binary — exactly one. STORING IS IDEMPOTENT: the same bytes \
                       hash the same and are kept once, so re-storing is a no-op rather than a \
                       duplicate. WHAT BELONGS HERE, and what does not (dec:what-lives-where): the \
                       content store holds what INFORMED the design; the codebase and anything \
                       shipped with the product stay natively in the repo where they are versioned \
                       as code always has been. Wire the hash into the graph yourself — a Fragment \
                       carrying it in `content_ref`, ANNOTATES-ing what it explains or YIELDED-ing \
                       what was extracted from it — because a stored blob nothing references is \
                       just an orphan.",
        annotations(read_only_hint = false)
    )]
    pub async fn content_put(
        &self,
        Parameters(req): Parameters<ContentPutReq>,
    ) -> Result<CallToolResult, McpError> {
        let bytes: Vec<u8> = match (req.text.clone(), req.base64.clone()) {
            (Some(t), None) => t.into_bytes(),
            (None, Some(b)) => {
                use base64::Engine as _;
                base64::engine::general_purpose::STANDARD
                    .decode(b.as_bytes())
                    .map_err(|e| {
                        McpError::invalid_params(format!("`base64` is not valid base64: {e}"), None)
                    })?
            }
            (Some(_), Some(_)) => {
                return Err(McpError::invalid_params(
                    "pass `text` OR `base64`, not both — two encodings of different bytes would \
                     store one of them and silently drop the other.",
                    None,
                ));
            }
            (None, None) => {
                return Err(McpError::invalid_params(
                    "nothing to store: pass `text` for text content or `base64` for binary.",
                    None,
                ));
            }
        };
        let store = self.content_store()?;
        let already = {
            let hash = reflow2_core::content_hash(&bytes);
            store.exists(&hash).map_err(dyno_err)?
        };
        let hash = store
            .put_allowing_large(&bytes, req.accept_large.unwrap_or(false))
            .map_err(dyno_err)?;
        ok_json(serde_json::json!({
            "hash": hash,
            "bytes": bytes.len(),
            "already_present": already,
            "store": store.root().display().to_string(),
        }))
    }

    #[tool(
        description = "Read content back by its hash, VERIFIED against it (cap:content-store). \
                       Returns `text` when the bytes are valid UTF-8 and `base64` when they are \
                       not, so a diagram and a transcript both come back usable. Bytes that no \
                       longer match the hash they are stored under are REFUSED rather than \
                       returned — a content hash nobody checks is only a filename. A missing blob \
                       fails loud naming the hash, which is the case someone who has the design \
                       but not the bytes will hit (req:content-reaches-every-seat). Finding the \
                       relevant PART of a large document is your job, not reflow2's \
                       (dec:agent-navigates-content) — read it as you would any file.",
        annotations(read_only_hint = true)
    )]
    pub async fn content_get(
        &self,
        Parameters(req): Parameters<ContentRefReq>,
    ) -> Result<CallToolResult, McpError> {
        let bytes = self.content_store()?.get(&req.hash).map_err(dyno_err)?;
        let mut out = serde_json::json!({ "hash": req.hash, "bytes": bytes.len() });
        match String::from_utf8(bytes) {
            Ok(text) => out["text"] = serde_json::Value::String(text),
            Err(e) => {
                use base64::Engine as _;
                out["base64"] = serde_json::Value::String(
                    base64::engine::general_purpose::STANDARD.encode(e.as_bytes()),
                );
            }
        }
        ok_json(out)
    }

    #[tool(
        description = "Whether the bytes for a content hash are present in this project's store \
                       (cap:content-store). Cheap: it does NOT read or verify the content, which \
                       is content_get's job — conflating them would make a presence check secretly \
                       expensive on a large blob. Use it to answer \"do I have everything this \
                       design points at?\" without pulling every diagram into context.",
        annotations(read_only_hint = true)
    )]
    pub async fn content_exists(
        &self,
        Parameters(req): Parameters<ContentRefReq>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.content_store()?;
        let present = store.exists(&req.hash).map_err(dyno_err)?;
        ok_json(serde_json::json!({
            "hash": req.hash,
            "present": present,
            "store": store.root().display().to_string(),
        }))
    }

    #[tool(
        description = "What content this design points at, whether the bytes are here, and what is \
                       here that nothing points at (cap:content-manifest). Answers the question \
                       someone hits after being handed the design ON ITS OWN: `missing` names \
                       content the graph references and this checkout does not have, so a diagram \
                       that will not open is a named finding rather than a silent absence. \
                       `orphaned` is the reverse — bytes stored and referenced by nothing, which is \
                       how a store grows without anyone deciding to. Each entry carries a readable \
                       name (the referencing Fragment's title) and WHAT IT IS FOR, so the manifest \
                       is legible without opening anything. DERIVED, never stored: it is computed \
                       from the graph plus the store, because a manifest kept as its own record \
                       would be a second source of truth and would drift the first time someone \
                       updated one and not the other. Pass `path` to also write the rendered \
                       markdown for committing, which is what makes a blob change readable in `git \
                       log` instead of a hex filename.",
        annotations(read_only_hint = false)
    )]
    pub async fn content_manifest(
        &self,
        Parameters(req): Parameters<ContentManifestReq>,
    ) -> Result<CallToolResult, McpError> {
        let store = self.content_store()?;
        let g = self.graph.read().await;
        let manifest = g.content_manifest(&store).map_err(dyno_err)?;
        let mut out = serde_json::to_value(&manifest).map_err(|e| {
            McpError::internal_error(format!("cannot serialize the manifest: {e}"), None)
        })?;
        if let Some(path) = req.path.as_deref() {
            std::fs::write(path, manifest.render()).map_err(|e| {
                McpError::internal_error(format!("cannot write the manifest to {path}: {e}"), None)
            })?;
            out["written_to"] = serde_json::Value::String(path.to_string());
        }
        ok_json(out)
    }

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
