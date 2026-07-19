//! Axis-Z (temporal) tests — the coherence loop's CHANGE step.
//!
//! The load-bearing property: **the past is never overwritten.** When a node is
//! edited, `record_change` snapshots its prior state pinned to an epoch, so the
//! old state is still reconstructable after the live node moves on.

use reflow2_core::nodes::{edge, node};
use reflow2_core::{
    ChangeAction, ChangeRecord, ChangeType, DesignGraph, EpochType, parse_snapshot_state,
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
