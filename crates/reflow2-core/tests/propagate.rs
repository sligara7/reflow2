//! PROPAGATE tests — the golden-thread blast-radius walk.
//!
//! Fixture (a full concept→verify thread):
//!
//! ```text
//!   Requirement ◀─SATISFIES── Capability ──ALLOCATED_TO──▶ Component
//!                                 ▲
//!                             REALIZES
//!                                 │
//!                             Artifact ◀──VERIFIES── Verification
//! ```
//!
//! Changing the Requirement should ripple *downstream* to the Capability, then
//! its Component and Artifact, then the Verification — each explained by its
//! edge chain.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, ImpactDirection, PropagateOptions};

/// Build the fixture thread on a fresh in-memory graph.
fn thread() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:latency", "Latency", "Respond within 200ms")
        .unwrap();
    g.add_capability("cap:fast-path", "Fast path", "Serve hot reads quickly")
        .unwrap();
    g.add_component("cmp:cache", "Cache", "In-memory cache")
        .unwrap();
    g.create_node(
        node::ARTIFACT,
        "art:cache-rs",
        Props::new().set("name", "cache.rs"),
    )
    .unwrap();
    g.create_node(
        node::VERIFICATION,
        "ver:cache-bench",
        Props::new().set("name", "Cache latency benchmark"),
    )
    .unwrap();

    g.satisfies("cap:fast-path", "req:latency").unwrap();
    g.allocate("cap:fast-path", "cmp:cache").unwrap();
    // Artifact REALIZES Capability; Verification VERIFIES Artifact.
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:cache-rs",
        node::CAPABILITY,
        "cap:fast-path",
        Props::new(),
    )
    .unwrap();
    g.create_edge(
        edge::VERIFIES,
        node::VERIFICATION,
        "ver:cache-bench",
        node::ARTIFACT,
        "art:cache-rs",
        Props::new(),
    )
    .unwrap();
    g
}

fn find<'a>(radius: &'a reflow2_core::BlastRadius, id: &str) -> &'a reflow2_core::ImpactedNode {
    radius
        .impacted
        .iter()
        .find(|n| n.node_id == id)
        .unwrap_or_else(|| panic!("expected {id} in blast radius"))
}

#[test]
fn change_to_requirement_ripples_downstream_along_the_thread() {
    let g = thread();
    let radius = g
        .propagate_from(&["req:latency"], PropagateOptions::default())
        .unwrap();

    assert!(radius.unknown_seeds.is_empty());
    assert!(!radius.was_truncated());

    // Every downstream node on the thread is reached, at its true hop distance.
    let cap = find(&radius, "cap:fast-path");
    assert_eq!(cap.distance, 1);
    assert_eq!(cap.direction, ImpactDirection::Downstream);

    let cmp = find(&radius, "cmp:cache");
    assert_eq!(cmp.distance, 2);

    let art = find(&radius, "art:cache-rs");
    assert_eq!(art.distance, 2);

    let ver = find(&radius, "ver:cache-bench");
    assert_eq!(ver.distance, 3);

    // Exactly these four — the seed is excluded, and decomposition (CONTAINS)
    // is not traversed so nothing else sneaks in.
    assert_eq!(radius.impacted.len(), 4);
}

#[test]
fn every_impact_is_explained_by_its_edge_chain() {
    let g = thread();
    let radius = g
        .propagate_from(&["req:latency"], PropagateOptions::default())
        .unwrap();

    // The Verification is 3 hops out; its `via` must spell the whole path:
    // SATISFIES (to the capability), REALIZES (to the artifact), VERIFIES.
    let ver = find(&radius, "ver:cache-bench");
    let chain: Vec<&str> = ver.via.iter().map(|h| h.edge_type.as_str()).collect();
    assert_eq!(chain, ["SATISFIES", "REALIZES", "VERIFIES"]);
    assert!(
        ver.via
            .iter()
            .all(|h| h.direction == ImpactDirection::Downstream),
        "the whole chain flows downstream"
    );
}

#[test]
fn depth_bound_reports_truncation_never_hides_it() {
    let g = thread();
    let radius = g
        .propagate_from(&["req:latency"], PropagateOptions { max_depth: 1 })
        .unwrap();

    // Only the direct (1-hop) capability is expanded...
    assert_eq!(radius.impacted.len(), 1);
    assert_eq!(radius.impacted[0].node_id, "cap:fast-path");
    // ...and the nodes just past the bound are counted, not silently dropped.
    assert!(radius.was_truncated());
    assert_eq!(radius.truncated_beyond_depth, 2); // Component + Artifact
}

#[test]
fn reactive_propagation_uses_change_event_targets_as_seeds() {
    use reflow2_core::{ChangeAction, ChangeRecord, ChangeType};
    let mut g = thread();
    g.add_epoch("epoch:v2", "v2", reflow2_core::EpochType::Revision, 2)
        .unwrap();
    g.record_change(ChangeRecord {
        epoch_id: "epoch:v2",
        change_event_id: "chg:relax-latency",
        name: "Relax latency to 300ms",
        change_type: ChangeType::ScopeChange,
        target_type: node::REQUIREMENT,
        target_id: "req:latency",
        action: ChangeAction::Modified,
    })
    .unwrap();

    let radius = g
        .propagate_change("chg:relax-latency", PropagateOptions::default())
        .unwrap();
    assert_eq!(radius.seeds, ["req:latency"]);
    // Same downstream radius as seeding the requirement directly.
    assert_eq!(radius.impacted.len(), 4);
}

#[test]
fn inference_edges_propagate_as_causal_and_flag_risk() {
    let mut g = thread();
    // The capability RISKS a separate component (an inference/risk edge).
    g.add_component("cmp:legacy", "Legacy store", "Old store being phased out")
        .unwrap();
    g.create_edge(
        "RISKS",
        node::CAPABILITY,
        "cap:fast-path",
        node::COMPONENT,
        "cmp:legacy",
        Props::new(),
    )
    .unwrap();

    let radius = g
        .propagate_from(&["req:latency"], PropagateOptions::default())
        .unwrap();

    let legacy = find(&radius, "cmp:legacy");
    assert_eq!(legacy.direction, ImpactDirection::Causal);
    assert!(
        legacy.crosses_risk_edge,
        "a path across RISKS must be flagged"
    );
    // Reached at distance 2 (req -> cap -> legacy).
    assert_eq!(legacy.distance, 2);
}

#[test]
fn unknown_seed_is_surfaced_not_dropped() {
    let g = thread();
    let radius = g
        .propagate_from(&["req:ghost"], PropagateOptions::default())
        .unwrap();
    assert_eq!(radius.unknown_seeds, ["req:ghost"]);
    assert!(radius.impacted.is_empty());
}
