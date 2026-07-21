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
}

impl ChangeType {
    /// The exact schema enum string.
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeType::RequirementCreep => "requirement_creep",
            ChangeType::NewFeature => "new_feature",
            ChangeType::TestFailureFix => "test_failure_fix",
            ChangeType::PerformanceOptimization => "performance_optimization",
            ChangeType::Refactor => "refactor",
            ChangeType::ScopeChange => "scope_change",
            ChangeType::ConstraintChange => "constraint_change",
            ChangeType::EnvironmentChange => "environment_change",
            ChangeType::Deprecation => "deprecation",
            ChangeType::Resync => "resync",
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

/// Deterministic id for the snapshot of `node_id` taken at `epoch_id`.
/// Stable so re-snapshotting the same node at the same epoch is idempotent
/// (create-or-replace) rather than accumulating duplicates.
fn snapshot_id(epoch_id: &str, node_id: &str) -> String {
    format!("snap:{epoch_id}:{node_id}")
}

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
        self.create_node(
            node::DESIGN_EPOCH,
            id,
            Props::new()
                .set("name", name)
                .set("epoch_type", epoch_type.as_str())
                .set("sequence", sequence),
        )
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
    /// excluded: a snapshot captures design structure, not the audit trail.
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
            if BOOKKEEPING_TYPES.contains(&other_type.as_str()) {
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

        let snap_id = snapshot_id(epoch_id, node_id);
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
    pub fn add_change_event(
        &mut self,
        id: &str,
        name: &str,
        change_type: ChangeType,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::CHANGE_EVENT,
            id,
            Props::new()
                .set("name", name)
                .set("change_type", change_type.as_str()),
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
        let snapshot = if rec.action.has_prior_state() {
            Some(self.snapshot_node(rec.epoch_id, rec.target_type, rec.target_id)?)
        } else {
            None
        };

        let change_event = self.add_change_event(rec.change_event_id, rec.name, rec.change_type)?;
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
