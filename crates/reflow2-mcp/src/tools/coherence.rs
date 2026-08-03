//! `coherence` tools — one slice of the MCP surface.
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

#[tool_router(router = coherence_router, vis = "pub")]
impl ReflowService {
    // ---- DETECT / analyze (deterministic, read-only) ----

    #[tool(
        description = "Find gaps in the design to ask the human about (DETECT). Pass `scope` (a \
                       node id) to answer for ONE PART of the design instead of all of it — the \
                       question a team that owns a subsystem asks day to day. The region is the \
                       propagation radius around that seed (`depth`, default 3), the same \
                       computation claim_region uses for \"the part I hold\", so \"my area\" means \
                       one thing everywhere. A scoped answer always reports what it left out: \
                       `total` across the whole design against `in_scope`, plus `out_of_scope` \
                       and `region_size`. Project-level rollups still appear when they touch \
                       your part, counted as `project_level` and carrying `scope: project` \
                       themselves — filtering is not the tool deciding what you may worry about.",
        annotations(read_only_hint = true)
    )]
    pub async fn detect_gaps(
        &self,
        Parameters(req): Parameters<ScopeReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        match req.scope.as_deref() {
            None => ok_json(g.detect_gaps().map_err(dyno_err)?),
            Some(seed) => ok_json(
                g.detect_gaps_in_scope(seed, req.depth.unwrap_or(DEFAULT_SCOPE_DEPTH))
                    .map_err(dyno_err)?,
            ),
        }
    }

    #[tool(
        description = "The coherence loop's outstanding debt, cheaply: what \
                       capture→detect→ask→decide steps are owed right now, computed from graph \
                       state alone (never from run history — looking is not writing). One call \
                       returns a short to-do list: anchored gaps never put to the user, \
                       questions still waiting or answered-but-unwritten, structural defects, \
                       capabilities claiming realized/verified with no passing check, recorded \
                       drift awaiting a disposition, and built capabilities nobody has checked \
                       against reality. Fire it between operational tasks instead of trying to \
                       remember the loop; `clean: true` means nothing is owed.",
        annotations(read_only_hint = true)
    )]
    pub async fn loop_status(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let status = g.loop_status().map_err(dyno_err)?;
        let mut payload = serde_json::to_value(&status).map_err(ser_err)?;
        // Whether the loop's own safety net exists (req:nudge-path-proven).
        // Machine-readable here, and in the handshake for the sessions that
        // never call this — which are precisely the ones a nudge is for.
        let nudge = crate::nudge::status(self.graph_path.as_deref());
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "nudge".into(),
                serde_json::to_value(&nudge).map_err(ser_err)?,
            );
            if let Some(advisory) = nudge.advisory() {
                obj.insert("nudge_advisory".into(), json!(advisory));
            }
        }
        ok_json(payload)
    }

    #[tool(
        description = "Blast radius of a recorded ChangeEvent along the golden thread. Returns \
                       a summary (counts by distance, the distance-1 ring, risk crossings); \
                       pass full=true for every impacted node with its hop chain.",
        annotations(read_only_hint = true)
    )]
    pub async fn propagate_change(
        &self,
        Parameters(req): Parameters<PropagateChangeReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = PropagateOptions {
            max_depth: req.max_depth.unwrap_or(5),
        };
        let g = self.graph.read().await;
        let radius = g
            .propagate_change(&req.change_event_id, opts)
            .map_err(dyno_err)?;
        if req.full.unwrap_or(false) {
            ok_json(radius)
        } else {
            ok_json(radius.summarize())
        }
    }

    #[tool(
        description = "Speculative blast radius from seed node ids (what would this touch?). \
                       Returns a summary (counts by distance, the distance-1 ring, risk \
                       crossings); pass full=true for every impacted node with its hop chain.",
        annotations(read_only_hint = true)
    )]
    pub async fn propagate_from(
        &self,
        Parameters(req): Parameters<PropagateFromReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = PropagateOptions {
            max_depth: req.max_depth.unwrap_or(5),
        };
        let seeds: Vec<&str> = req.seed_ids.iter().map(String::as_str).collect();
        let g = self.graph.read().await;
        let radius = g.propagate_from(&seeds, opts).map_err(dyno_err)?;
        if req.full.unwrap_or(false) {
            ok_json(radius)
        } else {
            ok_json(radius.summarize())
        }
    }

    #[tool(
        description = "The confirmation ledger (BL-35): for every capability with built \
                       artifacts, when was its claim last checked against reality, and what was \
                       the answer — drift events and whether each was resolved, accept claims \
                       split into design_holds vs design_updated, first baselines counted \
                       apart from both (they are not accepts), clean-reconcile confirmations \
                       with when they last happened, design edits on the record, and a state \
                       per capability: drifting (an observed divergence is unanswered), \
                       confirmed (examined, with the claim history visible), or unexamined \
                       (nobody has ever looked — NOT the same as confirmed).",
        annotations(read_only_hint = true)
    )]
    pub async fn confirmation_ledger(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.confirmation_ledger().map_err(dyno_err)?)
    }

    #[tool(
        description = "The 'what should I look at?' rollup report (SYNTHESIZE). Its `served_by` \
                       block names the reflow2 actually answering — version and binary build \
                       time — because an MCP server started before a rebuild keeps serving the \
                       old surface with nothing to say so (BL-32): the session that finds a \
                       mismatch between served_by and the repo should be restarted before \
                       trusting anything else it reads.",
        annotations(read_only_hint = true)
    )]
    pub async fn graph_report(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let mut report = serde_json::to_value(g.graph_report().map_err(dyno_err)?)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        report["served_by"] = served_by();
        self.ok_read(&g, report)
    }

    #[tool(
        description = "The graph report rendered as Markdown.",
        annotations(read_only_hint = true)
    )]
    pub async fn graph_report_markdown(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let report = g.graph_report().map_err(dyno_err)?;
        let mut md = report.to_markdown();
        // The rendering sibling of graph_report, and an orientation read in its
        // own right — carry the same read-side loop_hint (BL-91), as a trailing
        // blockquote since a Markdown document has no field to hang it on.
        if let Some(hint) = self.read_loop_hint(&g)? {
            md.push_str(&format!("\n\n> **loop_hint** — {hint}\n"));
        }
        Ok(ok_markdown(md))
    }

    #[tool(
        description = "Detect structural defects the machine can repair (HEAL). Pass `scope` (a \
                       node id, `depth` default 3) to ask it of one part of the design: not \
                       \"what is my team owed\" but \"is my part of the architecture sound\" — a \
                       cycle wholly inside one subsystem is that subsystem's to fix. Reports \
                       `total` against `in_scope` so a quiet corner never implies a quiet design.",
        annotations(read_only_hint = true)
    )]
    pub async fn detect_defects(
        &self,
        Parameters(req): Parameters<ScopeReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        match req.scope.as_deref() {
            None => ok_json(g.detect_defects().map_err(dyno_err)?),
            Some(seed) => ok_json(
                g.detect_defects_in_scope(seed, req.depth.unwrap_or(DEFAULT_SCOPE_DEPTH))
                    .map_err(dyno_err)?,
            ),
        }
    }

    #[tool(
        description = "Accept a structural defect the user has judged fine, recording WHY. It \
                       moves out of detect_defects into reviewed_defects — not deleted, not \
                       hidden — the mirror of acknowledge_gap, and for the same reason: a list \
                       that can never reach zero gets skimmed, so a genuine new defect must \
                       arrive into a list someone still reads. The reason becomes a real Decision \
                       node that outlives the session. Because a defect id hashes its category \
                       with its affected set, the review EXPIRES when that shape changes — the \
                       new shape has a new id nobody has accepted.",
        annotations(read_only_hint = false)
    )]
    pub async fn acknowledge_defect(
        &self,
        Parameters(req): Parameters<AcknowledgeDefectReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let decision_id = g
            .acknowledge_defect(&req.defect_id, &req.affected_ids, &req.reason)
            .map_err(dyno_err)?;
        ok_json(json!({ "acknowledged": req.defect_id, "decision_id": decision_id }))
    }

    #[tool(
        description = "Structural defects that were reviewed and accepted, each with the reason \
                       given. Worth re-reading when the architecture shifts: an acknowledgement \
                       is keyed to a defect's shape, so one still listed here still applies, and \
                       one whose shape has gone is reported as `retired` rather than vanishing.",
        annotations(read_only_hint = true)
    )]
    pub async fn reviewed_defects(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        self.ok_read(&g, g.reviewed_defects().map_err(dyno_err)?)
    }

    #[tool(
        description = "Withdraw a defect's acknowledgement, returning it to the open list. The \
                       Decision is superseded rather than deleted — the judgement was real and \
                       its record survives being changed. No-ops (returns withdrawn: false) when \
                       there was no acknowledgement.",
        annotations(read_only_hint = false)
    )]
    pub async fn withdraw_defect_acknowledgement(
        &self,
        Parameters(req): Parameters<DefectIdReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let withdrawn = g
            .withdraw_defect_acknowledgement(&req.defect_id)
            .map_err(dyno_err)?;
        ok_json(json!({ "withdrawn": withdrawn, "defect_id": req.defect_id }))
    }

    #[tool(
        description = "Propose a HEAL plan (never mutates; review then apply_heal).",
        annotations(read_only_hint = true)
    )]
    pub async fn propose_heal(
        &self,
        Parameters(req): Parameters<ProposeHealReq>,
    ) -> Result<CallToolResult, McpError> {
        let strategy: HealStrategy = match req.strategy.as_deref() {
            None => HealStrategy::default(),
            Some(s) => parse_enum(s, "heal strategy")?,
        };
        let opts = HealOptions {
            strategy,
            max_operations: req.max_operations,
        };
        let g = self.graph.read().await;
        ok_json(g.propose_heal(opts).map_err(dyno_err)?)
    }

    #[tool(
        description = "Evaluate how capabilities are allocated across components.",
        annotations(read_only_hint = true)
    )]
    pub async fn evaluate_allocation(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.evaluate_allocation().map_err(dyno_err)?)
    }

    #[tool(
        description = "Propose a capability→component allocation via Leiden clustering.",
        annotations(read_only_hint = true)
    )]
    pub async fn propose_allocation(
        &self,
        Parameters(req): Parameters<ProposeAllocationReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.propose_allocation(req.resolution).map_err(dyno_err)?)
    }

    #[tool(
        description = "Decomposition/hierarchy issues (matryoshka level checks).",
        annotations(read_only_hint = true)
    )]
    pub async fn hierarchy_issues(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.hierarchy_issues().map_err(dyno_err)?)
    }

    #[tool(
        description = "Surprising cross-community couplings (mined from the graph).",
        annotations(read_only_hint = true)
    )]
    pub async fn surprising_connections(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.surprising_connections().map_err(dyno_err)?)
    }

    #[tool(
        description = "All declining quality dimensions across the design, worst first.",
        annotations(read_only_hint = true)
    )]
    pub async fn dimension_drifts(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.dimension_drifts().map_err(dyno_err)?)
    }

    #[tool(
        description = "Quality-dimension drift for one target node.",
        annotations(read_only_hint = true)
    )]
    pub async fn dimension_drift(
        &self,
        Parameters(req): Parameters<DimensionDriftReq>,
    ) -> Result<CallToolResult, McpError> {
        let dim: Dimension = parse_enum(&req.dimension, "dimension")?;
        let g = self.graph.read().await;
        ok_json(g.dimension_drift(&req.target_id, dim).map_err(dyno_err)?)
    }

    #[tool(
        description = "Accept a gap the user has judged fine, recording WHY. It moves out of \
                       `detect_gaps` into `reviewed_gaps` — not deleted, not hidden. Use this \
                       once the user has actually decided something, so the open list means \
                       \"still needs attention\"; a list that can never reach zero gets skimmed. \
                       The reason is stored as a real Decision node in the graph, so it outlives \
                       this session. If the gap's affected nodes later change, the review \
                       expires and the gap returns for a fresh judgement.",
        annotations(read_only_hint = false)
    )]
    pub async fn acknowledge_gap(
        &self,
        Parameters(req): Parameters<AcknowledgeGapReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let decision_id = g
            .acknowledge_gap(&req.gap_id, &req.affected_ids, &req.reason)
            .map_err(dyno_err)?;
        ok_json(json!({ "acknowledged": req.gap_id, "decision_id": decision_id }))
    }

    #[tool(
        description = "Acknowledge MANY gaps in one call — the bulk form of acknowledge_gap. \
                       EACH GAP CARRIES ITS OWN REASON, which is the point: a batch of \
                       acknowledgements under one shared reason is exactly the erosion the \
                       ask-don't-repair rule exists to prevent, and would make a bulk form worse \
                       than the loop it replaces. The round trip collapses; the judgement stays \
                       per gap. ALL OF IT OR NONE OF IT — every item is attempted so you learn \
                       every failure at once, and if anything failed nothing is acknowledged.",
        annotations(read_only_hint = false)
    )]
    pub async fn acknowledge_gaps(
        &self,
        Parameters(req): Parameters<AcknowledgeGapsReq>,
    ) -> Result<CallToolResult, McpError> {
        let items: Vec<BulkGapAck> = req
            .gaps
            .into_iter()
            .map(|g| BulkGapAck {
                gap_id: g.gap_id,
                affected_ids: g.affected_ids,
                reason: g.reason,
            })
            .collect();
        let mut g = self.write_lock().await;
        let report = g.acknowledge_gaps(&items).map_err(dyno_err)?;
        bulk_result(report, |decision_id| json!({ "decision_id": decision_id }))
    }

    #[tool(
        description = "Gaps that were reviewed and accepted, each with the reason given. Worth \
                       re-reading when the design shifts.",
        annotations(read_only_hint = true)
    )]
    pub async fn reviewed_gaps(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.reviewed_gaps().map_err(dyno_err)?)
    }

    #[tool(
        description = "Withdraw a gap's acceptance: the Decision is marked superseded (kept, not \
                       deleted) and the gap returns to the open list.",
        annotations(read_only_hint = false)
    )]
    pub async fn withdraw_gap_acknowledgement(
        &self,
        Parameters(req): Parameters<GapIdReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let existed = g
            .withdraw_gap_acknowledgement(&req.gap_id)
            .map_err(dyno_err)?;
        // `withdrawn`, matching withdraw_question and delete_* (BL-57): every
        // "remove it if present" tool reports the same boolean shape.
        ok_json(json!({ "gap_id": req.gap_id, "withdrawn": existed }))
    }

    #[tool(
        description = "Does the BUILD separate what the DESIGN separates? Reports one fact and \
                       refuses a verdict: an artifact realizing N capabilities the design \
                       distinguishes is the build holding as one thing what the design holds as \
                       N. IT NEVER SAYS 'monolith', 'too big' or 'split it', carries NO severity, \
                       and rules on NEITHER side — N capabilities in one file may mean the file \
                       should be N files, or that the design over-decomposed, or that it is right \
                       for this phase (dec:report-dont-judge). THERE IS NO SIZE THRESHOLD: \
                       artifacts are compared against THIS design's own distribution, so an \
                       early-phase design where everything lives in one file has no outlier and \
                       is told nothing — a uniformly coarse design is not a broken one. Both \
                       cutoffs travel with the answer so they can be argued with, and \
                       `not_observed_about` names what it cannot see: unregistered artifacts, \
                       size of any kind, and outliers that mask each other. Pure arithmetic over \
                       REALIZES edges — no file I/O.",
        annotations(read_only_hint = true)
    )]
    pub async fn granularity_report(
        &self,
        Parameters(_req): Parameters<GranularityReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.granularity_report().map_err(dyno_err)?)
    }

    #[tool(
        description = "Apply a reviewed HealProposal atomically (rigid mode = no-op). Pass a \
                       proposal `propose_heal` returned — every operation is checked against what \
                       HEAL proposes for the graph as it stands now, and anything else is refused \
                       before a single write, so hand-editing the proposal or reusing a stale one \
                       fails rather than merging the wrong nodes. Merging deletes a node and \
                       cannot be undone. Read `discarded` in the result: it lists what the merge \
                       could not carry onto the survivor.",
        annotations(read_only_hint = false)
    )]
    pub async fn apply_heal(
        &self,
        Parameters(req): Parameters<ApplyHealReq>,
    ) -> Result<CallToolResult, McpError> {
        let proposal: HealProposal = parse_struct_param(req.proposal, "HealProposal")?;
        let mut g = self.write_lock().await;
        ok_json(g.apply_heal(&proposal).map_err(dyno_err)?)
    }
}
