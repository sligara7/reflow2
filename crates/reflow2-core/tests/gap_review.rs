//! Reviewing a gap — accept it, with a reason, and move it out of the open list.
//!
//! From the 2026-07-18 blind trial: an agent recorded a Decision saying some
//! couplings were intentional, and the gaps did not clear. Its verdict was that
//! a gap list which can never reach zero teaches you to skim it — "the exact
//! failure mode the tool exists to prevent". These tests pin the fix, including
//! the parts that keep it honest: nothing is deleted, nothing is hidden, and an
//! acknowledgement expires when the thing it was about changes.

use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;

/// A design with a Requirement nothing satisfies — one reliable open gap.
fn graph_with_a_gap() -> (DesignGraph, String, Vec<String>) {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.add_requirement("req:audit", "Audit trail", "every change is logged")
        .expect("req");
    g.add_capability("cap:other", "Something else", "unrelated")
        .expect("cap");

    let gap = g
        .detect_gaps()
        .expect("detect")
        .into_iter()
        .find(|c| c.gap_source == GapSource::UnsatisfiedRequirement)
        .expect("an unsatisfied requirement gap");
    let (id, affected) = (gap.id.clone(), gap.affected_ids.clone());
    (g, id, affected)
}

#[test]
fn an_accepted_gap_leaves_the_open_list_and_enters_the_reviewed_bucket() {
    let (mut g, gap_id, affected) = graph_with_a_gap();

    g.acknowledge_gap(
        &gap_id,
        &affected,
        "Logging is handled by the platform, not this design.",
    )
    .expect("acknowledge");

    assert!(
        !g.detect_gaps()
            .expect("detect")
            .iter()
            .any(|c| c.id == gap_id),
        "an accepted gap must not stay in the open list"
    );
    let reviewed = g.reviewed_gaps().expect("reviewed");
    let entry = reviewed
        .iter()
        .find(|r| r.gap.id == gap_id)
        .expect("it must appear in the reviewed bucket, not vanish");
    assert_eq!(
        entry.reason,
        "Logging is handled by the platform, not this design."
    );
}

#[test]
fn the_reason_is_a_real_decision_in_the_graph() {
    let (mut g, gap_id, affected) = graph_with_a_gap();
    let decision_id = g
        .acknowledge_gap(&gap_id, &affected, "Deliberate: the audit lives upstream.")
        .expect("acknowledge");

    let decision = g
        .get_node("Decision", &decision_id)
        .expect("get")
        .expect("the review is a Decision node");
    assert_eq!(
        decision.properties["rationale"].as_str(),
        Some("Deliberate: the audit lives upstream.")
    );
    assert_eq!(decision.properties["status"].as_str(), Some("accepted"));

    // and it is reachable from the node the gap was about
    let governed = g
        .outgoing("req:audit", Some("GOVERNED_BY"))
        .expect("outgoing");
    assert!(
        governed.iter().any(|e| e.to_id == decision_id),
        "the review must be reachable from the design, not only from the gap"
    );
}

#[test]
fn withdrawing_a_review_reopens_the_gap() {
    let (mut g, gap_id, affected) = graph_with_a_gap();
    g.acknowledge_gap(&gap_id, &affected, "On reflection, fine.")
        .expect("acknowledge");
    assert!(!g.detect_gaps().unwrap().iter().any(|c| c.id == gap_id));

    assert!(
        g.withdraw_gap_acknowledgement(&gap_id).expect("withdraw"),
        "withdrawing an existing review reports that it did something"
    );

    assert!(
        g.detect_gaps().unwrap().iter().any(|c| c.id == gap_id),
        "a withdrawn review must put the gap back in the open list"
    );
    assert!(
        !g.reviewed_gaps()
            .unwrap()
            .iter()
            .any(|r| r.gap.id == gap_id),
        "and take it out of the reviewed bucket"
    );
}

#[test]
fn withdrawing_supersedes_rather_than_deletes() {
    let (mut g, gap_id, affected) = graph_with_a_gap();
    let decision_id = g
        .acknowledge_gap(&gap_id, &affected, "Accepted for now.")
        .expect("acknowledge");
    g.withdraw_gap_acknowledgement(&gap_id).expect("withdraw");

    let decision = g
        .get_node("Decision", &decision_id)
        .expect("get")
        .expect("the Decision must survive — the past is not overwritten");
    assert_eq!(decision.properties["status"].as_str(), Some("superseded"));
    assert_eq!(
        decision.properties["rationale"].as_str(),
        Some("Accepted for now."),
        "the original reasoning is still readable after withdrawal"
    );
}

#[test]
fn withdrawing_something_never_reviewed_is_reported_not_faked() {
    let (mut g, _, _) = graph_with_a_gap();
    assert!(
        !g.withdraw_gap_acknowledgement("gap:doesnotexist")
            .expect("withdraw"),
        "no silent success for a review that was never made"
    );
}

#[test]
fn a_review_expires_when_the_situation_it_described_changes() {
    // The gap id hashes the source *and* the affected nodes. Accepting
    // "requirement X is unsatisfied" must not silence "requirement Y is
    // unsatisfied" — nor the same requirement in a different situation.
    let (mut g, gap_id, affected) = graph_with_a_gap();
    g.acknowledge_gap(&gap_id, &affected, "Handled elsewhere.")
        .expect("acknowledge");

    g.add_requirement("req:retention", "Retention", "keep logs for a year")
        .expect("second req");

    let open = g.detect_gaps().expect("detect");
    let unsatisfied: Vec<&str> = open
        .iter()
        .filter(|c| c.gap_source == GapSource::UnsatisfiedRequirement)
        .flat_map(|c| c.affected_ids.iter().map(String::as_str))
        .collect();
    assert_eq!(
        unsatisfied,
        vec!["req:retention"],
        "a new gap of the same kind must still surface; only the reviewed one is quiet"
    );
}

#[test]
fn re_acknowledging_updates_the_reason_rather_than_duplicating() {
    let (mut g, gap_id, affected) = graph_with_a_gap();
    g.acknowledge_gap(&gap_id, &affected, "First take.")
        .expect("first");
    g.acknowledge_gap(&gap_id, &affected, "Better reason after discussion.")
        .expect("second");

    let reviewed = g.reviewed_gaps().expect("reviewed");
    let entries: Vec<_> = reviewed.iter().filter(|r| r.gap.id == gap_id).collect();
    assert_eq!(entries.len(), 1, "one review per gap");
    assert_eq!(entries[0].reason, "Better reason after discussion.");
}

#[test]
fn acknowledging_does_not_invent_edges_to_nodes_that_are_gone() {
    let (mut g, gap_id, _) = graph_with_a_gap();
    // An affected id that no longer resolves must be skipped, not authored.
    g.acknowledge_gap(&gap_id, &["req:vanished".to_string()], "Fine.")
        .expect("acknowledge tolerates a missing endpoint");
    assert!(
        g.reviewed_gaps()
            .unwrap()
            .iter()
            .any(|r| r.gap.id == gap_id),
        "the review still applies even if one endpoint could not be linked"
    );
}
