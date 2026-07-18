//! Allocation evaluator — score the current function→service allocation over
//! the weighted DEPENDS_ON coupling graph (graph-analysis.md step 2).

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

/// Add a weighted `DEPENDS_ON` between two capabilities.
fn depends(g: &mut DesignGraph, from: &str, to: &str, weight: f64) {
    g.create_edge(
        edge::DEPENDS_ON,
        node::CAPABILITY,
        from,
        node::CAPABILITY,
        to,
        Props::new().set("weight", weight),
    )
    .unwrap();
}

fn cap(g: &mut DesignGraph, id: &str, component: &str) {
    g.add_capability(id, id, "does a thing").unwrap();
    g.add_component(component, component, "a part", None)
        .unwrap();
    g.allocate(id, component).unwrap();
}

#[test]
fn a_cohesive_allocation_scores_high_with_no_misplacements() {
    // Two components; each keeps its tightly-coupled capabilities together.
    let mut g = DesignGraph::open_in_memory().unwrap();
    cap(&mut g, "cap:a1", "cmp:a");
    cap(&mut g, "cap:a2", "cmp:a");
    cap(&mut g, "cap:b1", "cmp:b");
    cap(&mut g, "cap:b2", "cmp:b");
    depends(&mut g, "cap:a1", "cap:a2", 0.9); // internal to A
    depends(&mut g, "cap:b1", "cap:b2", 0.9); // internal to B

    let r = g.evaluate_allocation().unwrap();
    assert_eq!(r.modularity, 1.0, "no coupling crosses a boundary");
    assert_eq!(r.total_external, 0.0);
    assert!(r.misplaced.is_empty());
    assert!(r.god_components.is_empty());
    assert_eq!(r.unweighted_dependencies, 0);
    assert_eq!(r.components.len(), 2);
}

#[test]
fn a_capability_coupled_across_the_boundary_is_flagged_misplaced() {
    // cap:x is in A but couples 0.9 to B and only 0.1 within A.
    let mut g = DesignGraph::open_in_memory().unwrap();
    cap(&mut g, "cap:x", "cmp:a");
    cap(&mut g, "cap:a1", "cmp:a");
    cap(&mut g, "cap:b1", "cmp:b");
    cap(&mut g, "cap:b2", "cmp:b");
    depends(&mut g, "cap:x", "cap:a1", 0.1); // weak internal pull
    depends(&mut g, "cap:x", "cap:b1", 0.9); // strong cross-boundary pull
    depends(&mut g, "cap:b1", "cap:b2", 0.9); // keeps b1 clearly in B

    let r = g.evaluate_allocation().unwrap();

    // Only cap:x is misplaced, and the suggestion is B.
    assert_eq!(r.misplaced.len(), 1);
    let m = &r.misplaced[0];
    assert_eq!(m.capability_id, "cap:x");
    assert_eq!(m.current_component, "cmp:a");
    assert_eq!(m.suggested_component, "cmp:b");
    assert!(m.suggested_pull > m.current_pull);

    // Coupling crosses a boundary, so modularity is below 1.
    assert!(r.modularity < 1.0 && r.modularity > 0.0);
}

#[test]
fn unweighted_dependencies_are_counted_not_silently_defaulted() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    cap(&mut g, "cap:a", "cmp:a");
    cap(&mut g, "cap:b", "cmp:a");
    // A DEPENDS_ON with no weight facet.
    g.create_edge(
        edge::DEPENDS_ON,
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:b",
        Props::new(),
    )
    .unwrap();

    let r = g.evaluate_allocation().unwrap();
    assert_eq!(r.unweighted_dependencies, 1, "coverage must be surfaced");
    // It still counts as 1.0 internal (same component).
    assert_eq!(r.total_internal, 1.0);
}

#[test]
fn a_hub_component_between_two_subsystems_is_a_god_component() {
    // Two 2-component clusters joined only through cmp:hub:
    //   {a1,a2} — hub — {c1,c2}. Removing hub splits into two subsystems.
    let mut g = DesignGraph::open_in_memory().unwrap();
    for (c, comp) in [
        ("cap:a1", "cmp:a1"),
        ("cap:a2", "cmp:a2"),
        ("cap:hub", "cmp:hub"),
        ("cap:c1", "cmp:c1"),
        ("cap:c2", "cmp:c2"),
    ] {
        cap(&mut g, c, comp);
    }
    depends(&mut g, "cap:a1", "cap:a2", 0.5); // A-side cluster
    depends(&mut g, "cap:a1", "cap:hub", 0.5); // A → hub
    depends(&mut g, "cap:c1", "cap:c2", 0.5); // C-side cluster
    depends(&mut g, "cap:c1", "cap:hub", 0.5); // C → hub

    let r = g.evaluate_allocation().unwrap();
    assert_eq!(r.god_components, ["cmp:hub"]);
}

#[test]
fn multi_allocated_capabilities_are_surfaced() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:shared", "Shared", "in two places")
        .unwrap();
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();
    g.allocate("cap:shared", "cmp:a").unwrap();
    g.allocate("cap:shared", "cmp:b").unwrap();

    let r = g.evaluate_allocation().unwrap();
    assert_eq!(r.multi_allocated, ["cap:shared"]);
}

#[test]
fn proposes_clusters_matching_the_coupling_graph() {
    // Two tightly-coupled triangles joined by one weak bridge; no current
    // allocation. Leiden should propose exactly the two triangles.
    let mut g = DesignGraph::open_in_memory().unwrap();
    for c in ["cap:a1", "cap:a2", "cap:a3", "cap:b1", "cap:b2", "cap:b3"] {
        g.add_capability(c, c, "does a thing").unwrap();
    }
    depends(&mut g, "cap:a1", "cap:a2", 0.9);
    depends(&mut g, "cap:a2", "cap:a3", 0.9);
    depends(&mut g, "cap:a1", "cap:a3", 0.9);
    depends(&mut g, "cap:b1", "cap:b2", 0.9);
    depends(&mut g, "cap:b2", "cap:b3", 0.9);
    depends(&mut g, "cap:b1", "cap:b3", 0.9);
    depends(&mut g, "cap:a1", "cap:b1", 0.1); // weak bridge

    let p = g.propose_allocation(1.0).unwrap();

    assert_eq!(p.clusters.len(), 2, "leiden should find the two triangles");
    assert!(
        p.requires_human_review,
        "a proposal, not an applied allocation"
    );
    // No current allocation → all coupling crosses boundaries.
    assert_eq!(p.current_modularity, 0.0);
    assert!(p.proposed_modularity > p.current_modularity);

    let mut sets: Vec<Vec<String>> = p
        .clusters
        .iter()
        .map(|c| c.capability_ids.clone())
        .collect();
    sets.sort();
    assert_eq!(
        sets,
        vec![
            vec!["cap:a1".to_string(), "cap:a2".into(), "cap:a3".into()],
            vec!["cap:b1".to_string(), "cap:b2".into(), "cap:b3".into()],
        ]
    );
}

#[test]
fn proposal_beats_a_miscohesive_current_allocation() {
    // Current allocation splits both tight pairs across two components; the
    // proposer should regroup them for a higher modularity.
    let mut g = DesignGraph::open_in_memory().unwrap();
    for c in ["cap:a1", "cap:a2", "cap:b1", "cap:b2"] {
        g.add_capability(c, c, "does a thing").unwrap();
    }
    g.add_component("cmp:x", "X", "p", None).unwrap();
    g.add_component("cmp:y", "Y", "p", None).unwrap();
    g.allocate("cap:a1", "cmp:x").unwrap();
    g.allocate("cap:b1", "cmp:x").unwrap();
    g.allocate("cap:a2", "cmp:y").unwrap();
    g.allocate("cap:b2", "cmp:y").unwrap();
    depends(&mut g, "cap:a1", "cap:a2", 0.9); // tight, but split across x/y
    depends(&mut g, "cap:b1", "cap:b2", 0.9); // tight, but split across x/y
    depends(&mut g, "cap:a1", "cap:b1", 0.1);
    depends(&mut g, "cap:a2", "cap:b2", 0.1);

    let p = g.propose_allocation(1.0).unwrap();
    assert!(
        p.proposed_modularity > p.current_modularity,
        "proposed {} should beat current {}",
        p.proposed_modularity,
        p.current_modularity
    );
}
