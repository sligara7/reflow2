//! Axis-Z (temporal) tests — the coherence loop's CHANGE step.
//!
//! The load-bearing property: **the past is never overwritten.** When a node is
//! edited, `record_change` snapshots its prior state pinned to an epoch, so the
//! old state is still reconstructable after the live node moves on.

use reflow2_core::nodes::{edge, node};
use reflow2_core::{
    BaselineSource, ChangeAction, ChangeRecord, ChangeType, DesignGraph, EpochType,
    ScheduleOutcome, SnapshotEdge, parse_snapshot_edges, parse_snapshot_state,
};

#[test]
fn epochs_order_via_sequence_and_precedes() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_epoch("epoch:v1", "v1 baseline", EpochType::Baseline, 0)
        .unwrap();
    g.add_epoch("epoch:v1.1", "v1.1 creep", EpochType::Revision, 1)
        .unwrap();
    g.precedes("epoch:v1", "epoch:v1.1").unwrap();

    let base = g.get_node(node::DESIGN_EPOCH, "epoch:v1").unwrap().unwrap();
    assert_eq!(base.properties["epoch_type"].as_str(), Some("baseline"));
    assert_eq!(base.properties["sequence"].as_i64(), Some(0));

    let order = g.outgoing("epoch:v1", Some(edge::PRECEDES)).unwrap();
    assert_eq!(order.len(), 1);
    assert_eq!(order[0].to_id, "epoch:v1.1");
}

#[test]
fn record_change_preserves_pre_change_state() {
    let mut g = DesignGraph::open_in_memory().unwrap();

    // Baseline: a requirement, accepted at v1.
    g.add_epoch("epoch:v1", "v1 baseline", EpochType::Baseline, 0)
        .unwrap();
    g.add_requirement("req:latency", "Latency", "Respond within 200ms")
        .unwrap();

    // v1.1: the requirement is tightened (a modification). Record the change
    // FIRST (snapshots the old state), THEN apply the edit.
    g.add_epoch("epoch:v1.1", "v1.1 creep", EpochType::Revision, 1)
        .unwrap();
    let (snapshot, change_event) = g
        .record_change(ChangeRecord {
            epoch_id: "epoch:v1.1",
            change_event_id: "chg:tighten-latency",
            name: "Tighten latency to 100ms",
            change_type: ChangeType::RequirementCreep,
            target_type: node::REQUIREMENT,
            target_id: "req:latency",
            action: ChangeAction::Modified,
        })
        .unwrap();

    // Apply the actual edit (create-or-replace with the same id).
    g.add_requirement("req:latency", "Latency", "Respond within 100ms")
        .unwrap();

    // The live node now holds the NEW statement...
    let live = g
        .get_node(node::REQUIREMENT, "req:latency")
        .unwrap()
        .unwrap();
    assert_eq!(
        live.properties["statement"].as_str(),
        Some("Respond within 100ms")
    );

    // ...but the snapshot preserved the OLD statement — the past is intact.
    let snapshot = snapshot.expect("a Modified change must produce a snapshot");
    let old_state = parse_snapshot_state(&snapshot).unwrap();
    assert_eq!(
        old_state["statement"].as_str(),
        Some("Respond within 200ms"),
        "snapshot must hold the pre-change statement"
    );
    assert_eq!(
        snapshot.properties["target_id"].as_str(),
        Some("req:latency")
    );

    // Wiring: ChangeEvent -CHANGED-> requirement, with action=modified...
    let changed = g
        .outgoing(&change_event.node_id, Some(edge::CHANGED))
        .unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].to_id, "req:latency");
    assert_eq!(changed[0].properties["action"].as_str(), Some("modified"));

    // ...and both the ChangeEvent and the Snapshot are pinned to v1.1.
    let ce_epoch = g
        .outgoing(&change_event.node_id, Some(edge::AT_EPOCH))
        .unwrap();
    assert_eq!(ce_epoch.len(), 1);
    assert_eq!(ce_epoch[0].to_id, "epoch:v1.1");

    let snap_epoch = g.outgoing(&snapshot.node_id, Some(edge::AT_EPOCH)).unwrap();
    assert_eq!(snap_epoch.len(), 1);
    assert_eq!(snap_epoch[0].to_id, "epoch:v1.1");

    // The requirement carries a HAS_SNAPSHOT edge to its captured past.
    let has_snap = g.outgoing("req:latency", Some(edge::HAS_SNAPSHOT)).unwrap();
    assert_eq!(has_snap.len(), 1);
    assert_eq!(has_snap[0].to_id, snapshot.node_id);
}

#[test]
fn added_change_takes_no_snapshot() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_epoch("epoch:v1", "v1", EpochType::Baseline, 0)
        .unwrap();
    // A brand-new capability added at v1 — create it, then record the add.
    g.add_capability("cap:new", "New cap", "A freshly added capability", None)
        .unwrap();
    let (snapshot, _ce) = g
        .record_change(ChangeRecord {
            epoch_id: "epoch:v1",
            change_event_id: "chg:add-cap",
            name: "Add caching capability",
            change_type: ChangeType::NewFeature,
            target_type: node::CAPABILITY,
            target_id: "cap:new",
            action: ChangeAction::Added,
        })
        .unwrap();
    assert!(
        snapshot.is_none(),
        "an Added change has no prior state to snapshot"
    );
    assert_eq!(g.count_nodes(node::SNAPSHOT).unwrap(), 0);
}

#[test]
fn snapshot_of_missing_node_fails_loud() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_epoch("epoch:v1", "v1", EpochType::Baseline, 0)
        .unwrap();
    let err = g.snapshot_node("epoch:v1", node::COMPONENT, "cmp:ghost");
    assert!(
        err.is_err(),
        "snapshotting a nonexistent node must fail, not silently no-op"
    );
}

#[test]
fn snapshot_state_keys_are_sorted_for_byte_stable_exports() {
    // BL-58: `state` was serialized from a HashMap, so its key order was
    // process-random — two exports of identical history then differed. The
    // keys must come out sorted (deterministic across processes).
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:x", "X requirement", "must hold")
        .unwrap();
    let epoch = g
        .add_epoch("epoch:1", "e1", EpochType::Baseline, 0)
        .unwrap();

    let snap = g
        .snapshot_node(&epoch.node_id, node::REQUIREMENT, "req:x")
        .unwrap();
    let state = snap.properties["state"].as_str().unwrap();

    // Extract the top-level key appearance order and assert it is sorted.
    let keys: Vec<&str> = state
        .match_indices("\":")
        .filter_map(|(i, _)| state[..i].rfind('"').map(|s| &state[s + 1..i]))
        .collect();
    assert!(
        keys.len() >= 3,
        "the requirement has several properties: {state}"
    );
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(
        keys, sorted,
        "snapshot state keys must be sorted, got {keys:?}"
    );
}

// ---- BL-63 · a snapshot captures edges, so an edge move keeps its history ----

/// The reallocation demo that raised BL-63: "Service A does X, Y, Z" → later,
/// Z moves to Service B. Before BL-63 the snapshot held only cap:z's
/// properties, so a lazy reallocation (delete_edge + allocate, no Decision)
/// left Z on B with no trace it was ever on A. The snapshot must carry the
/// lost `ALLOCATED_TO`.
#[test]
fn a_reallocation_keeps_the_old_owner_in_the_snapshot() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_component("cmp:a", "Service A", "did X, Y, Z", None)
        .unwrap();
    g.add_component("cmp:b", "Service B", "takes Z", None)
        .unwrap();
    g.add_capability("cap:z", "Reconcile", "does Z", None)
        .unwrap();
    g.allocate("cap:z", "cmp:a").unwrap();
    g.add_epoch("epoch:v2", "reallocation", EpochType::Revision, 1)
        .unwrap();

    // The right-way sequence: record first (snapshot while cap:z still says
    // the OLD thing), then move the edge.
    let (snapshot, _ce) = g
        .record_change(ChangeRecord {
            epoch_id: "epoch:v2",
            change_event_id: "chg:move-z",
            name: "Z moves from A to B",
            change_type: ChangeType::ScopeChange,
            target_type: node::CAPABILITY,
            target_id: "cap:z",
            action: ChangeAction::Modified,
        })
        .unwrap();
    g.delete_edge(edge::ALLOCATED_TO, "cap:z", "cmp:a").unwrap();
    g.allocate("cap:z", "cmp:b").unwrap();

    let snapshot = snapshot.expect("Modified must snapshot");
    let edges = parse_snapshot_edges(&snapshot).unwrap();
    let old_owner: Vec<&SnapshotEdge> = edges
        .iter()
        .filter(|e| e.edge_type == "ALLOCATED_TO" && e.direction == "out")
        .collect();
    assert_eq!(
        old_owner.len(),
        1,
        "the snapshot must hold the pre-move allocation: {edges:?}"
    );
    assert_eq!(
        old_owner[0].other_id, "cmp:a",
        "A once owned Z, on the record"
    );
    assert_eq!(old_owner[0].other_type, node::COMPONENT);

    // The live graph says B owns Z now — history did not freeze the present.
    let live = g.outgoing("cap:z", Some(edge::ALLOCATED_TO)).unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].to_id, "cmp:b");
}

/// A snapshot captures design structure, not the audit trail: re-snapshotting
/// a node must not accumulate `HAS_SNAPSHOT`/`AT_EPOCH`/`CHANGED` edges from
/// earlier rounds of history — and the captured list must be deterministically
/// ordered (byte-stable exports, the BL-58 discipline).
#[test]
fn snapshot_edges_exclude_bookkeeping_and_are_sorted() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:r", "R", "must hold").unwrap();
    g.add_capability("cap:c", "C", "does it", None).unwrap();
    g.add_component("cmp:m", "M", "hosts it", None).unwrap();
    g.satisfies("cap:c", "req:r").unwrap();
    g.allocate("cap:c", "cmp:m").unwrap();
    g.add_epoch("epoch:1", "e1", EpochType::Baseline, 0)
        .unwrap();
    g.add_epoch("epoch:2", "e2", EpochType::Revision, 1)
        .unwrap();

    // First round of history: snapshot + change event against cap:c.
    g.record_change(ChangeRecord {
        epoch_id: "epoch:1",
        change_event_id: "chg:first",
        name: "first edit",
        change_type: ChangeType::Refactor,
        target_type: node::CAPABILITY,
        target_id: "cap:c",
        action: ChangeAction::Modified,
    })
    .unwrap();

    // Second snapshot: cap:c now carries HAS_SNAPSHOT (to the first snapshot)
    // and an incoming CHANGED (from chg:first). Neither may be captured.
    let snap2 = g
        .snapshot_node("epoch:2", node::CAPABILITY, "cap:c")
        .unwrap();
    let edges = parse_snapshot_edges(&snap2).unwrap();
    let types: Vec<&str> = edges.iter().map(|e| e.edge_type.as_str()).collect();
    assert!(
        !types.contains(&"HAS_SNAPSHOT") && !types.contains(&"CHANGED"),
        "bookkeeping edges leaked into the snapshot: {types:?}"
    );
    // What it must hold: the design edges — SATISFIES out, ALLOCATED_TO out,
    // CONTAINS in (from the project).
    assert!(types.contains(&"SATISFIES") && types.contains(&"ALLOCATED_TO"));

    // Deterministic order: sorted by (direction, edge_type, other_id).
    let keys: Vec<(&str, &str, &str)> = edges
        .iter()
        .map(|e| {
            (
                e.direction.as_str(),
                e.edge_type.as_str(),
                e.other_id.as_str(),
            )
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "snapshot edges must be sorted: {keys:?}");
}

/// A snapshot taken before BL-63 has no `edges` property. That is an empty
/// capture, not an error — the history was not recorded then, and inventing
/// one would overwrite the past with a guess.
#[test]
fn a_pre_bl63_snapshot_reads_as_no_edges_not_an_error() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let old_snap = g
        .create_node(
            node::SNAPSHOT,
            "snap:old",
            reflow2_core::nodes::Props::new()
                .set("target_id", "req:x")
                .set("target_type", node::REQUIREMENT)
                .set("state", "{\"name\":\"X\"}"),
        )
        .unwrap();
    let edges = parse_snapshot_edges(&old_snap).unwrap();
    assert!(
        edges.is_empty(),
        "absent edges must read as empty: {edges:?}"
    );
}

/// TWO REVISIONS IN ONE EPOCH KEEP TWO SNAPSHOTS.
///
/// The regression this file's own header promises against: "the past is never
/// overwritten". The snapshot id was `snap:{epoch}:{node}` and nothing else,
/// while `create_node` MERGES on an existing id — so a node revised twice
/// inside one epoch had its FIRST snapshot silently replaced by its second, and
/// `record_change` returned success both times. Found 2026-07-28 by amending
/// one requirement twice in a single epoch; the original text survived only in
/// a previously committed export, which is the git archaeology the
/// revise-design skill promises to make unnecessary.
#[test]
fn a_second_revision_in_one_epoch_does_not_overwrite_the_first_snapshot() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:r", "R", "the ORIGINAL statement")
        .unwrap();
    g.add_epoch("epoch:e", "e", EpochType::Revision, 10)
        .unwrap();

    // First revision: snapshot the original, then edit.
    g.snapshot_node("epoch:e", node::REQUIREMENT, "req:r")
        .unwrap();
    g.add_requirement("req:r", "R", "the FIRST amendment")
        .unwrap();

    // Second revision, same epoch: snapshot the first amendment, then edit.
    g.snapshot_node("epoch:e", node::REQUIREMENT, "req:r")
        .unwrap();
    g.add_requirement("req:r", "R", "the SECOND amendment")
        .unwrap();

    assert_eq!(
        g.count_nodes(node::SNAPSHOT).unwrap(),
        2,
        "two distinct revisions must leave two snapshots, not one"
    );

    // The ORIGINAL must still be reachable — that is the whole point.
    let first = g
        .get_node(node::SNAPSHOT, "snap:epoch:e:req:r")
        .unwrap()
        .expect("the first snapshot keeps the base id");
    let first_state = parse_snapshot_state(&first).unwrap();
    assert_eq!(
        first_state["statement"].as_str(),
        Some("the ORIGINAL statement"),
        "the first snapshot must still hold the pre-amendment state"
    );

    let second = g
        .get_node(node::SNAPSHOT, "snap:epoch:e:req:r:r2")
        .unwrap()
        .expect("a genuine second revision appends :r2");
    let second_state = parse_snapshot_state(&second).unwrap();
    assert_eq!(
        second_state["statement"].as_str(),
        Some("the FIRST amendment"),
        "the second snapshot holds the state as of the second revision"
    );
}

/// The first capture KEEPS the historical id, so existing graphs and exports do
/// not need migrating — only a real second revision appends a suffix.
#[test]
fn the_first_snapshot_in_an_epoch_keeps_the_unsuffixed_id() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:r", "R", "only ever stated once")
        .unwrap();
    g.add_epoch("epoch:e", "e", EpochType::Revision, 10)
        .unwrap();

    let snap = g
        .snapshot_node("epoch:e", node::REQUIREMENT, "req:r")
        .unwrap();
    assert_eq!(snap.node_id, "snap:epoch:e:req:r");
}

/// Re-snapshotting a node that has NOT moved returns the existing snapshot
/// rather than minting `:r2`. Without this, the fix for the overwrite bug would
/// invent the mirror-image lie — a history claiming revisions that never
/// happened — and `record_change` would stop being safe to retry.
#[test]
fn an_identical_re_snapshot_is_idempotent() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:r", "R", "unchanged between captures")
        .unwrap();
    g.add_epoch("epoch:e", "e", EpochType::Revision, 10)
        .unwrap();

    let a = g
        .snapshot_node("epoch:e", node::REQUIREMENT, "req:r")
        .unwrap();
    let b = g
        .snapshot_node("epoch:e", node::REQUIREMENT, "req:r")
        .unwrap();

    assert_eq!(
        a.node_id, b.node_id,
        "an unchanged node re-snapshots to the same node"
    );
    assert_eq!(
        g.count_nodes(node::SNAPSHOT).unwrap(),
        1,
        "no second snapshot may be minted when nothing changed"
    );
}

/// An EDGE move alone is a revision. BL-63 made snapshots capture design edges
/// precisely because re-allocation is a change with no property edit; if only
/// `state` were compared, that class of change would silently overwrite again.
#[test]
fn an_edge_only_change_counts_as_a_distinct_revision() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:c", "C", "a capability", None)
        .unwrap();
    g.add_component("cmp:a", "A", "first home", None).unwrap();
    g.add_component("cmp:b", "B", "second home", None).unwrap();
    g.add_epoch("epoch:e", "e", EpochType::Revision, 10)
        .unwrap();

    g.allocate("cap:c", "cmp:a").unwrap();
    g.snapshot_node("epoch:e", node::CAPABILITY, "cap:c")
        .unwrap();

    // Re-allocate: no property changes at all, only edges.
    g.delete_edge(edge::ALLOCATED_TO, "cap:c", "cmp:a").unwrap();
    g.allocate("cap:c", "cmp:b").unwrap();
    g.snapshot_node("epoch:e", node::CAPABILITY, "cap:c")
        .unwrap();

    assert_eq!(
        g.count_nodes(node::SNAPSHOT).unwrap(),
        2,
        "an allocation move is a revision even though no property changed"
    );
    let first = g
        .get_node(node::SNAPSHOT, "snap:epoch:e:cap:c")
        .unwrap()
        .unwrap();
    let edges = parse_snapshot_edges(&first).unwrap();
    assert!(
        edges.iter().any(|e| e.other_id == "cmp:a"),
        "the first snapshot must still record the ORIGINAL owner: {edges:?}"
    );
}

/// Returning to an EARLIER state is still a revision. Idempotence compares
/// against the tail of the chain and nothing before it, so A → B → A records
/// three captures. Matching any earlier snapshot instead would hand back the
/// first A-capture for the third revision and record only two — hiding an edit
/// that did happen, which is the quiet half of the bug this whole section
/// exists to close.
#[test]
fn returning_to_an_earlier_state_still_mints_a_revision() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:r", "R", "state A").unwrap();
    g.add_epoch("epoch:e", "e", EpochType::Revision, 10)
        .unwrap();

    g.snapshot_node("epoch:e", node::REQUIREMENT, "req:r")
        .unwrap();
    g.add_requirement("req:r", "R", "state B").unwrap();

    g.snapshot_node("epoch:e", node::REQUIREMENT, "req:r")
        .unwrap();
    g.add_requirement("req:r", "R", "state A").unwrap();

    let third = g
        .snapshot_node("epoch:e", node::REQUIREMENT, "req:r")
        .unwrap();

    assert_eq!(
        third.node_id, "snap:epoch:e:req:r:r3",
        "the third revision takes the next id, not the first snapshot holding the same state"
    );
    assert_eq!(
        g.count_nodes(node::SNAPSHOT).unwrap(),
        3,
        "A -> B -> A is three revisions, and the chain order is what makes them readable"
    );
}

// ---------------------------------------------------------------------------
// PLANNED EPOCHS — the forward half of the time axis
// (`req:epochs-can-be-planned`, first increment).
// ---------------------------------------------------------------------------

#[test]
fn an_epoch_is_arrived_unless_someone_plans_it() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_epoch("epoch:now", "Now", EpochType::Revision, 1)
        .unwrap();
    assert!(
        !g.epoch_is_planned("epoch:now").unwrap(),
        "add_epoch has always meant 'record the point I am at', and all 27 of its \
         existing call sites mean exactly that"
    );

    g.plan_epoch("epoch:later", "Later", EpochType::Milestone, 2)
        .unwrap();
    assert!(g.epoch_is_planned("epoch:later").unwrap());
}

/// Kind and tense are orthogonal. Folding `planned` into `epoch_type` would
/// have made this unsayable.
#[test]
fn a_planned_epoch_keeps_its_kind() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.plan_epoch("epoch:m", "A milestone we expect", EpochType::Milestone, 5)
        .unwrap();
    let e = g.get_node("DesignEpoch", "epoch:m").unwrap().unwrap();
    assert_eq!(e.properties["epoch_type"].as_str(), Some("milestone"));
    assert_eq!(e.properties["status"].as_str(), Some("planned"));
}

/// THE READER. Without this the status would be one more declared-and-unread
/// property — the defect this project keeps finding.
#[test]
fn history_cannot_be_recorded_into_an_epoch_that_has_not_happened() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "a thing").unwrap();
    g.plan_epoch("epoch:future", "Future", EpochType::Revision, 9)
        .unwrap();

    let err = g
        .record_change(ChangeRecord {
            epoch_id: "epoch:future",
            change_event_id: "chg:x",
            name: "something",
            target_type: "Requirement",
            target_id: "req:a",
            change_type: ChangeType::ScopeChange,
            action: ChangeAction::Modified,
        })
        .expect_err("a snapshot of the present cannot belong to a point that has not happened");
    let said = format!("{err:?}");
    assert!(
        said.contains("PLANNED") && said.contains("set_epoch_status"),
        "the refusal must say what would have worked (rule 4), got: {said}"
    );
}

#[test]
fn an_arrived_epoch_accepts_history_again() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "a thing").unwrap();
    g.plan_epoch("epoch:soon", "Soon", EpochType::Revision, 3)
        .unwrap();
    g.set_epoch_status("epoch:soon", "arrived").unwrap();

    g.record_change(ChangeRecord {
        epoch_id: "epoch:soon",
        change_event_id: "chg:x",
        name: "it arrived",
        target_type: "Requirement",
        target_id: "req:a",
        change_type: ChangeType::ScopeChange,
        action: ChangeAction::Modified,
    })
    .expect("once an epoch has arrived, history belongs in it");
}

#[test]
fn arrival_preserves_everything_else_about_the_epoch() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.plan_epoch("epoch:s", "Named plan", EpochType::ReleaseCut, 7)
        .unwrap();
    g.set_epoch_status("epoch:s", "arrived").unwrap();
    let e = g.get_node("DesignEpoch", "epoch:s").unwrap().unwrap();
    assert_eq!(e.properties["name"].as_str(), Some("Named plan"));
    assert_eq!(e.properties["epoch_type"].as_str(), Some("release_cut"));
    assert_eq!(e.properties["sequence"].as_i64(), Some(7));
}

#[test]
fn an_unknown_epoch_status_is_refused_by_name() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_epoch("epoch:x", "X", EpochType::Revision, 1).unwrap();
    let err = g
        .set_epoch_status("epoch:x", "someday")
        .expect_err("a status outside the enum must be refused");
    let said = format!("{err:?}");
    assert!(
        said.contains("planned") && said.contains("arrived"),
        "got: {said}"
    );
}

// ---- The satisfaction schedule (SCHEDULED_FOR) ---------------------------
//
// `req:epochs-can-be-planned`, second increment. The roadmap is a mapping of
// requirements and capabilities to the moment they are due — and WHICH KIND
// of claim that is, which is what makes a miss computable rather than merely
// disappointing.

/// The schedule reaches both of the paired views: a DesignEpoch for the time
/// axis, a Release for the capability-increment axis. One edge type, because
/// they are two views of one architecture rather than two mechanisms.
#[test]
fn a_schedule_points_at_either_paired_view() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:x", "X", "does x", None).unwrap();
    g.plan_epoch("epoch:5", "Epoch 5", EpochType::Milestone, 5)
        .unwrap();
    g.add_release("rel:inc13", "Increment 13", Some("13"), None)
        .unwrap();

    g.schedule_for(
        "Capability",
        "cap:x",
        "DesignEpoch",
        "epoch:5",
        "expected",
        None,
    )
    .expect("an epoch is a moment on the time axis");
    g.schedule_for(
        "Capability",
        "cap:x",
        "Release",
        "rel:inc13",
        "expected",
        None,
    )
    .expect("a release is a moment on the capability-increment axis");
}

/// Modality is what separates a plan from an obligation. Without it the
/// schedule cannot say which misses are violations, and `required` is the
/// scheduling face of a KPP.
#[test]
fn a_schedule_records_whether_it_is_a_plan_or_an_obligation() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:kpp", "A KPP", "must hold").unwrap();
    g.plan_epoch("epoch:27", "Epoch 27", EpochType::Milestone, 27)
        .unwrap();

    g.schedule_for(
        "Requirement",
        "req:kpp",
        "DesignEpoch",
        "epoch:27",
        "required",
        Some("2026-07-30"),
    )
    .unwrap();

    let sched = g
        .outgoing("req:kpp", Some(edge::SCHEDULED_FOR))
        .unwrap()
        .into_iter()
        .next()
        .expect("the schedule edge exists");
    assert_eq!(
        sched.properties.get("modality").and_then(|v| v.as_str()),
        Some("required"),
        "an obligation must be distinguishable from a plan"
    );
}

/// THE ABSENT MODALITY IS THE POINT. Delivery is computed from the golden
/// thread and never asserted, so a schedule that could record its own success
/// would be a second source of truth able to disagree with the first.
#[test]
fn a_schedule_cannot_claim_its_own_delivery() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:x", "X", "does x", None).unwrap();
    g.add_epoch("epoch:3", "Epoch 3", EpochType::Milestone, 3)
        .unwrap();

    let err = g
        .schedule_for(
            "Capability",
            "cap:x",
            "DesignEpoch",
            "epoch:3",
            "achieved",
            None,
        )
        .expect_err("`achieved` is not a schedule modality — delivery is computed");
    let said = format!("{err}");
    // Assert on the EXPLANATION, not merely on a refusal. The schema enum also
    // rejects `achieved`, with "invalid enum value" — so an assertion that only
    // checked for a refusal would pass with this guard deleted and prove
    // nothing. What this guard adds is the reason, and the reason is the point.
    assert!(
        said.contains("computed from the golden thread"),
        "the refusal must say WHY there is no `achieved`, not just that it is invalid: {said}"
    );
}

/// A schedule points at a MOMENT. Pointing it at an ordinary design node would
/// make "due at" meaningless, and the wildcard-source mistake is exactly what
/// keeping this separate from AT_EPOCH was meant to avoid.
#[test]
fn a_schedule_refuses_a_target_that_is_not_a_moment() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:x", "X", "does x", None).unwrap();
    g.add_requirement("req:y", "Y", "must hold").unwrap();

    let err = g
        .schedule_for(
            "Capability",
            "cap:x",
            "Requirement",
            "req:y",
            "expected",
            None,
        )
        .expect_err("a requirement is not a moment");
    let said = format!("{err}");
    assert!(
        said.contains("DesignEpoch") && said.contains("Release"),
        "the refusal must name both paired views: {said}"
    );
}

/// The default is `expected`, because the ordinary act of scheduling is
/// planning; an obligation is the deliberate one.
#[test]
fn scheduling_defaults_to_a_plan_rather_than_an_obligation() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:x", "X", "does x", None).unwrap();
    g.plan_epoch("epoch:5", "Epoch 5", EpochType::Milestone, 5)
        .unwrap();
    g.schedule_for(
        "Capability",
        "cap:x",
        "DesignEpoch",
        "epoch:5",
        "expected",
        None,
    )
    .unwrap();

    let sched = g
        .outgoing("cap:x", Some(edge::SCHEDULED_FOR))
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(
        sched.properties.get("modality").and_then(|v| v.as_str()),
        Some("expected")
    );
}

// ---- The arrival delta (dec:arrival-delta) --------------------------------
//
// `req:epochs-can-be-planned`, third increment, delivering obligation 2 of
// `req:plans-move-honestly`. Anthony's example is the fixture: epoch 3 planned
// A, B and C; when epoch 3 actually arrived only A and C landed, and B slipped
// to epoch 4. The property under test is that the graph can still SAY that
// afterwards — the plan must not have quietly rewritten itself to look as
// though epoch 3 was only ever about A and C.

/// Epoch 3 planned with A, B and C, and an arrived epoch to file history into.
/// Nothing is delivered yet; each test threads what it needs.
fn planned_epoch_3() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_epoch("epoch:now", "Now", EpochType::Revision, 1)
        .unwrap();
    g.plan_epoch("epoch:3", "Epoch 3", EpochType::Milestone, 3)
        .unwrap();
    g.plan_epoch("epoch:4", "Epoch 4", EpochType::Milestone, 4)
        .unwrap();
    for (id, name) in [("req:a", "A"), ("req:b", "B"), ("req:c", "C")] {
        g.add_requirement(id, name, "needed").unwrap();
        g.schedule_for(
            "Requirement",
            id,
            "DesignEpoch",
            "epoch:3",
            "expected",
            None,
        )
        .unwrap();
    }
    g
}

/// Record the plan for `epoch` — the call that must precede any move.
fn record_the_plan(g: &mut DesignGraph, change_id: &str, epoch: &str) {
    g.record_change(ChangeRecord {
        epoch_id: "epoch:now",
        change_event_id: change_id,
        name: "replan",
        change_type: ChangeType::Refactor,
        target_type: node::DESIGN_EPOCH,
        target_id: epoch,
        action: ChangeAction::Modified,
    })
    .unwrap();
}

/// Thread `req` all the way to delivered: satisfied by a realized capability
/// with a passing check. Delivery is COMPUTED from this, never asserted.
fn deliver(g: &mut DesignGraph, req: &str, suffix: &str) {
    let cap = format!("cap:{suffix}");
    let art = format!("art:{suffix}");
    let ver = format!("ver:{suffix}");
    g.add_capability(&cap, suffix, "does it", Some("realized"))
        .unwrap();
    g.satisfies(&cap, req).unwrap();
    g.add_artifact(&art, suffix, Some("code"), Some("src/x.rs"))
        .unwrap();
    g.realizes(&art, node::CAPABILITY, &cap, None, None)
        .unwrap();
    g.add_verification(&ver, suffix, Some("test"), None)
        .unwrap();
    g.verifies(&ver, node::CAPABILITY, &cap).unwrap();
    g.set_verification_status(&ver, "passing", None).unwrap();
}

/// THE FIX AT THE BOTTOM OF ALL OF THIS. `snapshot_node` excluded every edge
/// whose other endpoint was a bookkeeping type, and `DesignEpoch` is one — a
/// proxy that was exact until `SCHEDULED_FOR` made an edge to an epoch a
/// COMMITMENT. So `record_change` on a scheduled requirement, the obvious way
/// to record a slip, destroyed the due date it was called to preserve, and
/// reported success. Excluding by the edge's ROLE keeps it.
#[test]
fn a_schedule_edge_survives_a_snapshot_of_the_item() {
    let mut g = planned_epoch_3();
    let snap = g
        .snapshot_node("epoch:now", node::REQUIREMENT, "req:b")
        .unwrap();

    let edges = parse_snapshot_edges(&snap).unwrap();
    assert!(
        edges.iter().any(|e| e.edge_type == edge::SCHEDULED_FOR
            && e.other_id == "epoch:3"
            && e.direction == "out"),
        "the commitment must survive the snapshot, got: {edges:?}"
    );
    // The audit trail it travels with must still be excluded, or every
    // snapshot grows with its own history.
    assert!(
        !edges.iter().any(|e| e.edge_type == edge::AT_EPOCH),
        "role, not endpoint type — AT_EPOCH is still bookkeeping"
    );
}

/// THE LOAD-BEARING HALF, and the same shape as `record_change` refusing a
/// planned epoch. Re-pointing B's edge without recording the plan first is
/// exactly how epoch 3 comes to say it was only ever about A and C.
#[test]
fn un_scheduling_is_refused_while_the_plan_is_unrecorded() {
    let mut g = planned_epoch_3();
    let err = g
        .delete_edge(edge::SCHEDULED_FOR, "req:b", "epoch:3")
        .expect_err("a plan nobody recorded cannot be quietly moved");
    let said = format!("{err:?}");
    assert!(
        said.contains("record_change") && said.contains("epoch:3"),
        "the refusal must say what to do instead, got: {said}"
    );
}

/// And the refusal is actionable rather than a wall: record the plan, then the
/// plan may move.
#[test]
fn recording_the_change_first_lets_the_plan_move() {
    let mut g = planned_epoch_3();
    record_the_plan(&mut g, "chg:replan", "epoch:3");
    g.delete_edge(edge::SCHEDULED_FOR, "req:b", "epoch:3")
        .expect("the plan is on the record now");
    g.schedule_for(
        "Requirement",
        "req:b",
        "DesignEpoch",
        "epoch:4",
        "expected",
        None,
    )
    .unwrap();
}

/// ADDING to a plan destroys no earlier claim, so it is deliberately free. A
/// guard that fired here would tax correct work — the mistake this project
/// keeps finding in detectors that punish good practice.
#[test]
fn adding_to_a_plan_needs_no_snapshot() {
    let mut g = planned_epoch_3();
    g.add_requirement("req:d", "D", "needed").unwrap();
    g.schedule_for(
        "Requirement",
        "req:d",
        "DesignEpoch",
        "epoch:3",
        "expected",
        None,
    )
    .expect("growing a plan is not rewriting it");
}

/// An `expected` quietly becoming `required` rewrites what was promised, which
/// is the same loss as deleting the edge.
#[test]
fn changing_a_modality_is_a_rewrite_and_is_refused() {
    let mut g = planned_epoch_3();
    let err = g
        .schedule_for(
            "Requirement",
            "req:b",
            "DesignEpoch",
            "epoch:3",
            "required",
            None,
        )
        .expect_err("promoting a claim is rewriting it");
    assert!(format!("{err:?}").contains("record_change"));
}

/// Deleting the requirement takes its commitments with it — a second door onto
/// the same loss, and the one discontinuation actually walks through.
#[test]
fn deleting_a_scheduled_item_is_refused_too() {
    let mut g = planned_epoch_3();
    g.delete_node(node::REQUIREMENT, "req:b")
        .expect_err("retiring a scheduled item is a plan change");
    g.delete_node(node::DESIGN_EPOCH, "epoch:3")
        .expect_err("deleting the moment destroys the whole plan");
}

/// THE ONE ANTHONY'S OWN EXAMPLE TURNS ON, and the reason the baseline is the
/// FIRST snapshot rather than the last. Two replans leave epoch 3 holding
/// {A,B,C} then {A,C}; reading the LAST says the plan was always {A,C}, so B's
/// slip vanishes from the very report meant to show it.
#[test]
fn the_baseline_is_the_first_snapshot_not_the_last() {
    let mut g = planned_epoch_3();

    // B slips to epoch 4, recorded.
    record_the_plan(&mut g, "chg:replan1", "epoch:3");
    g.delete_edge(edge::SCHEDULED_FOR, "req:b", "epoch:3")
        .unwrap();
    g.schedule_for(
        "Requirement",
        "req:b",
        "DesignEpoch",
        "epoch:4",
        "expected",
        None,
    )
    .unwrap();
    // Then C is dropped outright, also recorded.
    record_the_plan(&mut g, "chg:replan2", "epoch:3");
    g.delete_edge(edge::SCHEDULED_FOR, "req:c", "epoch:3")
        .unwrap();

    deliver(&mut g, "req:a", "a");
    g.set_epoch_status("epoch:3", "arrived").unwrap();

    let d = g.arrival_delta("epoch:3").unwrap();
    assert_eq!(
        d.baseline,
        BaselineSource::FirstSnapshot {
            snapshot_id: "snap:epoch:now:epoch:3".into()
        }
    );
    assert_eq!(d.items.len(), 3, "the ORIGINAL commitment was three items");

    let outcome = |id: &str| {
        d.items
            .iter()
            .find(|i| i.item_id == id)
            .unwrap_or_else(|| panic!("{id} must still be in the baseline"))
            .outcome
            .clone()
    };
    assert_eq!(outcome("req:a"), ScheduleOutcome::Delivered);
    assert_eq!(
        outcome("req:b"),
        ScheduleOutcome::Deferred {
            now_due_at: vec!["epoch:4".into()]
        },
        "the slip must still be visible after two replans"
    );
    assert_eq!(outcome("req:c"), ScheduleOutcome::Discontinued);

    // And the trail says how it got from there to here.
    assert_eq!(d.movement.len(), 2);
    assert_eq!(d.movement[0].items.len(), 3);
    assert_eq!(d.movement[1].items.len(), 2);
}

/// THE FIFTH OUTCOME. The four assume every undelivered item was consciously
/// moved or dropped; the commonest case is that nobody touched it. Calling
/// that `discontinued` would put a withdrawal on the record nobody made, and
/// `deferred` would invent a date nobody chose.
#[test]
fn an_item_nobody_touched_is_outstanding_not_discontinued() {
    let mut g = planned_epoch_3();
    deliver(&mut g, "req:a", "a");
    g.set_epoch_status("epoch:3", "arrived").unwrap();

    let d = g.arrival_delta("epoch:3").unwrap();
    assert_eq!(
        d.baseline,
        BaselineSource::LiveEdges,
        "the plan never moved"
    );
    let b = d.items.iter().find(|i| i.item_id == "req:b").unwrap();
    assert_eq!(b.outcome, ScheduleOutcome::Outstanding);
}

/// `required` is the scheduling face of a KPP: a miss at arrival is a computed
/// violation, not a slip. `expected` misses in the same run must NOT be.
#[test]
fn a_required_miss_is_a_computed_violation() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.plan_epoch("epoch:3", "Epoch 3", EpochType::Milestone, 3)
        .unwrap();
    g.add_requirement("req:kpp", "KPP", "must hold").unwrap();
    g.add_requirement("req:nice", "Nice", "would be nice")
        .unwrap();
    g.schedule_for(
        "Requirement",
        "req:kpp",
        "DesignEpoch",
        "epoch:3",
        "required",
        None,
    )
    .unwrap();
    g.schedule_for(
        "Requirement",
        "req:nice",
        "DesignEpoch",
        "epoch:3",
        "expected",
        None,
    )
    .unwrap();
    g.set_epoch_status("epoch:3", "arrived").unwrap();

    let d = g.arrival_delta("epoch:3").unwrap();
    assert_eq!(d.missed_obligations, vec!["req:kpp".to_string()]);
}

/// A delivered obligation is not a violation — the negative that keeps the
/// count from being "every required item".
#[test]
fn a_required_claim_that_landed_is_not_a_violation() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.plan_epoch("epoch:3", "Epoch 3", EpochType::Milestone, 3)
        .unwrap();
    g.add_requirement("req:kpp", "KPP", "must hold").unwrap();
    g.schedule_for(
        "Requirement",
        "req:kpp",
        "DesignEpoch",
        "epoch:3",
        "required",
        None,
    )
    .unwrap();
    deliver(&mut g, "req:kpp", "kpp");
    g.set_epoch_status("epoch:3", "arrived").unwrap();

    assert!(
        g.arrival_delta("epoch:3")
            .unwrap()
            .missed_obligations
            .is_empty()
    );
}

/// Work scheduled after the baseline is reported apart from it, because a
/// delta that only measures against the plan cannot see the work that was not
/// in it — and that is usually where the estimate actually went.
#[test]
fn work_added_after_the_baseline_is_reported_separately() {
    let mut g = planned_epoch_3();
    record_the_plan(&mut g, "chg:replan", "epoch:3");
    g.add_requirement("req:surprise", "Surprise", "nobody planned this")
        .unwrap();
    g.schedule_for(
        "Requirement",
        "req:surprise",
        "DesignEpoch",
        "epoch:3",
        "expected",
        None,
    )
    .unwrap();
    g.set_epoch_status("epoch:3", "arrived").unwrap();

    let d = g.arrival_delta("epoch:3").unwrap();
    assert_eq!(d.items.len(), 3, "the baseline is untouched by later work");
    assert_eq!(
        d.added_after_baseline
            .iter()
            .map(|i| i.item_id.as_str())
            .collect::<Vec<_>>(),
        vec!["req:surprise"]
    );
}

/// A forecast is not a delta, and the difference is said out loud rather than
/// left to be inferred from a confident-looking list of misses.
#[test]
fn a_delta_against_an_epoch_that_has_not_arrived_says_so() {
    let g = planned_epoch_3();
    let d = g.arrival_delta("epoch:3").unwrap();
    assert_eq!(d.status.as_deref(), Some("planned"));
    assert!(
        d.notes.iter().any(|n| n.contains("has not arrived")),
        "got: {:?}",
        d.notes
    );
}

/// A schedule arrives at a moment, so asking this of anything else is refused
/// by name rather than answered with an empty delta.
#[test]
fn an_arrival_delta_refuses_a_target_that_is_not_a_moment() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:x", "X", "x").unwrap();
    let err = g
        .arrival_delta("req:x")
        .expect_err("a requirement is not a moment");
    assert!(format!("{err:?}").contains("Requirement"));
}

/// THE VACUOUS-TRUTH FIX (`dec:release-trigger-needs-a-required-item`). "Every
/// required item delivered" is trivially true of an increment that requires
/// nothing, so an empty release read as ready — the empty-release failure the
/// trigger decision used to REJECT a fixed cadence, arriving through the chosen
/// option's own back door.
#[test]
fn an_increment_that_promises_nothing_is_not_ready_to_cut() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_release("rel:empty", "Empty", Some("0.0.1"), None)
        .unwrap();
    g.add_capability("cap:someday", "Someday", "unbuilt", None)
        .unwrap();
    g.schedule_for(
        "Capability",
        "cap:someday",
        "Release",
        "rel:empty",
        "expected",
        None,
    )
    .unwrap();

    let d = g.arrival_delta("rel:empty").unwrap();
    assert!(
        d.missed_obligations.is_empty(),
        "nothing required, so nothing missed"
    );
    assert_eq!(d.required_count, 0);
    assert!(
        !d.ready_to_cut,
        "an increment with no obligations must NOT read as ready — that is the whole fix"
    );
    assert!(
        d.notes.iter().any(|n| n.contains("has not been scoped")),
        "and it must say WHY rather than just answering no: {:?}",
        d.notes
    );
}

/// The counterweight: one obligation, delivered, and the trigger fires.
#[test]
fn an_increment_with_its_obligations_met_is_ready() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_release("rel:real", "Real", Some("0.1.0"), None)
        .unwrap();
    g.add_requirement("req:ship", "Ship", "must ship").unwrap();
    g.schedule_for(
        "Requirement",
        "req:ship",
        "Release",
        "rel:real",
        "required",
        None,
    )
    .unwrap();
    deliver(&mut g, "req:ship", "ship");

    let d = g.arrival_delta("rel:real").unwrap();
    assert_eq!(d.required_count, 1);
    assert!(
        d.ready_to_cut,
        "one obligation, delivered — the trigger must fire"
    );
}
