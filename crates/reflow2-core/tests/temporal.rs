//! Axis-Z (temporal) tests — the coherence loop's CHANGE step.
//!
//! The load-bearing property: **the past is never overwritten.** When a node is
//! edited, `record_change` snapshots its prior state pinned to an epoch, so the
//! old state is still reconstructable after the live node moves on.

use reflow2_core::nodes::{edge, node};
use reflow2_core::{
    ChangeAction, ChangeRecord, ChangeType, DesignGraph, EpochType, SnapshotEdge,
    parse_snapshot_edges, parse_snapshot_state,
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
