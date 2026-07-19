//! Circular-dependency detection.
//!
//! The original Reflow's most-used architectural check (`circular_dependencies`
//! in `system_of_systems_graph_v2.py`). Two things it got right and one it did
//! not: cycles are real defects, they are not auto-fixable — but a naive check
//! over every relation reports loops that are just the golden thread closing on
//! itself. These tests pin the selectivity as much as the detection.

use reflow2_core::graph::DesignGraph;
use reflow2_core::heal::{HealCategory, HealOptions, HealSeverity};
use reflow2_core::nodes::{Props, edge, node};

fn project_with(components: &[(&str, &str)]) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    for (id, name) in components {
        g.add_component(id, name, "does a thing", None)
            .expect("component");
    }
    g
}

fn depends(g: &mut DesignGraph, from: &str, to: &str) {
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        from,
        node::COMPONENT,
        to,
        Props::new(),
    )
    .expect("depends_on");
}

fn cycles(g: &DesignGraph) -> Vec<Vec<String>> {
    g.detect_defects()
        .expect("detect")
        .into_iter()
        .filter(|i| i.category == HealCategory::CircularDependency)
        .map(|i| i.affected_ids)
        .collect()
}

#[test]
fn a_dependency_loop_is_detected() {
    let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B"), ("cmp:c", "C")]);
    depends(&mut g, "cmp:a", "cmp:b");
    depends(&mut g, "cmp:b", "cmp:c");
    depends(&mut g, "cmp:c", "cmp:a");

    let found = cycles(&g);
    assert_eq!(found.len(), 1, "one cluster, one issue: {found:?}");
    assert_eq!(found[0], vec!["cmp:a", "cmp:b", "cmp:c"]);
}

#[test]
fn an_acyclic_dependency_chain_is_clean() {
    let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B"), ("cmp:c", "C")]);
    depends(&mut g, "cmp:a", "cmp:b");
    depends(&mut g, "cmp:b", "cmp:c");

    assert!(cycles(&g).is_empty(), "a DAG has no circular dependency");
}

#[test]
fn components_looping_through_their_contracts_are_detected() {
    // A provides i1 which B consumes; B provides i2 which A consumes. Neither
    // DEPENDS_ON edge exists — the loop is entirely through the contracts, which
    // is exactly the shape a service-boundary cycle takes in practice.
    let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B")]);
    g.add_interface("ifc:1", "A's API").expect("i1");
    g.add_interface("ifc:2", "B's API").expect("i2");
    g.provides("cmp:a", "ifc:1").expect("a provides");
    g.consumes("cmp:b", "ifc:1").expect("b consumes");
    g.provides("cmp:b", "ifc:2").expect("b provides");
    g.consumes("cmp:a", "ifc:2").expect("a consumes");

    let found = cycles(&g);
    assert_eq!(
        found.len(),
        1,
        "the two components form one loop: {found:?}"
    );
    assert_eq!(found[0], vec!["cmp:a", "cmp:b"]);
    assert!(
        !found[0].iter().any(|id| id.starts_with("ifc:")),
        "the interface is the medium, not a participant — it must not appear in the cycle"
    );
}

#[test]
fn a_one_way_contract_is_not_a_cycle() {
    let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B")]);
    g.add_interface("ifc:1", "A's API").expect("i1");
    g.provides("cmp:a", "ifc:1").expect("a provides");
    g.consumes("cmp:b", "ifc:1").expect("b consumes");

    assert!(
        cycles(&g).is_empty(),
        "B depending on A is a dependency, not a loop"
    );
}

#[test]
fn the_golden_thread_closing_on_itself_is_not_a_cycle() {
    // Requirement ← Capability → Component, with the Component's artifact
    // realizing the capability. Mixing SATISFIES/ALLOCATED_TO/REALIZES into one
    // directed graph would report this as circular; it is just the thread.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.add_requirement("req:1", "Fast", "must be fast")
        .expect("req");
    g.add_capability("cap:1", "Speed", "goes fast", None)
        .expect("cap");
    g.add_component("cmp:1", "Engine", "makes it go", None)
        .expect("cmp");
    g.satisfies("cap:1", "req:1").expect("satisfies");
    g.allocate("cap:1", "cmp:1").expect("allocate");
    g.add_artifact("art:1", "engine.rs", None, None)
        .expect("artifact");
    g.realizes("art:1", node::CAPABILITY, "cap:1", None)
        .expect("realizes");

    assert!(
        cycles(&g).is_empty(),
        "traceability edges point in different semantic directions — not dependencies"
    );
}

#[test]
fn two_independent_loops_are_reported_separately() {
    let mut g = project_with(&[
        ("cmp:a", "A"),
        ("cmp:b", "B"),
        ("cmp:x", "X"),
        ("cmp:y", "Y"),
    ]);
    depends(&mut g, "cmp:a", "cmp:b");
    depends(&mut g, "cmp:b", "cmp:a");
    depends(&mut g, "cmp:x", "cmp:y");
    depends(&mut g, "cmp:y", "cmp:x");

    let found = cycles(&g);
    assert_eq!(found.len(), 2, "two clusters → two issues: {found:?}");
    assert_eq!(found[0], vec!["cmp:a", "cmp:b"]);
    assert_eq!(found[1], vec!["cmp:x", "cmp:y"]);
}

#[test]
fn a_self_dependency_is_caught() {
    let mut g = project_with(&[("cmp:a", "A")]);
    depends(&mut g, "cmp:a", "cmp:a");

    let found = cycles(&g);
    assert_eq!(found.len(), 1, "a node depending on itself is a loop");
    assert_eq!(found[0], vec!["cmp:a"]);
}

#[test]
fn a_cycle_is_critical_and_never_auto_applied() {
    let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B")]);
    depends(&mut g, "cmp:a", "cmp:b");
    depends(&mut g, "cmp:b", "cmp:a");

    let issue = g
        .detect_defects()
        .expect("detect")
        .into_iter()
        .find(|i| i.category == HealCategory::CircularDependency)
        .expect("cycle issue");
    assert_eq!(issue.severity, HealSeverity::Critical);
    assert_eq!(issue.suggested_fix_type, "break_cycle");
    assert!(
        issue.message.contains("cmp:a → cmp:b → cmp:a"),
        "the loop must be shown as a readable path, got {:?}",
        issue.message
    );

    // Breaking a cycle is a design decision: propose, never mutate.
    let proposal = g.propose_heal(HealOptions::default()).expect("propose");
    assert!(proposal.requires_human_review);
    assert!(
        proposal
            .generated_content
            .iter()
            .any(|c| c.kind == "cycle break"),
        "the fix must be left for a human, got {:?}",
        proposal.generated_content
    );
    assert!(
        !proposal
            .operations
            .iter()
            .any(|o| proposal.issues_addressed.contains(&o.issue_id) && o.issue_id == issue.id),
        "no mechanical operation may claim to have broken the cycle"
    );
}

#[test]
fn cycle_detection_is_deterministic() {
    let build = || {
        let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B"), ("cmp:c", "C")]);
        depends(&mut g, "cmp:b", "cmp:c");
        depends(&mut g, "cmp:c", "cmp:a");
        depends(&mut g, "cmp:a", "cmp:b");
        g
    };
    let first = build();
    let second = build();

    let ids_of = |g: &DesignGraph| -> Vec<String> {
        g.detect_defects()
            .expect("detect")
            .into_iter()
            .filter(|i| i.category == HealCategory::CircularDependency)
            .map(|i| format!("{}|{}", i.id, i.message))
            .collect()
    };
    assert_eq!(
        ids_of(&first),
        ids_of(&second),
        "same graph must yield the same issue id and the same cycle path"
    );
}
