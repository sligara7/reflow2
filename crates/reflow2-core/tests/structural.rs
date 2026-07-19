//! Structural (graph-topology) HEAL detectors, powered by `dynograph-graph`.
//!
//! The subtle one is single-point-of-failure: a design's golden thread is
//! tree-shaped, so a naive articulation-point check would flag every internal
//! node. These tests pin the *selective* behavior — only a node that separates
//! ≥2 real subsystems counts.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory};

fn has(g: &DesignGraph, cat: HealCategory) -> bool {
    g.detect_defects()
        .unwrap()
        .iter()
        .any(|d| d.category == cat)
}

/// A linear golden thread: req—cap—cmp plus an artifact/verification. Tree-shaped.
fn linear_thread() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_component("cmp:a", "Cmp A", "part a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.allocate("cap:a", "cmp:a").unwrap();
    g
}

#[test]
fn tree_shaped_thread_flags_no_single_point_of_failure() {
    // cap:a is an articulation point (removing it isolates req and cmp), but it
    // only separates leaves, not two real subsystems — so it must NOT be flagged.
    let g = linear_thread();
    assert!(
        !has(&g, HealCategory::SinglePointOfFailure),
        "leaf-cutting on a tree is not a real SPOF"
    );
}

#[test]
fn a_node_bridging_two_subsystems_is_a_single_point_of_failure() {
    // Two 2-node subsystems joined only through cap:hub:
    //   cmp:a — cap:hub — cmp:b, plus each cmp has its own artifact so each side
    //   is non-trivial (≥2 nodes) once cap:hub is removed.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_capability("cap:hub", "Hub", "central capability", None)
        .unwrap();
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();
    g.create_node(node::ARTIFACT, "art:a", Props::new().set("name", "a.rs"))
        .unwrap();
    g.create_node(node::ARTIFACT, "art:b", Props::new().set("name", "b.rs"))
        .unwrap();
    // hub allocated to both components; each component's artifact realizes the hub
    // — wait, keep the two sides distinct: artifacts hang off their components via
    // REALIZES to the hub would recentralize. Instead connect each artifact to its
    // side with DEPENDS_ON to the component (Interface-free structural link).
    g.allocate("cap:hub", "cmp:a").unwrap();
    g.allocate("cap:hub", "cmp:b").unwrap();
    g.create_edge(
        edge::DEPENDS_ON,
        node::ARTIFACT,
        "art:a",
        node::COMPONENT,
        "cmp:a",
        Props::new(),
    )
    .unwrap();
    g.create_edge(
        edge::DEPENDS_ON,
        node::ARTIFACT,
        "art:b",
        node::COMPONENT,
        "cmp:b",
        Props::new(),
    )
    .unwrap();

    // Removing cap:hub leaves {cmp:a, art:a} and {cmp:b, art:b}: two subsystems.
    assert!(has(&g, HealCategory::SinglePointOfFailure));
    let spof: Vec<String> = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == HealCategory::SinglePointOfFailure)
        .flat_map(|d| d.affected_ids)
        .collect();
    assert_eq!(spof, ["cap:hub"]);
}

#[test]
fn a_separate_cluster_is_a_disconnected_community() {
    // Main thread + a detached 2-node island (cap:x—cmp:x) linked to nothing else.
    let mut g = linear_thread();
    g.add_capability("cap:x", "X", "island cap", None).unwrap();
    g.add_component("cmp:x", "X part", "island part", None)
        .unwrap();
    g.allocate("cap:x", "cmp:x").unwrap(); // island internally connected, externally not

    assert!(has(&g, HealCategory::DisconnectedCommunity));
    let island: Vec<String> = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .find(|d| d.category == HealCategory::DisconnectedCommunity)
        .unwrap()
        .affected_ids;
    assert_eq!(island, ["cap:x", "cmp:x"]);
}

#[test]
fn a_fully_connected_thread_has_no_structural_defects() {
    let g = linear_thread();
    assert!(!has(&g, HealCategory::DisconnectedCommunity));
    assert!(!has(&g, HealCategory::SinglePointOfFailure));
    assert!(!has(&g, HealCategory::DeadEnd));
}

#[test]
fn an_isolated_component_is_a_dead_end() {
    let mut g = linear_thread();
    g.add_component("cmp:orphan", "Orphan", "connected to nothing", None)
        .unwrap();
    assert!(has(&g, HealCategory::DeadEnd));
    let dead: Vec<String> = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == HealCategory::DeadEnd)
        .flat_map(|d| d.affected_ids)
        .collect();
    assert_eq!(dead, ["cmp:orphan"]);
}

#[test]
fn structural_defects_are_generative_and_gated_for_review() {
    let mut g = linear_thread();
    g.add_component("cmp:orphan", "Orphan", "connected to nothing", None)
        .unwrap();
    let proposal = g
        .propose_heal(reflow2_core::HealOptions::default())
        .unwrap();
    // The dead end has no content-free fix, so it's a review-gated generative stub.
    assert!(proposal.requires_human_review);
    assert!(
        proposal
            .generated_content
            .iter()
            .any(|s| s.kind == "bridge"),
        "dead end should propose a bridging link for review"
    );
}

/// BL-5. The SPOF test used to ask "are there ≥2 non-trivial components after
/// removing this node?" — which silently assumed the design was connected to
/// start with. One unrelated island already satisfies it, so every articulation
/// point elsewhere reported as a single point of failure with nothing about its
/// fragility changed. Measuring against the baseline is the fix.
///
/// This is the blind trial's complaint from the other side: *"all 15 defects
/// vanished at once when I added two bookkeeping edges."* Those edges attached
/// an island, the count fell below the threshold, and the list cleared.
#[test]
fn an_unrelated_island_does_not_make_everything_a_single_point_of_failure() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_component("cmp:hub", "Hub", "holds both", None)
        .unwrap();

    // Two real subsystems, joined only through cmp:hub.
    for cap in ["cap:a", "cap:b"] {
        g.add_capability(cap, cap, "does a thing", None).unwrap();
        g.allocate(cap, "cmp:hub").unwrap();
        for i in 0..2 {
            let r = format!("req:{cap}-{i}");
            g.add_requirement(&r, &r, "s").unwrap();
            g.satisfies(cap, &r).unwrap();
        }
    }
    let spofs = |g: &DesignGraph| -> Vec<String> {
        g.detect_defects()
            .unwrap()
            .iter()
            .filter(|d| d.category == HealCategory::SinglePointOfFailure)
            .flat_map(|d| d.affected_ids.clone())
            .collect()
    };
    assert_eq!(
        spofs(&g),
        ["cmp:hub"],
        "the hub genuinely holds two subsystems together"
    );

    // Add a second, entirely separate part of the design. Nothing about the
    // first part's fragility changes, so nothing new should be flagged.
    g.add_component("cmp:island", "Island", "unrelated", None)
        .unwrap();
    g.add_capability("cap:island", "Island cap", "d", None)
        .unwrap();
    g.allocate("cap:island", "cmp:island").unwrap();

    assert_eq!(
        spofs(&g),
        ["cmp:hub"],
        "an unrelated island must not turn the capabilities into single points of failure"
    );
}
