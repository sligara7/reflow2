//! `built` tools — one slice of the MCP surface.
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

#[tool_router(router = built_router, vis = "pub")]
impl ReflowService {
    #[tool(
        description = "Declare which version of ANOTHER DESIGN this one depends on — the pin a \
                       seam analysis is taken AS OF. Records the source, the version (a tag or \
                       commit), the parts taken, and the build switches forwarded BY NAME, \
                       because a renamed feature is a downstream build break that no API diff or \
                       surface export would mention. This is what you MEAN to depend on; \
                       reconcile_dependencies compares it against what the build actually \
                       resolves. A declaration without a version is refused: the version is the \
                       whole point.",
        annotations(read_only_hint = false)
    )]
    pub async fn declare_dependency(
        &self,
        Parameters(req): Parameters<DeclareDependencyReq>,
    ) -> Result<CallToolResult, McpError> {
        let decl = reflow2_core::DependencyDeclaration {
            id: req.id,
            name: req.name,
            source: req.source,
            version: req.version,
            components: req.components,
            features: req.features,
            declared_in: req.declared_in,
            note: req.note,
        };
        let mut g = self.write_lock().await;
        g.declare_dependency(&decl).map_err(dyno_err)?;
        ok_json(g.dependency_manifest().map_err(dyno_err)?)
    }

    #[tool(
        description = "Check the declared dependencies against what the build ACTUALLY resolves, \
                       and return the reflow2.toml manifest. Catches the two opposite failures: \
                       the build taking something nothing declares (the reliance nobody agreed \
                       to, which breaks with nobody at fault) and a declaration the build no \
                       longer takes (a stale promise). Pass `observed` read fresh from the build \
                       files — Cargo.toml, docker-compose.yml, versions.env, whatever holds the \
                       pins. Declaring nothing reads as 'nobody has said', never as 'depends on \
                       nothing'.",
        annotations(read_only_hint = true)
    )]
    pub async fn reconcile_dependencies(
        &self,
        Parameters(req): Parameters<ReconcileDependenciesReq>,
    ) -> Result<CallToolResult, McpError> {
        let observed: Vec<reflow2_core::ObservedDependency> = req
            .observed
            .into_iter()
            .map(|o| reflow2_core::ObservedDependency {
                name: o.name,
                version: o.version,
                components: o.components,
                features: o.features,
                observed_in: o.observed_in,
            })
            .collect();
        let g = self.graph.read().await;
        let report = g.reconcile_dependencies(&observed).map_err(dyno_err)?;
        let manifest = g.dependency_manifest().map_err(dyno_err)?;
        ok_json(serde_json::json!({ "report": report, "manifest": manifest }))
    }

    #[tool(
        description = "Check the design against what was actually built. You supply what you \
                       observed — for each registered artifact, whether it still exists and its \
                       current content hash — and reflow2 reports the divergences: files that \
                       vanished, files whose content changed since they were registered, and \
                       files present but unknown to the design. reflow2 performs no file I/O; \
                       compute the hashes yourself (any algorithm, used consistently). The \
                       result's `propagation_seeds` are the design nodes the changes land on — \
                       feed them to `propagate_from` to see what a code change means upstream.",
        annotations(read_only_hint = false)
    )]
    pub async fn reconcile_artifacts(
        &self,
        Parameters(req): Parameters<ReconcileArtifactsReq>,
    ) -> Result<CallToolResult, McpError> {
        let observed: Vec<ObservedArtifact> = req
            .observed
            .into_iter()
            .map(|o| serde_json::from_value(JsonValue::Object(o)))
            .collect::<Result<_, _>>()
            .map_err(|e| McpError::invalid_params(format!("invalid observation: {e}"), None))?;
        let opts = ReconcileOptions {
            record_events: req.record_events,
            exhaustive: req.exhaustive,
            detected_at: req.detected_at,
        };
        let mut g = self.write_lock().await;
        ok_json(g.reconcile_artifacts(&observed, &opts).map_err(dyno_err)?)
    }

    #[tool(
        description = "Accept an artifact's current content as the new drift baseline — a \
                       two-sided decision. `disposition` is required: `design_holds` (the change \
                       carries no design meaning; recorded as a dated claim) or `design_updated` \
                       (behaviour moved and the design moved with it; pass \
                       `design_change_event_id` from the record_change that updated it, so code \
                       and design are one change). Silent accept does not exist: it is how a \
                       design erodes into fiction over N fix cycles while reporting zero gaps. \
                       Until you accept, the same checksum_change is reported on every reconcile. \
                       An artifact with NO checksum yet takes neither: pass \
                       `baseline_established`, which records a first baseline as what it is — \
                       nothing moved — instead of a change that never happened.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_artifact_checksum(
        &self,
        Parameters(req): Parameters<SetChecksumReq>,
    ) -> Result<CallToolResult, McpError> {
        let disposition = parse_disposition(
            &req.disposition,
            req.change_type.as_deref(),
            req.design_change_event_id.as_deref(),
        )?;
        let mut g = self.write_lock().await;
        let (artifact, change_event_id) = g
            .set_artifact_checksum(
                &req.artifact_id,
                &req.checksum,
                disposition,
                req.note.as_deref(),
                req.at.as_deref(),
            )
            .map_err(dyno_err)?;
        ok_json(serde_json::json!({
            "artifact": NodeDto::from(artifact),
            "change_event_id": change_event_id,
        }))
    }

    #[tool(
        description = "Accept MANY drift baselines in one call — the bulk form of \
                       set_artifact_checksum, which was 244 consecutive calls across 22 sessions \
                       of recorded usage. EACH ITEM CARRIES ITS OWN DISPOSITION, and that is the \
                       point rather than an inconvenience: a batch under one shared disposition \
                       would be exactly the silent bulk accept that erodes a design into fiction. \
                       The round trip collapses; the judgement stays per artifact. ALL OF IT OR \
                       NONE OF IT — every item is attempted so you learn every failure at once, \
                       and if anything failed no baseline moves.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_artifact_checksums(
        &self,
        Parameters(req): Parameters<SetChecksumsReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut accepts = Vec::with_capacity(req.accepts.len());
        for a in &req.accepts {
            let disposition = parse_disposition(
                &a.disposition,
                a.change_type.as_deref(),
                a.design_change_event_id.as_deref(),
            )?;
            accepts.push(BulkChecksumAccept {
                artifact_id: a.artifact_id.clone(),
                checksum: a.checksum.clone(),
                disposition,
                note: a.note.clone(),
                at: a.at.clone(),
            });
        }
        let mut g = self.write_lock().await;
        let report = g.set_artifact_checksums(&accepts).map_err(dyno_err)?;
        bulk_result(
            report,
            |(artifact, change_event_id)| json!({ "artifact": NodeDto::from(artifact), "change_event_id": change_event_id }),
        )
    }

    // ---- Artifact linking (connect real files to the design) ----

    #[tool(
        description = "Create an Artifact node — a real deliverable (file/spec/doc) that \
                          lives outside the graph, pointed to by `location`.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_artifact(
        &self,
        Parameters(req): Parameters<AddArtifactReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.add_artifact(
                &req.id,
                &req.name,
                req.artifact_type.as_deref(),
                req.location.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Link an Artifact to the Capability/Component it REALIZES (implements).",
        annotations(read_only_hint = false)
    )]
    pub async fn realizes(
        &self,
        Parameters(req): Parameters<RealizesReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.realizes(
                &req.artifact_id,
                &req.target_type,
                &req.target_id,
                req.completeness.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Link an Artifact to the node it DOCUMENTS (describes without \
                       implementing): a design doc, ADR, README, runbook, instruction file \
                       or diagram. Record a file this way when something would be WRONG if it \
                       drifted out of step with the design — not every file. Fails loud if \
                       either endpoint is missing. Distinct from REALIZES (implementation) \
                       and SPECIFIES (machine-readable contract).",
        annotations(read_only_hint = false)
    )]
    pub async fn documents(
        &self,
        Parameters(req): Parameters<DocumentsReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.documents(
                &req.artifact_id,
                &req.target_type,
                &req.target_id,
                req.doc_kind.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Register a real file against the design WITH provenance, atomically: \
                       Artifact + a provenance Fragment (YIELDED) + a REALIZES edge to the \
                       Capability/Component it implements. Fails loud if the target is missing. \
                       Use after building a file so as-designed vs as-built stays honest.",
        annotations(read_only_hint = false)
    )]
    pub async fn link_artifact(
        &self,
        Parameters(req): Parameters<LinkArtifactReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = LinkArtifactOptions {
            artifact_id: req.artifact_id,
            name: req.name,
            location: req.location,
            artifact_type: req.artifact_type,
            target_type: req.target_type,
            target_id: req.target_id,
            completeness: req.completeness,
            provenance: req.provenance,
            fragment_id: req.fragment_id,
            checksum: req.checksum,
        };
        let mut g = self.write_lock().await;
        with_loop_hint(
            g.link_artifact(opts).map_err(dyno_err)?,
            "loop: as-built moved — reconcile_artifacts confirms the design still describes \
             what's on disk; loop_status says what else is owed",
        )
    }
}
