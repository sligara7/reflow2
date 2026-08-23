//! `temporal_tools` tools — one slice of the MCP surface.
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

#[tool_router(router = temporal_tools_router, vis = "pub")]
impl ReflowService {
    #[tool(
        description = "Order one DesignEpoch after another (earlier PRECEDES later) — the chain \
                       axis Z exists to record. Epochs also carry a `sequence` integer, but the \
                       explicit edge is what makes the history walkable as a graph rather than \
                       sortable as a list.",
        annotations(read_only_hint = false)
    )]
    pub async fn precedes(
        &self,
        Parameters(req): Parameters<PrecedesReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        g.precedes(&req.earlier_epoch, &req.later_epoch)
            .map_err(dyno_err)?;
        ok_json(serde_json::json!({
            "earlier": req.earlier_epoch, "later": req.later_epoch
        }))
    }

    #[tool(
        description = "Pin any node to a DesignEpoch (AT_EPOCH) — e.g. a Release to its \
                       release_cut epoch, so the release and the design state it was cut from \
                       are joined on axis Z. Generic: AT_EPOCH is declared from any type.",
        annotations(read_only_hint = false)
    )]
    pub async fn pin_at_epoch(
        &self,
        Parameters(req): Parameters<PinAtEpochReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        g.pin_at_epoch(&req.node_type, &req.node_id, &req.epoch_id)
            .map_err(dyno_err)?;
        ok_json(serde_json::json!({
            "pinned": req.node_id, "at_epoch": req.epoch_id
        }))
    }

    #[tool(
        description = "Schedule a Requirement, Capability or QUESTION against the moment it is DUE \
                       — the \
                       satisfaction schedule, which is what makes a roadmap answerable \
                       (req:epochs-can-be-planned). The target is a DesignEpoch for the time axis \
                       or a Release for the capability-increment axis: two paired views of one \
                       architecture, so one edge serves both. `modality` says which kind of claim \
                       this is — `expected` is a plan, `required` is an obligation whose miss at \
                       arrival is a computed violation rather than a slip (the scheduling face of \
                       a KPP). THERE IS NO `achieved` MODALITY: delivery is computed from the \
                       golden thread and never asserted, so a schedule that recorded its own \
                       success would be a second source of truth able to disagree with the first. \
                       DELIBERATELY NOT add_epoch's AT_EPOCH, which means `belongs to` rather \
                       than `due at`. To reschedule, record the change against the epoch rather \
                       than re-pointing this edge — moving it silently would erase the slip and \
                       let the plan rewrite its own history. ⭐ SCHEDULING A `Question` IS HOW THE \
                       RESOLUTION OF A GAP GETS PLANNED: gaps are recomputed every run and are not \
                       nodes, so there is nothing to schedule, but the Question `gap_to_prompt` \
                       mints when a gap is put to somebody IS durable — and it is DELIVERED WHEN \
                       ANSWERED, needing no artifact and no check, because the whole content of \
                       closing a gap is that the person whose judgement it needed gave one. A \
                       WITHDRAWN question reports `discontinued`, not `outstanding` — somebody \
                       said.",
        annotations(read_only_hint = false)
    )]
    pub async fn schedule_for(
        &self,
        Parameters(req): Parameters<ScheduleForReq>,
    ) -> Result<CallToolResult, McpError> {
        let modality = req.modality.as_deref().unwrap_or("expected");
        let mut g = self.write_lock().await;
        g.schedule_for(
            &req.item_type,
            &req.item_id,
            &req.target_type,
            &req.target_id,
            modality,
            req.recorded_at.as_deref(),
        )
        .map_err(dyno_err)?;
        ok_json(serde_json::json!({
            "scheduled": req.item_id,
            "for": req.target_id,
            "modality": modality
        }))
    }

    #[tool(
        description = "What was PLANNED for an epoch or release against what was actually \
                       DELIVERED — the planned-versus-delivered delta (dec:arrival-delta). Ask it \
                       when a moment arrives: 'what didn't we achieve that we were supposed to in \
                       increment 10?'. Every item comes back with one of five outcomes — \
                       `delivered` (the plan held), `deferred` (still intended, the date moved, \
                       and where to), `discontinued` (no longer intended at all), or `outstanding` \
                       (still pointed here, not delivered, and NOBODY HAS SAID which of the \
                       previous two it is — that is the question to put to the user, never to \
                       default). Work scheduled after the baseline is reported separately, because \
                       a delta measured only against the plan cannot see the work that was not in \
                       it. `missed_obligations` are `required` claims that did not land: computed \
                       violations rather than slips. NOTHING HERE IS STORED — the plan lives in \
                       the epoch's snapshots and delivery is computed from the golden thread, so \
                       recording the outcome would create a second source of truth able to \
                       disagree with the first. The baseline is the target's FIRST snapshot, with \
                       every later one returned as the movement trail; where none exists the plan \
                       never moved and the live edges are the baseline. Read `notes` — it says \
                       what this computation cannot see.",
        annotations(read_only_hint = true)
    )]
    pub async fn arrival_delta(
        &self,
        Parameters(req): Parameters<ArrivalDeltaReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.arrival_delta(&req.target_id).map_err(dyno_err)?)
    }

    #[tool(
        description = "Derive a Keep a Changelog-shaped DRAFT between two moments of THIS design \
                       — compare_designs' sibling: that one compares two as-designed records, \
                       this one compares two moments of one design and renders the difference in \
                       the format the industry already reads. Buckets (Added/Changed/Deprecated/\
                       Removed/Fixed) are MAPPED from vocabulary the graph already records, and \
                       every entry names the rule that placed it; anything no rule covers comes \
                       back in `unmapped` rather than being guessed or dropped. Omit both ends \
                       for `[Unreleased]` — everything after the last DEPLOYED release, which \
                       makes 'what would this increment's changelog say?' answerable BEFORE \
                       cutting it. THE OUTPUT IS A DRAFT: no entry says what a CONSUMER should \
                       do, because the graph holds what moved and never what it costs \
                       downstream — `needs_a_human` names that obligation instead of inventing \
                       it. Nothing is stored; a stored changelog would be a second source of \
                       truth able to disagree with the graph.",
        annotations(read_only_hint = true)
    )]
    pub async fn changelog_view(
        &self,
        Parameters(req): Parameters<ChangelogViewReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(
            g.changelog_view(req.from.as_deref(), req.to.as_deref())
                .map_err(dyno_err)?,
        )
    }

    // ---- Temporal / CHANGE (deterministic, mutating) ----

    #[tool(
        description = "Create a `DesignEpoch` that HAS HAPPENED — a point on the time axis you \
                       are recording, which is what an epoch has always meant here. NOTE THE \
                       STORED TYPE NAME is `DesignEpoch`, not `Epoch`: that is the string \
                       `get_node` and `scan_nodes` want. For a point that has NOT happened yet, \
                       use plan_epoch instead; planning is a deliberate act and reads better as \
                       its own verb than as a flag. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing \u{2014} omitted \
                       fields keep their stored value, so correcting one never means re-sending \
                       a 2 KB field you did not touch.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_epoch(
        &self,
        Parameters(req): Parameters<AddEpochReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let ty = reflow2_core::nodes::node::DESIGN_EPOCH;
        let mut __rf = crate::service::RequiredFields::new(&g, ty, &req.id)?;
        let epoch_type_s = __rf.str("epoch_type", req.epoch_type);
        let name = __rf.str("name", req.name);
        let sequence = __rf.i64("sequence", req.sequence);
        // Collect every field before refusing, so a caller learns all of them
        // at once. Parsing the enum comes AFTER, or a missing name would be
        // masked by an unparseable type.
        __rf.finish()?;
        let epoch_type: EpochType = parse_enum(&epoch_type_s, "epoch type")?;
        ok_json(NodeDto::from(
            g.add_epoch(&req.id, &name, epoch_type, sequence)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create an Epoch that has NOT happened yet — a claim about the future \
                       rather than a record of the past, and the forward half of the time axis \
                       (req:epochs-can-be-planned). `epoch_type` still applies: KIND and TENSE are \
                       orthogonal, so a planned MILESTONE and a planned RELEASE CUT are both \
                       sayable — which is why `planned` is its own property rather than a value \
                       folded into the type enum. A planned epoch REFUSES record_change: a \
                       snapshot captures the present, so it cannot belong to a point that has not \
                       happened. Call set_epoch_status when it arrives. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing \u{2014} omitted \
                       fields keep their stored value, so correcting one never means re-sending \
                       a 2 KB field you did not touch.",
        annotations(read_only_hint = false)
    )]
    pub async fn plan_epoch(
        &self,
        Parameters(req): Parameters<AddEpochReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let ty = reflow2_core::nodes::node::DESIGN_EPOCH;
        let mut __rf = crate::service::RequiredFields::new(&g, ty, &req.id)?;
        let epoch_type_s = __rf.str("epoch_type", req.epoch_type);
        let name = __rf.str("name", req.name);
        let sequence = __rf.i64("sequence", req.sequence);
        // Collect every field before refusing, so a caller learns all of them
        // at once. Parsing the enum comes AFTER, or a missing name would be
        // masked by an unparseable type.
        __rf.finish()?;
        let epoch_type: EpochType = parse_enum(&epoch_type_s, "epoch type")?;
        ok_json(NodeDto::from(
            g.plan_epoch(&req.id, &name, epoch_type, sequence)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Move an Epoch between `planned` and `arrived`. `planned` → `arrived` is \
                       ARRIVAL: the moment a claim about the future becomes a point in the past, \
                       after which history can be recorded into it and the planned-versus- \
                       delivered delta becomes answerable. The reverse exists so a premature \
                       arrival can be corrected; it is not a way to un-happen an epoch. \
                       Everything else about the epoch is preserved.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_epoch_status(
        &self,
        Parameters(req): Parameters<EpochStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_epoch_status(&req.epoch_id, &req.status)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create a ChangeEvent (seed for propagate_change). Pass `affected` to say \
                       in the same call what it changed — a CHANGED edge is drawn to each entry, \
                       which is what makes the event propagatable. TWO QUESTIONS, NOT ONE: \
                       `change_type` says WHY, and `subject` says WHICH AXIS — `system` (the \
                       thing changed) or `record` (the thing did not change and only the design's \
                       knowledge of it did, e.g. a first baseline, a re-sync, a question settled). \
                       Leaving `subject` out is a true answer and is never inferred from \
                       `change_type`, because the mapping is not total. Use `defect_fix` when the \
                       design was right and the code was wrong, and `test_failure_fix` only when a \
                       check actually caught it — the difference is provenance, and it is the one \
                       five sessions each guessed differently before these existed. Use \
                       `documentation` when the thing was right and only its description of itself \
                       was wrong; the test is behavioural, not file-shaped, so a normative document \
                       that changes what somebody DOES takes a real label instead. \
                       TEXT GOES IN `summary` (what changed — indexed and searchable) and \
                       `rationale` (why, and the lesson). THERE IS NO `description` FIELD: \
                       reaching for one is the commonest mistake here, and it is refused \
                       rather than stored, so write the two that exist. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing \u{2014} omitted \
                       fields keep their stored value, so correcting one never means re-sending \
                       a 2 KB field you did not touch.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_change_event(
        &self,
        Parameters(req): Parameters<AddChangeEventReq>,
    ) -> Result<CallToolResult, McpError> {
        let g0 = self.write_lock().await;
        let mut __rf = crate::service::RequiredFields::new(
            &g0,
            reflow2_core::nodes::node::CHANGE_EVENT,
            &req.id,
        )?;
        let ct_s = __rf.str("change_type", req.change_type);
        let name = __rf.str("name", req.name);
        __rf.finish()?;
        drop(g0);
        let change_type: ChangeType = parse_enum(&ct_s, "change type")?;
        reject_reserved_change_type(change_type)?;
        let subject = req
            .subject
            .as_deref()
            .map(|s| parse_enum::<reflow2_core::ChangeSubject>(s, "change subject"))
            .transpose()?;
        let affected = req.affected.unwrap_or_default();
        let mut g = self.write_lock().await;
        // Validate the whole list before writing anything: storage accepts
        // dangling edges (this check is the only one there is), and a partial
        // write — event created, third entry refused — would leave a record
        // claiming less than the caller said. Refuse first, write whole.
        for a in &affected {
            match a.action.as_deref() {
                None | Some("added") | Some("modified") | Some("removed") => {}
                Some(other) => {
                    return Err(McpError::invalid_params(
                        format!(
                            "unknown affected action {other:?} for {}: expected added / \
                             modified / removed. Nothing was written.",
                            a.node_id
                        ),
                        None,
                    ));
                }
            }
            if g.get_node(&a.node_type, &a.node_id)
                .map_err(dyno_err)?
                .is_none()
            {
                return Err(McpError::invalid_params(
                    format!(
                        "affected node not found: {} {:?}. Nothing was written — every \
                         affected entry must already exist.",
                        a.node_type, a.node_id
                    ),
                    None,
                ));
            }
        }
        let event = g
            .add_change_event(
                &req.id,
                &name,
                change_type,
                subject,
                req.summary.as_deref(),
                req.rationale.as_deref(),
            )
            .map_err(dyno_err)?;
        let mut changed = Vec::new();
        for a in &affected {
            let action = a.action.as_deref().unwrap_or("modified");
            g.create_edge(
                reflow2_core::nodes::edge::CHANGED,
                reflow2_core::nodes::node::CHANGE_EVENT,
                &req.id,
                &a.node_type,
                &a.node_id,
                reflow2_core::nodes::Props::new().set("action", action),
            )
            .map_err(dyno_err)?;
            changed.push(json!({ "node_id": a.node_id, "action": action }));
        }
        ok_json(json!({
            "event": NodeDto::from(event),
            "changed": changed,
        }))
    }

    #[tool(
        description = "Record a change to a node in an epoch (snapshots the prior state). \
                       CONVENTION: record the change BEFORE you make it — the snapshot captures \
                       the state as it is now, so calling this afterwards preserves what you \
                       already replaced. TWO QUESTIONS, NOT ONE: `change_type` says WHY, and \
                       `subject` says WHICH AXIS — `system` (the thing changed) or `record` \
                       (the thing did not change and only the design's knowledge of it did, \
                       e.g. a re-sync or a drift you are accepting). Leaving `subject` out is \
                       a true answer and is never inferred from `change_type`.",
        annotations(read_only_hint = false)
    )]
    pub async fn record_change(
        &self,
        Parameters(req): Parameters<RecordChangeReq>,
    ) -> Result<CallToolResult, McpError> {
        let change_type: ChangeType = parse_enum(&req.change_type, "change type")?;
        reject_reserved_change_type(change_type)?;
        let action = parse_enum(&req.action, "change action")?;
        let subject = req
            .subject
            .as_deref()
            .map(|s| parse_enum::<reflow2_core::ChangeSubject>(s, "change subject"))
            .transpose()?;
        let rec = ChangeRecord {
            epoch_id: &req.epoch_id,
            change_event_id: &req.change_event_id,
            name: &req.name,
            target_type: &req.target_type,
            target_id: &req.target_id,
            change_type,
            subject,
            action,
        };
        let mut g = self.write_lock().await;
        let (prior, current) = g.record_change(rec).map_err(dyno_err)?;
        ok_json(json!({
            "prior_snapshot": prior.map(NodeDto::from),
            "current": NodeDto::from(current),
        }))
    }
}
