//! Accepted structural defects leave the open list — the gap side's bargain,
//! finally kept on the defect side.
//!
//! Friction found by using reflow2 on itself (`req:reviewed-defects`,
//! 2026-07-25): six architectural defects, every one carrying a Decision
//! explaining why it stands, reported identically on every run for weeks. The
//! reasoning `acknowledge_gap` was built on applies word for word — "a list that
//! can never reach zero gets skimmed" — so a genuine seventh defect would have
//! arrived into a list nobody read carefully.
//!
//! The property worth testing hardest is not that acceptance hides things. It is
//! that acceptance **expires**: a defect id hashes its category with its affected
//! set, so when the shape changes the review no longer matches, and the design
//! gets asked again rather than staying quietly accepted.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::node;

/// Two capabilities that depend on each other — a circular dependency, which is
/// deterministic and needs no community structure to detect. (The
/// single-point-of-failure shape would be the more familiar honestly-accepted
/// defect, but it needs real subsystems to arise, and this test is about the
/// review layer rather than about any one detector.)
fn cycle() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    for (id, name) in [("cap:a", "A"), ("cap:b", "B")] {
        g.add_capability(id, name, "a capability", None).unwrap();
    }
    depends(&mut g, "cap:a", "cap:b");
    depends(&mut g, "cap:b", "cap:a");
    g
}

fn depends(g: &mut DesignGraph, from: &str, to: &str) {
    g.create_edge(
        "DEPENDS_ON",
        node::CAPABILITY,
        from,
        node::CAPABILITY,
        to,
        reflow2_core::nodes::Props::new().build(),
    )
    .unwrap();
}

fn first_defect(g: &DesignGraph) -> reflow2_core::HealIssue {
    g.open_defects()
        .unwrap()
        .into_iter()
        .next()
        .expect("the fixture has a defect")
}

#[test]
fn an_accepted_defect_leaves_the_open_list_and_keeps_its_reason() {
    let mut g = cycle();
    let defect = first_defect(&g);
    let before = g.open_defects().unwrap().len();

    let decision_id = g
        .acknowledge_defect(
            &defect.id,
            &defect.affected_ids,
            "Inherent to a single-writer store: a second copy of the hub would be duplication, \
             not resilience.",
        )
        .unwrap();

    let after = g.open_defects().unwrap();
    assert_eq!(
        after.len(),
        before - 1,
        "an accepted defect must leave the open list: {after:?}"
    );

    let reviewed = g.reviewed_defects().unwrap();
    let entry = reviewed
        .iter()
        .find(|r| r.defect_id == defect.id)
        .expect("it must appear in the reviewed list, not vanish");
    assert!(entry.reason.contains("single-writer"), "{}", entry.reason);
    assert_eq!(entry.decision_id, decision_id);
    assert!(
        entry.defect.is_some(),
        "a live shape still shows its defect, so the reader can judge it again"
    );
    assert!(entry.retired.is_none());
}

#[test]
fn the_reason_is_a_real_decision_that_outlives_the_session() {
    // Not a flag on the issue — the issue is computed and cannot hold anything.
    // The reason has to live in the graph or it lives nowhere.
    let mut g = cycle();
    let defect = first_defect(&g);
    g.acknowledge_defect(&defect.id, &defect.affected_ids, "Accepted architecture.")
        .unwrap();

    let decision = g
        .get_node(node::DECISION, &format!("decision:ack:{}", defect.id))
        .unwrap()
        .expect("the review is a Decision node");
    assert_eq!(
        decision.properties["status"].as_str(),
        Some("accepted"),
        "explicit, because a new Decision is `proposed` since 2026-07-25 — and this one really \
         is settled"
    );
    assert!(
        decision.properties["rationale"]
            .as_str()
            .unwrap()
            .contains("Accepted architecture"),
        "the WHY is the point of the record"
    );
}

#[test]
fn acceptance_expires_when_the_shape_changes() {
    // THE test. A defect id hashes its category and its affected set, so growing
    // the affected set mints a new id that nothing has accepted — the design gets
    // asked again instead of staying quietly accepted on a judgement made about a
    // different architecture.
    let mut g = cycle();
    let defect = first_defect(&g);
    g.acknowledge_defect(&defect.id, &defect.affected_ids, "Fine for now.")
        .unwrap();
    assert!(
        g.open_defects().unwrap().iter().all(|d| d.id != defect.id),
        "accepted, so quiet"
    );

    // Re-route the cycle through a third capability: a → b → c → a. The old
    // two-node cycle is gone and a three-node one has taken its place, so the
    // affected set differs and the id differs with it.
    //
    // (Merely EXTENDING the cycle would not have expired anything, and that is
    // correct rather than a gap in the mechanism — the accepted a↔b cycle would
    // still be exactly there. My first version of this test assumed otherwise and
    // was wrong; the distinction is worth keeping in front of whoever reads this.)
    g.add_capability("cap:c", "C", "a third", None).unwrap();
    g.delete_edge("DEPENDS_ON", "cap:b", "cap:a").unwrap();
    depends(&mut g, "cap:b", "cap:c");
    depends(&mut g, "cap:c", "cap:a");

    let open = g.open_defects().unwrap();
    assert!(
        open.iter().any(|d| d.id != defect.id),
        "the new shape is a new defect nobody has accepted: {open:?}"
    );
    let reviewed = g.reviewed_defects().unwrap();
    let stale = reviewed
        .iter()
        .find(|r| r.defect_id == defect.id)
        .expect("the judgement is kept, not deleted");
    assert!(
        stale.retired.is_some(),
        "and it is reported as outlived rather than silently still applying: {stale:?}"
    );
    assert!(
        stale.defect.is_none(),
        "there is no live candidate to show for it any more"
    );
}

#[test]
fn withdrawing_returns_it_to_the_open_list_and_keeps_the_record() {
    let mut g = cycle();
    let defect = first_defect(&g);
    g.acknowledge_defect(&defect.id, &defect.affected_ids, "Accepted.")
        .unwrap();
    assert!(g.withdraw_defect_acknowledgement(&defect.id).unwrap());

    assert!(
        g.open_defects().unwrap().iter().any(|d| d.id == defect.id),
        "withdrawn, so open again"
    );
    let decision = g
        .get_node(node::DECISION, &format!("decision:ack:{}", defect.id))
        .unwrap()
        .expect("superseded, never deleted");
    assert_eq!(decision.properties["status"].as_str(), Some("superseded"));
    assert!(
        decision.properties["rationale"].as_str().is_some(),
        "the original reasoning survives the withdrawal (req:intent-preserved)"
    );
}

#[test]
fn withdrawing_something_never_accepted_is_a_no_op() {
    let mut g = cycle();
    assert!(
        !g.withdraw_defect_acknowledgement("heal:0000000000000000")
            .unwrap()
    );
}

#[test]
fn a_defect_review_is_not_mistaken_for_a_gap_review() {
    // Both live under `decision:ack:`, so without a namespace guard an accepted
    // DEFECT would surface in reviewed_gaps as a retired GAP, and each list would
    // report the other's judgements.
    let mut g = cycle();
    let defect = first_defect(&g);
    g.acknowledge_defect(&defect.id, &defect.affected_ids, "Accepted.")
        .unwrap();

    let gap_reviews = g.reviewed_gaps().unwrap();
    assert!(
        gap_reviews.iter().all(|r| !r.gap_id.contains("heal:")),
        "the gap list must not claim a defect review: {gap_reviews:?}"
    );
}
