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
                       capture→detect→ask→decide steps are owed now, computed from graph state, \
                       never run history. Anchored gaps never put to the user, questions waiting \
                       or answered-but-unwritten, open decisions a named person was ASKED to \
                       settle (a `proposed` Decision carrying an AUTHORED_BY `role=approver`; \
                       one with no approver is somebody thinking out loud and stays quiet), \
                       structural defects, capabilities claiming realized/verified with no \
                       passing check, drift awaiting a disposition, and built capabilities \
                       nobody has checked against reality. `clean: true` means nothing is owed, \
                       and those decisions are LISTED in `assigned_decisions`. Pass \
                       `contributor_id` to ask WHAT NEEDS THIS PERSON. Scoped, TWO things are \
                       attributed: decisions they were asked to settle, and open gaps standing \
                       on ground they OWN (`gaps_on_owned_ground`, each naming which owned nodes \
                       it touches). Every other debt class is a fact about the DESIGN and comes \
                       back design-wide under `scope.not_attributable` rather than filtered to \
                       zero, because \"nothing is owed to you\"and \"I cannot tell whose this \
                       is\"must never be the same answer — so scoped, `clean` means nothing is \
                       owed BY THAT PERSON, not that the design is clean. An unknown \
                       contributor_id is REFUSED: a typo would otherwise give the most \
                       reassuring reply there is. `verifications` is a DIGEST: counts by status, \
                       how many never ran, and every check not currently passing. `graph_report` \
                       carries every check with its last run.",
        annotations(read_only_hint = true)
    )]
    pub async fn loop_status(
        &self,
        Parameters(req): Parameters<LoopScopeReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let status = g
            .loop_status_for(req.contributor_id.as_deref())
            .map_err(dyno_err)?;
        let mut payload = serde_json::to_value(&status).map_err(ser_err)?;
        // The debt rollup is seven integers and a to-do list; the per-check roll
        // beside it grew with the design until the whole reply was 74 KB and no
        // longer fit in a harness turn — `cap:loop-status` says ONE CHEAP CALL,
        // and the build stopped meeting it somewhere around the hundredth check.
        // Digest here, never at the core: `graph_report` still serves the full
        // list, and both still come from `verification_recency`, so the two
        // surfaces cannot drift apart (the invariant report.rs states).
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "verifications".into(),
                verification_digest(&status.verifications).map_err(ser_err)?,
            );
        }
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
        // Has the SHARED RECORD moved since this seat last looked? The signal
        // existed and was read only by `export_graph`, so the design knew at
        // the first moment of a session and spoke at the last
        // (dec:idea-the-graph-notices-the-record-moved-without-being-asked,
        // option A). Reported here because this is the orientation call the
        // session-start hook already points sessions at.
        //
        // GATED, and the gate is the design: silent whenever the file has not
        // moved, which is the whole of ordinary solo work. The export is built
        // only when something HAS moved, so the common path costs one file read
        // and no comparison.
        if let Some(graph_path) = self.graph_path.as_deref() {
            let live_nodes = g.count_all_nodes().unwrap_or(0);
            let debts =
                crate::sync_debt::sync_debt(graph_path, live_nodes, &|| g.export_graph().ok());
            if let Some(obj) = payload.as_object_mut() {
                let behind: Vec<_> = debts.iter().filter(|d| d.is_actionable()).collect();
                if !behind.is_empty() {
                    obj.insert(
                        "record_moved".into(),
                        json!(
                            behind
                                .iter()
                                .map(|d| d.message())
                                .collect::<Vec<_>>()
                                .join(" ")
                        ),
                    );
                }
                // Every known target, including the quiet ones — "checked three
                // records, all in step" and "checked nothing" must not share an
                // answer.
                if !debts.is_empty() {
                    obj.insert(
                        "sync".into(),
                        serde_json::to_value(&debts).map_err(ser_err)?,
                    );
                }
            }
        }
        ok_json(payload)
    }

    #[tool(
        description = "Which decisions to settle next — a rough guide, not an ordering, for a design with more open \
                       questions than anyone can hold at once. FOUR BANDS, and the split is the design. `marked`: \
                       open decisions carrying AUTHORED_BY `role=approver` — the user's own word, durable across \
                       sessions, self-clearing when the status moves. NO SCORE REORDERS THESE. `ranked`: the top \
                       few of what is NOT marked, since ranking somebody's own marks back at them says nothing they \
                       do not know. `unexplored`: ONE from the zero-scoring pool — a REQUIRED exploration term, \
                       because every signal here is built on connectedness and would otherwise bury a decision \
                       nothing points at, forever. `shaping`: the few ACCEPTED decisions most of the LIVE design \
                       hangs off — not a to-do list, but what a newcomer needs to read the rest. SCORES ARE COARSE \
                       ON PURPOSE. Ranked: governed nodes (+1 each), those SCHEDULED into an increment (+2), \
                       CONTRADICTS an accepted Decision (+2). Shaping: live governed count, where live drops \
                       dropped/deferred Requirements and superseded/rejected Decisions — that filter is the whole \
                       refinement, since raw in-degree's top hit here governed ten requirements of which nine were \
                       dropped. Type breadth is reported as `shapes`, never ranked. WHAT IT CANNOT DO: a Decision \
                       has no timestamp, so staleness never enters; one linked to nothing scores zero however \
                       important — `unranked_pool` and `not_shown` report how many, so a short answer never reads \
                       as the whole set. Review records excluded throughout.",
        annotations(read_only_hint = true)
    )]
    pub async fn what_next(
        &self,
        Parameters(req): Parameters<crate::service::WhatNextReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.what_next(req.limit.unwrap_or(4)).map_err(dyno_err)?)
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
                       block says whether this server is STILL THE CODE IT WAS STARTED FROM — a \
                       server that predates a rebuild keeps serving the old surface, and until \
                       2026-08-09 nothing said so. READ `served_by.stale`: `true` means every \
                       computed number here came from a binary no longer on disk; `null` means \
                       the server could not tell (never read it as false). It is DERIVED, not \
                       compared: a version literal left four sessions drawing opposite \
                       conclusions from the same true value, and two builds in one working \
                       session share a version anyway. `stale_note` carries the fix, which is \
                       `--stop-shared` plus any tool call — NOT a session restart, which \
                       re-attaches to the same daemon and silently changes nothing.",
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
        description = "What can this design's graph actually SAY about the quality axes — the \
                       'ilities'? `DimensionAssessment.score` is only ever ASSERTED by a person \
                       or an LLM, while reflow2 separately computes modularity, articulation \
                       points, dependency cycles, misplaced capabilities, decomposition \
                       mismatches, build granularity and the trajectory bands — and connects \
                       none of it to the axis it informs. This connects them. IT NEVER DERIVES A \
                       SCORE and never writes to the graph: collapsing three cycles into \
                       `maintainability: 0.62` asserts a precision nobody has, which is exactly \
                       why TRL was kept out of that same float. ADVERSE IS INHERITED, NEVER \
                       RE-JUDGED — a finding counts against an axis only where the computation \
                       that produced it already calls it a defect; a ratio, a trajectory position \
                       and a granularity observation are reported as CONTEXT, because the modules \
                       that produce them deliberately refuse to grade. THE ANSWER IS NOT BLANKET: \
                       four of the nine axes — performance, security, scalability, observability \
                       — cannot be informed by a design graph at all and say so with the reason, \
                       rather than reading clean. The output worth reading is `worth_weighing`: \
                       targets where somebody asserted a good score on an axis a detector found \
                       something against. That is a disagreement between two records, and reflow2 \
                       rules on neither.",
        annotations(read_only_hint = true)
    )]
    pub async fn ility_report(
        &self,
        Parameters(_req): Parameters<IlityReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.ility_report().map_err(dyno_err)?)
    }

    #[tool(
        description = "Where does this design sit on the trajectory from FUNCTION to STRUCTURE? \
                       Designs normally get function right first and structure right later, \
                       iteratively and organically — so a well-developed function layer with no \
                       declared seams is a NORMAL POSITION, not debt. Reports seven bands \
                       (intent, function, allocation, seams, realization, assurance, operation), \
                       each as a count over a population with the question it answers, and names \
                       the lowest-scoring one as the FRONTIER. THE FRONTIER IS RELATIVE, so there \
                       is no threshold anywhere in this reading: reflow2 states where a design \
                       IS and REFUSES to say where it SHOULD be, because a demonstrator may sit \
                       at function-first forever and be right while a fielded increment may not \
                       — the same rule that keeps it from defaulting a TRL gate. Bands scoring \
                       ABOVE the frontier are reported as normal rather than as work done out of \
                       order, because real designs run ahead of themselves. A band with nothing \
                       to measure reads as unmeasured, never as zero. No stage name is emitted: \
                       a label no computation reads would be a distinction that does not earn \
                       its keep. Pure arithmetic over edges already in the graph — no file I/O.",
        annotations(read_only_hint = true)
    )]
    pub async fn maturity_report(
        &self,
        Parameters(_req): Parameters<MaturityReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.maturity_report().map_err(dyno_err)?)
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
        description = "Has the SHARED RECORD moved since this graph last looked? Answers the \
                       question a session should ask FIRST when other people write to the same \
                       design: somebody pulled their work into the committed export and this \
                       graph has never seen it. One entry per file this seat has synced with — \
                       `in_step` (exactly where this graph left it), `behind` (THE ACTIONABLE ONE \
                       — the record holds nodes this graph lacks; the ids are named and \
                       `import_graph` on that path takes them in), `moved_but_current` (somebody \
                       exported, you already hold it all), `missing`, `unreadable`. AN EMPTY \
                       ANSWER MEANS THIS SEAT HAS NEVER SYNCED WITH ANY FILE — never that all is \
                       well; the quiet targets are listed for exactly that reason. BEING AHEAD OF \
                       THE RECORD IS NOT REPORTED: unexported work is the normal state of a \
                       working session, and the check is gated on the file's content hash so it \
                       is silent unless somebody ELSE has been there. IT NEVER ACTS — no \
                       auto-import, because import is an upsert and an unasked one would silently \
                       overwrite live work (dec:ask-not-repair); it names the remedy and leaves \
                       the choice. `loop_status` carries the same finding as `record_moved` when \
                       there is one.",
        annotations(read_only_hint = true)
    )]
    pub async fn sync_status(
        &self,
        Parameters(_req): Parameters<SyncStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let Some(graph_path) = self.graph_path.as_deref() else {
            return ok_json(json!({
                "sync": [],
                "note": "this server holds no graph path, so it has no sync record to check",
            }));
        };
        let live_nodes = g.count_all_nodes().unwrap_or(0);
        let debts = crate::sync_debt::sync_debt(graph_path, live_nodes, &|| g.export_graph().ok());
        ok_json(json!({
            "sync": debts,
            "behind": debts.iter().filter(|d| d.is_actionable()).map(|d| d.message()).collect::<Vec<_>>(),
            "checked": debts.len(),
        }))
    }

    #[tool(
        description = "What did this design BUILD that it records no consumer for? Reports one \
                       fact and refuses a verdict. A Capability at `realized`/`verified` with no \
                       incoming DEPENDS_ON, no PART_OF_FLOW and no Actor INTERACTS_WITH is \
                       reported as 'THIS DESIGN RECORDS NOTHING THAT CONSUMES IT' — and NEVER as \
                       'unused', 'dead' or 'delete it'. THAT WORDING IS THE FEATURE: reflow2 \
                       reads a design, never a running system, so a capability real users call \
                       daily whose consumer nobody modelled is INDISTINGUISHABLE here from one \
                       dead since it shipped, and a detector that collapsed the two would \
                       recommend deleting working code. ABSENCE IS ONLY INFORMATIVE WHEN PRESENCE \
                       IS THE HABIT: if the design records a consumer for fewer than half of what \
                       it built, the list is WITHHELD and the ratio itself is the finding, \
                       because naming everything would report the modelling style rather than \
                       what was built (measured on reflow2's own design, where the raw signal \
                       named 100 of 110). `signals_read` names the edges counted so a missing one \
                       can be argued for, and `not_observed_about` names what it cannot see. Pure \
                       arithmetic over existing edges — no file I/O.",
        annotations(read_only_hint = true)
    )]
    pub async fn consumption_report(
        &self,
        Parameters(_req): Parameters<ConsumptionReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.consumption_report().map_err(dyno_err)?)
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

/// Summarise the verification roll for `loop_status` — the digest that keeps
/// "one cheap call" true as the check count grows.
///
/// What survives in full is what a reader would act on: every check that is
/// **not currently passing**, in the loud-first order `verification_recency`
/// already sorts them into. The passing remainder is counted, never dropped
/// silently — `total` against `omitted` says exactly what is not here, the same
/// contract `scan_nodes` states for a capped page.
///
/// `never_run` is the one count worth promoting out of the roll: a `passing`
/// with no `last_run_at` is an assertion, not a measurement, and that is
/// invisible in a status tally alone.
fn verification_digest(
    all: &[reflow2_core::report::VerificationRecency],
) -> Result<serde_json::Value, serde_json::Error> {
    let mut by_status: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for v in all {
        *by_status.entry(v.status.as_str()).or_insert(0) += 1;
    }
    let attention: Vec<_> = all.iter().filter(|v| v.status != "passing").collect();
    let never_run = all.iter().filter(|v| v.last_run_at.is_none()).count();

    Ok(json!({
        "total": all.len(),
        "by_status": by_status,
        "never_run": never_run,
        "attention": serde_json::to_value(&attention)?,
        "omitted": all.len() - attention.len(),
        "full_list": "graph_report — every check with its status and last run",
    }))
}
