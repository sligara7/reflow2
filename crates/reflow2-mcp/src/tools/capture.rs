//! `capture` tools — one slice of the MCP surface.
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

/// How many near matches to report. Small on purpose: a list nobody reads
/// every time is worse than no list, which is why `unexpected_coupling` was
/// retired as a gap.
const NEAR_MATCH_LIMIT: usize = 3;

/// How close to the NEW NODE'S OWN score a hit must be to count as near.
///
/// This is the whole reason the check can be honest. BM25 scores are NOT
/// comparable across queries — measured on this design, the same topic scored
/// 4.48 and 35.75 for two different phrasings — so no absolute threshold means
/// anything. Within ONE query against ONE corpus they are comparable, and the
/// node just written is in that corpus and ranks for its own text. So the
/// baseline is the node itself, and every comparison stays inside a single
/// query.
const NEAR_MATCH_RATIO: f32 = 0.5;

/// How many hits to ask the index for before filtering. Wider than the report
/// so the self-hit is unlikely to fall outside the window.
const NEAR_MATCH_WINDOW: usize = 12;

/// Below this many words, the check DECLINES TO JUDGE.
///
/// Found by CI, on an unrelated test that writes `req:from-a` "Session A wrote
/// this." and `req:from-b` "Session B wrote this." — refused as near-duplicates,
/// which is plainly wrong to any reader.
///
/// The reason is not a bad threshold, it is an absent signal: two near-empty
/// documents share most of their tokens BY CONSTRUCTION, so the ratio compares
/// noise to noise and always fires. That is the genesis case as well — a young
/// design is all short statements, and a check that refused every early
/// requirement would be turned off on day one.
///
/// So this declines rather than guesses, which is the same answer
/// `served_by.stale` gives with no `/proc` and `search_first` gives with no
/// in-query baseline: "I cannot tell" is a real result and must not be dressed
/// up as "nothing similar".
const NEAR_MATCH_MIN_WORDS: usize = 12;

/// What the design already holds that reads like what was just written.
#[derive(serde::Serialize)]
pub(crate) struct NearMatch {
    node_id: String,
    node_type: String,
    name: String,
}

/// The advisory attached to a newly-created capture node: what already looked
/// similar, or an honest statement that the check could not run.
///
/// # Why this exists at the TOOL and not in a skill
///
/// "Search before you add" is already written into FIVE served skills
/// (capture-intent, revise-design, brainstorm, kpp-proposal,
/// retire-from-design), and it still only fires when somebody loads one.
/// `req:skill-use-survives-a-long-session` (accepted) says skill use must be
/// triggered by the situation and "THE USER MUST NEVER BE THE TRIGGER" —
/// measured on this repo 2026-07-31 and confirmed from the field 2026-08-11 by
/// a second user who named exactly the two skills he types. This is
/// `req:a-discipline-is-delivered-at-the-tool-not-in-a-catalogue` applied to
/// the one discipline whose absence produces near-duplicate nodes.
///
/// # What it never does
///
/// It never refuses a write and never merges anything. A near-duplicate is
/// sometimes CORRECT — `req:accreted-intent-becomes-a-design` exists because a
/// body of intent that contradicts itself is still a design, and somebody
/// saying the same thing twice in different words is signal, not error. So this
/// reports and the human decides, which is `dec:three-party-checks`.
#[derive(serde::Serialize)]
pub(crate) struct SearchFirst {
    /// Existing nodes whose text overlaps. Empty means the check ran and found
    /// nothing close — a real answer, not an absence.
    near_matches: Vec<NearMatch>,
    /// Set only when the check COULD NOT run. Distinguishing "nothing similar"
    /// from "I could not look" is `req:no-silent-fallback`: an empty list that
    /// might mean either would be the more reassuring reading and the wrong one.
    #[serde(skip_serializing_if = "Option::is_none")]
    unavailable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// Look for what the design already says, using the new node as its own
/// baseline. Returns `None` when the node id already existed — re-calling a
/// constructor is a deliberate REVISION (BL-183), and reporting a node's
/// resemblance to itself would be noise on every edit.
pub(crate) fn search_first(
    g: &DesignGraph,
    node_id: &str,
    existed_before: bool,
    text: &str,
) -> Option<SearchFirst> {
    if existed_before {
        return None;
    }
    if text.split_whitespace().count() < NEAR_MATCH_MIN_WORDS {
        // Not enough text to carry a judgement. Silent by design: an
        // `unavailable` note on every short capture would be the noise this
        // check exists to avoid, and there is nothing actionable to say.
        return None;
    }
    // Deliberately NOT scoped to node_type: a Requirement and a Capability can
    // say the same thing, and that pair is the one worth catching.
    let result = match g.search_design(text, None, NEAR_MATCH_WINDOW) {
        Ok(r) => r,
        Err(e) => {
            return Some(SearchFirst {
                near_matches: Vec::new(),
                unavailable: Some(format!("could not search the design: {e}")),
                note: Some(
                    "Nothing is claimed about whether this duplicates something. The design was \
                     not searched."
                        .into(),
                ),
            });
        }
    };
    let Some(self_score) = result
        .hits
        .iter()
        .find(|h| h.node_id == node_id)
        .map(|h| h.score)
    else {
        // The node did not rank for its own text, so there is no baseline and
        // any comparison would be across-query. Say so rather than guess.
        return Some(SearchFirst {
            near_matches: Vec::new(),
            unavailable: Some(
                "the new node did not rank for its own text, so there was no in-query baseline to \
                 compare against"
                    .into(),
            ),
            note: None,
        });
    };
    let floor = self_score * NEAR_MATCH_RATIO;
    let near: Vec<NearMatch> = result
        .hits
        .iter()
        .filter(|h| h.node_id != node_id && h.score >= floor)
        .take(NEAR_MATCH_LIMIT)
        .map(|h| NearMatch {
            node_id: h.node_id.clone(),
            node_type: h.node_type.clone(),
            name: h.name.clone(),
        })
        .collect();
    let note = (!near.is_empty()).then(|| {
        "The design already holds these, and their wording overlaps what you just wrote. THIS IS \
         NOT A DUPLICATE CLAIM — read them and decide. If one covers the same need, revise or \
         link it instead (revise-design); if the overlap is only wording, ignore this."
            .to_string()
    });
    Some(SearchFirst {
        near_matches: near,
        unavailable: None,
        note,
    })
}

/// One property this call overwrote, and what it said before.
///
/// **The prior value is echoed in full, deliberately.** A hash tells a caller
/// that something was lost; only the value lets them put it back. The whole
/// reason this block exists is that the prior text was unrecoverable — a
/// summary would reproduce the failure it is meant to end.
#[derive(serde::Serialize)]
pub(crate) struct ReplacedField {
    field: String,
    prior: JsonValue,
}

/// What a constructor call did to a node that was ALREADY THERE.
///
/// # Why this exists
///
/// `search_first` deliberately goes quiet on a revision (see its docs: a node's
/// resemblance to itself is noise). Nothing filled that silence, so a merge onto
/// an existing id and a fresh create returned **the same shape** — no signal that
/// anything was replaced, and no prior value.
///
/// Reported four times by three agents across two versions and three projects
/// before this was written: `add_constraint` twice on one id overwrote a
/// multi-paragraph statement ("I lost the prior text and could not honestly
/// reconstruct it"); a `record_change` snapshot taken after a sibling merge
/// stored the NEW statement as the prior one ("the timeline for that revision is
/// a lie"); an accepted Decision was widened from a debugging hypothesis and the
/// user had to walk it back; and a malformed payload replaced a Decision's text
/// while replying exactly like a create. Every one of those was caught — when it
/// was caught at all — by an agent reading the echoed properties by hand.
///
/// # What it never does
///
/// It never refuses and never rolls back. Re-calling a constructor to sharpen a
/// node is CORRECT and is what `revise-design` tells you to do; the merge is not
/// the defect. Saying nothing about it is. So this reports and the caller
/// decides, which is `dec:three-party-checks` — the same posture as
/// `search_first` next to it.
#[derive(serde::Serialize)]
pub(crate) struct Revision {
    /// Properties whose value this call CHANGED, with what they said before.
    /// Empty with `changed: false` means the call rewrote the node with what it
    /// already held.
    replaced: Vec<ReplacedField>,
    /// Properties this call introduced. Additive, nothing lost — separated from
    /// `replaced` because conflating them would make every ordinary enrichment
    /// look like an overwrite.
    added: Vec<String>,
    /// Whether this call changed anything at all. A revision that changed
    /// nothing and one that replaced a paragraph are currently the same reply,
    /// which is the `wrote nothing` / `wrote something` ambiguity reported
    /// against `export_graph` in the same week.
    changed: bool,
    /// sha256 over the node's properties as they stood BEFORE this call,
    /// canonically serialised. Lets a caller prove a restore landed.
    prior_content_hash: String,
    /// The Snapshot holding the state this call replaced, when one exists.
    ///
    /// **Computed, not assumed.** The note below used to state the
    /// snapshot-first rule unconditionally, whether or not the caller had
    /// followed it — advice that never varies is advice a reader learns to
    /// skim. `req:a-discipline-is-delivered-at-the-tool-not-in-a-catalogue`
    /// asks for the outcome to be computed instead, because that survives an
    /// agent which ignores every hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    prior_state_preserved_in: Option<String>,
    note: String,
}

/// Canonical sha256 of a property map — keys sorted, compact separators, so the
/// same properties always hash the same regardless of map ordering.
fn properties_hash(props: &HashMap<String, Value>) -> String {
    // DELEGATED TO THE CORE since 2026-08-17, and the delegation is the point
    // rather than tidiness. This value became a PRECONDITION when
    // `create_node` gained `expected_content_hash`: the number reported here is
    // the number the engine compares against. Two implementations of one hash
    // would let a caller's correct expectation be refused by an engine that
    // agreed with them — and the divergence would appear only under
    // contention, which is the one condition nobody tests by hand.
    reflow2_core::node_content_hash(props)
}

/// Compare what a node held before a constructor call against what it holds
/// now. `None` when there was no prior node — a create has nothing to report,
/// and a block present and empty on every create is the noise `search_first`
/// already refuses to become.
pub(crate) fn revision_of(
    g: &DesignGraph,
    prior: Option<&StoredNode>,
    now: &NodeDto,
) -> Option<Revision> {
    let prior = prior?;
    let mut replaced = Vec::new();
    let mut added: Vec<String> = Vec::new();

    for (key, new_value) in &now.properties {
        match prior.properties.get(key) {
            Some(old) if old != new_value => replaced.push(ReplacedField {
                field: key.clone(),
                prior: serde_json::to_value(old).unwrap_or(JsonValue::Null),
            }),
            Some(_) => {}
            None => added.push(key.clone()),
        }
    }
    replaced.sort_by(|a, b| a.field.cmp(&b.field));
    added.sort();

    let changed = !replaced.is_empty() || !added.is_empty();
    let prior_content_hash = properties_hash(&prior.properties);
    // THE COMPUTED HALF. Ask the graph whether the state being replaced is
    // preserved, instead of restating the snapshot-first rule at a caller who
    // may already have followed it.
    let preserved = g
        .snapshot_preserving(&prior.node_id, &prior_content_hash)
        .ok()
        .flatten();

    let note = if replaced.is_empty() && changed {
        "This call added properties to an existing node and overwrote nothing.".to_string()
    } else if !changed {
        "This node already held exactly what this call passed; nothing moved.".to_string()
    } else if let Some(snapshot) = &preserved {
        format!(
            "This call REPLACED {} propert{} on a node that already existed — and the state \
             it replaced IS PRESERVED, in `{snapshot}`. Nothing is lost and the history is \
             right, so there is nothing to do about it.",
            replaced.len(),
            if replaced.len() == 1 { "y" } else { "ies" },
        )
    } else {
        format!(
            "This call REPLACED {} propert{} on a node that already existed, AND NO SNAPSHOT \
             HOLDS THE STATE IT REPLACED — checked, not assumed. The prior value{} above {} \
             now the only copy in existence, and this reply is the only place {} appears. \
             `record_change` BEFORE the merge is what puts the old state in the design's own \
             timeline; called now it would snapshot the REPLACEMENT and the history would be \
             wrong. To undo: write the prior value{} back, then record_change, then re-apply.",
            replaced.len(),
            if replaced.len() == 1 { "y" } else { "ies" },
            if replaced.len() == 1 { "" } else { "s" },
            if replaced.len() == 1 { "is" } else { "are" },
            if replaced.len() == 1 { "it" } else { "they" },
            if replaced.len() == 1 { "" } else { "s" },
        )
    };

    Some(Revision {
        replaced,
        added,
        changed,
        prior_content_hash,
        prior_state_preserved_in: preserved,
        note,
    })
}

/// Attach the advisory blocks a capture result carries — `search_first` when a
/// create resembles something already there, `revision` when the call landed on
/// a node that already existed.
///
/// The two are mutually exclusive by construction and that is the design:
/// `search_first` answers *should this be a new node at all*, `revision`
/// answers *what did writing it cost*. A create can only face the first
/// question and a revision can only face the second.
pub(crate) fn with_capture_notes<T: serde::Serialize>(
    value: T,
    hint: &str,
    found: Option<SearchFirst>,
    revision: Option<Revision>,
) -> Result<CallToolResult, McpError> {
    let mut v = serde_json::to_value(value).map_err(ser_err)?;
    if let Some(obj) = v.as_object_mut() {
        obj.insert("loop_hint".into(), JsonValue::String(hint.to_string()));
        if let Some(sf) = found {
            // Only speak when there is something to say: a block that is
            // present and empty on every single create is the noise this is
            // trying not to become.
            if !sf.near_matches.is_empty() || sf.unavailable.is_some() {
                obj.insert(
                    "search_first".into(),
                    serde_json::to_value(sf).map_err(ser_err)?,
                );
            }
        }
        if let Some(rev) = revision {
            // Always emitted on a revision, INCLUDING when nothing changed.
            // "Your merge was a no-op" is exactly as worth knowing as "your
            // merge replaced a paragraph", and the two are indistinguishable
            // without it.
            obj.insert(
                "revision".into(),
                serde_json::to_value(rev).map_err(ser_err)?,
            );
        }
    }
    ok_json(v)
}

/// Force the choice Anthony named: **sharpen the existing node, or say why this
/// one is different.** Reporting alone is not enough, because by the time a
/// report is read the near-duplicate already exists.
///
/// # The shape, and why it is this shape
///
/// This is the TWO-SIDED ACCEPT (BL-33), which `partnership.md` describes as
/// forcing "the uncomfortable question at the exact moment an agreeable agent
/// would glide past it". `set_artifact_checksum` already works this way: it
/// refuses a first baseline recorded as `design_holds` and NAMES
/// `baseline_established` as the way through. A refusal that names what would
/// have worked is `req:a-refusal-names-what-would-have-worked`.
///
/// # Both routes already exist, which is why no new vocabulary is needed
///
/// * SHARPEN — call the same constructor with the EXISTING id. Constructors
///   merge (BL-183), so what you pass overwrites and what you omit survives.
///   A revision is exempt from this check by construction.
/// * CREATE ANYWAY — pass `distinct_from` naming the ids you read and rejected.
///   That is the deliberate decision, and it is recorded in the call rather
///   than assumed from silence.
///
/// # What it must never do
///
/// It must never refuse on a WEAK resemblance. The bar is
/// [`NEAR_MATCH_RATIO`], measured conservative: a moderately-similar pair did
/// not clear it during development, which is the behaviour wanted — a check
/// that refused often would be routed around, and a rule people route around
/// is worse than no rule.
fn refuse_unless_deliberate(
    found: &Option<SearchFirst>,
    distinct_from: Option<&Vec<String>>,
    node_id: &str,
) -> Result<(), McpError> {
    let Some(sf) = found else { return Ok(()) };
    if sf.near_matches.is_empty() {
        return Ok(());
    }
    let acknowledged: std::collections::HashSet<&str> = distinct_from
        .map(|v| v.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let unacknowledged: Vec<&NearMatch> = sf
        .near_matches
        .iter()
        .filter(|m| !acknowledged.contains(m.node_id.as_str()))
        .collect();
    if unacknowledged.is_empty() {
        return Ok(());
    }
    let listed = unacknowledged
        .iter()
        .map(|m| format!("  {} ({}) — {}", m.node_id, m.node_type, m.name))
        .collect::<Vec<_>>()
        .join("\n");
    let ids = unacknowledged
        .iter()
        .map(|m| format!("\"{}\"", m.node_id))
        .collect::<Vec<_>>()
        .join(", ");
    Err(McpError::invalid_params(
        format!(
            "The design already says something close to this, so `{node_id}` was NOT created. \
             Read these and decide — sharpening an existing node or starting a new one are \
             different acts, and this is the moment to choose:\n\n{listed}\n\nTWO WAYS ON, both \
             deliberate:\n  SHARPEN — call this same tool with the EXISTING id. Constructors \
             merge, so what you pass overwrites and what you omit survives; nothing is lost and \
             the new detail lands on the node that already holds the idea.\n  START A NEW ONE — \
             call again with `distinct_from: [{ids}]`, which records that you read them and \
             judged this different.\n\nThis is not a duplicate accusation. Saying the same thing \
             twice in different words is sometimes real signal, which is why the second route \
             exists and why nothing was merged for you."
        ),
        None,
    ))
}

#[tool_router(router = capture_router, vis = "pub")]
impl ReflowService {
    // ---- GENESIS (bootstrap the graph from a brief) ----

    #[tool(
        description = "Bootstrap the design graph: create the Project + a genesis Epoch anchor \
                       and return a next-steps checklist. Guarded and idempotent — a no-op that \
                       reports already_initialized if a Project exists (unless rescan). Call this \
                       first, then seed the brief into Requirements/Capabilities via the add_* \
                       tools and run detect_gaps.",
        annotations(read_only_hint = false)
    )]
    pub async fn genesis(
        &self,
        Parameters(req): Parameters<GenesisReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = GenesisOptions {
            project_id: req.project_id,
            name: req.name,
            domain: req.domain,
            objective: req.objective,
            mode: req.mode,
            rescan: req.rescan,
        };
        let mut g = self.write_lock().await;
        ok_json(g.genesis(opts).map_err(dyno_err)?)
    }

    // ---- Golden-thread constructors (deterministic, mutating) ----

    #[tool(
        description = "Create a Project node.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_project(
        &self,
        Parameters(req): Parameters<IdName>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.add_project(&req.id, &req.name).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create a Requirement node. A new one lands at `proposed`; only the \
                       user's word moves it off, through set_requirement_status. CALLING THIS \
                       AGAIN WITH AN EXISTING ID REVISES that node: what you pass overwrites, \
                       and every field you do NOT pass keeps its current value instead of \
                       reverting to a default — so rewording a requirement never silently \
                       un-confirms it (BL-183).",
        annotations(read_only_hint = false)
    )]
    pub async fn add_requirement(
        &self,
        Parameters(req): Parameters<RequirementReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let node_ty = reflow2_core::nodes::node::REQUIREMENT;
        let prior = g.get_node(node_ty, &req.id).map_err(dyno_err)?;
        let existed = prior.is_some();
        let node = NodeDto::from(
            g.add_requirement(&req.id, &req.name, &req.statement)
                .map_err(dyno_err)?,
        );
        let found = search_first(
            &g,
            &req.id,
            existed,
            &format!("{} {}", req.name, req.statement),
        );
        if let Err(e) = refuse_unless_deliberate(&found, req.distinct_from.as_ref(), &req.id) {
            // Roll back the node we just wrote. The check needs the node in the
            // index to have an in-query baseline, so the write comes first and
            // is undone when the caller has not yet chosen. Only ever undoes a
            // node THIS call created — `existed` guards a revision.
            if !existed {
                let _ = g.delete_node(node_ty, &req.id);
            }
            return Err(e);
        }
        let revision = revision_of(&g, prior.as_ref(), &node);
        with_capture_notes(
            node,
            "loop: when this capture batch lands, run detect_gaps (detect-and-ask) — \
             loop_status says what's owed",
            found,
            revision,
        )
    }

    #[tool(
        description = "Record what a Capability TAKES IN and PUTS OUT \u{2014} its functional \
                       signature, which is the black-box interface at that tier \
                       (`req:recursive-black-box-decomposition`: every element of a design is a \
                       black box with inner function AND INTERFACES). Pass `inputs` / `outputs` as \
                       lists; the JSON encoding the schema stores is done for you. \
                       `capability_type` is free text \u{2014} validation / transform / query / \
                       persistence / decision / actuation / io / compute \u{2014} deliberately \
                       domain-neutral so a biology or hardware design is not forced into software \
                       words. OMITTING A FIELD LEAVES IT ALONE, so two people describing the same \
                       thing from opposite ends cannot overwrite each other; AN EMPTY LIST IS A \
                       STATEMENT (\"takes nothing in\" is a real claim about a source or a \
                       generator) and is distinct from nobody having said; and an unknown id is \
                       REFUSED rather than created.",
        annotations(read_only_hint = false, idempotent_hint = true)
    )]
    pub async fn set_capability_signature(
        &self,
        Parameters(req): Parameters<CapabilitySignatureReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_capability_signature(
                &req.capability_id,
                req.capability_type.as_deref(),
                req.inputs.as_deref(),
                req.outputs.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create a Capability node. `status` defaults to `planned`; set it when \
                       recording something that already exists, so adopting a running system \
                       does not describe it as entirely unbuilt. CALLING THIS AGAIN WITH AN \
                       EXISTING ID REVISES that node: what you pass overwrites, and every field \
                       you do NOT pass keeps its current value instead of reverting to a default \
                       — so sharpening a description never silently unbuilds a verified \
                       capability (BL-183).",
        annotations(read_only_hint = false)
    )]
    pub async fn add_capability(
        &self,
        Parameters(req): Parameters<CapabilityReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let node_ty = reflow2_core::nodes::node::CAPABILITY;
        let prior = g.get_node(node_ty, &req.id).map_err(dyno_err)?;
        let existed = prior.is_some();
        let node = NodeDto::from(
            g.add_capability(&req.id, &req.name, &req.description, req.status.as_deref())
                .map_err(dyno_err)?,
        );
        let found = search_first(
            &g,
            &req.id,
            existed,
            &format!("{} {}", req.name, req.description),
        );
        if let Err(e) = refuse_unless_deliberate(&found, req.distinct_from.as_ref(), &req.id) {
            // Roll back the node we just wrote. The check needs the node in the
            // index to have an in-query baseline, so the write comes first and
            // is undone when the caller has not yet chosen. Only ever undoes a
            // node THIS call created — `existed` guards a revision.
            if !existed {
                let _ = g.delete_node(node_ty, &req.id);
            }
            return Err(e);
        }
        let revision = revision_of(&g, prior.as_ref(), &node);
        with_capture_notes(
            node,
            "loop: wire satisfies to the requirement this serves, then run detect_gaps when \
             the capture batch lands (detect-and-ask)",
            found,
            revision,
        )
    }

    #[tool(
        description = "Set a Requirement's lifecycle status: `proposed` (the default) / \
                       `accepted` / `deferred` / `dropped` / `met`. Every move off `proposed` \
                       records the USER's word, never your own judgment: capture at `proposed` \
                       and move the status only when the user has actually confirmed, deferred \
                       or dropped it — certainty is derived from this status, so promoting it \
                       yourself forges their signature (dec:certainty-derived). A `dropped` or \
                       `met` requirement stops raising unsatisfied_requirement.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_requirement_status(
        &self,
        Parameters(req): Parameters<RequirementStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_requirement_status(&req.requirement_id, &req.status)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Choose how much this project lets a machine change its design on its own: \
                       `flexible` (apply_heal applies structural repairs) or `rigid` (apply_heal \
                       proposes them and stops, so a human decides). That one gate is ALL the \
                       mode currently changes — said plainly because the older schema wording, \
                       \"design is the source of truth\", promised a breadth the code does not \
                       implement. ASK THE USER; do not pick for them. Until 2026-07-30 the mode \
                       could only be set at genesis, so every design ever made carried the \
                       `flexible` DEFAULT and could never move off it — a governance choice \
                       nobody made and nobody could revisit. The default records that nobody \
                       has chosen, not that flexible was chosen (req:mode-is-chosen-and-changeable).",
        annotations(read_only_hint = false)
    )]
    pub async fn set_project_mode(
        &self,
        Parameters(req): Parameters<ProjectModeReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_project_mode(&req.project_id, &req.mode)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Set a Capability's lifecycle status: `planned` (the default) / \
                       `in_progress` / `realized` / `verified`. Use it as a capability moves \
                       through its life; to record one that already ships, pass `status` to \
                       add_capability instead and save a write.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_capability_status(
        &self,
        Parameters(req): Parameters<CapabilityStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_capability_status(&req.capability_id, &req.status)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record how a node entered the graph: `authored` (the default, someone \
                       stated it) / `planned` / `inferred` (read back out of an existing system) \
                       / `healed` / `reconciled` / `imported`. Accepted on Requirement, \
                       Capability, Component and Interface. Mark inferred requirements as such — \
                       a requirement backed out of the code that implements it is satisfied by \
                       construction and cannot contradict anything, and a reader has no other way \
                       to tell. For bulk adoption prefer import_graph, which carries this at \
                       create time.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_provenance(
        &self,
        Parameters(req): Parameters<ProvenanceReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_provenance(&req.node_type, &req.node_id, &req.provenance)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create a Component node. Pass `level` when the part is an assembly \
                       rather than a leaf (`subsystem`, `system`, `system_of_systems`, \
                       `enterprise`; default `component`), then use contain_component to nest \
                       it — that pair is what gives hierarchy_issues something to check.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_component(
        &self,
        Parameters(req): Parameters<ComponentReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let node_ty = reflow2_core::nodes::node::COMPONENT;
        let prior = g.get_node(node_ty, &req.id).map_err(dyno_err)?;
        let existed = prior.is_some();
        let node = NodeDto::from(
            g.add_component(&req.id, &req.name, &req.description, req.level.as_deref())
                .map_err(dyno_err)?,
        );
        let found = search_first(
            &g,
            &req.id,
            existed,
            &format!("{} {}", req.name, req.description),
        );
        if let Err(e) = refuse_unless_deliberate(&found, req.distinct_from.as_ref(), &req.id) {
            // Roll back the node we just wrote. The check needs the node in the
            // index to have an in-query baseline, so the write comes first and
            // is undone when the caller has not yet chosen. Only ever undoes a
            // node THIS call created — `existed` guards a revision.
            if !existed {
                let _ = g.delete_node(node_ty, &req.id);
            }
            return Err(e);
        }
        let revision = revision_of(&g, prior.as_ref(), &node);
        with_capture_notes(
            node,
            "loop: structural change — run detect_defects (check-health) when the batch lands",
            found,
            revision,
        )
    }

    #[tool(
        description = "Nest one Component inside another (parent CONTAINS child) — the assembly \
                       spine. The parent should sit exactly one level above the child: nesting \
                       two components at the same level is reported as a level_mismatch, and \
                       skipping a level as a missing_intermediate_level. Set `level` on both via \
                       add_component first, or every containment looks like a mismatch.",
        annotations(read_only_hint = false)
    )]
    pub async fn contain_component(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.contain_component(&req.from_id, &req.to_id)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Link a Capability to a Requirement it SATISFIES.",
        annotations(read_only_hint = false)
    )]
    pub async fn satisfies(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.satisfies(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Split a Requirement into a smaller one: `from_id` DECOMPOSES `to_id`. Use \
                       when a child is a 1:1 piece of its parent adding NO new information (\"the \
                       app must have a checkout system\" → enter-a-card, apply-a-discount, \
                       receive-a-receipt). Delivery rolls UP this edge: the parent is delivered \
                       when EVERY child is, so a decomposed parent needs no capability of its own. \
                       Do NOT use for a requirement that adds new technical necessity nobody asked \
                       for — that is *derived*, it belongs to the Decision that forced it \
                       (set_requirement_lineage `derived` + governed_by), and re-opening that \
                       decision may remove its reason to exist. Marks the child `decomposed`. \
                       Refuses a cycle: a tree that contains itself has no leaves and could never \
                       roll up.",
        annotations(read_only_hint = false)
    )]
    pub async fn decomposes(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.decomposes(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Set where a Requirement came from — `original` (the stakeholder's own \
                       word), `decomposed` (a 1:1 split of a parent, normally set for you by \
                       `decomposes`), or `derived` (technical necessity nobody asked for, created \
                       by a design decision — pair it with governed_by to that Decision). Distinct \
                       from `provenance`, which says how the node entered the graph rather than \
                       where the need came from. The classes behave differently: delivery rolls up \
                       a decomposition, and a derived requirement may lose its reason to exist if \
                       the decision behind it is re-opened.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_requirement_lineage(
        &self,
        Parameters(req): Parameters<RequirementLineageReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_requirement_lineage(&req.requirement_id, &req.lineage)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Allocate a Capability to a Component (ALLOCATED_TO).",
        annotations(read_only_hint = false)
    )]
    pub async fn allocate(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.allocate(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create an Interface node — a contract between parts (an API, event, \
                       data feed, CLI, library boundary, or physical/human connection point). \
                       Model one whenever two Components talk to each other, then pair it with \
                       `provides` and `consumes`: that pairing is what makes a change on one \
                       side of a boundary surface the other side.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_interface(
        &self,
        Parameters(req): Parameters<IdName>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let node_ty = reflow2_core::nodes::node::INTERFACE;
        let prior = g.get_node(node_ty, &req.id).map_err(dyno_err)?;
        let existed = prior.is_some();
        let node = NodeDto::from(g.add_interface(&req.id, &req.name).map_err(dyno_err)?);
        let found = search_first(&g, &req.id, existed, &req.name);
        let revision = revision_of(&g, prior.as_ref(), &node);
        with_capture_notes(
            node,
            "loop: structural change — wire provides/consumes, then run detect_defects \
             (check-health) when the batch lands",
            found,
            revision,
        )
    }

    #[tool(
        description = "Create a Flow — an ordered process linking Capabilities end to end (a \
                       user journey, an assembly sequence, an operating loop). Attach each step \
                       with `part_of_flow` (+ step_order); join steps with TRIGGERS edges via \
                       `create_edge`, giving each a `role` property saying what the transition \
                       means ('feeds', 'forces resync') — in a process the backward edges are \
                       the point, and without a role they are indistinguishable from forward \
                       ones. Read it back with `flow_report`.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_flow(
        &self,
        Parameters(req): Parameters<AddFlowReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.add_flow(
                &req.id,
                &req.name,
                req.description.as_deref(),
                req.flow_type.as_deref(),
                req.entry_point.as_deref(),
                req.exit_point.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record that a Capability is a step of a Flow (PART_OF_FLOW), with its \
                       position (`step_order`). A step without one is listed after the ordered \
                       steps, and `flow_report` says so rather than inventing an order.",
        annotations(read_only_hint = false)
    )]
    pub async fn part_of_flow(
        &self,
        Parameters(req): Parameters<PartOfFlowReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.part_of_flow(&req.capability_id, &req.flow_id, req.step_order)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Read a Flow back as facts: steps in stated order, the TRIGGERS \
                       transitions among them with their roles, and the cycles. Cycles are \
                       REPORTED, never judged — a process's loops are its design, so they do \
                       not appear in detect_defects (whose circular_dependency stays scoped to \
                       DEPENDS_ON and contracts, where a cycle really is a defect). Anything \
                       the model left unstated (an unmatched entry/exit point, steps without \
                       step_order, transitions without a role) is confessed by name.",
        annotations(read_only_hint = true)
    )]
    pub async fn flow_report(
        &self,
        Parameters(req): Parameters<FlowReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.flow_report(&req.flow_id).map_err(dyno_err)?)
    }

    #[tool(
        description = "Record that a Component PROVIDES an Interface — it is the side that \
                       implements the contract. `from_id` is the Component, `to_id` the Interface.",
        annotations(read_only_hint = false)
    )]
    pub async fn provides(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.provides(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record that a Component CONSUMES an Interface — it is the side that \
                       depends on the contract. `from_id` is the Component, `to_id` the \
                       Interface. Once both sides are recorded, `propagate_change` on either \
                       Component reaches the other, and `detect_gaps` reports a contract that \
                       is consumed but never provided.",
        annotations(read_only_hint = false)
    )]
    pub async fn consumes(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.consumes(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Link a Project to a child node it CONTAINS.",
        annotations(read_only_hint = false)
    )]
    pub async fn contains(
        &self,
        Parameters(req): Parameters<ContainsReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.contains(&req.project_id, &req.child_type, &req.child_id)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create a Constraint — a limit the design must respect, vs a Requirement \
                       which is a goal to achieve. For a numeric budget (BL-11) set `quantity` \
                       (unit-bearing name like mass_kg / latency_ms / cost_usd), `limit`, and \
                       `direction` (maximum = stay at or under, the default). Then attach the \
                       spenders with `constrains` and read the rollup with `budget_report`. \
                       `category: kpp` marks a KEY PERFORMANCE PARAMETER — inviolable intent, a \
                       threshold that if missed fails the whole effort — and its violations are \
                       computed and ranked above ordinary gaps. On a kpp, `limit` is the \
                       threshold and `objective` is what success looks like. Never set kpp on \
                       your own reading of the wording: criticality is a claim about \
                       consequence, so ask the user first (the kpp-proposal skill).",
        annotations(read_only_hint = false)
    )]
    pub async fn add_constraint(
        &self,
        Parameters(req): Parameters<AddConstraintReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let node_ty = reflow2_core::nodes::node::CONSTRAINT;
        let prior = g.get_node(node_ty, &req.id).map_err(dyno_err)?;
        let existed = prior.is_some();
        let node = NodeDto::from(
            g.add_constraint(
                &req.id,
                &req.name,
                &req.statement,
                req.category.as_deref(),
                req.quantity.as_deref(),
                req.limit,
                req.objective,
                req.direction.as_deref(),
            )
            .map_err(dyno_err)?,
        );
        let found = search_first(
            &g,
            &req.id,
            existed,
            &format!("{} {}", req.name, req.statement),
        );
        let revision = revision_of(&g, prior.as_ref(), &node);
        with_capture_notes(
            node,
            "loop: a Constraint binds what it CONSTRAINS — wire it, then run detect_gaps",
            found,
            revision,
        )
    }

    #[tool(
        description = "Give an Interface its external ROLE, which is what makes composition \
                       computable: `published` (this design OFFERS the contract and others may \
                       rely on it), `required` (this design NEEDS one of these FROM OUTSIDE), \
                       `both` (rare, and therefore meaningful), or `internal` (plumbing its owner \
                       may change freely). An Interface is internal until someone says otherwise, \
                       because publishing is a commitment. `published` is the distinction a \
                       systems-engineering ICD publishes and that MOSA calls a modular system \
                       interface. THE ROLE IS ON THE INTERFACE, NOT THE COMPONENT: a component \
                       both publishes and subscribes, so a per-node role collapses to `both` and \
                       pairs with everything (dec:pairing-role-placement). It is READ, not just \
                       stored: propagate reports which published boundaries a change crosses so \
                       \"is this part severable\" is computed instead of asserted, and pair_designs \
                       matches `published`/`both` against `required`/`both` to compute a seam. \
                       NOT a claim the boundary has held; whether it stayed stable is its drift \
                       history.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_interface_designation(
        &self,
        Parameters(req): Parameters<InterfaceDesignationReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_interface_designation(&req.interface_id, &req.designation)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Designate a Requirement as a PROMISE THIS DESIGN PUBLISHES — a behavioural \
                       commitment a consumer may rely on — or back to INTERNAL intent nobody \
                       outside sees. Use it for the things an ICD states in prose and no \
                       structural export can carry: 'a missing store fails loud rather than \
                       falling back', 'ordering is preserved', 'an empty result means no match, \
                       not an error'. Published requirements travel with export_surface; \
                       everything else is still withheld and still counted. Internal until \
                       someone says otherwise, because publishing is a commitment — the same rule \
                       as set_interface_designation. It is NOT a claim the promise is kept; \
                       whether it held is its verification and drift history.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_requirement_designation(
        &self,
        Parameters(req): Parameters<RequirementDesignationReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_requirement_designation(&req.requirement_id, &req.designation)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record that a Constraint CONSTRAINS a target, with the target's \
                       `contribution` to the budget (in the Constraint's quantity unit) and the \
                       `basis` for the number (estimated/evidence/measured). An edge without a \
                       contribution is reported by budget_report as unstated — never treated as \
                       zero.",
        annotations(read_only_hint = false)
    )]
    pub async fn constrains(
        &self,
        Parameters(req): Parameters<ConstrainsReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.constrains(
                &req.constraint_id,
                &req.target_type,
                &req.target_id,
                req.contribution,
                req.basis.as_deref(),
                req.measured_at.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Roll a budget Constraint up (BL-11): total of stated contributions vs \
                       the limit, the worst dependency path among contributors (the \
                       path-cumulative rollup — end-to-end latency, mass down a chain), basis \
                       coverage (estimated vs measured), and an honest verdict — `incomplete` \
                       when any contribution is unstated, because a partial sum passed off as a \
                       total is how budgets lie. Contributors with no stated number are listed, \
                       never zeroed.",
        annotations(read_only_hint = true)
    )]
    pub async fn budget_report(
        &self,
        Parameters(req): Parameters<BudgetReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.budget_report(&req.constraint_id).map_err(dyno_err)?)
    }

    #[tool(
        description = "Record a Decision and why it was made (an ADR). Use this whenever the user \
                       chooses between real alternatives — the rationale is what stops the choice \
                       being silently reversed later. Link it with `governed_by`. It lands \
                       `proposed`: recording a choice is not the same as settling it, so reaching \
                       `accepted` is a separate act (`set_decision_status`, or `collapse_decision` \
                       when a fork is chosen). That is deliberate — an accepted Decision is what \
                       where-am-i reads back to the user as \"what you decided\", so asserting it \
                       on their behalf would be the forgery dec:certainty-derived forbids for \
                       requirement status. BEHAVIOUR CHANGED 2026-07-25: this used to default to \
                       `accepted`.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_decision(
        &self,
        Parameters(req): Parameters<DecisionReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let node_ty = reflow2_core::nodes::node::DECISION;
        let prior = g.get_node(node_ty, &req.id).map_err(dyno_err)?;
        let existed = prior.is_some();
        let node = NodeDto::from(
            g.add_decision(&req.id, &req.name, &req.decision, req.rationale.as_deref())
                .map_err(dyno_err)?,
        );
        let found = search_first(
            &g,
            &req.id,
            existed,
            &format!("{} {}", req.name, req.decision),
        );
        if let Err(e) = refuse_unless_deliberate(&found, req.distinct_from.as_ref(), &req.id) {
            // Roll back the node we just wrote. The check needs the node in the
            // index to have an in-query baseline, so the write comes first and
            // is undone when the caller has not yet chosen. Only ever undoes a
            // node THIS call created — `existed` guards a revision.
            if !existed {
                let _ = g.delete_node(node_ty, &req.id);
            }
            return Err(e);
        }
        let revision = revision_of(&g, prior.as_ref(), &node);
        with_capture_notes(
            node,
            "loop: a Decision lands `proposed` — only the owner's word moves it \
             (set_decision_status)",
            found,
            revision,
        )
    }

    #[tool(
        description = "Link a node to the Decision or DesignRule that shapes it (GOVERNED_BY). \
                       PASS `ruling: parks` WHEN THE RULING DECLARES THIS NODE'S UNATTACHED OR \
                       UNSATISFIED STATE CORRECT — a registered document that deliberately draws \
                       no claim edges, a requirement an accepted Decision forbids satisfying. \
                       Detectors then report it as PARKED and count it in detect_defects's \
                       `swept.parked`, instead of filing a deliberate state as a defect. \
                       Measured cost of not having this: a fleet watched defects go 88 -> 97 \
                       across ten CORRECT writes, so the right action degraded the instrument \
                       and a later reader had an incentive to stop registering documents at all. \
                       The ruling must be an ACCEPTED Decision — a `proposed` one is somebody \
                       thinking out loud, and a musing must not suppress a finding.",
        annotations(read_only_hint = false)
    )]
    pub async fn governed_by(
        &self,
        Parameters(req): Parameters<GovernedByReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.governed_by(
                &req.from_type,
                &req.from_id,
                &req.to_type,
                &req.to_id,
                req.ruling.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record a Contributor — who authors and decides the DESIGN \
                       itself: a person, an automated coding agent, or an \
                       organization. Distinct from an Actor (add via create_node), \
                       which is who the designed system SERVES. Create one per \
                       session for whoever is driving, then attribute their design \
                       nodes with authored_by — the structured 'who' behind \
                       provenance's 'how'.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_contributor(
        &self,
        Parameters(req): Parameters<ContributorReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.add_contributor(
                &req.id,
                &req.name,
                req.kind.as_deref(),
                req.handle.as_deref(),
                req.description.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Attribute a design node to a Contributor (AUTHORED_BY) — \
                       whose word this Decision/Requirement/… is. `role` is \
                       author (default), reviewer, or approver. This is the \
                       structured author behind a node; it is deliberately not a \
                       traceability edge, so it never enlarges a blast radius. \
                       Record it when a decision is MADE, not at session end — \
                       captured-when-decided is what keeps the authorship honest.",
        annotations(read_only_hint = false)
    )]
    pub async fn authored_by(
        &self,
        Parameters(req): Parameters<AuthoredByReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.authored_by(
                &req.from_type,
                &req.from_id,
                &req.contributor_id,
                req.role.as_deref(),
                req.acted_at.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record whose AREA a node is (OWNED_BY) — durable, standing, and never \
                       released. THE THIRD 'WHO' AXIS: `authored_by` says who WROTE it (past \
                       tense, never changes), `claim_region` says who is IN it RIGHT NOW \
                       (transient, advisory, released at checkout), and this says whose ground \
                       it is, which survives every session. Use it for the ordinary case of two \
                       people splitting a design — these parts are mine, those are yours. DO NOT \
                       use a claim for this: claims are session-scoped by their own description \
                       and never expire on a shared server, so standing ones would drown the \
                       report that shows who is actively working where. Deliberately NOT a \
                       traceability edge, so ownership never enlarges a blast radius — owning \
                       something says who ANSWERS for it, not that changing it changes them. \
                       `note` is what is actually owned and any bound on it. AN UNOWNED NODE IS \
                       NOT A GAP: most of a mature design legitimately has no owner, so absence \
                       is never reported. Once recorded, `loop_status` with a `contributor_id` \
                       lists the open gaps standing on that person's ground.",
        annotations(read_only_hint = false)
    )]
    pub async fn owned_by(
        &self,
        Parameters(req): Parameters<OwnedByReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.owned_by(
                &req.from_type,
                &req.from_id,
                &req.contributor_id,
                req.note.as_deref(),
                req.since.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Fill in what a consumer of this contract must AGREE with — the paradigm \
                       (sync/async), the payload format, the field-level schema, the endpoint and \
                       permitted operations, authentication, transport security, and the error \
                       model. Structured rather than prose because prose cannot be compared: two \
                       designs can be linked and still not be checkable for disagreement unless \
                       the seam is described in comparable terms. Every field is optional and \
                       omitting one LEAVES IT ALONE, so a spec can be filled in over time by \
                       different people. Unset reads as `unspecified`, never a flattering default \
                       — silence about authentication must not read as `none`. Rate limits, \
                       timeouts and concurrency do NOT belong here: they are numeric limits with \
                       a unit and a direction, so record them as a `Constraint` and point it at \
                       this interface with `constrains`.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_interface_spec(
        &self,
        Parameters(req): Parameters<InterfaceSpecReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_interface_spec(
                &req.interface_id,
                req.medium.as_deref(),
                req.paradigm.as_deref(),
                req.payload_format.as_deref(),
                req.payload_schema.as_deref(),
                req.endpoint.as_deref(),
                req.operations.as_deref(),
                req.auth.as_deref(),
                req.transport_security.as_deref(),
                req.error_model.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }
}
