//! `operate_tools` tools — one slice of the MCP surface.
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

#[tool_router(router = operate_tools_router, vis = "pub")]
impl ReflowService {
    #[tool(
        description = "Record a Release — a packaged, operable version: a container image, a \
                       published package, a manufactured build. Part of answering the \
                       `no_deploy_operate` gap. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing \u{2014} omitted \
                       fields keep their stored value, so correcting one never means re-sending \
                       a 2 KB field you did not touch.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_release(
        &self,
        Parameters(req): Parameters<ReleaseReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let mut __rf =
            crate::service::RequiredFields::new(&g, reflow2_core::nodes::node::RELEASE, &req.id)?;
        let name = __rf.str("name", req.name);
        ok_json(NodeDto::from(
            g.add_release(
                &req.id,
                &name,
                req.version.as_deref(),
                req.unit_type.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record an Environment — where a Release runs: a cloud region, a lab bench, \
                       a physical site. More than a deploy target; it is the context whose rules \
                       the design must satisfy. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing \u{2014} omitted \
                       fields keep their stored value, so correcting one never means re-sending \
                       a 2 KB field you did not touch.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_environment(
        &self,
        Parameters(req): Parameters<EnvironmentReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let mut __rf = crate::service::RequiredFields::new(
            &g,
            reflow2_core::nodes::node::ENVIRONMENT,
            &req.id,
        )?;
        let name = __rf.str("name", req.name);
        ok_json(NodeDto::from(
            g.add_environment(
                &req.id,
                &name,
                req.env_type.as_deref(),
                req.location.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record a Resource the built thing needs — a database, a queue, a secret, a \
                       GPU, power, bandwidth. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing \u{2014} omitted \
                       fields keep their stored value, so correcting one never means re-sending \
                       a 2 KB field you did not touch.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_resource(
        &self,
        Parameters(req): Parameters<ResourceReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let mut __rf =
            crate::service::RequiredFields::new(&g, reflow2_core::nodes::node::RESOURCE, &req.id)?;
        let name = __rf.str("name", req.name);
        ok_json(NodeDto::from(
            g.add_resource(&req.id, &name, req.provider.as_deref())
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Deploy a Release to an Environment (planned/active/rolled_back).",
        annotations(read_only_hint = false)
    )]
    pub async fn deploy_to(
        &self,
        Parameters(req): Parameters<DeployToReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.deploy_to(&req.release_id, &req.environment_id, req.status.as_deref())
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record that a Release ships an Artifact or Component (INCLUDES) — the \
                       as-released view. Pass as_checksum to freeze the artifact's content hash \
                       as shipped: the artifact node's own checksum is the live drift baseline \
                       and moves with every accept, so without the frozen copy a past release's \
                       manifest would quietly rewrite itself. A Release with no INCLUDES edges \
                       is a version number, not a manifest.",
        annotations(read_only_hint = false)
    )]
    pub async fn release_includes(
        &self,
        Parameters(req): Parameters<ReleaseIncludesReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.release_includes(
                &req.release_id,
                &req.target_type,
                &req.target_id,
                req.as_checksum.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Derive a Release's whole INCLUDES manifest from the design in one call: \
                       every Artifact and every Component, with each artifact's current checksum \
                       frozen as shipped. This is the bulk form of release_includes, which was \
                       the single largest line item in reflow2's recorded usage — about 144 \
                       consecutive calls per release cut, all typing out something the graph \
                       already knew. Nothing is written unless apply is true, so read the \
                       manifest before you package it. Re-running is safe: an entry already in \
                       the manifest is reported as already_present and its frozen checksum is \
                       never rewritten, because what a past release shipped must not move with \
                       the live drift baseline. without_checksum names the artifacts whose entry \
                       cannot say WHAT shipped.",
        annotations(read_only_hint = false)
    )]
    pub async fn release_includes_all(
        &self,
        Parameters(req): Parameters<ReleaseIncludesAllReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(
            g.release_includes_all(&req.release_id, &req.exclude, req.apply)
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "The as-released view (BL-34): what a Release actually shipped — artifacts \
                       with their frozen cut-time checksums, components, the capabilities that \
                       build covers, the built capabilities it leaves out, and where it is \
                       deployed. This is the query 'does what we released match what we \
                       designed?' — compare capabilities_covered against the design's \
                       capability list, and built_capabilities_not_covered is the diff.",
        annotations(read_only_hint = true)
    )]
    pub async fn release_report(
        &self,
        Parameters(req): Parameters<ReleaseReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.release_report(&req.release_id).map_err(dyno_err)?)
    }

    #[tool(
        description = "Record an OBSERVED technology-readiness level (TRL or MRL, 1-9) for an \
                       enabling technology — the input fact a derived roadmap is computed from \
                       (BL-68). CONVENTION: this is an observation, not a plan. A level you \
                       EXPECT a technology to reach later is forecast_readiness, never this — \
                       recording a projection as an observation puts a fiction inside the \
                       machinery the roadmap is computed from, where it propagates. A rung \
                       outside 1-9 is refused rather than clamped. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing \u{2014} omitted \
                       fields keep their stored value, so correcting one never means re-sending \
                       a 2 KB field you did not touch.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_readiness(
        &self,
        Parameters(req): Parameters<AddReadinessReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let ty = reflow2_core::nodes::node::READINESS_ASSESSMENT;
        let mut __rf = crate::service::RequiredFields::new(&g, ty, &req.id)?;
        let kind_s = __rf.str("kind", req.kind);
        let target_type = __rf.str("target_type", req.target_type);
        let target_id = __rf.str("target_id", req.target_id);
        let level = __rf.i64("level", req.level);
        // Every field first, THEN the refusal — so a caller missing three of
        // them learns all three, rather than one per round trip.
        __rf.finish()?;
        let kind = parse_readiness_kind(&kind_s)?;
        ok_json(NodeDto::from(
            g.add_readiness(&ReadinessObservation {
                id: &req.id,
                target_type: &target_type,
                target_id: &target_id,
                kind,
                level,
                evidence: req.evidence.as_deref(),
                assessed_at: req.assessed_at.as_deref(),
            })
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "State that an increment cannot deliver until an enabling technology \
                       reaches a given readiness level — the JUDGEMENT half of BL-68, and the \
                       one reflow2 will never make for you. The threshold rides this edge \
                       rather than either endpoint so one increment can demand TRL 7 of one \
                       technology and TRL 4 of another, and so a demonstrator and a fielded \
                       increment can demand different levels of the SAME technology. \
                       CONVENTION: there is no default threshold. An increment with no gate \
                       reports 'ungated', never 'ready' — silence about a gate is not evidence \
                       there is none.",
        annotations(read_only_hint = false)
    )]
    pub async fn gate_on(
        &self,
        Parameters(req): Parameters<GateOnReq>,
    ) -> Result<CallToolResult, McpError> {
        let kind = parse_readiness_kind(&req.kind)?;
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.gate_on(&ReadinessGate {
                subject_type: &req.subject_type,
                subject_id: &req.subject_id,
                target_type: &req.target_type,
                target_id: &req.target_id,
                kind,
                min_level: req.min_level,
                rationale: req.rationale.as_deref(),
            })
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record a PROJECTED readiness level valid from a future epoch — 'this \
                       converter reaches TRL 7 in 2035' — as a TemporalFact marked \
                       basis=forecast (BL-68). It is deliberately not a DimensionObservation: \
                       `observed_at` says OBSERVED, and nobody observed anything in 2035. \
                       CONVENTION: confidence is YOURS to state and reflow2 never derives one \
                       from the horizon, because a decay curve is a judgement about risk \
                       appetite. The epoch must already exist — plan_epoch it first.",
        annotations(read_only_hint = false)
    )]
    pub async fn forecast_readiness(
        &self,
        Parameters(req): Parameters<ForecastReadinessReq>,
    ) -> Result<CallToolResult, McpError> {
        let kind = parse_readiness_kind(&req.kind)?;
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.forecast_readiness(&ReadinessForecast {
                id: &req.id,
                target_type: &req.target_type,
                target_id: &req.target_id,
                kind,
                level: req.level,
                epoch_id: &req.epoch_id,
                confidence: req.confidence,
                statement: req.statement.as_deref(),
            })
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "The DERIVED roadmap for one increment (BL-68): the earliest epoch by \
                       which every technology it is GATED_ON clears the level demanded of it, \
                       with the reason named — 'cannot deliver before 2035, because the \
                       converter is TRL 3 today, projected 7 at 2035, and this increment needs \
                       7'. The answer is the max over per-gate clearing epochs, because an \
                       increment waits for its slowest dependency. Four verdicts, and two of \
                       them are refusals: `ungated` (no threshold stated — NOT ready), \
                       `achievable_now`, `gated_until`, and `indeterminate` (a gate has no \
                       level and no clearing forecast, so no date can be derived — reported \
                       loudly rather than dropped from the max, which would return an \
                       optimistic date built by ignoring the inconvenient evidence).",
        annotations(read_only_hint = true)
    )]
    pub async fn readiness_report(
        &self,
        Parameters(req): Parameters<ReadinessReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.readiness_report(&req.subject_id).map_err(dyno_err)?)
    }

    #[tool(
        description = "Compare what is observed RUNNING against what DEPLOYED_TO declares — the \
                       as-fielded reconcile, sibling of reconcile_artifacts one phase later \
                       (BL-9). Supply one entry per environment you actually looked at, listing \
                       the releases running there (empty list = nothing runs, a positive \
                       statement). Reports deployment_missing (declared active, not running), \
                       deployment_undeclared (running, never declared) and \
                       deployment_contradicted (running while declared planned/rolled_back), \
                       plus ids the design has never heard of. Only Releases run and only \
                       Environments host — components and libraries never produce drift here. \
                       With record_events each divergence becomes a persistent DriftEvent (and \
                       an unresolved_drift gap) that a later reconcile resolves automatically \
                       when the divergence is gone; the design-side fix is deploy_to with the \
                       true status.",
        annotations(read_only_hint = false)
    )]
    pub async fn reconcile_deployment(
        &self,
        Parameters(req): Parameters<ReconcileDeploymentReq>,
    ) -> Result<CallToolResult, McpError> {
        let observed: Vec<reflow2_core::ObservedEnvironment> = req
            .observed
            .into_iter()
            .map(|o| reflow2_core::ObservedEnvironment {
                environment_id: o.environment_id,
                running: o.running,
            })
            .collect();
        let options = reflow2_core::FieldedOptions {
            record_events: req.record_events,
            exhaustive: req.exhaustive,
            detected_at: req.detected_at,
        };
        let mut g = self.write_lock().await;
        ok_json(
            g.reconcile_deployment(&observed, &options)
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "Record that a Component or Release needs a Resource, with how critical it \
                       is (optional/recommended/required).",
        annotations(read_only_hint = false)
    )]
    pub async fn require_resource(
        &self,
        Parameters(req): Parameters<RequireResourceReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.require_resource(
                &req.from_type,
                &req.from_id,
                &req.resource_id,
                req.criticality.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }
}
