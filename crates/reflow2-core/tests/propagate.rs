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
    g.add_capability(
        "cap:fast-path",
        "Fast path",
        "Serve hot reads quickly",
        None,
    )
    .unwrap();
    g.add_component("cmp:cache", "Cache", "In-memory cache", None)
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
    g.add_component(
        "cmp:legacy",
        "Legacy store",
        "Old store being phased out",
        None,
    )
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

#[test]
fn centrality_ranks_a_hub_impact_above_a_leaf_at_the_same_distance() {
    // Two capabilities both satisfy req:r (both impacted at distance 1), but
    // cap:hub is a routing hub (allocated to two components + realized by an
    // artifact) while cap:leaf connects to nothing else.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:r", "R", "need").unwrap();
    g.add_capability("cap:hub", "Hub", "central", None).unwrap();
    g.add_capability("cap:leaf", "Leaf", "peripheral", None)
        .unwrap();
    g.add_component("cmp:1", "C1", "p1", None).unwrap();
    g.add_component("cmp:2", "C2", "p2", None).unwrap();
    g.create_node(node::ARTIFACT, "art:1", Props::new().set("name", "a1"))
        .unwrap();

    g.satisfies("cap:hub", "req:r").unwrap();
    g.satisfies("cap:leaf", "req:r").unwrap();
    g.allocate("cap:hub", "cmp:1").unwrap();
    g.allocate("cap:hub", "cmp:2").unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:1",
        node::CAPABILITY,
        "cap:hub",
        Props::new(),
    )
    .unwrap();

    let radius = g
        .propagate_from(&["req:r"], PropagateOptions::default())
        .unwrap();
    let find = |id: &str| radius.impacted.iter().find(|n| n.node_id == id).unwrap();
    let pos = |id: &str| {
        radius
            .impacted
            .iter()
            .position(|n| n.node_id == id)
            .unwrap()
    };

    // Same distance...
    assert_eq!(find("cap:hub").distance, 1);
    assert_eq!(find("cap:leaf").distance, 1);
    // ...but the hub is more central...
    assert!(
        find("cap:hub").centrality > find("cap:leaf").centrality,
        "hub {} should out-rank leaf {}",
        find("cap:hub").centrality,
        find("cap:leaf").centrality
    );
    // ...so it ranks first.
    assert!(pos("cap:hub") < pos("cap:leaf"));
}

#[test]
fn summary_counts_every_band_and_keeps_ring_and_risk_crossings() {
    // BL-49: the summary is what a session reads by default, so it must count
    // everything the full radius holds — a summary that hides nodes would be a
    // silent cap.
    let mut g = thread();
    g.add_component(
        "cmp:legacy",
        "Legacy store",
        "Old store being phased out",
        None,
    )
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
    let summary = radius.summarize();

    assert_eq!(summary.total_impacted, radius.impacted.len());
    let banded: usize = summary.counts_by_distance.iter().map(|b| b.count).sum();
    assert_eq!(
        banded, summary.total_impacted,
        "every impacted node sits in exactly one distance band"
    );

    // The distance-1 ring is the capability, reached over SATISFIES.
    assert_eq!(summary.direct_ring.len(), 1);
    assert_eq!(summary.direct_ring[0].node_id, "cap:fast-path");
    assert_eq!(summary.direct_ring[0].edge_type, "SATISFIES");
    assert!(!summary.direct_ring[0].is_risk);

    // The risk crossing survives the compression, at its true distance.
    assert!(
        summary
            .risk_crossings
            .iter()
            .any(|r| r.node_id == "cmp:legacy" && r.distance == 2),
        "a path across RISKS must stay visible in the summary"
    );

    // Bookkeeping carries through — the summary never hides the bound.
    assert_eq!(summary.seeds, radius.seeds);
    assert_eq!(summary.max_depth, radius.max_depth);
    assert_eq!(
        summary.truncated_beyond_depth,
        radius.truncated_beyond_depth
    );
}

#[test]
fn a_changed_artifact_reaches_the_release_that_ships_it() {
    // INCLUDES joined the direction table with the v0.5.0 release modelling:
    // a changed artifact means the next cut differs, so the release belongs
    // in the blast radius — and before this, every release was invisible to
    // impact entirely.
    let mut g = thread();
    g.create_node(node::RELEASE, "rel:v1", Props::new().set("name", "v1"))
        .unwrap();
    g.create_edge(
        edge::INCLUDES,
        node::RELEASE,
        "rel:v1",
        node::ARTIFACT,
        "art:cache-rs",
        Props::new(),
    )
    .unwrap();

    let radius = g
        .propagate_from(&["art:cache-rs"], PropagateOptions::default())
        .unwrap();
    let rel = find(&radius, "rel:v1");
    assert_eq!(rel.distance, 1);
    assert_eq!(
        rel.direction,
        ImpactDirection::Downstream,
        "the release is a downstream packaging of its contents"
    );
}
