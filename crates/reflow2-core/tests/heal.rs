//! HEAL tests — detect structural defects, propose, apply atomically.
//!
//! The two behaviors that matter most: HEAL *proposes* (never mutates during
//! detection/proposal), and the one content-free repair — duplicate **merge** —
//! actually applies, re-points the merged node's edges, and verifies the defect
//! is gone. Generative fixes are gated behind `requires_human_review` — and
//! since 2026-08-08 so is every merge, because "content-free" was never the
//! same as "consequence-free": the merge DELETES a node, and a proposal that
//! deletes design content now says what it would destroy before anyone can
//! apply it (`would_destroy`).

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{
    DesignGraph, HealCategory, HealOp, HealOperation, HealOptions, HealProposal, HealSeverity,
    HealStrategy,
};

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
        Props::new().set("basis", "asserted"),
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
    // CHANGED 2026-08-08, and the old assertion is the finding. This read:
    //   "A structural-only proposal needs no human review and is high-confidence."
    //   assert!(!proposal.requires_human_review);
    //   assert!(proposal.confidence > 0.8);
    // "Structural-only" was doing the damage. The one structural repair HEAL
    // performs is a MERGE, and a merge DELETES A NODE with no undo — so the
    // proposal this test blessed as needing no human review is a proposal to
    // destroy design. dev_storyflow followed exactly that signal into ten
    // deletions and stood their fleet down from the whole HEAL surface.
    // The test encoded the assumption rather than catching it, which is why it
    // is rewritten here rather than deleted.
    assert!(
        proposal.requires_human_review,
        "a proposal whose only operation deletes a node is not a proposal that needs no review"
    );
    assert!(proposal.confidence <= 0.5);
    assert_eq!(
        proposal.would_destroy.len(),
        1,
        "and it must say what it would destroy, before anyone can apply it"
    );
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
        Props::new().set("basis", "asserted"),
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
    // An Artifact that realizes nothing → generative owner fix.
    //
    // This used to use an unallocated Capability, until BL-42 removed that
    // check from HEAL: DETECT already asks `unallocated_capability`, and
    // reporting it here as well was the same finding twice (20 of 31 defects
    // on the storyflow trial). The Artifact orphan is the one HEAL keeps,
    // because DETECT has no counterpart for it — and it still exercises what
    // this test is actually about: a generative fix is gated, never applied.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_component("cmp:c", "C", "part c", None).unwrap();
    g.add_artifact("art:loose", "loose.rs", Some("code"), Some("src/loose.rs"))
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
        Props::new().set("basis", "asserted"),
    )
    .unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:c",
        node::CAPABILITY,
        "cap:d",
        Props::new().set("basis", "asserted"),
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
fn merge_keeps_the_survivors_edge_and_reports_the_dropped_properties() {
    // Both capabilities are allocated to cmp:c, and the doomed one's edge
    // carries a property the survivor's lacks. The survivor's edge wins — the
    // colliding edge is not re-pointed, so the doomed one's properties do not
    // land on top of it — and the drop is reported (BL-47's second finding:
    // report-then-clobber was the wrong half of two-sided accept).
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
            .any(|d| d.reason.contains("dropped")),
        "the collision must be reported as a drop, got: {:?}",
        report.discarded
    );
    let alloc = g.outgoing("cap:a", Some(edge::ALLOCATED_TO)).unwrap();
    assert_eq!(alloc.len(), 1);
    assert!(
        !alloc[0].properties.contains_key("rationale"),
        "the survivor's edge must keep its own properties, not the doomed one's: {:?}",
        alloc[0].properties
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
        Props::new().set("basis", "asserted"),
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

// ---- BL-29 · chained merges ------------------------------------------------

/// a↔b and b↔c both DUPLICATES, with the chain's far end carrying the only
/// copy of a real edge. Two merges, each individually sanctioned, sharing
/// `cap:b` — applied together in the wrong order, the second used to re-point
/// `cap:c`'s edges onto the already-deleted `cap:b`, and the storage layer
/// accepted the dangling edge while the report said `verified`.
fn chained_graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_component("cmp:d", "D", "part d", None).unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_capability("cap:b", "Cap B", "does a", None).unwrap();
    g.add_capability("cap:c", "Cap C", "does a", None).unwrap();
    g.allocate("cap:c", "cmp:d").unwrap(); // unique to the chain's far end
    for (x, y) in [("cap:a", "cap:b"), ("cap:b", "cap:c")] {
        g.create_edge(
            edge::DUPLICATES,
            node::CAPABILITY,
            x,
            node::CAPABILITY,
            y,
            Props::new().set("basis", "asserted"),
        )
        .unwrap();
    }
    g
}

/// The sanctioned merge op for a detected duplicate pair, exactly as a client
/// could hand-build it.
fn merge_op(g: &DesignGraph, keep: &str, remove: &str) -> HealOperation {
    let issue = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .filter(|i| i.category == HealCategory::Duplicate)
        .find(|i| i.affected_ids == vec![keep.to_string(), remove.to_string()])
        .expect("the duplicate issue exists");
    HealOperation {
        issue_id: issue.id,
        op: HealOp::Merge {
            keep_type: "Capability".into(),
            keep_id: keep.into(),
            remove_type: "Capability".into(),
            remove_id: remove.into(),
        },
    }
}

fn hand_proposal(ops: Vec<HealOperation>) -> HealProposal {
    HealProposal {
        target_id: "proj:x".into(),
        strategy: HealStrategy::Balanced,
        issues_addressed: vec![],
        operations: ops,
        generated_content: vec![],
        skipped_operations: vec![],
        // Left empty on purpose: apply re-derives every operation and does NOT
        // consult the proposal's own advisory fields (see the note at
        // `apply_heal`), so a hand-built proposal understating what it destroys
        // must still be judged on its operations. That is the property these
        // refusal tests exist to pin.
        would_destroy: vec![],
        // Same reasoning as `would_destroy` above: apply re-derives everything
        // and never reads these, so a hand-built proposal may understate its
        // own sweep and must still be judged on its operations.
        scope: "whole design".into(),
        projects_in_scope: 1,
        merge_candidates_considered: 0,
        confidence: 0.9,
        requires_human_review: false,
        summary: "hand-built".into(),
    }
}

#[test]
fn a_chained_duplicate_is_split_across_rounds_and_converges() {
    let mut g = chained_graph();

    // Round 1: only one link of the chain is applicable; the other is deferred
    // with the reason stated, never silently dropped.
    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    assert_eq!(
        proposal.operations.len(),
        1,
        "one merge per chain per round: {:?}",
        proposal.operations
    );
    assert!(
        proposal
            .skipped_operations
            .iter()
            .any(|s| s.reason.contains("chained duplicate")),
        "the deferred link must say why: {:?}",
        proposal.skipped_operations
    );

    // Propose/apply rounds resolve the whole chain.
    let mut rounds = 0;
    loop {
        let proposal = g.propose_heal(HealOptions::default()).unwrap();
        if proposal.operations.is_empty() {
            break;
        }
        let report = g.apply_heal(&proposal).unwrap();
        assert!(report.applied);
        rounds += 1;
        assert!(rounds <= 3, "the chain must converge, not oscillate");
    }
    assert_eq!(rounds, 2, "a two-link chain takes exactly two rounds");

    // Converged: one survivor holding the far end's unique allocation, no
    // duplicate left, and every remaining edge anchored on a live node.
    assert!(g.get_node(node::CAPABILITY, "cap:a").unwrap().is_some());
    assert!(g.get_node(node::CAPABILITY, "cap:b").unwrap().is_none());
    assert!(g.get_node(node::CAPABILITY, "cap:c").unwrap().is_none());
    let allocs = g.incoming("cmp:d", Some(edge::ALLOCATED_TO)).unwrap();
    assert_eq!(allocs.len(), 1);
    assert_eq!(allocs[0].from_id, "cap:a");
    assert!(
        !g.detect_defects()
            .unwrap()
            .iter()
            .any(|i| i.category == HealCategory::Duplicate),
        "the whole chain must be resolved"
    );
}

#[test]
fn sanctioned_merges_sharing_a_node_are_refused_before_any_write() {
    // Each op alone is exactly what HEAL sanctions, so op-matching passes both;
    // ordered (a,b) first, applying them used to delete cap:b and then re-point
    // cap:c's edges onto it — the reproduced corruption this guard exists for.
    let mut g = chained_graph();
    let ops = vec![
        merge_op(&g, "cap:a", "cap:b"),
        merge_op(&g, "cap:b", "cap:c"),
    ];

    let err = g
        .apply_heal(&hand_proposal(ops))
        .expect_err("merges sharing a node must be refused");
    assert!(
        err.to_string().contains("no longer exists"),
        "the refusal must explain the hazard, got: {err}"
    );

    // Refused before any write: the whole chain is still there.
    assert!(g.get_node(node::CAPABILITY, "cap:b").unwrap().is_some());
    assert!(g.get_node(node::CAPABILITY, "cap:c").unwrap().is_some());
    let allocs = g.incoming("cmp:d", Some(edge::ALLOCATED_TO)).unwrap();
    assert_eq!(allocs.len(), 1);
    assert_eq!(allocs[0].from_id, "cap:c");
}

#[test]
fn merging_the_middle_of_a_chain_repoints_the_duplicate_claim() {
    // Apply only (a,b) — sanctioned, and legal on its own. cap:b carried the
    // user's still-unresolved claim `b DUPLICATES c`; the merge must leave that
    // claim behind as a↔c, not let it vanish with b.
    let mut g = chained_graph();
    let proposal = hand_proposal(vec![merge_op(&g, "cap:a", "cap:b")]);
    let report = g.apply_heal(&proposal).unwrap();
    assert_eq!(report.operations_applied, 1);

    let mut dup_partners: Vec<String> = g
        .outgoing("cap:a", Some(edge::DUPLICATES))
        .unwrap()
        .into_iter()
        .map(|e| e.to_id)
        .chain(
            g.incoming("cap:a", Some(edge::DUPLICATES))
                .unwrap()
                .into_iter()
                .map(|e| e.from_id),
        )
        .collect();
    dup_partners.sort();
    assert_eq!(
        dup_partners,
        vec!["cap:c".to_string()],
        "the chain's unresolved half must survive as a DUPLICATES on the survivor"
    );

    // And the next round finds and can resolve it.
    assert!(
        g.detect_defects()
            .unwrap()
            .iter()
            .any(|i| i.category == HealCategory::Duplicate
                && i.affected_ids == vec!["cap:a".to_string(), "cap:c".to_string()]),
        "re-detection must see the re-pointed claim"
    );
}

#[test]
fn an_edge_joining_the_merging_pair_is_reported_not_silently_dropped() {
    // A real (non-DUPLICATES) edge between the two nodes being merged cannot be
    // re-pointed — it would become a self-loop — so it dies with the merge. That
    // is a genuine loss and must appear in `discarded`.
    let mut g = dup_graph();
    g.create_edge(
        edge::TRIGGERS,
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:b",
        Props::new(),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    let report = g.apply_heal(&proposal).unwrap();

    assert!(
        report
            .discarded
            .iter()
            .any(|d| d.reference.contains("TRIGGERS") && d.reason.contains("self-loop")),
        "the pair-joining edge must be named in discarded: {:?}",
        report.discarded
    );
}

// ---- BL-29 · the survivor rule: provenance wins, id breaks ties ------------

#[test]
fn an_authored_node_survives_a_merge_with_an_inferred_one_regardless_of_id() {
    // cap:a would win the id tiebreak; it is the machine's guess, so the
    // authored cap:z survives instead — the guess must never delete the
    // human's words.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_component("cmp:c", "C", "part c", None).unwrap();
    g.add_capability("cap:a", "Guessed", "read out of the code", None)
        .unwrap();
    g.add_capability("cap:z", "Stated", "what the stakeholder said", None)
        .unwrap();
    g.set_provenance(node::CAPABILITY, "cap:a", "inferred")
        .unwrap();
    g.allocate("cap:a", "cmp:c").unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:z",
        Props::new().set("basis", "asserted"),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    match &proposal.operations[0].op {
        HealOp::Merge {
            keep_id, remove_id, ..
        } => {
            assert_eq!(keep_id, "cap:z", "authored survives");
            assert_eq!(remove_id, "cap:a", "the inferred twin is merged away");
        }
        other => panic!("expected a Merge, got {other:?}"),
    }

    let report = g.apply_heal(&proposal).unwrap();
    assert!(report.verified);
    assert!(g.get_node(node::CAPABILITY, "cap:z").unwrap().is_some());
    assert!(g.get_node(node::CAPABILITY, "cap:a").unwrap().is_none());
    // The guess's structure still carries over to the surviving words.
    let allocs = g.outgoing("cap:z", Some(edge::ALLOCATED_TO)).unwrap();
    assert_eq!(allocs.len(), 1);
    assert_eq!(allocs[0].to_id, "cmp:c");
}

#[test]
fn equal_provenance_falls_back_to_the_smaller_id() {
    // dup_graph's pair carries the schema default — both authored — so the
    // pre-decision rule still decides: cap:a survives. This is also the
    // behaviour of every graph written before the property existed.
    let g = dup_graph();
    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    match &proposal.operations[0].op {
        HealOp::Merge {
            keep_id, remove_id, ..
        } => {
            assert_eq!(keep_id, "cap:a");
            assert_eq!(remove_id, "cap:b");
        }
        other => panic!("expected a Merge, got {other:?}"),
    }
}

#[test]
fn a_planned_stub_never_outlives_the_authored_twin_on_the_alphabet() {
    // The 2026-07-20 self-adopt shape (BL-47): the stub sorts first, so under
    // an id tiebreak it would delete the authored node's words. Its explicit
    // `planned` must lose to the survivor's `authored` outright. (The unset-
    // provenance half of BL-47 — a vintage node with no property at all — is
    // pinned at the provenance_rank seam in src/heal.rs, because schema
    // defaults materialize on create and today's API cannot build one.)
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_capability("cap:a-stub", "Planned twin", "the genesis scaffold", None)
        .unwrap();
    g.add_capability("cap:kit", "Authored", "what the user stated", None)
        .unwrap();
    g.set_provenance(node::CAPABILITY, "cap:a-stub", "planned")
        .unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:a-stub",
        node::CAPABILITY,
        "cap:kit",
        Props::new().set("basis", "asserted"),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    match &proposal.operations[0].op {
        HealOp::Merge {
            keep_id, remove_id, ..
        } => {
            assert_eq!(keep_id, "cap:kit", "authored survives");
            assert_eq!(remove_id, "cap:a-stub", "the planned stub is merged away");
        }
        other => panic!("expected a Merge, got {other:?}"),
    }
}

#[test]
fn the_provenance_order_is_graded_not_binary() {
    // Neither node is authored: the machine's guess (inferred) still outranks
    // machine-generated fill (healed), so cap:z survives its smaller-id twin.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_capability("cap:a", "Filled", "generated", None)
        .unwrap();
    g.add_capability("cap:z", "Guessed", "read out of the code", None)
        .unwrap();
    g.set_provenance(node::CAPABILITY, "cap:a", "healed")
        .unwrap();
    g.set_provenance(node::CAPABILITY, "cap:z", "inferred")
        .unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:z",
        Props::new().set("basis", "asserted"),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    match &proposal.operations[0].op {
        HealOp::Merge {
            keep_id, remove_id, ..
        } => {
            assert_eq!(keep_id, "cap:z");
            assert_eq!(remove_id, "cap:a");
        }
        other => panic!("expected a Merge, got {other:?}"),
    }
}

#[test]
fn a_self_loop_duplicates_edge_never_becomes_a_merge() {
    // BL-53: `x DUPLICATES x` drove a sanctioned merge that deleted the node
    // itself (re-pointing skipped every edge, the delete removed the survivor),
    // reported as applied/verified. It must be refused at derivation, which
    // covers propose and apply alike.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:a", "A", "does a", None).unwrap();
    g.create_edge(
        "DUPLICATES",
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:a",
        // `asserted` on purpose: BL-53's guard lives in the merge DERIVATION,
        // so the self-loop has to reach it to be refused there. A `suspected`
        // self-loop never gets that far, which would make this test pass for
        // the wrong reason and leave the real guard uncovered.
        Props::new().set("basis", "asserted"),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    assert!(
        !proposal
            .operations
            .iter()
            .any(|op| matches!(&op.op, HealOp::Merge { .. })),
        "a self-loop must not produce a merge operation"
    );
    assert!(
        proposal
            .skipped_operations
            .iter()
            .any(|s| s.reason.contains("cannot duplicate itself")),
        "the refusal must be reported, not silent: {:?}",
        proposal.skipped_operations
    );

    // Applying the (merge-free) proposal must leave the node standing.
    g.apply_heal(&proposal).unwrap();
    assert!(
        g.get_node(node::CAPABILITY, "cap:a").unwrap().is_some(),
        "the node must survive"
    );
}

// ---- 2026-08-08 · a merge says what it destroys BEFORE the act -------------
//
// dev_storyflow's field report: their scoped detect_defects returned five
// duplicate findings, propose_heal turned them into ten node deletions, and the
// proposal reported `requires_human_review: false` with confidence 0.9 — so the
// served check-health skill was right to call it the mechanical half, and the
// fleet stood itself down from the whole HEAL surface as a result.
//
// The cost WAS reported — in `HealReport::discarded`, which is the receipt of an
// irreversible act. The person deciding reads the PROPOSAL. These pin the
// disclosure at the only place it helps.

#[test]
fn a_proposal_that_would_delete_a_node_demands_human_review() {
    // The defect exactly: this proposal generates no content, so before
    // 2026-08-08 `requires_human_review` was false and confidence 0.9 — for a
    // proposal whose entire content is an irreversible deletion.
    let g = dup_graph();
    let proposal = g.propose_heal(HealOptions::default()).unwrap();

    assert!(
        matches!(proposal.operations[0].op, HealOp::Merge { .. }),
        "fixture must produce a merge for this test to mean anything"
    );
    assert!(
        proposal.generated_content.is_empty(),
        "the whole point is that a DELETION demands review even with nothing generated"
    );
    assert!(
        proposal.requires_human_review,
        "a proposal that deletes a node must demand review; generating a sentence \
         has always demanded it and deleting a node did not"
    );
    assert_eq!(
        proposal.confidence, 0.5,
        "confidence must reflect that a human still has to look"
    );
}

#[test]
fn the_proposal_names_the_properties_the_merge_would_destroy() {
    // storyflow's pair, reproduced in miniature: two requirements a human
    // asserted as duplicates, differing in exactly the two fields that carry
    // consequence — and both `authored`, so the ALPHABET picks the victim.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_requirement(
        "req:a-backend",
        "Backend as product",
        "third parties consume the API",
    )
    .unwrap();
    g.add_requirement(
        "req:z-content-bible",
        "Graph is the content bible",
        "panels are grounded",
    )
    .unwrap();
    // NOTE the trap, hit while writing this: the Rust `create_node` REPLACES
    // (graph.rs), unlike the MCP tool of the same name which merges — so
    // passing only priority/status here dropped the required `statement` and
    // the call was refused. Every property the node keeps must be restated.
    g.create_node(
        node::REQUIREMENT,
        "req:z-content-bible",
        Props::new()
            .set("name", "Graph is the content bible")
            .set("statement", "panels are grounded")
            .set("priority", "critical")
            .set("status", "proposed"),
    )
    .unwrap();
    g.create_node(
        node::REQUIREMENT,
        "req:a-backend",
        Props::new()
            .set("name", "Backend as product")
            .set("statement", "third parties consume the API")
            .set("priority", "medium")
            .set("status", "accepted"),
    )
    .unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::REQUIREMENT,
        "req:a-backend",
        node::REQUIREMENT,
        "req:z-content-bible",
        Props::new().set("basis", "asserted"),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    assert_eq!(
        proposal.would_destroy.len(),
        1,
        "one merge, one disclosure: {:?}",
        proposal.would_destroy
    );
    let note = &proposal.would_destroy[0];
    assert_eq!(
        note.reference, "req:z-content-bible",
        "the alphabet keeps req:a-backend, so the critical one is the victim"
    );
    assert!(
        note.reason.contains("priority 'critical' -> 'medium'"),
        "the proposal must say a critical priority dies here: {}",
        note.reason
    );
    assert!(
        note.reason.contains("status 'proposed' -> 'accepted'"),
        "and that the status changes under it: {}",
        note.reason
    );
    assert!(
        note.reason.contains("THE ALPHABET CHOSE THE VICTIM"),
        "equal provenance means nothing about the DESIGN decided this, and \
         saying so is the whole disclosure: {}",
        note.reason
    );
}

#[test]
fn a_provenance_decided_merge_does_not_blame_the_alphabet() {
    // The counterweight. When provenance genuinely decides, the note must NOT
    // cry alphabet — otherwise the warning is noise and gets skimmed, which is
    // how the fleet read past the `next` string twice.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_capability("cap:a", "Guessed", "read out of the code", None)
        .unwrap();
    g.add_capability("cap:z", "Stated", "what the stakeholder said", None)
        .unwrap();
    g.set_provenance(node::CAPABILITY, "cap:a", "inferred")
        .unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:z",
        Props::new().set("basis", "asserted"),
    )
    .unwrap();

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    let note = &proposal.would_destroy[0];
    assert_eq!(note.reference, "cap:a", "the machine's guess is the victim");
    assert!(
        !note.reason.contains("THE ALPHABET"),
        "provenance decided this one, so the alphabet must not be blamed: {}",
        note.reason
    );
    assert!(
        note.reason.contains("provenance 'inferred' -> 'authored'"),
        "and it should still say what dies with it: {}",
        note.reason
    );
    // Still a deletion, so still a human's call.
    assert!(proposal.requires_human_review);
}

// ---- 2026-08-08 · a finding says when its node is a hub --------------------
//
// dev_storyflow: a scoped detect_defects returned `in_scope: 5`, every one a
// duplicate — and one node was in THREE of the pairs while another was in the
// other two. Five findings were TWO nodes. Mid-stand-down, the count read as
// five independent judgements corroborating each other.

#[test]
fn a_node_in_several_duplicate_findings_is_named_as_a_hub() {
    // storyflow's shape in miniature: cap:hub is asserted duplicate of three
    // unrelated capabilities, so three findings are really one node.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_capability(
        "cap:hub",
        "Hub",
        "the node the scorer pairs with everything",
        None,
    )
    .unwrap();
    for id in ["cap:one", "cap:two", "cap:three"] {
        g.add_capability(id, id, "unrelated", None).unwrap();
        g.create_edge(
            edge::DUPLICATES,
            node::CAPABILITY,
            "cap:hub",
            node::CAPABILITY,
            id,
            Props::new().set("basis", "asserted"),
        )
        .unwrap();
    }

    let issues = g.detect_defects().unwrap();
    let dups: Vec<_> = issues
        .iter()
        .filter(|i| i.category == HealCategory::Duplicate)
        .collect();
    assert_eq!(dups.len(), 3, "three pairs");

    for issue in &dups {
        let hub = issue
            .hubs
            .iter()
            .find(|h| h.node_id == "cap:hub")
            .unwrap_or_else(|| panic!("every finding must name the shared node: {:?}", issue.hubs));
        assert_eq!(
            hub.in_findings, 3,
            "and say how many findings it appears in, so three findings do not \
             read as three independent judgements"
        );
        // The one-off partner is NOT a hub — otherwise the signal is noise.
        assert!(
            !issue.hubs.iter().any(|h| h.node_id.starts_with("cap:on")
                || h.node_id == "cap:two"
                || h.node_id == "cap:three"),
            "a node in exactly one finding is not a hub: {:?}",
            issue.hubs
        );
    }
}

#[test]
fn an_ordinary_duplicate_pair_reports_no_hub() {
    // The counterweight. If `hubs` were populated for every finding it would be
    // noise, and a warning that fires always is a warning nobody reads (BL-42).
    let g = dup_graph();
    let issues = g.detect_defects().unwrap();
    let dup = issues
        .iter()
        .find(|i| i.category == HealCategory::Duplicate)
        .expect("fixture has a duplicate");
    assert!(
        dup.hubs.is_empty(),
        "one pair, no shared node, nothing to say: {:?}",
        dup.hubs
    );
}

// ---- a proposal describes its own sweep -------------------------------------
//
// req:a-report-says-what-it-swept-and-whether-its-checks-ran.
//
// Both halves come from one fleet report (sb-boss, 2026-08-15) and both were
// reproduced on reflow2's own graph. The stake is not tidiness: this fleet runs
// propose_heal as the read-only evidence step of a standing stop on apply_heal,
// which DELETES NODES. A zero that cannot say which kind it is sits one skim
// away from lifting that gate.

/// THE CASE. No duplicate exists, so the pair scorer never ran — and the reply
/// must say so rather than presenting an empty operation list as a clean bill.
#[test]
fn a_zero_says_it_had_nothing_to_examine() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_requirement("req:r", "R", "need r").unwrap();

    let p = g.propose_heal(HealOptions::default()).unwrap();

    assert!(p.operations.is_empty(), "precondition: nothing to merge");
    assert_eq!(
        p.merge_candidates_considered, 0,
        "the scorer was given nothing"
    );
    assert!(
        p.summary.contains("not a pass"),
        "the line a person reads must say this zero is not a pass: {}",
        p.summary
    );
}

/// COUNTERWEIGHT, and the one that stops this becoming a different lie: when
/// the scorer WAS given something, the reply must not claim it had nothing.
#[test]
fn a_scorer_that_ran_is_reported_as_having_run() {
    let g = dup_graph();
    let p = g.propose_heal(HealOptions::default()).unwrap();

    assert_eq!(
        p.merge_candidates_considered, 1,
        "one asserted duplicate reached the scorer"
    );
    assert!(!p.operations.is_empty(), "and it proposed the merge");
    assert!(
        !p.summary.contains("not a pass"),
        "an exercised check must NOT be labelled vacuous: {}",
        p.summary
    );
}

/// The third state, which is the one the fleet actually wanted to be able to
/// see: candidates existed, were examined, and none survived scoring.
#[test]
fn examined_but_nothing_proposed_is_its_own_answer() {
    let mut g = dup_graph();
    // A `suspected` basis is deliberately never mergeable (dec:ask-not-repair),
    // so the candidate is raised as a gap and no operation is built.
    g.delete_edge(edge::DUPLICATES, "cap:a", "cap:b").unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:a",
        node::CAPABILITY,
        "cap:b",
        Props::new().set("basis", "suspected"),
    )
    .unwrap();

    let p = g.propose_heal(HealOptions::default()).unwrap();
    assert!(p.operations.is_empty());
    assert_eq!(
        p.merge_candidates_considered, 0,
        "a suspected pair never reaches the scorer, so it was not considered"
    );
}

/// THE SCOPE HALF. One Project can be named without ambiguity.
#[test]
fn a_single_project_is_named_and_the_scope_is_still_the_whole_design() {
    let g = dup_graph();
    let p = g.propose_heal(HealOptions::default()).unwrap();

    assert_eq!(p.target_id, "proj:x");
    assert_eq!(p.projects_in_scope, 1);
    assert_eq!(p.scope, "whole design", "the sweep is never one project");
}

/// COUNTERWEIGHT: with more than one Project, the label must STOP naming one.
/// This is the case reflow2's own graph cannot produce — it holds exactly one
/// Project, which is why the self-host never saw the defect.
#[test]
fn more_than_one_project_is_never_labelled_with_one_of_them() {
    let mut g = dup_graph();
    g.add_project("proj:a-sibling", "A sibling library")
        .unwrap();

    let p = g.propose_heal(HealOptions::default()).unwrap();

    assert_eq!(p.projects_in_scope, 2);
    assert_ne!(
        p.target_id, "proj:a-sibling",
        "naming the alphabetically-first project is the reported defect"
    );
    assert_ne!(p.target_id, "proj:x");
    assert!(
        p.summary.contains("whole design"),
        "and the summary says what was actually swept: {}",
        p.summary
    );
}

// ---- a withdrawn decision contradicting its successor is not a defect -------
//
// req:a-detector-reads-the-properties-that-qualify-its-own-finding.
//
// dragon Boss, 2026-08-15, found by doing what a peer had just recommended:
// recording a refuted remedy as its own Decision at `status: rejected` and
// linking it to the replacement with CONTRADICTS — the honest relation, and the
// one describe_schema's hint points at. detect_defects then reported a warning.
//
// The incentive is why this is worth a fix rather than a shrug: recording a
// refutation CORRECTLY added a defect, and burying it in prose added none.

fn withdrawn_pair(status: &str) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_decision("dec:old", "The refuted remedy", "we tried this", None)
        .unwrap();
    g.add_decision("dec:new", "What replaced it", "we do this instead", None)
        .unwrap();
    g.set_decision_status("dec:old", status).unwrap();
    g.set_decision_status("dec:new", "accepted").unwrap();
    g.create_edge(
        edge::CONTRADICTS,
        node::DECISION,
        "dec:old",
        node::DECISION,
        "dec:new",
        Props::new().set("alignment", "opposing"),
    )
    .unwrap();
    g
}

fn contradictions(g: &DesignGraph) -> Vec<String> {
    g.detect_defects()
        .unwrap()
        .into_iter()
        .filter(|i| i.category == HealCategory::Contradiction)
        .map(|i| i.message)
        .collect()
}

/// THE CASE. A rejected decision contradicting its successor is the healthy
/// shape — "tried in thought, refuted, here is what we did instead".
#[test]
fn a_rejected_decision_contradicting_its_successor_is_not_a_defect() {
    let g = withdrawn_pair("rejected");
    assert!(
        contradictions(&g).is_empty(),
        "recording a refutation correctly must not cost a defect: {:?}",
        contradictions(&g)
    );
}

/// Superseded is the same shape by a different route.
#[test]
fn a_superseded_decision_contradicting_its_successor_is_not_a_defect() {
    let g = withdrawn_pair("superseded");
    assert!(contradictions(&g).is_empty(), "{:?}", contradictions(&g));
}

/// COUNTERWEIGHT, and the one that decides whether this is a fix or a hole:
/// a LIVE disagreement must still be reported. Two accepted decisions that
/// contradict each other is the case the detector exists for, and a status
/// check that silenced it would trade a false positive for a false negative —
/// strictly the worse bug.
#[test]
fn two_live_decisions_that_contradict_are_still_reported() {
    let mut g = withdrawn_pair("rejected");
    // Bring the withdrawn one back to life. Nothing else changes.
    g.set_decision_status("dec:old", "accepted").unwrap();

    assert_eq!(
        contradictions(&g).len(),
        1,
        "an unresolved conflict between live decisions is the whole point"
    );
}

/// COUNTERWEIGHT 2: `proposed` is NOT withdrawn. A parked idea that conflicts
/// with an accepted decision is a real thing to settle, and treating "not yet
/// accepted" as "no longer intended" would hide exactly the disagreements a
/// brainstorm is supposed to surface.
#[test]
fn a_proposed_decision_is_not_treated_as_withdrawn() {
    let mut g = withdrawn_pair("rejected");
    g.set_decision_status("dec:old", "proposed").unwrap();

    assert_eq!(
        contradictions(&g).len(),
        1,
        "proposed means undecided, not abandoned"
    );
}

/// The alignment rule it sits beside must be untouched: corroboration between
/// two LIVE nodes is still silent, and for the original reason.
#[test]
fn supporting_alignment_is_still_skipped_independently() {
    let mut g = withdrawn_pair("rejected");
    g.set_decision_status("dec:old", "accepted").unwrap();
    g.delete_edge(edge::CONTRADICTS, "dec:old", "dec:new")
        .unwrap();
    g.create_edge(
        edge::CONTRADICTS,
        node::DECISION,
        "dec:old",
        node::DECISION,
        "dec:new",
        Props::new().set("alignment", "supporting"),
    )
    .unwrap();

    assert!(contradictions(&g).is_empty(), "{:?}", contradictions(&g));
}

// ---- 2026-08-16 · the degree-zero rule stops being a Decision rule ---------
//
// `req:a-report-says-what-it-swept-and-whether-its-checks-ran`, the false-green
// half. dev_storyflow's fleet ran `detect_defects` over a DesignEpoch carrying
// NO EDGES AT ALL, in two separate packages, through every health call of a
// session, and got clean back every time — from the pass whose whole job is
// structural soundness. A node with no edges is the most detectable structural
// defect there is and needs no judgement to identify; the rule simply only ran
// on `Decision`.
//
// Measured on reflow2's own graph the day this landed: 75 of 2406 nodes were
// degree-zero and SEVEN of them were visible to this detector.

/// The reported ids for `orphan_node`, so each test below reads as a claim.
fn orphans(g: &DesignGraph) -> Vec<String> {
    g.detect_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == HealCategory::OrphanNode)
        .flat_map(|d| d.affected_ids)
        .collect()
}

/// The field case, exactly: an epoch nothing is recorded against.
#[test]
fn an_epoch_with_no_edges_is_not_clean() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.create_node(
        node::DESIGN_EPOCH,
        "epoch:empty",
        Props::new()
            .set("name", "marks nothing")
            .set("epoch_type", "milestone")
            .set("sequence", 1_i64),
    )
    .unwrap();

    assert_eq!(orphans(&g), ["epoch:empty"]);
}

/// A check counted among the passing that says what it checks to nobody.
/// `ver:the-export-survives-being-read-back` is reflow2's own instance, and it
/// is why this is not merely tidiness: an unattached Verification is credited
/// to no capability and still raises the passing count.
#[test]
fn a_verification_that_verifies_nothing_is_reported() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_verification("ver:loose", "checks something", Some("test"), Some("unit"))
        .unwrap();

    assert_eq!(orphans(&g), ["ver:loose"]);

    // …and one VERIFIES edge silences it. The rule is degree-zero, so it is
    // self-limiting by construction: it cannot grow into a per-convention nag,
    // whatever type it runs on.
    g.add_capability("cap:c", "C", "does c", None).unwrap();
    g.verifies("ver:loose", node::CAPABILITY, "cap:c").unwrap();
    assert!(!orphans(&g).contains(&"ver:loose".to_string()));
}

/// NOT EVERY ATTACHMENT IS AN EDGE, and the first cut of this widening assumed
/// one was. `TemporalFact.subject_id` is a required indexed property: a fact
/// names the node it is about without ever drawing a link, and 48 of reflow2's
/// own 212 facts are degree-zero for exactly that reason. Reporting them would
/// have been 48 false findings shipped inside the change whose subject is
/// instruments that overstate.
///
/// What survives is the case that is genuinely wrong — a pointer to a node the
/// design no longer has — and it is graded below a Verification, because both
/// are reported and severity says which to read first, never which one is real.
#[test]
fn a_pointer_property_is_attachment_and_only_a_dangling_one_is_reported() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_capability("cap:c", "C", "does c", None).unwrap();
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:about-something",
        Props::new()
            .set("subject_id", "cap:c")
            .set("statement", "cap:c was realized"),
    )
    .unwrap();
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:about-a-ghost",
        Props::new().set("subject_id", "cap:deleted-last-year").set(
            "statement",
            "something that is not here any more was realized",
        ),
    )
    .unwrap();
    g.add_verification("ver:loose", "checks something", Some("test"), Some("unit"))
        .unwrap();

    let by_id: std::collections::HashMap<String, HealSeverity> = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == HealCategory::OrphanNode)
        .map(|d| (d.affected_ids[0].clone(), d.severity))
        .collect();

    assert!(
        !by_id.contains_key("fact:about-something"),
        "a fact that names a live node is attached, edge or no edge: {by_id:?}"
    );
    assert_eq!(by_id.get("fact:about-a-ghost"), Some(&HealSeverity::Info));
    assert_eq!(by_id.get("ver:loose"), Some(&HealSeverity::Warning));
}

/// BL-42, held open while the rest of the rule generalized. A Capability with
/// no `ALLOCATED_TO` and a Requirement nothing `SATISFIES` were once reported
/// here AS WELL AS by DETECT — the same finding twice, in two vocabularies, and
/// on storyflow it became 20 of 31 defects. Widening the rule must not quietly
/// undo that, and degree-zero is precisely the case where it would.
#[test]
fn detect_still_owns_the_types_it_asks_about_by_name() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:loose", "R", "nothing satisfies it")
        .unwrap();
    g.add_capability("cap:loose", "C", "nothing allocates it", None)
        .unwrap();
    g.create_node(
        node::INTERFACE,
        "iface:loose",
        Props::new().set("name", "nobody provides or consumes it"),
    )
    .unwrap();

    assert!(
        orphans(&g).is_empty(),
        "asked once, by DETECT: {:?}",
        orphans(&g)
    );
}

/// The other half of not-flattening, and the one that keeps this change from
/// becoming `req:a-deliberate-state-is-not-a-defect`'s own example. A Project
/// alone means the design is EMPTY — what every design looks like on its first
/// day, and what genesis produces by construction. An advisory DesignRule can
/// bind the process rather than a node. Neither is a defect, and firing on them
/// would report the normal state of correct work as a problem.
#[test]
fn an_empty_design_and_a_process_rule_are_resting_states_not_defects() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:alone", "P").unwrap();
    g.create_node(
        node::DESIGN_RULE,
        "rule:branch-first",
        Props::new()
            .set("name", "Branch, then PR")
            .set("statement", "Nothing lands on main directly.")
            .set("enforced", false),
    )
    .unwrap();

    assert!(orphans(&g).is_empty(), "{:?}", orphans(&g));
}
