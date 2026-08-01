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

// ---- BL-141 · the finding says what the detector actually walked ------------
//
// An adopt pass over an ~11k-LOC research repo produced FOUR `critical`
// circular dependencies and every one was false. Each was one Interface node
// standing for two contracts — `ifc:midi-file` meaning both "MIDI we read" and
// "MIDI we emit" — so a reader and a writer of the same file format looked like
// mutual dependency. `dependency_pairs` collapses the Interface out at exactly
// the point the cycle edge is created, and the message printed only `A → B → A`,
// so a coarse model and a tangled call graph were indistinguishable.

fn cycle_message(g: &DesignGraph) -> String {
    g.detect_defects()
        .expect("detect")
        .into_iter()
        .find(|i| i.category == HealCategory::CircularDependency)
        .expect("cycle issue")
        .message
}

/// The phantom shape, reproduced: ONE interface both parts provide and consume.
/// The finding must name it, and must say no DEPENDS_ON edge was involved.
#[test]
fn a_loop_through_one_shared_contract_names_that_contract() {
    let mut g = project_with(&[("cmp:renderer", "R"), ("cmp:transcriber", "T")]);
    g.add_interface("ifc:midi-file", "MIDI").expect("iface");
    // Each reads MIDI and writes MIDI, as a single modelled contract.
    g.provides("cmp:renderer", "ifc:midi-file")
        .expect("r provides");
    g.consumes("cmp:renderer", "ifc:midi-file")
        .expect("r consumes");
    g.provides("cmp:transcriber", "ifc:midi-file")
        .expect("t provides");
    g.consumes("cmp:transcriber", "ifc:midi-file")
        .expect("t consumes");

    let msg = cycle_message(&g);
    assert!(
        msg.contains("ifc:midi-file"),
        "the shared contract must be named — that alone makes it diagnosable: {msg}"
    );
    assert!(
        msg.contains("SAME contract"),
        "one interface for the whole loop is the case worth calling out: {msg}"
    );
    assert!(
        msg.contains("no DEPENDS_ON"),
        "it must say which edge kinds it did NOT walk: {msg}"
    );
}

/// THE COUNTERWEIGHT, and the reason the discriminator is the interface COUNT
/// rather than "contracts were involved". A real service-boundary cycle also
/// runs entirely through contracts — but through TWO, one per direction. It
/// must NOT be described as one Interface standing for two contracts.
#[test]
fn a_genuine_two_contract_service_cycle_is_not_blamed_on_the_model() {
    let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B")]);
    g.add_interface("ifc:1", "A's API").expect("i1");
    g.add_interface("ifc:2", "B's API").expect("i2");
    g.provides("cmp:a", "ifc:1").expect("a provides");
    g.consumes("cmp:b", "ifc:1").expect("b consumes");
    g.provides("cmp:b", "ifc:2").expect("b provides");
    g.consumes("cmp:a", "ifc:2").expect("a consumes");

    let msg = cycle_message(&g);
    assert!(
        !msg.contains("SAME contract"),
        "two contracts, one per direction, is a real cycle — not a modelling artefact: {msg}"
    );
    assert!(
        msg.contains("ifc:1") && msg.contains("ifc:2"),
        "both contracts are still named, because that is what it walked: {msg}"
    );
}

/// A plain DEPENDS_ON tangle names no contract and says so positively, rather
/// than leaving the reader to infer it from an absence.
#[test]
fn a_direct_dependency_cycle_says_it_is_direct() {
    let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B")]);
    depends(&mut g, "cmp:a", "cmp:b");
    depends(&mut g, "cmp:b", "cmp:a");

    let msg = cycle_message(&g);
    assert!(
        msg.contains("direct DEPENDS_ON"),
        "a real code tangle must read as one: {msg}"
    );
    assert!(
        !msg.contains("ifc:"),
        "no contract was walked, so none may be named: {msg}"
    );
}

/// A cycle with BOTH a DEPENDS_ON hop and a contract hop is neither case, and
/// must not claim `no DEPENDS_ON` — the claim that would send a reader to check
/// their interface model when there is real code coupling in the loop.
#[test]
fn a_mixed_cycle_claims_neither_pure_case() {
    let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B")]);
    g.add_interface("ifc:1", "A's API").expect("i1");
    g.provides("cmp:a", "ifc:1").expect("a provides");
    g.consumes("cmp:b", "ifc:1").expect("b consumes"); // b depends on a
    depends(&mut g, "cmp:a", "cmp:b"); // and a depends on b, directly

    let msg = cycle_message(&g);
    assert!(msg.contains("mixed"), "{msg}");
    assert!(
        !msg.contains("no DEPENDS_ON"),
        "there IS a DEPENDS_ON edge in this loop: {msg}"
    );
    assert!(
        msg.contains("ifc:1"),
        "the contract half is still named: {msg}"
    );
}

/// THE REAL BL-141 CASE, taken from the reporting project's own design rather
/// than invented: `cmp:fundamental-detection ⇄ cmp:midi-renderer` through
/// `ifc:midi-file` and `ifc:wav-audio`, both `medium: data`. A renderer reads
/// MIDI and writes WAV; a transcriber reads WAV and writes MIDI. Two programs
/// sharing two file formats, depending on each other at no point in time.
///
/// **Structurally identical to the genuine two-contract service cycle above** —
/// same node count, same edge shape, same interface count. ONLY the medium
/// separates them, which is why it has to be reported.
#[test]
fn a_round_trip_through_file_formats_reports_the_medium() {
    let mut g = project_with(&[("cmp:transcriber", "T"), ("cmp:renderer", "R")]);
    for (id, name) in [("ifc:midi", "MIDI file"), ("ifc:wav", "WAV audio")] {
        g.add_interface(id, name).expect("iface");
        g.set_interface_spec(
            id,
            Some("data"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("medium");
    }
    // Transcriber: reads WAV, writes MIDI.
    g.consumes("cmp:transcriber", "ifc:wav").expect("t reads");
    g.provides("cmp:transcriber", "ifc:midi").expect("t writes");
    // Renderer: reads MIDI, writes WAV. The inverse.
    g.consumes("cmp:renderer", "ifc:midi").expect("r reads");
    g.provides("cmp:renderer", "ifc:wav").expect("r writes");

    let msg = cycle_message(&g);
    assert!(
        msg.contains("ifc:midi") && msg.contains("ifc:wav"),
        "both formats must be named: {msg}"
    );
    assert!(
        msg.contains("library/data medium"),
        "the medium is the ONLY thing separating this from a real service cycle, \
         so it must be stated: {msg}"
    );
}

/// THE COUNTERWEIGHT that keeps the medium claim honest: the same shape over a
/// run-time medium is a real service cycle and must NOT be described as
/// read-or-linked-against. Without this, the medium sentence would be printed
/// on every contract cycle and mean nothing.
#[test]
fn the_same_shape_over_a_runtime_medium_makes_no_foundation_claim() {
    let mut g = project_with(&[("cmp:a", "A"), ("cmp:b", "B")]);
    for (id, name) in [("ifc:1", "A's API"), ("ifc:2", "B's API")] {
        g.add_interface(id, name).expect("iface");
        g.set_interface_spec(
            id,
            Some("REST"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("medium");
    }
    g.provides("cmp:a", "ifc:1").expect("a provides");
    g.consumes("cmp:b", "ifc:1").expect("b consumes");
    g.provides("cmp:b", "ifc:2").expect("b provides");
    g.consumes("cmp:a", "ifc:2").expect("a consumes");

    let msg = cycle_message(&g);
    assert!(
        !msg.contains("library/data medium"),
        "REST is carried at run time — this is a real cycle: {msg}"
    );
    assert!(msg.contains("every hop is a contract"), "{msg}");
}
