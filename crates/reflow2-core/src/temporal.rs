//! Axis Z — change over time. The temporal layer that lets the graph
//! **remember the past instead of overwriting it** (docs/three-axes.md).
//!
//! This is the substrate the coherence loop's **CHANGE** step stands on: every
//! edit is recorded as a [`ChangeEvent`] pinned to a [`DesignEpoch`], and the
//! prior state of what changed is captured as an immutable `Snapshot` before it
//! is overwritten. Nothing here reasons or calls an LLM — it is deterministic
//! bookkeeping (docs/interaction-surfaces.md, "deterministic ops").
//!
//! The four temporal node types and their edges are defined in
//! `schema/temporal.yaml`; the enums below mirror that schema's `enum` values
//! exactly, so the typed API cannot produce an out-of-vocabulary value.
//!
//! [`ChangeEvent`]: crate::nodes::node::CHANGE_EVENT
//! [`DesignEpoch`]: crate::nodes::node::DESIGN_EPOCH

use dynograph_core::{DynoError, Value};
use dynograph_storage::{StoredEdge, StoredNode};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

/// Node types whose edges are *bookkeeping about* the design rather than part
/// of it — history, provenance, observation, questions. A snapshot captures a
/// node's design structure, not its audit trail: including these would make
/// every snapshot grow with each prior snapshot (its own `HAS_SNAPSHOT`
/// edges), and a diff across epochs would drown in meta-history (BL-63).
const BOOKKEEPING_TYPES: &[&str] = &[
    node::DESIGN_EPOCH,
    node::SNAPSHOT,
    node::CHANGE_EVENT,
    node::TEMPORAL_FACT,
    node::DIMENSION_ASSESSMENT,
    node::DIMENSION_OBSERVATION,
    node::FRAGMENT,
    node::DRIFT_EVENT,
    node::QUESTION,
];

/// Edge types that are design content **even though** they point at a
/// bookkeeping node, so [`BOOKKEEPING_TYPES`] must not swallow them: the
/// exclusion above is about an edge's ROLE, and the endpoint's type is only a
/// proxy for it (Anthony, 2026-07-31).
///
/// The proxy was exact when it was written — every edge to a `DesignEpoch` was
/// audit trail (`AT_EPOCH`, `OCCURS_DURING`), so excluding the type excluded
/// the role. `SCHEDULED_FOR` broke that: an edge to an epoch is now a
/// COMMITMENT — when this was due — which is design content by any reading,
/// and `req:intent-preserved` says the past is never overwritten.
///
/// What the proxy cost while it held: a `record_change` on a scheduled
/// requirement — the obvious way to record "this slipped" — captured a
/// snapshot with the schedule edge silently dropped, so the old due date was
/// destroyed by the very call whose job is preserving it, and the call
/// reported success. Only the epoch-side snapshot preserved it, and nothing
/// said so.
const COMMITMENT_EDGES: &[&str] = &[edge::SCHEDULED_FOR];

/// One edge of a snapshotted node, as captured into the Snapshot's `edges`
/// property (BL-63). `direction` is from the snapshotted node's point of view:
/// `"out"` means the node was the edge's source, `"in"` its target.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotEdge {
    /// `"out"` or `"in"`, relative to the snapshotted node.
    pub direction: String,
    /// The edge type (e.g. `ALLOCATED_TO`).
    pub edge_type: String,
    /// Node type of the other endpoint.
    pub other_type: String,
    /// Node id of the other endpoint.
    pub other_id: String,
    /// The edge's properties, key-sorted for byte-stable serialization.
    pub properties: std::collections::BTreeMap<String, Value>,
}

/// A plan as a comparable set: `(item_type, item_id, modality)` per scheduled
/// item. Sorted, so "has this plan moved?" is one equality test.
type ScheduleSet = std::collections::BTreeSet<(String, String, String)>;

/// A schedule edge's modality, defaulting to `expected` when absent — the
/// ordinary case, and the weaker of the two, so an unlabelled claim is never
/// promoted to an obligation nobody made (`req:defaults-do-not-assert`).
fn modality_of(modality: Option<&Value>) -> String {
    modality
        .and_then(Value::as_str)
        .unwrap_or("expected")
        .to_string()
}

/// Which revision a snapshot id names — `…:r3` is 3, an unsuffixed id is the
/// first. Mirrors the id rule `snapshot_node` owns.
fn revision_of(snapshot_id: &str) -> usize {
    snapshot_id
        .rsplit_once(":r")
        .and_then(|(_, n)| n.parse::<usize>().ok())
        .unwrap_or(1)
}

/// What became of one scheduled item when its moment arrived.
///
/// Anthony's three outcomes, the complement he did not name (an item delivered
/// that nobody planned arrives as `added_after_baseline` rather than a variant
/// here), and a FIFTH the four assume away: `Outstanding`. The four presume
/// every undelivered item was consciously moved or dropped, when the commonest
/// case is that nobody touched it and it did not happen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum ScheduleOutcome {
    /// The plan held — computed from the golden thread, never asserted.
    Delivered,
    /// Still intended; the date moved. Where it now points.
    Deferred {
        /// The moments it is now due at, sorted.
        now_due_at: Vec<String>,
    },
    /// No longer intended at all — the claim is gone from the schedule
    /// entirely. Distinct from `Deferred` the way `retire-from-design`
    /// distinguishes them: one is a date change, the other a withdrawal, and
    /// conflating them either loses a commitment or embalms one.
    Discontinued,
    /// Still pointed at a moment that has arrived, and not delivered. Nobody
    /// has said whether it slips or drops — the one question
    /// `req:plans-move-honestly` says must be ASKED, never defaulted.
    Outstanding,
}

/// One item on a schedule, with what became of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScheduledItem {
    /// Node type of the scheduled item (`Requirement` or `Capability`).
    pub item_type: String,
    /// Node id of the scheduled item.
    pub item_id: String,
    /// `expected` (a plan) or `required` (an obligation).
    pub modality: String,
    /// What became of it.
    #[serde(flatten)]
    pub outcome: ScheduleOutcome,
}

/// One recorded state of a plan — a snapshot of the target, read for its
/// schedule. The trail these form is how a total slip is read: the baseline
/// says what was promised, the trail says how it got from there to here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanRevision {
    /// The Snapshot this was read from.
    pub snapshot_id: String,
    /// The epoch the snapshot was FILED in — when the plan was recorded, which
    /// is never the planned epoch itself (history cannot be filed into a moment
    /// that has not happened).
    pub recorded_in_epoch: Option<String>,
    /// The items scheduled at that moment, sorted.
    pub items: Vec<(String, String)>,
}

/// Where the baseline came from — stated rather than implied, because the two
/// cases mean different things about how much the plan is known to have moved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "baseline")]
pub enum BaselineSource {
    /// The target's earliest snapshot — the original commitment.
    FirstSnapshot {
        /// Which snapshot.
        snapshot_id: String,
    },
    /// No snapshot exists, so the plan has never been recorded as moving, so
    /// the live edges ARE the original plan.
    LiveEdges,
}

/// The planned-versus-delivered delta. See
/// [`arrival_delta`](DesignGraph::arrival_delta).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArrivalDelta {
    /// `DesignEpoch` or `Release`.
    pub target_type: String,
    /// The moment being asked about.
    pub target_id: String,
    /// Its status, so a forecast is never mistaken for a delta.
    pub status: Option<String>,
    /// Where the baseline came from.
    #[serde(flatten)]
    pub baseline: BaselineSource,
    /// The baseline's items, with outcomes.
    pub items: Vec<ScheduledItem>,
    /// Scheduled here after the baseline was taken — the complement Anthony
    /// named, and usually the most informative part, because unplanned work is
    /// where the estimate actually went.
    pub added_after_baseline: Vec<ScheduledItem>,
    /// Every recorded state of the plan, oldest first.
    pub movement: Vec<PlanRevision>,
    /// `required` claims not delivered — computed violations, not slips.
    pub missed_obligations: Vec<String>,
    /// How many items were scheduled `required` at all.
    ///
    /// Reported because `missed_obligations` being empty is AMBIGUOUS without
    /// it: an increment where every obligation landed and one that promised
    /// nothing both report an empty list. `ready_to_cut` resolves it; this is
    /// the number that lets a reader see why (`dec:release-trigger-needs-a-required-item`).
    pub required_count: usize,
    /// Whether the cut trigger fires: every `required` item delivered, AND at
    /// least one was required.
    ///
    /// THE SECOND CLAUSE IS THE FIX. `dec:release-trigger` said a cut fires when
    /// every required item is delivered, which is VACUOUSLY TRUE of an increment
    /// that requires nothing — so an empty release read as ready. That is the
    /// empty-release failure the same decision used to REJECT a fixed cadence,
    /// arriving through the chosen option's own back door. Requiring one
    /// obligation also makes "this increment has been scoped" a precondition of
    /// cutting it: an increment with nothing required has never had the question
    /// "what must ship for this to mean anything?" put to it.
    pub ready_to_cut: bool,
    /// What this computation cannot see, said out loud rather than left to be
    /// inferred from a confident-looking number.
    pub notes: Vec<String>,
}

/// Kind of [`DesignEpoch`](crate::nodes::node::DESIGN_EPOCH) —
/// mirrors `temporal.yaml` `DesignEpoch.epoch_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpochType {
    /// A checksummed baseline (generalizes the P2→P3 Anchor).
    Baseline,
    /// An ordinary forward revision (the schema default).
    Revision,
    /// A named milestone.
    Milestone,
    /// An epoch cut in response to an incident (e.g. a hotfix).
    IncidentResponse,
    /// The epoch a release was cut at.
    ReleaseCut,
}

impl EpochType {
    /// The exact schema enum string.
    pub fn as_str(self) -> &'static str {
        match self {
            EpochType::Baseline => "baseline",
            EpochType::Revision => "revision",
            EpochType::Milestone => "milestone",
            EpochType::IncidentResponse => "incident_response",
            EpochType::ReleaseCut => "release_cut",
        }
    }
}

/// WHICH AXIS a [`ChangeEvent`](crate::nodes::node::CHANGE_EVENT) sits on: did
/// the SYSTEM change, or did only the design's KNOWLEDGE of it change?
///
/// `change_type` was carrying both questions at once, and that is why five
/// sessions across three projects each picked a DIFFERENT least-wrong value for
/// the same kind of event. ⭐ THE SPLIT WAS ALREADY NAMED IN THIS FILE and left
/// unusable: [`BaselineEstablished`](ChangeType::BaselineEstablished)'s own doc
/// says "every other variant answers why the THING changed; this one says the
/// thing did not change and only the design's KNOWLEDGE of it did" — an axis
/// with exactly one member, reserved so nobody could reach it.
///
/// **Optional, and absent means nobody said.** It is never inferred from
/// `change_type`: the mapping is not total (a `resync` can be either) and
/// guessing would put a claim on the record that no one made
/// (`req:defaults-do-not-assert`). Every ChangeEvent written before 2026-08-15
/// therefore has no subject, which is true rather than missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSubject {
    /// The system moved — code, behaviour, scope, a component, a contract.
    System,
    /// The system did not move; the design's record of it did. A first
    /// baseline, a re-sync, a question settled, a correction to what we knew.
    Record,
}

impl ChangeSubject {
    /// The exact schema enum string.
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeSubject::System => "system",
            ChangeSubject::Record => "record",
        }
    }
}

/// Why the design changed — mirrors `temporal.yaml` `ChangeEvent.change_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    /// A requirement added or widened after the baseline.
    RequirementCreep,
    /// A newly introduced capability/feature.
    NewFeature,
    /// A fix forced by a failed verification.
    TestFailureFix,
    /// **The design was right and the code was wrong.** A defect against intent
    /// that was already accepted — found in the field, by a user, by review, or
    /// by anything other than a check that failed.
    ///
    /// Distinct from [`TestFailureFix`](Self::TestFailureFix), which is the same
    /// repair with a different PROVENANCE: a check caught it. That difference is
    /// one this project already cares about everywhere else — `set_provenance`
    /// grades authored/inferred/reconciled, and `cap:independent-evidence`
    /// refuses to let evidence a value was fitted to count as validating it.
    ///
    /// ADDED 2026-08-15 AFTER FIVE FORCED CHOICES, and the count is the argument:
    /// two maintainer sessions, the dev_storyflow fleet, Alex's session, and the
    /// very edit that recorded his report — four people across three projects,
    /// each reaching for a different least-wrong value, none of them agreeing,
    /// because there was nothing to agree on. The same missing meaning was
    /// absent from a SECOND surface too: `set_artifact_checksum`'s disposition
    /// had no answer for "the code was wrong and now matches intent that never
    /// changed". One absence is a wording gap; two independent absences of the
    /// same category is a category the vocabulary did not have.
    DefectFix,
    /// A change made to improve performance.
    PerformanceOptimization,
    /// A structural change with no behavior change.
    Refactor,
    /// A change to what is in/out of scope.
    ScopeChange,
    /// A change to a constraint.
    ConstraintChange,
    /// A change driven by the operating environment.
    EnvironmentChange,
    /// Something removed/retired.
    Deprecation,
    /// A re-sync back to coherence (a HEAL outcome).
    Resync,
    /// An `Artifact` registered with no checksum got its **first** one: the
    /// record moved and the code did not (BL-157).
    ///
    /// The odd one out, and deliberately so. Every other variant answers *why
    /// the thing changed*; this one says the thing did not change and only the
    /// design's knowledge of it did. It exists because the alternative — the
    /// least-wrong `refactor` — puts a change that never happened on axis Z.
    ///
    /// **Reserved for [`set_artifact_checksum`](crate::graph::DesignGraph::set_artifact_checksum)'s
    /// `BaselineEstablished` disposition.** The generic change-recording paths
    /// refuse it, so the confirmation ledger's count of first baselines cannot
    /// be inflated by hand.
    BaselineEstablished,
}

impl ChangeType {
    /// The exact schema enum string.
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeType::RequirementCreep => "requirement_creep",
            ChangeType::NewFeature => "new_feature",
            ChangeType::TestFailureFix => "test_failure_fix",
            ChangeType::DefectFix => "defect_fix",
            ChangeType::PerformanceOptimization => "performance_optimization",
            ChangeType::Refactor => "refactor",
            ChangeType::ScopeChange => "scope_change",
            ChangeType::ConstraintChange => "constraint_change",
            ChangeType::EnvironmentChange => "environment_change",
            ChangeType::Deprecation => "deprecation",
            ChangeType::Resync => "resync",
            ChangeType::BaselineEstablished => "baseline_established",
        }
    }
}

/// What a [`ChangeEvent`](crate::nodes::node::CHANGE_EVENT) did to a node —
/// mirrors `temporal.yaml` `CHANGED.action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeAction {
    /// The node was created by this change.
    Added,
    /// The node's properties/edges were modified.
    Modified,
    /// The node was removed by this change.
    Removed,
}

impl ChangeAction {
    /// The exact schema enum string.
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeAction::Added => "added",
            ChangeAction::Modified => "modified",
            ChangeAction::Removed => "removed",
        }
    }

    /// Whether there is prior state worth snapshotting before this action.
    /// An `Added` node has no past; `Modified`/`Removed` do.
    fn has_prior_state(self) -> bool {
        !matches!(self, ChangeAction::Added)
    }
}

/// A change to record via [`DesignGraph::record_change`]. Bundled so the call
/// site reads as named fields rather than a long positional argument list
/// (mirrors the `PersistInput` convention in the predecessor `ir2`).
#[derive(Debug, Clone, Copy)]
pub struct ChangeRecord<'a> {
    /// The epoch this change happens at (the ChangeEvent/Snapshot are pinned here).
    pub epoch_id: &'a str,
    /// Id for the new ChangeEvent node.
    pub change_event_id: &'a str,
    /// Human-readable name of the change.
    pub name: &'a str,
    /// Why the design changed.
    pub change_type: ChangeType,
    /// Node type of what changed.
    pub target_type: &'a str,
    /// Node id of what changed.
    pub target_id: &'a str,
    /// What the change did to the target.
    pub action: ChangeAction,
}

/// Deterministic id for the FIRST snapshot of `node_id` taken at `epoch_id`.
/// Later revisions within the same epoch append `:r2`, `:r3` — see
/// `snapshot_node`, which owns that rule. This used to be the whole id, and
/// "idempotent (create-or-replace)" used to be its documented virtue; replace
/// is what silently destroyed the earlier revision.
fn snapshot_id(epoch_id: &str, node_id: &str) -> String {
    format!("snap:{epoch_id}:{node_id}")
}

/// Ceiling on distinct snapshots of one node within one epoch. Not a storage
/// limit — a signal. An epoch is meant to bound a round of *work*; a node
/// revised this many times inside one is a sign the epoch has stopped meaning
/// anything, and the error says so rather than growing history quietly.
const MAX_SNAPSHOT_REVISIONS: usize = 64;

/// Axis-Z (temporal) operations. See the module docs.
impl DesignGraph {
    // ---- Epochs -----------------------------------------------------------

    /// Create a [`DesignEpoch`](crate::nodes::node::DESIGN_EPOCH): a named
    /// version/milestone of the design. `sequence` is the monotonic ordering
    /// key across epochs (also wire [`precedes`](Self::precedes) for explicit
    /// ordering edges).
    pub fn add_epoch(
        &mut self,
        id: &str,
        name: &str,
        epoch_type: EpochType,
        sequence: i64,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::DESIGN_EPOCH,
            id,
            Props::new()
                .set("name", name)
                .set("epoch_type", epoch_type.as_str())
                .set("sequence", sequence),
        )
    }

    /// Create an epoch that has NOT happened yet — a claim about the future
    /// rather than a record of the past (`req:epochs-can-be-planned`).
    ///
    /// Separate from [`add_epoch`](Self::add_epoch) rather than a flag on it,
    /// for two reasons. `add_epoch` has 27 call sites that all mean "record the
    /// point I am at", and every one of them is still correct; and planning is a
    /// deliberate act, so it reads better as its own verb than as an argument
    /// someone might pass by accident — the same reasoning that keeps
    /// `Interface.designation` internal until someone publishes on purpose.
    ///
    /// `epoch_type` still applies: kind and tense are orthogonal, so a planned
    /// MILESTONE and a planned RELEASE CUT are both sayable.
    pub fn plan_epoch(
        &mut self,
        id: &str,
        name: &str,
        epoch_type: EpochType,
        sequence: i64,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::DESIGN_EPOCH,
            id,
            Props::new()
                .set("name", name)
                .set("epoch_type", epoch_type.as_str())
                .set("sequence", sequence)
                .set("status", "planned"),
        )
    }

    /// Move an epoch between `planned` and `arrived`, preserving everything else.
    ///
    /// `planned` to `arrived` is ARRIVAL — the moment a claim about the future
    /// becomes a point in the past, and the moment the planned-versus-delivered
    /// delta becomes computable. The reverse direction exists so a premature
    /// arrival can be corrected; it is not a way to un-happen an epoch.
    pub fn set_epoch_status(
        &mut self,
        epoch_id: &str,
        status: &str,
    ) -> Result<StoredNode, DynoError> {
        if !matches!(status, "planned" | "arrived") {
            return Err(DynoError::Validation {
                node_type: node::DESIGN_EPOCH.into(),
                property: "status".into(),
                message: format!(
                    "'{status}' is not an epoch status (one of planned, arrived). `planned` is a \
                     claim about a point that has not happened; `arrived` is a record of one that \
                     has."
                ),
            });
        }
        let Some(existing) = self.get_node(node::DESIGN_EPOCH, epoch_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::DESIGN_EPOCH.into(),
                node_id: epoch_id.into(),
            });
        };
        let mut props = Props::new().set("status", status);
        for (k, v) in &existing.properties {
            if k != "status" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::DESIGN_EPOCH, epoch_id, props)
    }

    /// Has this epoch happened? Absent reads as `arrived`, matching the schema
    /// default and the meaning every epoch written before the property existed
    /// already had.
    pub fn epoch_is_planned(&self, epoch_id: &str) -> Result<bool, DynoError> {
        Ok(self
            .get_node(node::DESIGN_EPOCH, epoch_id)?
            .and_then(|e| {
                e.properties
                    .get("status")
                    .and_then(dynograph_core::Value::as_str)
                    .map(|s| s == "planned")
            })
            .unwrap_or(false))
    }

    /// `earlier PRECEDES later` — an explicit ordering edge between epochs.
    pub fn precedes(&mut self, earlier_epoch: &str, later_epoch: &str) -> Result<(), DynoError> {
        self.create_edge(
            edge::PRECEDES,
            node::DESIGN_EPOCH,
            earlier_epoch,
            node::DESIGN_EPOCH,
            later_epoch,
            Props::new(),
        )?;
        Ok(())
    }

    /// Pin any node (a Snapshot, a ChangeEvent, …) to the epoch it belongs to
    /// via `AT_EPOCH`.
    pub fn pin_at_epoch(
        &mut self,
        node_type: &str,
        node_id: &str,
        epoch_id: &str,
    ) -> Result<(), DynoError> {
        self.create_edge(
            edge::AT_EPOCH,
            node_type,
            node_id,
            node::DESIGN_EPOCH,
            epoch_id,
            Props::new(),
        )?;
        Ok(())
    }

    /// Schedule a Requirement or Capability against the moment it is due —
    /// `SCHEDULED_FOR`, the satisfaction schedule (`req:epochs-can-be-planned`).
    ///
    /// The target is a `DesignEpoch` or a `Release`: the two paired views of
    /// one architecture, time and capability-increment. `modality` says which
    /// kind of claim this is — `expected` (a plan) or `required` (an
    /// obligation whose miss at arrival is a computed violation).
    ///
    /// There is no `achieved` modality, and the absence is the point:
    /// delivery is computed from the golden thread, never asserted
    /// (`req:completion-computed`). A schedule that records its own success
    /// is a second source of truth that can disagree with the first.
    ///
    /// Rescheduling is a RECORDED CHANGE on the target epoch, not an edit to
    /// this edge — re-pointing it would make the slip invisible and let the
    /// plan silently rewrite its own history (`dec:arrival-delta`).
    pub fn schedule_for(
        &mut self,
        item_type: &str,
        item_id: &str,
        target_type: &str,
        target_id: &str,
        modality: &str,
        recorded_at: Option<&str>,
    ) -> Result<(), DynoError> {
        if !matches!(modality, "expected" | "required") {
            return Err(DynoError::Validation {
                node_type: edge::SCHEDULED_FOR.into(),
                property: "modality".into(),
                message: format!(
                    "'{modality}' is not a schedule modality (one of expected, required). \
                     `expected` is a plan; `required` is an obligation whose miss at arrival is a \
                     violation. There is no `achieved` — delivery is computed from the golden \
                     thread, never recorded here."
                ),
            });
        }
        if !matches!(target_type, node::DESIGN_EPOCH | node::RELEASE) {
            return Err(DynoError::Validation {
                node_type: target_type.into(),
                property: "SCHEDULED_FOR.to".into(),
                message: format!(
                    "a schedule points at a moment, so '{target_type}' cannot be one: use a \
                     {} for the time axis or a {} for the capability-increment axis.",
                    node::DESIGN_EPOCH,
                    node::RELEASE
                ),
            });
        }
        let mut props = Props::new().set("modality", modality);
        if let Some(at) = recorded_at {
            props = props.set("recorded_at", at);
        }
        // Changing an existing claim's modality REWRITES what was promised —
        // an `expected` quietly becoming `required`, or the reverse — which is
        // the same loss as deleting the edge and is guarded the same way.
        // Creating the edge for the first time is not a rewrite and is free.
        if let Some(existing) = self
            .outgoing(item_id, Some(edge::SCHEDULED_FOR))?
            .into_iter()
            .find(|e| e.to_id == target_id)
        {
            let had = existing
                .properties
                .get("modality")
                .and_then(Value::as_str)
                .unwrap_or("expected");
            if had != modality {
                self.guard_schedule_loss(
                    target_id,
                    &format!("changing '{item_id}' from `{had}` to `{modality}`"),
                )?;
            }
        }
        self.create_edge(
            edge::SCHEDULED_FOR,
            item_type,
            item_id,
            target_type,
            target_id,
            props,
        )?;
        Ok(())
    }

    // ---- The schedule's movement, and the delta at arrival ----------------

    /// Every `SCHEDULED_FOR` currently pointing at `target_id`, as
    /// `(item_type, item_id, modality)`. Sorted, so two reads of an unchanged
    /// plan compare equal.
    fn live_schedule(&self, target_id: &str) -> Result<ScheduleSet, DynoError> {
        let index = self.node_type_index()?;
        let mut out = ScheduleSet::new();
        for e in self.incoming(target_id, Some(edge::SCHEDULED_FOR))? {
            let Some(item_type) = index.get(&e.from_id) else {
                continue; // dangling — the same skip snapshot_node makes
            };
            out.insert((
                item_type.clone(),
                e.from_id.clone(),
                modality_of(e.properties.get("modality")),
            ));
        }
        Ok(out)
    }

    /// The schedule a Snapshot captured — the plan as it stood when that
    /// snapshot was taken. Reads the `in`-direction `SCHEDULED_FOR` edges,
    /// which is why [`COMMITMENT_EDGES`] has to keep them.
    fn snapshot_schedule(snapshot: &StoredNode) -> Result<ScheduleSet, DynoError> {
        let mut out = ScheduleSet::new();
        for e in parse_snapshot_edges(snapshot)? {
            if e.direction != "in" || e.edge_type != edge::SCHEDULED_FOR {
                continue;
            }
            out.insert((
                e.other_type,
                e.other_id,
                modality_of(e.properties.get("modality")),
            ));
        }
        Ok(out)
    }

    /// Every Snapshot of `node_id`, **oldest first**.
    ///
    /// A Snapshot carries no clock of its own, so the order is (sequence of the
    /// epoch it was filed in, revision within that epoch, then id to break
    /// ties). That is the only total order the graph actually holds, and it is
    /// the one `snapshot_node`'s `:rN` suffix was designed to be read by.
    pub fn snapshots_of(&self, node_id: &str) -> Result<Vec<StoredNode>, DynoError> {
        let mut rows: Vec<(i64, usize, String, StoredNode)> = Vec::new();
        for e in self.outgoing(node_id, Some(edge::HAS_SNAPSHOT))? {
            let Some(snap) = self.get_node(node::SNAPSHOT, &e.to_id)? else {
                continue;
            };
            let sequence = self
                .outgoing(&e.to_id, Some(edge::AT_EPOCH))?
                .first()
                .and_then(|at| self.get_node(node::DESIGN_EPOCH, &at.to_id).ok().flatten())
                .and_then(|ep| ep.properties.get("sequence").and_then(Value::as_i64))
                .unwrap_or(0);
            rows.push((sequence, revision_of(&e.to_id), e.to_id.clone(), snap));
        }
        rows.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
        Ok(rows.into_iter().map(|(_, _, _, s)| s).collect())
    }

    /// Whether `target_id`'s CURRENT schedule is already preserved in its
    /// latest snapshot — the precondition for any edit that would destroy part
    /// of it. An empty schedule has nothing to lose and is always recorded.
    pub fn schedule_is_recorded(&self, target_id: &str) -> Result<bool, DynoError> {
        let live = self.live_schedule(target_id)?;
        if live.is_empty() {
            return Ok(true);
        }
        let snapshots = self.snapshots_of(target_id)?;
        let Some(latest) = snapshots.last() else {
            return Ok(false);
        };
        Ok(Self::snapshot_schedule(latest)? == live)
    }

    /// Refuse an edit that would destroy part of a plan nobody has recorded.
    ///
    /// This is the load-bearing half, and it is the same shape as
    /// `record_change` refusing a planned epoch: without it, `status` and
    /// `SCHEDULED_FOR` alike would be declared-and-unconsulted fields. Deferring
    /// B from epoch 3 by simply re-pointing its edge leaves the graph saying
    /// epoch 3 was only ever about A and C — the slip is invisible, and the
    /// plan has silently rewritten its own history, which is exactly what
    /// `req:intent-preserved` forbids (`dec:arrival-delta`, Anthony 2026-07-31).
    ///
    /// ADDING to a plan is deliberately NOT guarded: it destroys no earlier
    /// claim, so requiring a snapshot first would tax correct work — the
    /// mistake this project keeps finding in detectors that punish good
    /// practice. Only removal, re-pointing and a modality rewrite are losses.
    pub(crate) fn guard_schedule_loss(&self, target_id: &str, what: &str) -> Result<(), DynoError> {
        if self.schedule_is_recorded(target_id)? {
            return Ok(());
        }
        Err(DynoError::Validation {
            node_type: edge::SCHEDULED_FOR.into(),
            property: "modality".into(),
            message: format!(
                "the plan for '{target_id}' is not on the record, so {what} would destroy it \
                 silently. Call record_change against '{target_id}' first (filed in an epoch that \
                 has ARRIVED, taking '{target_id}' as its target) — that snapshots every schedule \
                 edge pointing at it, and then the plan may move."
            ),
        })
    }

    /// The planned-versus-delivered delta for an epoch or release
    /// (`dec:arrival-delta`, delivering obligation 2 of
    /// `req:plans-move-honestly`).
    ///
    /// **Nothing here is stored.** Both inputs already exist — the plan lives in
    /// the snapshots, delivery is computed from the golden thread — and writing
    /// the outcome down would create a second source of truth that can disagree
    /// with the first (`req:completion-computed`). It is the same argument that
    /// keeps `achieved` out of the `modality` enum.
    ///
    /// The baseline is the **first** snapshot of the target, with every later
    /// one reported as the movement trail (Anthony 2026-07-31). The last
    /// snapshot would have measured only the most recent revision: with two
    /// replans, epoch 3 holds `{A,B,C}` then `{A,C}`, and reading the last says
    /// the plan was always `{A,C}` — B's slip vanishing from the very report
    /// meant to show it. Where no snapshot exists the plan never moved, so the
    /// live edges ARE the original plan and are used as the baseline.
    pub fn arrival_delta(&self, target_id: &str) -> Result<ArrivalDelta, DynoError> {
        let index = self.node_type_index()?;
        let target_type = index
            .get(target_id)
            .cloned()
            .ok_or_else(|| DynoError::NodeNotFound {
                node_type: "*".into(),
                node_id: target_id.to_string(),
            })?;
        if !matches!(target_type.as_str(), node::DESIGN_EPOCH | node::RELEASE) {
            return Err(DynoError::Validation {
                node_type: target_type.clone(),
                property: "arrival_delta".into(),
                message: format!(
                    "'{target_id}' is a {target_type}; a schedule arrives at a moment, so ask \
                     this of a {} or a {}.",
                    node::DESIGN_EPOCH,
                    node::RELEASE
                ),
            });
        }

        let mut notes: Vec<String> = Vec::new();
        let status = self.get_node(&target_type, target_id)?.and_then(|n| {
            n.properties
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
        if status.as_deref() == Some("planned") {
            notes.push(format!(
                "'{target_id}' has not arrived yet — this is a forecast against the plan as it \
                 stands, not a delta. Nothing has been missed until the moment passes."
            ));
        }

        let snapshots = self.snapshots_of(target_id)?;
        let mut movement = Vec::new();
        for snap in &snapshots {
            let mut items: Vec<(String, String)> = Self::snapshot_schedule(snap)?
                .into_iter()
                .map(|(t, i, _)| (t, i))
                .collect();
            items.sort();
            movement.push(PlanRevision {
                snapshot_id: snap.node_id.clone(),
                recorded_in_epoch: self
                    .outgoing(&snap.node_id, Some(edge::AT_EPOCH))?
                    .first()
                    .map(|e| e.to_id.clone()),
                items,
            });
        }

        let live = self.live_schedule(target_id)?;
        let (baseline, baseline_source) = match snapshots.first() {
            Some(first) => (
                Self::snapshot_schedule(first)?,
                BaselineSource::FirstSnapshot {
                    snapshot_id: first.node_id.clone(),
                },
            ),
            None => (live.clone(), BaselineSource::LiveEdges),
        };
        if baseline.is_empty() && movement.iter().any(|r| !r.items.is_empty()) {
            notes.push(
                "the baseline snapshot captured an EMPTY plan, so everything scheduled since \
                 reads as added after it. The first snapshot was taken before this plan was \
                 written, not after — read the movement trail rather than the headline."
                    .to_string(),
            );
        }

        let baselined: std::collections::BTreeSet<&String> =
            baseline.iter().map(|(_, id, _)| id).collect();
        let mut items = Vec::new();
        for (item_type, item_id, modality) in &baseline {
            items.push(self.scheduled_item(target_id, item_type, item_id, modality)?);
        }
        let mut added_after_baseline = Vec::new();
        for (item_type, item_id, modality) in &live {
            if !baselined.contains(item_id) {
                added_after_baseline
                    .push(self.scheduled_item(target_id, item_type, item_id, modality)?);
            }
        }

        // A `required` claim missed at arrival is a computed violation, not a
        // slip — the scheduling face of a KPP, which is what gives inviolable
        // intent a deadline dimension (`dec:schedule-is-an-edge-with-modality`).
        // Computed across BOTH sets: an obligation added late is still an
        // obligation.
        let missed_obligations: Vec<String> = items
            .iter()
            .chain(added_after_baseline.iter())
            .filter(|i| i.modality == "required" && i.outcome != ScheduleOutcome::Delivered)
            .map(|i| i.item_id.clone())
            .collect();

        let required_count = items
            .iter()
            .chain(added_after_baseline.iter())
            .filter(|i| i.modality == "required")
            .count();
        // ⭐ NAME THE REMEDY WHERE THE READER IS. An item reported `outstanding`
        // that HAS a passing check but no artifact is the exact shape of model
        // work — a re-decomposition, a retirement, a ruling — and `outstanding`
        // says "nobody has said whether this was deferred or dropped", which is
        // a false question about finished work. The declaration that fixes it
        // is `Capability.delivery: model`, and a reader looking at this reply
        // has no way to know that exists. Measured 2026-08-20: the mechanism
        // being built, tested and unreachable from where the person is stuck
        // was the same failure four separate times in one session.
        let checked_but_unbuilt: Vec<&str> = items
            .iter()
            .filter(|i| i.outcome == ScheduleOutcome::Outstanding)
            .filter(|i| i.item_type == node::CAPABILITY)
            .filter(|i| {
                self.capability_has_evidence(&i.item_id).unwrap_or(false)
                    && !self.capability_is_realized(&i.item_id).unwrap_or(true)
            })
            .map(|i| i.item_id.as_str())
            .collect();
        if !checked_but_unbuilt.is_empty() {
            notes.push(format!(
                "{} outstanding item(s) have a PASSING CHECK but nothing on disk realizing \
                 them: {}. If the deliverable was a change to the DESIGN rather than a file \
                 — a re-decomposition, a retirement, a governance ruling — say so with \
                 set_capability_delivery(<id>, \"model\") and delivery is computed from the \
                 check alone. If it is simply unbuilt, `outstanding` is correct and this note \
                 is not for you.",
                checked_but_unbuilt.len(),
                checked_but_unbuilt.join(", ")
            ));
        }

        let ready_to_cut = missed_obligations.is_empty() && required_count > 0;
        if required_count == 0 {
            notes.push(format!(
                "nothing is scheduled `required` for '{target_id}', so there is no obligation to \
                 miss and it is NOT ready to cut. An increment that promises nothing has not been \
                 scoped; say what must ship for it to mean anything."
            ));
        }

        Ok(ArrivalDelta {
            target_type,
            target_id: target_id.to_string(),
            status,
            baseline: baseline_source,
            items,
            added_after_baseline,
            movement,
            missed_obligations,
            required_count,
            ready_to_cut,
            notes,
        })
    }

    /// One scheduled item's outcome. Delivery wins over everything: an item
    /// that shipped is delivered even if a copy of its claim points somewhere
    /// later.
    fn scheduled_item(
        &self,
        target_id: &str,
        item_type: &str,
        item_id: &str,
        modality: &str,
    ) -> Result<ScheduledItem, DynoError> {
        let outcome = if self.item_is_delivered(item_type, item_id)? {
            ScheduleOutcome::Delivered
        } else if item_type == node::QUESTION && self.question_status(item_id)? == "withdrawn" {
            // WITHDRAWN IS SOMEBODY'S DECISION, so it must not fall through to
            // `outstanding` — which means "nobody has said whether this was
            // deferred or discontinued". Somebody said. Reporting it as
            // outstanding would ask again, every run, about a question already
            // taken off the table: the same false reading that made an epoch's
            // delivered work look unfinished until 2026-08-20.
            ScheduleOutcome::Discontinued
        } else {
            let mut elsewhere: Vec<String> = self
                .outgoing(item_id, Some(edge::SCHEDULED_FOR))?
                .into_iter()
                .map(|e| e.to_id)
                .collect();
            elsewhere.sort();
            if elsewhere.iter().any(|t| t == target_id) {
                // STILL POINTED HERE, and here has arrived. Neither deferred
                // nor discontinued — nobody has said which, and this is the
                // one question `req:plans-move-honestly` says must be ASKED
                // and never defaulted. Reporting it as a fifth outcome rather
                // than forcing it into one of Anthony's four is the whole
                // difference between a report and a guess.
                ScheduleOutcome::Outstanding
            } else if elsewhere.is_empty() {
                ScheduleOutcome::Discontinued
            } else {
                ScheduleOutcome::Deferred {
                    now_due_at: elsewhere,
                }
            }
        };
        Ok(ScheduledItem {
            item_type: item_type.to_string(),
            item_id: item_id.to_string(),
            modality: modality.to_string(),
            outcome,
        })
    }

    /// Delivered, by the same computation the delivery rollup uses — satisfied
    /// by a realized, passing capability. Never read from a status field
    /// (`req:completion-computed`).
    ///
    /// ⭐ WHAT COUNTS AS "REALIZED" DEPENDS ON `Capability.delivery`, and that
    /// is the only thing the author gets to declare here. Both branches still
    /// demand EVIDENCE — a passing check — so nothing became assertable:
    ///
    /// - `artifact` (the default): a file must realize it AND a check must
    ///   pass. Unchanged, and the case almost every capability is in.
    /// - `model`: the deliverable IS the design change, so there is no file to
    ///   point at and the check is the whole of the evidence.
    ///
    /// WHY, measured 2026-08-20: `epoch:the-declared-walls-hold` was set
    /// arrived with both its capabilities genuinely delivered and this reported
    /// BOTH as `outstanding`, because delivery required an artifact and model
    /// work produces none. `outstanding` means "nobody has said whether this
    /// was deferred or discontinued", which is a false statement about
    /// finished work — and it is asked again on every run. Re-decompositions,
    /// retirements and governance rulings would accumulate as phantom
    /// incompletions until somebody stopped scheduling that kind of work,
    /// which is most of what systems engineering is.
    ///
    /// 🛑 THE REJECTED FIX, recorded so nobody retries it: inferring `model`
    /// from the ABSENCE of an artifact. The commonest reason a capability has
    /// no file is THAT NOBODY HAS BUILT IT YET, so that rule reports unbuilt
    /// work as delivered the moment a check is attached to it — a false green
    /// in the dangerous direction, and the same class of wrong answer as a
    /// detector reporting clean because it had nothing to run on.
    fn item_is_delivered(&self, item_type: &str, item_id: &str) -> Result<bool, DynoError> {
        match item_type {
            node::REQUIREMENT => self.requirement_is_delivered(item_id),
            node::CAPABILITY => {
                if !self.capability_has_evidence(item_id)? {
                    return Ok(false);
                }
                Ok(self.capability_delivers_by_model(item_id)?
                    || self.capability_is_realized(item_id)?)
            }
            // A QUESTION IS DELIVERED WHEN IT IS ANSWERED, and nothing else
            // could stand in for that. There is no artifact to look for and no
            // check to run: the whole content of closing a gap is that the
            // person whose judgement it needed gave one. `answer_question` is
            // the only thing that sets this, so delivery stays computed from
            // the record rather than asserted beside it.
            node::QUESTION => Ok(self.question_status(item_id)? == "answered"),
            _ => Ok(false),
        }
    }

    /// A Question's `status` — `asked`, `answered` or `withdrawn`. Absent reads
    /// as `asked`, the schema default, so a Question written before this
    /// existed is never mistaken for a settled one.
    fn question_status(&self, question_id: &str) -> Result<String, DynoError> {
        Ok(self
            .get_node(node::QUESTION, question_id)?
            .and_then(|n| {
                n.properties
                    .get("status")
                    .and_then(|v| v.as_str().map(str::to_string))
            })
            .unwrap_or_else(|| "asked".to_string()))
    }

    /// Whether this capability's deliverable is a change to the DESIGN rather
    /// than a file — `Capability.delivery == "model"`. Absent reads as
    /// `artifact`, the schema default, so every capability written before this
    /// existed keeps the stricter rule rather than quietly loosening.
    fn capability_delivers_by_model(&self, capability_id: &str) -> Result<bool, DynoError> {
        Ok(self
            .get_node(node::CAPABILITY, capability_id)?
            .and_then(|n| {
                n.properties
                    .get("delivery")
                    .and_then(|v| v.as_str().map(|s| s == "model"))
            })
            .unwrap_or(false))
    }

    // ---- Snapshots (never overwrite the past) -----------------------------

    /// Capture the **current** state of an existing node as an immutable
    /// `Snapshot` pinned to `epoch_id`, wired `node -HAS_SNAPSHOT-> snapshot`
    /// and `snapshot -AT_EPOCH-> epoch`.
    ///
    /// The snapshot holds the node's **properties** (`state`) and its **design
    /// edges** (`edges`, BL-63): a large class of design change is an edge
    /// move, not a property edit — a re-allocation deletes `ALLOCATED_TO` one
    /// component and draws it to another — and before BL-63 the only durable
    /// record of the old owner was a hand-authored Decision. Edges touching
    /// bookkeeping nodes (history, provenance, observations, questions) are
    /// excluded: a snapshot captures design structure, not the audit trail —
    /// **except** where the edge's own role is design content, which is what
    /// [`COMMITMENT_EDGES`] names. A `SCHEDULED_FOR` points at a `DesignEpoch`
    /// and is a commitment, so it survives the exclusion; that is the
    /// difference between recording that a plan moved and destroying the
    /// evidence that it did.
    ///
    /// Call this *before* overwriting the node, so the snapshot preserves the
    /// pre-change state. Fails loud if the target node does not exist — you
    /// cannot snapshot what was never there (AGENTS.md rule 4).
    pub fn snapshot_node(
        &mut self,
        epoch_id: &str,
        node_type: &str,
        node_id: &str,
    ) -> Result<StoredNode, DynoError> {
        let current =
            self.get_node(node_type, node_id)?
                .ok_or_else(|| DynoError::NodeNotFound {
                    node_type: node_type.to_string(),
                    node_id: node_id.to_string(),
                })?;

        // Sort the properties before serializing: `StoredNode.properties` is a
        // `HashMap`, whose iteration order is seeded per process, so an unsorted
        // `to_string` writes byte-different `state` for the same node on every
        // run — which then makes two exports of identical history differ,
        // defeating the byte-stable-export promise (BL-58). A `BTreeMap` fixes
        // the key order.
        let sorted: std::collections::BTreeMap<&String, &Value> =
            current.properties.iter().collect();
        let state = serde_json::to_string(&sorted)
            .map_err(|e| DynoError::Serialization(format!("snapshot state for {node_id}: {e}")))?;

        // Capture the node's design edges (BL-63). The type index resolves the
        // other endpoint's type both to record it and to exclude bookkeeping
        // neighbours; an edge whose endpoint has no type is dangling and is
        // skipped, matching the drift module's precedent (BL-58).
        let index = self.node_type_index()?;
        let mut edges: Vec<SnapshotEdge> = Vec::new();
        for (stored, direction) in self
            .outgoing(node_id, None)?
            .iter()
            .map(|e| (e, "out"))
            .chain(self.incoming(node_id, None)?.iter().map(|e| (e, "in")))
        {
            let StoredEdge {
                from_id,
                to_id,
                edge_type,
                properties,
                ..
            } = stored;
            let other_id = if direction == "out" { to_id } else { from_id };
            let Some(other_type) = index.get(other_id) else {
                continue; // dangling edge — nothing to capture on that side
            };
            if BOOKKEEPING_TYPES.contains(&other_type.as_str())
                && !COMMITMENT_EDGES.contains(&edge_type.as_str())
            {
                continue;
            }
            edges.push(SnapshotEdge {
                direction: direction.to_string(),
                edge_type: edge_type.clone(),
                other_type: other_type.clone(),
                other_id: other_id.clone(),
                properties: properties
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            });
        }
        // Deterministic order for byte-stable exports (same discipline as
        // `state` above).
        edges.sort_by(|a, b| {
            (&a.direction, &a.edge_type, &a.other_id).cmp(&(
                &b.direction,
                &b.edge_type,
                &b.other_id,
            ))
        });
        let edges_json = serde_json::to_string(&edges)
            .map_err(|e| DynoError::Serialization(format!("snapshot edges for {node_id}: {e}")))?;

        // NEVER OVERWRITE AN EARLIER SNAPSHOT. The id was `snap:{epoch}:{node}`
        // and nothing else, while `create_node` MERGES on an existing id — so a
        // node revised TWICE in one epoch had its first snapshot silently
        // replaced by its second, and `record_change` reported success both
        // times. That contradicts req:intent-preserved ("the past is never
        // overwritten"), which this very section is named after, and it
        // falsified the revise-design skill's closing promise that a reader can
        // answer "what did this say before" without git archaeology. Found
        // 2026-07-28 by amending one requirement twice in a single epoch; the
        // pre-amendment text survived only in a previously committed export.
        //
        // The id stays `snap:{epoch}:{node}` for the FIRST capture, because
        // existing graphs and exports carry those ids and a test pins one. Only
        // a genuine second revision within the same epoch appends `:r2`, `:r3`,
        // so HAS_SNAPSHOT becomes one-to-many exactly when history requires it
        // and not before.
        //
        // An IDENTICAL re-capture returns the existing snapshot rather than
        // minting a duplicate: snapshotting a node that has not moved is a
        // no-op, not a new version, and treating it as one would make the
        // history claim edits that never happened — the mirror of the bug being
        // fixed. Idempotence here also keeps `record_change` safe to retry.
        //
        // That comparison is against the TAIL of the chain and nothing earlier,
        // which is the whole reason this walks to the end instead of returning
        // on the first match. A node edited A → B → A inside one epoch has
        // three genuine revisions; matching any earlier snapshot would hand
        // back the A-capture for the third and record two, hiding an edit that
        // DID happen — the same class of loss as the overwrite above, just
        // quieter. Matching only the tail keeps `:rN` order readable as the
        // order the revisions occurred in, which is the only ordering a reader
        // has: every snapshot in an epoch is pinned to that one epoch.
        let base_id = snapshot_id(epoch_id, node_id);
        let id_at = |revision: usize| {
            if revision == 1 {
                base_id.clone()
            } else {
                format!("{base_id}:r{revision}")
            }
        };

        let mut revision = 1usize;
        let mut tail = None;
        while let Some(existing) = self.get_node(node::SNAPSHOT, &id_at(revision))? {
            tail = Some(existing);
            revision += 1;
        }
        if let Some(existing) = tail {
            let same = |key: &str, want: &str| {
                existing
                    .properties
                    .get(key)
                    .and_then(Value::as_str)
                    .is_some_and(|had| had == want)
            };
            if same("state", &state) && same("edges", &edges_json) {
                return Ok(existing);
            }
        }
        // Checked only when a new capture is actually needed, so an idempotent
        // retry against a full chain still succeeds rather than erroring.
        if revision > MAX_SNAPSHOT_REVISIONS {
            return Err(DynoError::Query(format!(
                "node '{node_id}' already has {MAX_SNAPSHOT_REVISIONS} distinct snapshots in \
                 epoch '{epoch_id}'; open a new epoch rather than revising further in this one"
            )));
        }
        let snap_id = id_at(revision);

        let snapshot = self.create_node(
            node::SNAPSHOT,
            &snap_id,
            Props::new()
                .set("target_id", node_id)
                .set("target_type", node_type)
                .set("state", state)
                .set("edges", edges_json),
        )?;

        self.create_edge(
            edge::HAS_SNAPSHOT,
            node_type,
            node_id,
            node::SNAPSHOT,
            &snap_id,
            Props::new(),
        )?;
        self.pin_at_epoch(node::SNAPSHOT, &snap_id, epoch_id)?;

        Ok(snapshot)
    }

    // ---- Change events ----------------------------------------------------

    /// Create a [`ChangeEvent`](crate::nodes::node::CHANGE_EVENT) — a
    /// first-class record of *why* the design changed. `name` and
    /// `change_type` are required.
    ///
    /// `summary` and `rationale` are the schema's own text fields — `summary`
    /// is indexed for full text and is the embedding field, `rationale` is
    /// "why the change was made". THEY ARE PARAMETERS RATHER THAN A FOLLOW-UP
    /// WRITE BECAUSE THEIR ABSENCE HERE WAS MEASURED: two projects reported
    /// the same workaround on the same day (2026-08-19), each making a second
    /// `create_node` call to hang an UNDECLARED `description` on the event,
    /// because the constructor took none of the text the skills tell you to
    /// write. A flag that fires on every legitimate write teaches callers to
    /// ignore it, which is the opposite of what `undeclared` is for.
    pub fn add_change_event(
        &mut self,
        id: &str,
        name: &str,
        change_type: ChangeType,
        subject: Option<ChangeSubject>,
        summary: Option<&str>,
        rationale: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::CHANGE_EVENT,
            id,
            Props::new()
                .set("name", name)
                .set("change_type", change_type.as_str())
                .set_opt("subject", subject.map(ChangeSubject::as_str))
                .set_opt("summary", summary)
                .set_opt("rationale", rationale),
        )
    }

    /// `ChangeEvent CHANGED target` with an `action` — the link from a change
    /// to the node it added/modified/removed.
    pub fn changed(
        &mut self,
        change_event_id: &str,
        target_type: &str,
        target_id: &str,
        action: ChangeAction,
    ) -> Result<(), DynoError> {
        self.create_edge(
            edge::CHANGED,
            node::CHANGE_EVENT,
            change_event_id,
            target_type,
            target_id,
            Props::new().set("action", action.as_str()),
        )?;
        Ok(())
    }

    // ---- Composed: the CHANGE step ----------------------------------------

    /// Record a change end-to-end — the coherence loop's **CHANGE** step:
    ///
    /// 1. for `Modified`/`Removed`, snapshot the target's **pre-change** state
    ///    pinned to `epoch_id` (so the past is never lost); `Added` has no prior
    ///    state, so no snapshot is taken;
    /// 2. create a [`ChangeEvent`](crate::nodes::node::CHANGE_EVENT) and pin it
    ///    to the epoch (`AT_EPOCH`);
    /// 3. wire `ChangeEvent -CHANGED-> target` with `action`.
    ///
    /// Call this **before** applying the actual edit to the target node (for
    /// `Modified`), so step 1 captures the old state. Returns the snapshot (if
    /// any) and the change event.
    ///
    /// This does not itself mutate the target — it records the change around
    /// your edit. That keeps the primitive composable: the caller owns the edit
    /// (a `create_node` replace, a `delete_node`, …); this owns the history.
    pub fn record_change(
        &mut self,
        rec: ChangeRecord<'_>,
    ) -> Result<(Option<StoredNode>, StoredNode), DynoError> {
        // A snapshot captures the state of things NOW, so it cannot belong to a
        // point that has not happened. Refusing here is what makes
        // `DesignEpoch.status` a property something READS rather than one more
        // declared-and-unconsulted field (`req:defaults-do-not-assert`'s sibling
        // defect), and it fails loud rather than quietly filing history under a
        // future date (`req:no-silent-fallback`).
        if self.epoch_is_planned(rec.epoch_id)? {
            return Err(DynoError::Validation {
                node_type: node::DESIGN_EPOCH.into(),
                property: "status".into(),
                message: format!(
                    "epoch '{}' is PLANNED — it has not happened, so history cannot be recorded \
                     into it. Record this change in an epoch that has arrived, or call \
                     set_epoch_status to mark this one `arrived` first if it now has.",
                    rec.epoch_id
                ),
            });
        }
        let snapshot = if rec.action.has_prior_state() {
            Some(self.snapshot_node(rec.epoch_id, rec.target_type, rec.target_id)?)
        } else {
            None
        };

        // UNSTATED on purpose. `record_change` is the generic path and cannot
        // know which axis the caller is on — a resync can be either — so it
        // passes None rather than inferring one. Absent means nobody said,
        // which is true; a guess here would be a claim nobody made.
        let change_event = self.add_change_event(
            rec.change_event_id,
            rec.name,
            rec.change_type,
            None,
            None,
            None,
        )?;
        self.pin_at_epoch(node::CHANGE_EVENT, rec.change_event_id, rec.epoch_id)?;
        self.changed(
            rec.change_event_id,
            rec.target_type,
            rec.target_id,
            rec.action,
        )?;

        Ok((snapshot, change_event))
    }
}

impl DesignGraph {
    /// Is a node's PRIOR state actually preserved anywhere, or is this
    /// replacement unrecoverable?
    ///
    /// Returns the id of a Snapshot holding exactly `content_hash`, or `None`.
    ///
    /// # Why compute it rather than restate the rule
    ///
    /// `req:a-discipline-is-delivered-at-the-tool-not-in-a-catalogue`. The
    /// revision block a write already returns says *"`record_change` BEFORE
    /// the merge is what puts the old state in the design's own timeline"* —
    /// **unconditionally**, whether the caller snapshotted or not. That is the
    /// catalogue problem in miniature: advice delivered regardless of state,
    /// which a reader learns to skim precisely because it never varies.
    ///
    /// The requirement's stronger form is to compute the OUTCOME rather than
    /// track the invocation — *"do not track whether a skill was INVOKED,
    /// compute whether its OUTCOME IS PRESENT"* — because that survives an
    /// agent which ignores every hint. dev_storyflow's dragon Boss proposed the
    /// identical shape independently: *"have delete_edge / a revising
    /// create_node report whether the target has a snapshot at the current
    /// epoch — NOT BLOCK, JUST SAY."*
    ///
    /// # Why by content hash and not by epoch
    ///
    /// "At the current epoch" needs a notion of *current* that reflow2 does not
    /// have, and it answers a weaker question. Matching the hash answers the
    /// one a caller actually has: **is the state I just replaced recoverable?**
    /// Verified 2026-08-18 that a Snapshot's stored `state` hashes to exactly
    /// the `prior_content_hash` a revision reports, so the comparison is exact
    /// rather than approximate.
    ///
    /// # Honest cost
    ///
    /// Scans every Snapshot, so it is O(snapshots) per revising write. At 144
    /// snapshots that is nothing; on a design with a hundred thousand it would
    /// need an index on `target_id`. Stated rather than discovered later.
    pub fn snapshot_preserving(
        &self,
        target_id: &str,
        content_hash: &str,
    ) -> Result<Option<String>, DynoError> {
        for snapshot in self.scan_nodes(node::SNAPSHOT)? {
            if snapshot.properties.get("target_id").and_then(Value::as_str) != Some(target_id) {
                continue;
            }
            // A snapshot that will not parse is not evidence of preservation,
            // and it is also not this function's business to complain about —
            // `detect_defects` owns malformed nodes.
            let Ok(state) = parse_snapshot_state(&snapshot) else {
                continue;
            };
            if crate::graph::node_content_hash(&state) == content_hash {
                return Ok(Some(snapshot.node_id));
            }
        }
        Ok(None)
    }
}

/// Read the `state` JSON a [`snapshot_node`](DesignGraph::snapshot_node) stored
/// back into a property bag. A convenience for callers diffing across epochs.
pub fn parse_snapshot_state(
    snapshot: &StoredNode,
) -> Result<std::collections::HashMap<String, Value>, DynoError> {
    let state = snapshot
        .properties
        .get("state")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            DynoError::Serialization(format!(
                "snapshot '{}' has no string `state` property",
                snapshot.node_id
            ))
        })?;
    serde_json::from_str(state)
        .map_err(|e| DynoError::Serialization(format!("parse snapshot state: {e}")))
}

/// Read the `edges` JSON a [`snapshot_node`](DesignGraph::snapshot_node) stored
/// back into typed [`SnapshotEdge`]s. A snapshot taken before BL-63 has no
/// `edges` property; that is an empty capture, not an error — the edge history
/// simply was not recorded then, and pretending otherwise would invent a past.
pub fn parse_snapshot_edges(snapshot: &StoredNode) -> Result<Vec<SnapshotEdge>, DynoError> {
    let Some(edges) = snapshot.properties.get("edges").and_then(Value::as_str) else {
        return Ok(Vec::new());
    };
    serde_json::from_str(edges)
        .map_err(|e| DynoError::Serialization(format!("parse snapshot edges: {e}")))
}
