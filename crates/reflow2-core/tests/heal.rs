//! HEAL tests — detect structural defects, propose, apply atomically.
//!
//! The two behaviors that matter most: HEAL *proposes* (never mutates during
//! detection/proposal), and the one content-free repair — duplicate **merge** —
//! actually applies, re-points the merged node's edges, and verifies the defect
//! is gone. Generative fixes are gated behind `requires_human_review`.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory, HealOp, HealOptions, HealStrategy};

/// Two capabilities marked as duplicates; `cap:a` also satisfies a requirement,
/// so a correct merge must carry that edge onto the survivor.
fn dup_graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap(); // flexible by default
    g.add_requirement("req:r", "R", "need r").unwrap();
    g.add_component("cmp:c", "C", "part c").unwrap();
    g.add_capability("cap:a", "Cap A", "does a").unwrap();
    g.add_capability("cap:b", "Cap B", "also does a").unwrap();
    // cap:a is well-connected; cap:b is the redundant twin.
    g.satisfies("cap:a", "req:r").unwrap();
    g.allocate("cap:a", "cmp:c").unwrap();
    g.allocate("cap:b", "cmp:c").unwrap();
    // cap:a DUPLICATES cap:b (canonical keep = "cap:a", the smaller id).
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:b",
        Props::new(),
    )
    .unwrap();
    g
}

#[test]
fn detect_finds_duplicate_and_orphans() {
    let g = dup_graph();
    let issues = g.detect_defects().unwrap();
    let cats: Vec<HealCategory> = issues.iter().map(|i| i.category).collect();
    assert!(cats.contains(&HealCategory::Duplicate));
    // No orphans here: req satisfied, caps allocated, no lone artifacts.
    assert!(!cats.contains(&HealCategory::OrphanNode));
}

#[test]
fn proposal_computes_without_mutating() {
    let g = dup_graph();
    let before = g.count_nodes(node::CAPABILITY).unwrap();
    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    let after = g.count_nodes(node::CAPABILITY).unwrap();
    assert_eq!(before, after, "propose must not mutate the graph");

    // The duplicate becomes a structural Merge op keeping the canonical id.
    assert_eq!(proposal.operations.len(), 1);
    match &proposal.operations[0].op {
        HealOp::Merge {
            keep_id, remove_id, ..
        } => {
            assert_eq!(keep_id, "cap:a");
            assert_eq!(remove_id, "cap:b");
        }
        other => panic!("expected a Merge op, got {other:?}"),
    }
    // A structural-only proposal needs no human review and is high-confidence.
    assert!(!proposal.requires_human_review);
    assert!(proposal.confidence > 0.8);
}

#[test]
fn apply_merge_repoints_edges_and_verifies() {
    let mut g = dup_graph();
    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    let report = g.apply_heal(&proposal).unwrap();

    assert!(report.applied);
    assert_eq!(report.operations_applied, 1);
    assert!(
        report.verified,
        "post-repair check must confirm the dup is gone"
    );
    assert!(report.unresolved_issue_ids.is_empty());

    // cap:b is gone; cap:a remains.
    assert!(g.get_node(node::CAPABILITY, "cap:b").unwrap().is_none());
    assert!(g.get_node(node::CAPABILITY, "cap:a").unwrap().is_some());
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), 1);

    // cap:b's allocation was re-pointed onto cap:a (which was already allocated;
    // the edge just coalesces). cap:a still allocated to cmp:c.
    let alloc = g.outgoing("cap:a", Some(edge::ALLOCATED_TO)).unwrap();
    assert_eq!(alloc.len(), 1);
    assert_eq!(alloc[0].to_id, "cmp:c");

    // The DUPLICATES edge is gone, so re-detection finds no duplicate.
    let cats: Vec<HealCategory> = g
        .detect_defects()
        .unwrap()
        .iter()
        .map(|i| i.category)
        .collect();
    assert!(!cats.contains(&HealCategory::Duplicate));
}

#[test]
fn merge_carries_a_unique_edge_onto_the_survivor() {
    // cap:b has an allocation cap:a lacks — the merge must preserve it.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_component("cmp:c", "C", "part c").unwrap();
    g.add_component("cmp:d", "D", "part d").unwrap();
    g.add_capability("cap:a", "Cap A", "does a").unwrap();
    g.add_capability("cap:b", "Cap B", "does a").unwrap();
    g.allocate("cap:a", "cmp:c").unwrap();
    g.allocate("cap:b", "cmp:d").unwrap(); // unique to cap:b
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:b",
        Props::new(),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    g.apply_heal(&proposal).unwrap();

    let allocs: Vec<String> = g
        .outgoing("cap:a", Some(edge::ALLOCATED_TO))
        .unwrap()
        .into_iter()
        .map(|e| e.to_id)
        .collect();
    assert!(allocs.contains(&"cmp:c".to_string()));
    assert!(
        allocs.contains(&"cmp:d".to_string()),
        "cap:b's unique allocation must survive the merge"
    );
}

#[test]
fn generative_fixes_require_human_review_and_are_not_applied() {
    // An orphan capability (no allocation) → generative owner fix.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_component("cmp:c", "C", "part c").unwrap();
    g.add_capability("cap:lonely", "Lonely", "unallocated")
        .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    assert!(proposal.requires_human_review);
    assert!(
        proposal.operations.is_empty(),
        "no auto-applicable structural op"
    );
    // The orphan capability's fix is a generative owner edge (the isolated
    // component here also yields a generative stub — both are review-gated).
    assert!(
        proposal
            .generated_content
            .iter()
            .any(|s| s.kind == "owner edge"),
        "orphan capability should propose an owner edge for review"
    );

    // Applying it changes nothing structurally (generation is deferred).
    let before = g.count_nodes(node::CAPABILITY).unwrap();
    let report = g.apply_heal(&proposal).unwrap();
    assert_eq!(report.operations_applied, 0);
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), before);
}

#[test]
fn conservative_strategy_addresses_nothing_when_only_warnings_exist() {
    let g = dup_graph(); // duplicate is a WARNING
    let proposal = g
        .propose_heal(HealOptions {
            strategy: HealStrategy::Conservative,
            max_operations: None,
        })
        .unwrap();
    assert!(proposal.issues_addressed.is_empty());
    assert!(proposal.operations.is_empty());
}

#[test]
fn rigid_mode_proposes_but_never_auto_applies() {
    let mut g = dup_graph();
    // Flip the project to rigid.
    g.create_node(
        node::PROJECT,
        "proj:x",
        Props::new().set("name", "X").set("mode", "rigid"),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    assert!(!proposal.operations.is_empty(), "rigid still proposes");

    let report = g.apply_heal(&proposal).unwrap();
    assert!(report.blocked_by_mode);
    assert!(!report.applied);
    // The duplicate is untouched.
    assert!(g.get_node(node::CAPABILITY, "cap:b").unwrap().is_some());
}

#[test]
fn max_operations_cap_surfaces_overflow_never_drops_it() {
    // Two independent duplicate pairs → two merge ops; cap at 1.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    for id in ["a", "b", "c", "d"] {
        g.add_capability(&format!("cap:{id}"), id, "does").unwrap();
    }
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:b",
        Props::new(),
    )
    .unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:c",
        node::CAPABILITY,
        "cap:d",
        Props::new(),
    )
    .unwrap();

    let proposal = g
        .propose_heal(HealOptions {
            strategy: HealStrategy::Balanced,
            max_operations: Some(1),
        })
        .unwrap();
    assert_eq!(proposal.operations.len(), 1);
    assert_eq!(
        proposal.skipped_operations.len(),
        1,
        "overflow must be surfaced"
    );
    assert!(
        proposal.skipped_operations[0]
            .reason
            .contains("max_operations")
    );
}
