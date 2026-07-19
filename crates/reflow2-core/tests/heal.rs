//! HEAL tests — detect structural defects, propose, apply atomically.
//!
//! The two behaviors that matter most: HEAL *proposes* (never mutates during
//! detection/proposal), and the one content-free repair — duplicate **merge** —
//! actually applies, re-points the merged node's edges, and verifies the defect
//! is gone. Generative fixes are gated behind `requires_human_review`.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory, HealOp, HealOptions, HealProposal, HealStrategy};

/// Two capabilities marked as duplicates; `cap:a` also satisfies a requirement,
/// so a correct merge must carry that edge onto the survivor.
fn dup_graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap(); // flexible by default
    g.add_requirement("req:r", "R", "need r").unwrap();
    g.add_component("cmp:c", "C", "part c", None).unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_capability("cap:b", "Cap B", "also does a", None)
        .unwrap();
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
    g.add_component("cmp:c", "C", "part c", None).unwrap();
    g.add_component("cmp:d", "D", "part d", None).unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_capability("cap:b", "Cap B", "does a", None).unwrap();
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
    g.add_component("cmp:c", "C", "part c", None).unwrap();
    g.add_capability("cap:lonely", "Lonely", "unallocated", None)
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
        g.add_capability(&format!("cap:{id}"), id, "does", None)
            .unwrap();
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

// ---- BL-29 · the proposal is checked, not trusted --------------------------

/// Build a proposal the way an MCP client can: hand-written JSON, straight into
/// `apply_heal`. This is the shape that deleted a node it had no business
/// touching.
fn hand_crafted(issue_id: &str, keep: &str, remove: &str) -> HealProposal {
    serde_json::from_value(serde_json::json!({
        "target_id": "proj:1",
        "summary": "hand-written",
        "strategy": "balanced",
        "issues_addressed": [],
        "operations": [{
            "issue_id": issue_id,
            "op": {"Merge": {
                "keep_type": "Capability", "keep_id": keep,
                "remove_type": "Capability", "remove_id": remove}}
        }],
        "generated_content": [],
        "skipped_operations": [],
        "requires_human_review": true,
        "confidence": 0.0
    }))
    .expect("a client can send exactly this")
}

#[test]
fn a_merge_no_detector_asked_for_is_refused() {
    // Verified as a live defect before the fix: two capabilities with no
    // DUPLICATES edge between them, which detect_defects reports only as
    // orphans, were merged on request and one was deleted.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:1", "P").unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:keep", "Keeper", "survivor", None)
        .unwrap();
    g.add_capability("cap:doomed", "Doomed", "not a duplicate of anything", None)
        .unwrap();
    g.satisfies("cap:keep", "req:a").unwrap();
    g.satisfies("cap:doomed", "req:a").unwrap();

    assert!(
        !g.detect_defects()
            .unwrap()
            .iter()
            .any(|d| d.category == HealCategory::Duplicate),
        "precondition: nothing calls these duplicates"
    );

    let err = g
        .apply_heal(&hand_crafted("heal:madeup", "cap:keep", "cap:doomed"))
        .expect_err("a proposal HEAL never made must be refused");
    assert!(
        err.to_string().contains("not one HEAL proposes"),
        "got: {err}"
    );

    // And the refusal happened before any write.
    assert!(
        g.get_node(node::CAPABILITY, "cap:doomed")
            .unwrap()
            .is_some(),
        "a refused proposal must leave the graph untouched"
    );
}

#[test]
fn a_real_issue_id_with_a_fabricated_operation_is_still_refused() {
    // The subtler attack: quote an issue id that genuinely exists, but pair it
    // with a merge of two other nodes.
    let mut g = dup_graph();
    g.add_capability("cap:bystander", "Bystander", "uninvolved", None)
        .unwrap();
    let real_id = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .find(|d| d.category == HealCategory::Duplicate)
        .unwrap()
        .id;

    let err = g
        .apply_heal(&hand_crafted(&real_id, "cap:a", "cap:bystander"))
        .expect_err("the issue id is real but the operation is not the one it implies");
    assert!(
        err.to_string().contains("not one HEAL proposes"),
        "got: {err}"
    );
    assert!(
        g.get_node(node::CAPABILITY, "cap:bystander")
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_proposal_heal_actually_made_still_applies() {
    // The guard must not break the real flow.
    let mut g = dup_graph();
    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    let report = g.apply_heal(&proposal).unwrap();

    assert!(report.applied);
    assert_eq!(report.operations_applied, 1);
    assert!(report.verified);
    assert!(g.get_node(node::CAPABILITY, "cap:b").unwrap().is_none());
}

#[test]
fn a_proposal_goes_stale_when_the_defect_is_resolved_by_hand() {
    // Propose, then remove the DUPLICATES edge by hand, then apply. The issue no
    // longer holds, so the merge must not run on the strength of a stale
    // proposal.
    let mut g = dup_graph();
    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    g.delete_edge(edge::DUPLICATES, "cap:a", "cap:b").unwrap();

    let err = g
        .apply_heal(&proposal)
        .expect_err("the defect is gone, so its repair is no longer sanctioned");
    assert!(
        err.to_string().contains("not one HEAL proposes"),
        "got: {err}"
    );
    assert!(g.get_node(node::CAPABILITY, "cap:b").unwrap().is_some());
}

// ---- BL-29 · a merge says what it could not carry --------------------------

#[test]
fn merge_reports_the_properties_it_could_not_keep() {
    let mut g = dup_graph();
    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    let report = g.apply_heal(&proposal).unwrap();

    let lost = report
        .discarded
        .iter()
        .find(|d| d.reference == "cap:b")
        .unwrap_or_else(|| {
            panic!(
                "the removed node's properties vanished silently: {:?}",
                report.discarded
            )
        });
    assert!(
        lost.reason.contains("description") && lost.reason.contains("name"),
        "must name what was let go, got: {}",
        lost.reason
    );
}

#[test]
fn a_merge_that_loses_nothing_reports_nothing() {
    // The report must not cry wolf: a survivor with no colliding edges and a
    // removed node carrying only what merges cleanly should stay quiet about
    // edges, even though its own properties are always noted.
    let mut g = dup_graph();
    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    let report = g.apply_heal(&proposal).unwrap();

    assert!(
        !report
            .discarded
            .iter()
            .any(|d| d.reason.contains("not a known node")),
        "no edge should have been unmovable here: {:?}",
        report.discarded
    );
}

#[test]
fn merge_reports_an_edge_whose_properties_are_overwritten() {
    // Both capabilities are allocated to cmp:c, and the doomed one's edge
    // carries a property. create_edge is an upsert on (type, from, to), so the
    // survivor's version of that edge is overwritten rather than kept beside it.
    let mut g = dup_graph();
    g.create_edge(
        edge::ALLOCATED_TO,
        node::CAPABILITY,
        "cap:b",
        node::COMPONENT,
        "cmp:c",
        Props::new().set("rationale", "the doomed one's reason"),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    let report = g.apply_heal(&proposal).unwrap();

    assert!(
        report
            .discarded
            .iter()
            .any(|d| d.reason.contains("overwrite")),
        "an upsert collision must be reported, got: {:?}",
        report.discarded
    );
}

#[test]
fn a_cross_type_merge_is_refused_rather_than_half_applied() {
    // DUPLICATES is declared `from: "*" to: "*"`, so this edge is schema-valid.
    // Merging across types would re-point one type's edges onto another and be
    // rejected part-way through, after earlier operations had committed.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:1", "P").unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "A", "does a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::REQUIREMENT,
        "req:a",
        node::CAPABILITY,
        "cap:a",
        Props::new(),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    assert!(
        proposal.operations.is_empty(),
        "a cross-type merge must never become an applicable operation"
    );
    assert!(
        proposal
            .skipped_operations
            .iter()
            .any(|s| s.reason.contains("across node types")),
        "and it must say why, not vanish: {:?}",
        proposal.skipped_operations
    );
}
