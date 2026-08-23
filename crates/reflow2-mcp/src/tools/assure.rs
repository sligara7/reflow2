//! `assure` tools — one slice of the MCP surface.
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

#[tool_router(router = assure_router, vis = "pub")]
impl ReflowService {
    // ---- P4 Verification / P5 Operation / Decisions (the write side) ----

    #[tool(
        description = "Record a Verification — a check that something meets its intent. `method` \
                       says HOW you looked: test, analysis, inspection and demonstration are the \
                       four canonical ones, plus measurement, observation (watching it run in the \
                       field, unchanged), review and simulation. Answers the \
                       `build_without_verification` and `unverified_capability` gaps. Pair it with \
                       `verifies` to say what it checks. ⚠️ KEEP `name` SHORT — it is what a list \
                       renders. Put the account of what the check IS in `description`, and what a \
                       RUN FOUND in `findings` on set_verification_status. Measured on reflow2's \
                       own graph before those were reachable: median Verification name 76 words, \
                       longest 654, because `description` was declared and this constructor had \
                       no parameter for it, so everyone wrote reports into the name. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing \u{2014} omitted \
                       fields keep their stored value, so correcting one never means re-sending \
                       a 2 KB field you did not touch.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_verification(
        &self,
        Parameters(req): Parameters<VerificationReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let mut __rf = crate::service::RequiredFields::new(
            &g,
            reflow2_core::nodes::node::VERIFICATION,
            &req.id,
        )?;
        let name = __rf.str("name", req.name);
        ok_json(NodeDto::from(
            g.add_verification(
                &req.id,
                &name,
                req.method.as_deref(),
                req.level.as_deref(),
                req.description.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Set a Verification's outcome (planned/passing/failing/skipped/blocked), \
                       preserving what the check is. OMITTING `last_run_at` LEAVES IT ALONE — it \
                       is not cleared, so marking a check `failing` after a regression keeps the \
                       evidence that it ever ran. A failing check is a live signal: \
                       `propagate_from` it to see which capability and requirement it affects. \
                       CONVENTION: a check left at `planned` is not confirmation — verified means \
                       a check that PASSES, not one that exists. PASS `findings` TO RECORD WHAT \
                       THIS RUN FOUND — the evidence, as distinct from what the check IS. It \
                       lives here rather than on the constructor because a finding belongs to a \
                       RUN, and omitting it LEAVES IT ALONE exactly like `last_run_at`. NOT \
                       VALIDATED: reflow2 records what you say a run found and never judges it, \
                       so `passing` beside findings describing a failure is a contradiction only \
                       a reader can catch — which is a real 2026-08-07 field report, where a \
                       check recorded \"EXIT 0, verdict STALE\" and stayed passing forever.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_verification_status(
        &self,
        Parameters(req): Parameters<VerificationStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_verification_status(
                &req.verification_id,
                &req.status,
                req.last_run_at.as_deref(),
                req.findings.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Set a Verification's kind: `verification` (built right — checks the spec) \
                       or `validation` (the right thing — checks the operational intent). A \
                       distinct axis from method/level. A capability with a passing verification \
                       but no passing validation raises the `unvalidated_capability` gap — this is \
                       how you answer it (or acknowledge). Replaces the retired VALIDATES edge.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    pub async fn set_verification_kind(
        &self,
        Parameters(req): Parameters<VerificationKindReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_verification_kind(&req.verification_id, &req.kind)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Link a Verification to what it checks (VERIFIES).",
        annotations(read_only_hint = false)
    )]
    pub async fn verifies(
        &self,
        Parameters(req): Parameters<VerifiesReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.verifies(&req.verification_id, &req.target_type, &req.target_id)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record what a check HELD FIXED and what it VARIED for one claim — the \
                       input scope of its evidence. A passing check proves the points it \
                       actually drove, so a suite that pins the same seed, ordering or locale \
                       every time reads as full coverage while resting on one point of the \
                       space. Set on the VERIFIES edge, not the Verification, because scope is a \
                       fact about the CLAIM: one suite can cover one capability broadly and \
                       touch another at a single point. `evidence_report` then names the \
                       parameters every passing check pinned and none swept. Reported as a fact, \
                       never a gap — pinning a seed is normal, and a detector that fired on it \
                       would punish correct work. The check must already verify the target.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    pub async fn set_evidence_scope(
        &self,
        Parameters(req): Parameters<EvidenceScopeReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.set_evidence_scope(
                &req.verification_id,
                &req.target_type,
                &req.target_id,
                &req.pinned,
                &req.swept,
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record that a value was FITTED to a piece of evidence, so that same \
                       evidence can no longer count as its validation (CALIBRATED_AGAINST). Use \
                       it for any empirically-calibrated value — a coefficient fitted to a \
                       published anchor, a constant tuned to a dataset, a model matched to a \
                       measurement. Agreement with that evidence afterwards is a FIT, NOT A \
                       TEST, and `evidence_report` marks any check resting on it as consumed and \
                       excludes it from independent evidence. This has to be recorded rather \
                       than detected: no check inside a design can establish its own \
                       independence, so nothing can find the circularity by analysis. Also a \
                       traceability edge — correcting the anchor puts every value fitted to it \
                       in the blast radius.",
        annotations(read_only_hint = false)
    )]
    pub async fn calibrated_against(
        &self,
        Parameters(req): Parameters<CalibratedAgainstReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.calibrated_against(
                &req.from_type,
                &req.from_id,
                &req.evidence_type,
                &req.evidence_id,
                req.note.as_deref(),
                req.calibrated_at.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Compare what a real test run REPORTED against what each Verification \
                       records — the P4 reconcile, last of the three feedback loops (BL-30): \
                       reconcile_artifacts asks about the code, this about the outcomes, \
                       reconcile_deployment about what runs. Supply one entry per check the \
                       run executed ('passed'/'failed'/'skipped'). A recorded 'passing' that \
                       the run failed is the dangerous direction and sorts first — the design \
                       believed proven what is actually broken. With record_events each \
                       divergence is a persistent DriftEvent (and unresolved_drift gap), \
                       auto-resolved when a later run agrees; the design-side answer is \
                       set_verification_status with what the run actually said.",
        annotations(read_only_hint = false)
    )]
    pub async fn reconcile_verification(
        &self,
        Parameters(req): Parameters<ReconcileVerificationReq>,
    ) -> Result<CallToolResult, McpError> {
        let observed: Vec<reflow2_core::ObservedVerification> = req
            .observed
            .into_iter()
            .map(|o| reflow2_core::ObservedVerification {
                verification_id: o.verification_id,
                outcome: o.outcome,
            })
            .collect();
        let options = reflow2_core::VerifyReconcileOptions {
            record_events: req.record_events,
            exhaustive: req.exhaustive,
            detected_at: req.detected_at,
        };
        let mut g = self.write_lock().await;
        ok_json(
            g.reconcile_verification(&observed, &options)
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "Record WHERE a check was actually carried out (PERFORMED_IN). Without it a \
                       check run on a simulation rig and the same check run in the field are \
                       indistinguishable, so a capability proven only against a model reads \
                       exactly like one proven against reality. Point it at an Environment whose \
                       `env_type` says what kind of place that is — `simulation` for a rig, a \
                       digital twin, a physics model.",
        annotations(read_only_hint = false)
    )]
    pub async fn performed_in(
        &self,
        Parameters(req): Parameters<PerformedInReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.create_edge(
                reflow2_core::nodes::edge::PERFORMED_IN,
                reflow2_core::nodes::node::VERIFICATION,
                &req.verification_id,
                reflow2_core::nodes::node::ENVIRONMENT,
                &req.environment_id,
                reflow2_core::nodes::Props::new(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Where did each capability's evidence actually come from? Lists the \
                       environments its PASSING checks were performed in, and flags the ones \
                       proven ONLY in simulation — the risk that simulating first is supposed to \
                       buy down, which only works if you can still tell model from reality \
                       afterwards. REPORTS, NEVER RANKS: it will not claim lab beats staging \
                       beats field, because which of those is 'more real' is domain-specific and \
                       a wrong ordering gets worked around rather than corrected. A check naming \
                       no environment is counted as UNPLACED, never assumed real — silence is not \
                       evidence of the field.",
        annotations(read_only_hint = true)
    )]
    pub async fn evidence_report(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.evidence_report().map_err(dyno_err)?)
    }

    #[tool(
        description = "What has the design never been told about? You sweep the tree and supply \
                       what you saw (reflow2 does no file I/O); this answers with the regions no \
                       node claims, rolled up to the shallowest wholly-unclaimed directory and \
                       ranked by mass, so the biggest silence sorts first. Every other detector \
                       reasons about nodes ALREADY in the graph, so without this a design covering \
                       30% of a system reports the same `0 open gaps` as one covering all of it. \
                       Deliberately not a score and never a pass/fail: a registered artifact whose \
                       location is a directory claims everything beneath it, so modelling a \
                       vendored mass as one opaque unit is correct rather than a hole. THAT SAME \
                       RULE IS ALSO HOW THIS GOES FALSELY GREEN, so the answer now says what it is \
                       standing on: `opaque_claims` are subtrees claimed ON PURPOSE, \
                       `pending_expansion` are PLACEHOLDERS nobody has expanded yet, and until an \
                       artifact declares one (set_artifact_intent) the two are indistinguishable — \
                       a registration check once read GREEN over 359 individually unreferenceable \
                       files. Read them next to `claimed`: that is how you say '53 artifacts, of \
                       which 3 stand in for the rest' instead of just 'covered'. Exclusions \
                       come back named. Run it at the end of an adopt pass, so a thin pass is \
                       measured rather than felt.",
        annotations(read_only_hint = true)
    )]
    pub async fn coverage_report(
        &self,
        Parameters(req): Parameters<CoverageReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let observed: Vec<ObservedPath> = req
            .observed
            .into_iter()
            .map(|o| serde_json::from_value(JsonValue::Object(o)))
            .collect::<Result<_, _>>()
            .map_err(|e| McpError::invalid_params(format!("invalid observation: {e}"), None))?;
        let g = self.graph.read().await;
        ok_json(
            g.coverage_report(&observed, &req.exclusions, req.swept_at.as_deref())
                .map_err(dyno_err)?,
        )
    }
}
