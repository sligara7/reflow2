//! An acknowledgement records WHOSE judgement it was.
//!
//! `acknowledge_gap` mints an `accepted` Decision — settled intent by
//! `rule:design-intent-moves-only-on-the-owners-word` — and until now drew no
//! `AUTHORED_BY` edge and had no parameter to supply one. **The one write whose
//! entire purpose is to record that somebody decided something could not say
//! who.**
//!
//! Measured 2026-08-23: acknowledging 50 gaps in one detect-and-ask pass
//! produced 49 nodes that failed this project's own `check_intent_authority`
//! gate in a single stroke. It stayed invisible because acknowledgements were
//! rare enough that the gate's dated grandfather set absorbed the historical
//! ones — **a defect that only shows at scale is one the tool's own dogfooding
//! was too small to find.**
//!
//! Three properties, and the middle one is the design decision rather than a
//! shortcut:
//!
//! 1. A named approver is recorded as `AUTHORED_BY role=approver`.
//! 2. **An absent approver is allowed and REPORTED.** Refusing would leave a
//!    design that has modelled no `Contributor` unable to accept a gap at all,
//!    which is exactly where a solo user needs it — so the absence is said out
//!    loud instead (`dec:report-dont-judge` on the write side).
//! 3. **A name that matches no Contributor is REFUSED, and nothing is
//!    written.** A typo would otherwise attach the owner's authority to
//!    somebody who does not exist, which is worse than recording none — and a
//!    half-written acknowledgement would be worse than either.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{edge, node};

fn design_with_a_gap() -> (DesignGraph, String, Vec<String>) {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:t", "A project").unwrap();
    g.add_requirement("req:t", "A need", "The system shall work.")
        .unwrap();
    let gap = g.detect_gaps().unwrap().into_iter().next().expect("a gap");
    let ids = gap.affected_ids.clone();
    (g, gap.id, ids)
}

#[test]
fn a_named_approver_is_recorded_on_the_decision() {
    let (mut g, gap, ids) = design_with_a_gap();
    g.add_contributor("who:a", "Anthony", None, None, None)
        .unwrap();
    let decision = g
        .acknowledge_gap_by(&gap, &ids, "accepted", Some("who:a"), Some("2026-08-23"))
        .unwrap();

    let edges = g.outgoing(&decision, Some(edge::AUTHORED_BY)).unwrap();
    let edge = edges.first().expect("the approver edge");
    assert_eq!(edge.to_id, "who:a");
    assert_eq!(
        edge.properties.get("role").and_then(|v| v.as_str()),
        Some("approver"),
        "author or reviewer would not satisfy the intent-authority rule; only approver does"
    );
}

#[test]
fn no_approver_is_allowed_because_a_design_may_have_modelled_nobody() {
    // Refusing here would make the tool unusable on a design that has never
    // recorded a Contributor — which is most solo designs on day one.
    let (mut g, gap, ids) = design_with_a_gap();
    let decision = g
        .acknowledge_gap_by(&gap, &ids, "fine for now", None, None)
        .unwrap();
    assert!(
        g.get_node(node::DECISION, &decision).unwrap().is_some(),
        "the acknowledgement is recorded even with nobody's name on it"
    );
    assert!(
        g.outgoing(&decision, Some(edge::AUTHORED_BY))
            .unwrap()
            .is_empty(),
        "and it must not invent an approver to fill the hole"
    );
}

#[test]
fn an_approver_who_does_not_exist_is_refused_and_nothing_is_written() {
    // THE HALF-WRITE IS THE REAL HAZARD. The Decision is minted before the
    // approver is checked, so a naive implementation leaves an accepted
    // Decision behind with no name on it — the exact state this whole change
    // exists to prevent, produced by the code meant to prevent it.
    let (mut g, gap, ids) = design_with_a_gap();
    let err = g
        .acknowledge_gap_by(&gap, &ids, "accepted", Some("who:ghost"), None)
        .expect_err("a name nobody can check must be refused");
    let text = err.to_string();
    assert!(
        text.contains("who:ghost"),
        "the refusal must name the id: {text}"
    );
    assert!(
        text.contains("add_contributor"),
        "and say what would have worked: {text}"
    );

    let ack = format!("decision:ack:{}", gap.trim_start_matches("gap:"));
    assert!(
        g.get_node(node::DECISION, &ack).unwrap().is_none(),
        "a refused acknowledgement must leave NOTHING behind — a Decision with no approver is \
         precisely the state being fixed"
    );
}

#[test]
fn the_gap_is_still_acknowledged_whichever_way_it_went() {
    // The approver is about attribution, never about whether the judgement
    // took effect. Both accepted forms must move the gap out of the open list.
    for approver in [None, Some("who:a")] {
        let (mut g, gap, ids) = design_with_a_gap();
        g.add_contributor("who:a", "Anthony", None, None, None)
            .unwrap();
        let open_before = g.detect_gaps().unwrap().len();
        g.acknowledge_gap_by(&gap, &ids, "accepted", approver, None)
            .unwrap();
        assert!(
            g.detect_gaps().unwrap().len() < open_before,
            "acknowledging with approver={approver:?} did not close the gap"
        );
        assert!(
            g.reviewed_gaps().unwrap().iter().any(|r| r.gap_id == gap),
            "and it must appear in reviewed_gaps, not vanish"
        );
    }
}

#[test]
fn the_old_signature_still_works_and_records_nobody() {
    // `acknowledge_gap` is unchanged for every existing caller, which is why
    // this is additive rather than a break — and why the reporting half
    // matters: a caller that never passes an approver must be TOLD, not
    // silently accommodated.
    let (mut g, gap, ids) = design_with_a_gap();
    let decision = g.acknowledge_gap(&gap, &ids, "fine").unwrap();
    assert!(
        g.outgoing(&decision, Some(edge::AUTHORED_BY))
            .unwrap()
            .is_empty()
    );
}
