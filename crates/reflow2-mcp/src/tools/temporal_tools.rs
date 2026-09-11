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
use reflow2_core::temporal::{ChangeRecord, Repair};
use reflow2_core::{
    AgentAnswer, AgentBackend, AskedQuestion, ChangeType, DEFAULT_SCOPE_DEPTH, DesignGraph,
    Dimension, DriftDisposition, DynoError, EpochType, GapCandidate, GenesisOptions, HealOptions,
    HealProposal, HealStrategy, IngestOptions, LinkArtifactOptions, LoopStatus, ObservedArtifact,
    ObservedPath, PromptCollector, PropagateOptions, ReadinessForecast, ReadinessGate,
    ReadinessKind, ReadinessObservation, ReconcileOptions, StoredNode, Value,
};

use crate::dto::{EdgeDto, NodeDto};
use crate::service::*;

/// `sequence` is required to create an epoch, and the generic required-field
/// helper cannot say what value would work — so the hint is computed here and
/// names the current maximum. SHARED by add_epoch and plan_epoch: the first cut
/// put it in add_epoch alone (Alex's fix, 2026-09-05) and the sibling gave the
/// bare refusal the day after
/// (`fact:defect-add-epoch-requires-a-sequence-the-refusal-cannot-name-because-the-required-field-helper-is-generic`,
/// recurrence #2). A fix placed in one handler rather than where both share is
/// how a class recurs on its sibling.
fn refuse_without_a_sequence_hint(
    g: &reflow2_core::graph::DesignGraph,
    req: &AddEpochReq,
) -> Result<(), McpError> {
    let ty = reflow2_core::nodes::node::DESIGN_EPOCH;
    // Alex (2026-09-05): `sequence` was required and the refusal could not
    // say what value would work, so the caller scanned epochs to learn it.
    // The generic required-field helper cannot know an epoch hint, so the
    // hint is computed HERE and names the current maximum. NOT auto-
    // assigned: a sequence is a position claim, and sequences are spaced
    // on purpose (a planned epoch once moved 360 -> 365 to make room for a
    // release cut), so max+1 would quietly pack and position.
    // dec:idea-add-epoch-assigns-the-next-sequence-or-names-it.
    if req.sequence.is_none() && g.get_node(ty, &req.id).map_err(dyno_err)?.is_none() {
        let epochs = g.scan_nodes(ty).map_err(dyno_err)?;
        let max = epochs
            .iter()
            .filter_map(|n| n.properties.get("sequence").and_then(|v| v.as_i64()))
            .max();
        let also: Vec<&str> = [
            ("name", req.name.is_none()),
            ("epoch_type", req.epoch_type.is_none()),
        ]
        .iter()
        .filter(|(_, missing)| *missing)
        .map(|(f, _)| *f)
        .collect();
        let also_s = if also.is_empty() {
            String::new()
        } else {
            format!(" Also missing: {}.", also.join(", "))
        };
        let where_s = match max {
            Some(m) => format!(
                "the highest existing sequence is {m} across {} epoch(s), so any value \
                 above {m} places this after everything — {} is the next integer, and \
                 leaving a gap (sequences are commonly spaced by 10) keeps room for a \
                 re-plan or a release cut",
                epochs.len(),
                m + 1
            ),
            None => {
                "there are no epochs yet, so any integer works (1, or 10 to leave room)".to_string()
            }
        };
        return Err(McpError::invalid_params(
            format!(
                "`sequence` is required to CREATE DesignEpoch '{}': {where_s}. It is not \
                 auto-assigned because a sequence is a position claim only you can \
                 make.{also_s}",
                req.id
            ),
            None,
        ));
    }
    Ok(())
}

/// `description` is the DesignEpoch's embedding field and neither core
/// constructor takes it, so both `add_epoch` and `plan_epoch` set it here after
/// the node lands. SHARED between the two on purpose: the sequence-hint helper
/// beside it exists because a fix put in one of these handlers and not the
/// other let the same defect recur on the sibling the next day
/// (`fact:defect-add-epoch-requires-a-sequence-the-refusal-cannot-name-because-the-required-field-helper-is-generic`,
/// recurrence #2). Omitted leaves whatever the node already holds, which is what
/// makes "call again with only what you are changing" true of this field too.
fn set_epoch_prose(
    g: &mut reflow2_core::graph::DesignGraph,
    id: &str,
    description: Option<&str>,
    checksum: Option<&str>,
) -> Result<Option<reflow2_core::StoredNode>, McpError> {
    let mut props = reflow2_core::nodes::Props::new();
    let mut any = false;
    if let Some(d) = description {
        props = props.set("description", d);
        any = true;
    }
    if let Some(c) = checksum {
        props = props.set("checksum", c);
        any = true;
    }
    if !any {
        return Ok(None);
    }
    let node = g
        .upsert_node(reflow2_core::nodes::node::DESIGN_EPOCH, id, props)
        .map_err(dyno_err)?;
    Ok(Some(node))
}

#[tool_router(router = temporal_tools_router, vis = "pub")]
impl ReflowService {
    #[tool(
        description = "Order one DesignEpoch after another (earlier PRECEDES later) — the chain \
                       axis Z exists to record. Epochs also carry a `sequence` integer, but the \
                       explicit edge is what makes the history walkable as a graph rather than \
                       sortable as a list. \
                       Ask for this when you want to record that one point in the design's history comes before another.",
        annotations(read_only_hint = false)
    )]
    pub async fn precedes(
        &self,
        Parameters(req): Parameters<PrecedesReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        g.precedes(&req.earlier_epoch, &req.later_epoch)
            .map_err(dyno_err)?;
        ok_json(serde_json::json!({
            "earlier": req.earlier_epoch, "later": req.later_epoch
        }))
    }

    #[tool(
        description = "Pin any node to a DesignEpoch (AT_EPOCH) — e.g. a Release to its \
                       release_cut epoch, so the release and the design state it was cut from \
                       are joined on axis Z. Generic: AT_EPOCH is declared from any type. \
                       Ask for this when you want to snapshot or freeze how an item looks today, at a point in time, so it can be compared later.",
        annotations(read_only_hint = false)
    )]
    pub async fn pin_at_epoch(
        &self,
        Parameters(req): Parameters<PinAtEpochReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let node_type = crate::service::resolve_node_type(
            &g,
            req.node_type.as_deref(),
            &req.node_id,
            "node_type",
        )?;
        g.pin_at_epoch(&node_type, &req.node_id, &req.epoch_id)
            .map_err(dyno_err)?;
        ok_json(serde_json::json!({
            "pinned": req.node_id, "at_epoch": req.epoch_id
        }))
    }

    #[tool(
        description = "Schedule a Requirement, Capability, QUESTION, Verification or Decision against the \
                       moment it is DUE — the satisfaction schedule, which is what makes a roadmap answerable \
                       (req:epochs-can-be-planned). The target is a DesignEpoch for the time axis or a Release \
                       for the capability-increment axis: two paired views of one architecture, so one edge \
                       serves both. `modality` says which kind of claim this is — `expected` is a plan, \
                       `required` is an obligation whose miss at arrival is a computed violation rather than a \
                       slip. THERE IS NO `achieved` MODALITY: delivery is computed from the golden thread and \
                       never asserted, so a schedule that recorded its own success would be a second source of \
                       truth able to disagree with the first. DELIBERATELY NOT add_epoch's AT_EPOCH, which \
                       means `belongs to` rather than `due at`. To reschedule, record the change against the \
                       epoch rather than re-pointing this edge — moving it silently would erase the slip. ⭐ \
                       SCHEDULING A `Question` IS HOW THE RESOLUTION OF A GAP GETS PLANNED: gaps are recomputed \
                       every run and are not nodes, so there is nothing to schedule, but the Question \
                       `gap_to_prompt` mints when a gap is put to somebody IS durable — and it is DELIVERED \
                       WHEN ANSWERED, needing no artifact and no check, because the whole content of closing a \
                       gap is that the person whose judgement it needed gave one. A WITHDRAWN question reports \
                       `discontinued`, not `outstanding`. Ask for this to put a piece of work into a release, \
                       increment or milestone.",
        annotations(read_only_hint = false)
    )]
    pub async fn schedule_for(
        &self,
        Parameters(req): Parameters<ScheduleForReq>,
    ) -> Result<CallToolResult, McpError> {
        let modality = req.modality.as_deref().unwrap_or("expected");
        let mut g = self.write_lock().await?;
        let target_type = crate::service::resolve_node_type(
            &g,
            req.target_type.as_deref(),
            &req.target_id,
            "target_type",
        )?;
        let item_type = crate::service::resolve_node_type(
            &g,
            req.item_type.as_deref(),
            &req.item_id,
            "item_type",
        )?;
        g.schedule_for(
            &item_type,
            &req.item_id,
            &target_type,
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
        description = "What was PLANNED for an epoch or release against what was actually DELIVERED — the \
                       planned-versus-delivered delta (dec:arrival-delta). Ask it when a moment arrives: 'what \
                       didn't we achieve that we were supposed to in increment 10?'. Every item comes back with \
                       one of five outcomes — `delivered` (the plan held), `deferred` (still intended, the date \
                       moved, and where to), `discontinued` (no longer intended at all), or `outstanding` \
                       (still pointed here, not delivered, and NOBODY HAS SAID which of the previous two it is \
                       — that is the question to put to the user, never to default). Work scheduled after the \
                       baseline is reported separately, because a delta measured only against the plan cannot \
                       see the work that was not in it. `missed_obligations` are `required` claims that did not \
                       land: computed violations rather than slips. NOTHING HERE IS STORED — the plan lives in \
                       the epoch's snapshots and delivery is computed from the golden thread, so recording the \
                       outcome would create a second source of truth able to disagree with the first. The \
                       baseline is the target's FIRST snapshot, with every later one returned as the movement \
                       trail; where none exists the plan never moved and the live edges are the baseline. Read \
                       `notes` — it says what this computation cannot see. Ask for this when you want to know \
                       what was planned for a release or increment that did not actually ship — the \
                       planned-versus-delivered gap.",
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

    #[tool(
        description = "Record work THIS SESSION did BY HAND that reflow2 already serves, or \
                       should — the negative space. ⭐ IT IS A STRONGER SIGNAL THAN AN ABSENT \
                       CALL BECAUSE IT CARRIES INTENT: reflow2 can count which tools were never \
                       called, but `dec:bl-155` measured 40 of 132 unused and states outright \
                       that it CANNOT TELL UNUSED FROM UNREACHABLE. A session that wrote a script \
                       to do X proves somebody wanted X badly enough to build it, which a zero in \
                       a usage table never shows. `diagnosis` is the whole value and the set is \
                       CLOSED — `tool_missing` (nothing does this), `tool_not_found` (something \
                       does and you did not find it: a DISCOVERABILITY failure, a different \
                       repair), `tool_refused` (you reached for one and it would not), `unknown` \
                       (you cannot say, which is an honest answer and better than a wrong \
                       bucket). Naming a `reflow2_tool` that is not served is REFUSED, because \
                       that means the diagnosis is really `tool_missing` and a phantom would sit \
                       in the table somebody reads to decide what to improve. The same work \
                       reported twice is ONE record. 🛑 WHAT YOU WRITE STAYS IN THIS DESIGN: it \
                       is free text naming your own domain, so it must never be lifted into a \
                       telemetry payload (`req:telemetry-carries-usage-never-design-content` — \
                       log the verb, never the object).",
        annotations(read_only_hint = false)
    )]
    pub async fn report_manual_work(
        &self,
        Parameters(req): Parameters<ReportManualWorkReq>,
    ) -> Result<CallToolResult, McpError> {
        // ⭐ THE ROUTER ANSWERS FOR ITSELF. `has_route` asks the live surface
        // whether it serves that name, so there is no second copy of the tool
        // list to drift — which is why this check lives here and not in the
        // core, where it would have had to be one.
        if let Some(t) = req.reflow2_tool.as_deref()
            && !self.tool_router.has_route(t)
        {
            return Err(McpError::invalid_params(
                format!(
                    "reflow2 serves no tool named {t:?}. If NOTHING does this, the diagnosis is \
                     `tool_missing` and the tool name should be omitted. If something does, name \
                     it exactly as the surface spells it — find_tools will tell you."
                ),
                None,
            ));
        }
        let mut g = self.write_lock().await?;
        let id = g
            .report_manual_work(
                &req.what,
                &req.diagnosis,
                req.reflow2_tool.as_deref(),
                req.at.as_deref(),
            )
            .map_err(dyno_err)?;
        ok_json(serde_json::json!({ "recorded": id }))
    }

    #[tool(
        description = "Every piece of hand-rolled work this design has recorded, with the \
                       diagnosis that separates a MISSING tool from an UNFINDABLE one. Read it \
                       when deciding what to build or what to surface: a run of \
                       `tool_not_found` against a tool that exists is a discoverability repair, \
                       and a run of `tool_missing` is a feature nobody has written. Empty means \
                       nobody has reported any — which is NOT the same as nobody having done work \
                       by hand, and must not be read as it, since the signal depends on a session \
                       noticing and saying so.",
        annotations(read_only_hint = true)
    )]
    pub async fn manual_work_report(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.manual_work_report().map_err(dyno_err)?)
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
                       a 2 KB field you did not touch. \
                       Ask for this when you want to mark a point in time in the design's history.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_epoch(
        &self,
        Parameters(req): Parameters<AddEpochReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let ty = reflow2_core::nodes::node::DESIGN_EPOCH;
        refuse_without_a_sequence_hint(&g, &req)?;
        let mut __rf = crate::service::RequiredFields::new(&g, ty, &req.id)?;
        let epoch_type_s = __rf.str("epoch_type", req.epoch_type);
        let name = __rf.str("name", req.name);
        let sequence = __rf.i64("sequence", req.sequence);
        // Collect every field before refusing, so a caller learns all of them
        // at once. Parsing the enum comes AFTER, or a missing name would be
        // masked by an unparseable type.
        __rf.finish()?;
        let epoch_type: EpochType = parse_enum(&epoch_type_s, "epoch type")?;
        let node = g
            .add_epoch(&req.id, &name, epoch_type, sequence)
            .map_err(dyno_err)?;
        let node = set_epoch_prose(
            &mut g,
            &req.id,
            req.description.as_deref(),
            req.checksum.as_deref(),
        )?
        .unwrap_or(node);
        ok_json(NodeDto::from(node))
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
        let mut g = self.write_lock().await?;
        let ty = reflow2_core::nodes::node::DESIGN_EPOCH;
        refuse_without_a_sequence_hint(&g, &req)?;
        let mut __rf = crate::service::RequiredFields::new(&g, ty, &req.id)?;
        let epoch_type_s = __rf.str("epoch_type", req.epoch_type);
        let name = __rf.str("name", req.name);
        let sequence = __rf.i64("sequence", req.sequence);
        // Collect every field before refusing, so a caller learns all of them
        // at once. Parsing the enum comes AFTER, or a missing name would be
        // masked by an unparseable type.
        __rf.finish()?;
        let epoch_type: EpochType = parse_enum(&epoch_type_s, "epoch type")?;
        let node = g
            .plan_epoch(&req.id, &name, epoch_type, sequence)
            .map_err(dyno_err)?;
        let node = set_epoch_prose(
            &mut g,
            &req.id,
            req.description.as_deref(),
            req.checksum.as_deref(),
        )?
        .unwrap_or(node);
        ok_json(NodeDto::from(node))
    }

    #[tool(
        description = "Move an Epoch between `planned` and `arrived`. `planned` → `arrived` is \
                       ARRIVAL: the moment a claim about the future becomes a point in the past, \
                       after which history can be recorded into it and the planned-versus- \
                       delivered delta becomes answerable. The reverse exists so a premature \
                       arrival can be corrected; it is not a way to un-happen an epoch. \
                       Everything else about the epoch is preserved. CARRIES `prose_currency` WHEN THE STATUS ACTUALLY MOVES and the node holds prose: the description was written under the OLD status and this call did not touch it, so the block names both statuses and QUOTES the prose so you can judge it here rather than in another call. It never says the prose is wrong - only a person can. From a 2026-09-02 field report where a capability went `realized` twenty minutes after a description saying the fix was not installed, and nothing noticed.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_epoch_status(
        &self,
        Parameters(req): Parameters<EpochStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let prior = crate::tools::capture::prior_status(
            &g,
            reflow2_core::nodes::node::DESIGN_EPOCH,
            &req.epoch_id,
        );
        let node = NodeDto::from(
            g.set_epoch_status(&req.epoch_id, &req.status)
                .map_err(dyno_err)?,
        );
        ok_json(
            crate::prose_currency::with_prose_currency(node, prior.as_deref()).map_err(ser_err)?,
        )
    }

    #[tool(
        description = "Record a dated finding or defect as a TemporalFact — what was observed, \
                       about which node, on what date. Use it the moment you have something to \
                       write down about how the system actually behaved: a defect met, a \
                       measurement taken, an observation that dates. \
                       \u{1F6D1} IF WHAT YOU ARE ABOUT TO WRITE IS A CAUSE — an explanation for \
                       something you did not predict, anything phrased \"because\", \"the problem \
                       is\", \"that explains it\" — GET THE `root-cause` SKILL FIRST and follow \
                       it, then come back here. That is not a formality: the skill generates \
                       candidate causes from the design instead of from memory and forces a \
                       measurement that could refute the favourite one, and the first plausible \
                       explanation is exactly what it exists to stop. Measured over 91 sessions \
                       of a real project, between a fifth and a quarter of sessions doing this \
                       work ever opened the skill written for it, which is why this sentence is \
                       in the tool rather than only in a rule. \
                       `subject_id` MUST RESOLVE: a finding about a node the design does not \
                       have is refused, not stored. Pass `caused_by` with `cause_evidence` to \
                       draw the CAUSES edge in the same call — the repair is recoverable from \
                       the diff and the cause is not. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing.",
        annotations(read_only_hint = false)
    )]
    pub async fn record_finding(
        &self,
        Parameters(req): Parameters<crate::service::RecordFindingReq>,
    ) -> Result<CallToolResult, McpError> {
        // The evidence requirement is checked BEFORE anything is written, so a
        // caller who names a cause without saying why gets a refusal rather
        // than a finding plus a silently-missing edge. Same reasoning as
        // add_change_event validating its whole `affected` list first: refuse
        // first, write whole.
        if req.caused_by.is_some()
            && req
                .cause_evidence
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(McpError::invalid_params(
                "`caused_by` needs `cause_evidence`: WHY that node is the cause, in a sentence. \
                 A CAUSES edge with no evidence is an assertion the next reader can neither \
                 check nor overturn. Nothing was written."
                    .to_string(),
                None,
            ));
        }
        let g0 = self.write_lock().await?;
        let mut __rf = crate::service::RequiredFields::new(
            &g0,
            reflow2_core::nodes::node::TEMPORAL_FACT,
            &req.id,
        )?;
        let statement = __rf.str("statement", req.statement);
        let subject_id = __rf.str("subject_id", Some(req.subject_id.clone()));
        __rf.finish()?;
        drop(g0);

        let mut g = self.write_lock().await?;
        // The subject's type is resolved from the id by the same convention
        // every other read uses, and a cross-type collision refuses rather
        // than guesses. The node_ref check in the core then refuses an id that
        // resolves to nothing — this call only makes that refusal reachable
        // with a type the caller did not have to know.
        let subject_type = crate::service::resolve_node_type(
            &g,
            req.node_type.as_deref(),
            &subject_id,
            "node_type",
        )?;
        if g.get_node(&subject_type, &subject_id)
            .map_err(dyno_err)?
            .is_none()
        {
            return Err(McpError::invalid_params(
                format!(
                    "subject not found: {subject_type} {subject_id:?}. A finding about a node \
                     the design does not have is not a record. Nothing was written."
                ),
                None,
            ));
        }
        let mut props = reflow2_core::nodes::Props::new()
            .set("subject_id", subject_id.as_str())
            .set("statement", statement.as_str());
        if let Some(v) = req.name.as_deref() {
            props = props.set("name", v);
        }
        props = props.set("fact_type", req.fact_type.as_deref().unwrap_or("finding"));
        props = props.set("basis", req.basis.as_deref().unwrap_or("measured"));
        if let Some(c) = req.confidence {
            props = props.set("confidence", c);
        }
        for (k, v) in [
            ("valid_from", req.valid_from.as_deref()),
            ("valid_to", req.valid_to.as_deref()),
            ("value", req.value.as_deref()),
        ] {
            if let Some(v) = v {
                props = props.set(k, v);
            }
        }
        let node = g
            .upsert_node(reflow2_core::nodes::node::TEMPORAL_FACT, &req.id, props)
            .map_err(dyno_err)?;
        // The subject carries the fact, so a reader who has the node finds the
        // finding without having to search for it.
        g.create_edge(
            reflow2_core::nodes::edge::HAS_TEMPORAL_FACT,
            &subject_type,
            &subject_id,
            reflow2_core::nodes::node::TEMPORAL_FACT,
            &req.id,
            reflow2_core::nodes::Props::new(),
        )
        .map_err(dyno_err)?;
        let mut caused_by = serde_json::Value::Null;
        if let Some(cause_id) = req.caused_by.as_deref() {
            let cause_type = crate::service::resolve_node_type(
                &g,
                req.caused_by_type.as_deref(),
                cause_id,
                "caused_by_type",
            )?;
            if g.get_node(&cause_type, cause_id)
                .map_err(dyno_err)?
                .is_none()
            {
                return Err(McpError::invalid_params(
                    format!(
                        "cause not found: {cause_type} {cause_id:?}. The finding WAS written; \
                         the CAUSES edge was not. Re-send with an id that resolves."
                    ),
                    None,
                ));
            }
            g.create_edge(
                reflow2_core::nodes::edge::CAUSES,
                &cause_type,
                cause_id,
                reflow2_core::nodes::node::TEMPORAL_FACT,
                &req.id,
                reflow2_core::nodes::Props::new()
                    .set("evidence", req.cause_evidence.as_deref().unwrap_or("")),
            )
            .map_err(dyno_err)?;
            caused_by = json!({ "node_id": cause_id, "node_type": cause_type });
        }
        ok_json(json!({
            "finding": NodeDto::from(node),
            "subject": { "node_id": subject_id, "node_type": subject_type },
            "caused_by": caused_by,
        }))
    }

    #[tool(
        description = "Create a ChangeEvent (seed for propagate_change). Pass `affected` to say in the same \
                       call what it changed — a CHANGED edge is drawn to each entry, which is what makes the \
                       event propagatable. TWO QUESTIONS, NOT ONE: `change_type` says WHY, and `subject` says \
                       WHICH AXIS — `system` (the thing changed) or `record` (the thing did not change and only \
                       the design's knowledge of it did, e.g. a first baseline, a re-sync, a question settled). \
                       Leaving `subject` out is a true answer and is never inferred from `change_type`, because \
                       the mapping is not total. Use `defect_fix` when the design was right and the code was \
                       wrong, and `test_failure_fix` only when a check actually caught it. Use `documentation` \
                       when the thing was right and only its description of itself was wrong; the test is \
                       behavioural, not file-shaped, so a normative document that changes what somebody DOES \
                       takes a real label instead. TEXT GOES IN `summary` (what changed — indexed and \
                       searchable) and `rationale` (why, and the lesson). THERE IS NO `description` FIELD: \
                       reaching for one is the commonest mistake here, and it is refused rather than stored, so \
                       write the two that exist. CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: \
                       call it again with the same id and only what you are changing \u{2014} omitted fields \
                       keep their stored value, so correcting one never means re-sending a 2 KB field you did \
                       not touch. Ask for this to record that something changed and why — log a change and the \
                       reason.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_change_event(
        &self,
        Parameters(req): Parameters<AddChangeEventReq>,
    ) -> Result<CallToolResult, McpError> {
        // The commonest mistake, caught where it is made. `description` is not a
        // ChangeEvent field; accepting it (rather than letting serde refuse with
        // a bare field list) lets the refusal name the two fields that ARE the
        // prose. Reported by two projects, and by a third AFTER the tool
        // description already said it — because a refusal is read at the moment
        // of failure and a 2 KB description is not.
        if req.description.is_some() {
            return Err(McpError::invalid_params(
                "a ChangeEvent has no `description`. The prose goes in `summary` (WHAT changed —                  indexed and searchable) or `rationale` (WHY, and the lesson). Re-send with one                  of those instead of `description`.",
                None,
            ));
        }
        let g0 = self.write_lock().await?;
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
        let mut g = self.write_lock().await?;
        // Validate the whole list before writing anything: storage accepts
        // dangling edges (this check is the only one there is), and a partial
        // write — event created, third entry refused — would leave a record
        // claiming less than the caller said. Refuse first, write whole.
        for a in &affected {
            let a_type = crate::service::resolve_node_type(
                &g,
                a.node_type.as_deref(),
                &a.node_id,
                "node_type",
            )?;
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
            if g.get_node(&a_type, &a.node_id).map_err(dyno_err)?.is_none() {
                return Err(McpError::invalid_params(
                    format!(
                        "affected node not found: {} {:?}. Nothing was written — every \
                         affected entry must already exist.",
                        a_type, a.node_id
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
                req.detected_at.as_deref(),
            )
            .map_err(dyno_err)?;
        let mut changed = Vec::new();
        for a in &affected {
            let a_type = crate::service::resolve_node_type(
                &g,
                a.node_type.as_deref(),
                &a.node_id,
                "node_type",
            )?;
            let action = a.action.as_deref().unwrap_or("modified");
            g.create_edge(
                reflow2_core::nodes::edge::CHANGED,
                reflow2_core::nodes::node::CHANGE_EVENT,
                &req.id,
                &a_type,
                &a.node_id,
                reflow2_core::nodes::Props::new().set("action", action),
            )
            .map_err(dyno_err)?;
            changed.push(json!({ "node_id": a.node_id, "action": action }));
        }
        // The second of the two tools with observed instances. `summary`
        // swallowing `rationale` is how `chg:the-relatedness-judgement-rides-
        // the-refusal` lost its reasoning, and how bhome's ChangeEvent lost
        // its own on 2026-08-31. Warns; never refuses — the event above is
        // already written.
        let mut out = json!({
            "event": NodeDto::from(event),
            "changed": changed,
        });
        if let Some(am) = crate::tools::capture::absorbed_markup(&[
            ("name", Some(&name)),
            ("summary", req.summary.as_deref()),
            ("rationale", req.rationale.as_deref()),
        ]) {
            out["absorbed_markup"] = serde_json::to_value(am).map_err(ser_err)?;
        }
        // AN UNDATED EVENT SAYS SO. F-06, hxm_program: nine events in one day,
        // every date in the prose and none in the field, so nothing downstream
        // could order them. This project's standing posture is that an absent
        // property means NOBODY SAID and is REPORTED rather than defaulted —
        // `invalidates` answers `rerun_owed: null`, `repair_report` counts
        // `unstated` first, `detect_defects` says what it could not have found.
        // The ChangeEvent was the one place a date went missing in silence.
        //
        // WARNS, NEVER REFUSES: the event above is already written, and an
        // undated change is a true state of the record. Naming the COST rather
        // than the absence is the difference between this and a bare "no date".
        if req.detected_at.is_none() {
            out["undated"] = JsonValue::String(
                "This ChangeEvent carries no `detected_at`, so nothing can place it in time. \
                 Two readings go quiet as a result: `changelog_view` windows entries BY EPOCH \
                 and orders them by date, and the verification digest orders changes against a \
                 check's `last_run_at` to say whether a run predates a repair. Neither can say \
                 so about this event — they will simply not mention it. Re-send with \
                 `detected_at` (a plain date is enough), or leave it: an undated change is a \
                 true record of one nobody dated, and this is a note rather than a refusal."
                    .to_string(),
            );
        }
        ok_json(out)
    }

    #[tool(
        description = "Record a change to a node in an epoch (snapshots the prior state). \
                       CONVENTION: record the change BEFORE you make it — the snapshot captures \
                       the state as it is now, so calling this afterwards preserves what you \
                       already replaced. TWO QUESTIONS, NOT ONE: `change_type` says WHY, and \
                       `subject` says WHICH AXIS — `system` (the thing changed) or `record` \
                       (the thing did not change and only the design's knowledge of it did, \
                       e.g. a re-sync or a drift you are accepting). Leaving `subject` out is \
                       a true answer and is never inferred from `change_type`. \
                       ⭐ CHANGING SOMETHING THAT ALREADY EXISTS? LOAD THE `impact-check` \
                       SKILL FIRST (`get_skill`) — it propagates from what you are about to \
                       touch and shows the blast radius, so you edit what is actually \
                       affected and learn what rotted before the edit, not after.",
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
        let target_type = self
            .resolve_type(req.target_type.as_deref(), &req.target_id, "target_type")
            .await?;
        // CLAUSE (b) OF `req:a-fix-says-whether-it-corrected-the-cause`,
        // enforced at the only place a caller can get it wrong. The core type
        // makes a containment-without-a-stand-in unconstructable; this is the
        // JSON boundary, where the two arrive as separate optional fields and
        // the pairing has to be checked. REFUSED BOTH WAYS: a containment with
        // nothing named is the invisible debt the requirement exists to end,
        // and a stand-in without a containment is a claim about a fix that says
        // it reached its cause — incoherent, and the likelier typo.
        let repair = match (req.repair.as_deref(), req.stands_in_for) {
            (None, None) => None,
            (Some("corrected_cause"), None) => Some(Repair::CorrectedCause),
            (Some("contained_symptom"), Some(stands_in_for)) => {
                Some(Repair::ContainedSymptom { stands_in_for })
            }
            (Some("contained_symptom"), None) => {
                return Err(McpError::invalid_params(
                    "`repair: contained_symptom` needs `stands_in_for`: what would the proper fix \
                     be? A patch nobody wrote down is indistinguishable from a design decision six \
                     weeks later, which is the cost this field exists to make visible. Say it in a \
                     sentence — or pass `corrected_cause` if this fix did reach the cause.",
                    None,
                ));
            }
            (None, Some(_)) | (Some("corrected_cause"), Some(_)) => {
                return Err(McpError::invalid_params(
                    "`stands_in_for` only means something with `repair: contained_symptom` — it \
                     names the fix a WORKAROUND is standing in for. A repair that corrected its \
                     cause stands in for nothing, and one that said nothing has not claimed to be \
                     either.",
                    None,
                ));
            }
            (Some(other), _) => {
                return Err(McpError::invalid_params(
                    format!(
                        "unknown repair '{other}'. Legal values: corrected_cause, \
                         contained_symptom. Omitting it is also a true answer and means nobody said."
                    ),
                    None,
                ));
            }
        };
        let rec = ChangeRecord {
            epoch_id: &req.epoch_id,
            change_event_id: &req.change_event_id,
            name: &req.name,
            target_type: &target_type,
            target_id: &req.target_id,
            change_type,
            subject,
            action,
            repair,
        };
        let mut g = self.write_lock().await?;
        let (prior, current) = g.record_change(rec).map_err(dyno_err)?;
        ok_json(json!({
            "prior_snapshot": prior.map(NodeDto::from),
            "current": NodeDto::from(current),
        }))
    }
}
