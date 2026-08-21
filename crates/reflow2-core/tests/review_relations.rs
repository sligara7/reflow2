//! Relating a node to what is already there — and the record left when nothing
//! is honestly related.
//!
//! `dec:idea-what-notices-an-idea-that-connects-to-nothing`, accepted by
//! Anthony 2026-08-21. The whole design turns on ONE distinction, so most of
//! these cases exist to pin it:
//!
//! ⭐ **"NOBODY LOOKED" AND "SOMEBODY LOOKED AND THERE WAS NOTHING" MUST NOT BE
//! THE SAME STATE.** Both carry no relation. If the detector cannot tell them
//! apart it reports the person who did the work, which is worse than not
//! detecting at all — the careful answer becomes indistinguishable from the
//! missing one and then both get complained about. `no_relation_note` is the
//! whole of that distinction, which is why the write refuses when given
//! neither an edge nor a note.
//!
//! 🛑 **AND THE DETECTOR MUST NOT MANUFACTURE PRESSURE TO FABRICATE.** A false
//! neighbour is worse than a missing one: a missing edge leaves an idea hard to
//! find, an invented one puts a wrong neighbour in front of every later reader
//! and anything searching by neighbourhood repeats it forever. That is why
//! there is no "suggest relations" call anywhere here, and why the finding is
//! aggregate and low-severity rather than a per-idea demand.

use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::relate::{RelationLink, ReviewState};

fn link(relation: &str, other: &str) -> RelationLink {
    RelationLink {
        relation: relation.into(),
        other_type: node::DECISION.into(),
        other_id: other.into(),
        evidence: "the earlier one already noticed the same discarded material".into(),
        incoming: false,
    }
}

/// Two proposed Decisions and nothing joining them — the starting state this
/// whole mechanism is about.
fn two_ideas() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    for id in ["dec:a", "dec:b"] {
        g.create_node(
            node::DECISION,
            id,
            Props::new()
                .set("name", id)
                .set("decision", "an idea, thought out loud")
                .set("status", "proposed"),
        )
        .expect("decision");
    }
    g
}

fn unreviewed_gap(g: &DesignGraph) -> Option<usize> {
    g.detect_gaps()
        .expect("gaps")
        .into_iter()
        .find(|x| x.gap_source == GapSource::UnreviewedIdeas)
        .map(|x| x.affected_ids.len())
}

#[test]
fn a_relation_is_drawn_with_its_reason() {
    let mut g = two_ideas();
    let out = g
        .review_relations(
            node::DECISION,
            "dec:a",
            &[link(edge::ANTICIPATES, "dec:b")],
            None,
        )
        .expect("review");

    assert_eq!(out.state, ReviewState::Linked);
    assert_eq!(out.drawn, vec!["ANTICIPATES -> dec:b"]);
    assert_eq!(out.note, None);

    let e = g.outgoing("dec:a", Some(edge::ANTICIPATES)).expect("edges");
    assert_eq!(e.len(), 1);
    assert!(
        e[0].properties
            .get("evidence")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("discarded material")),
        "the reason travels ON the edge — a relation the next reader cannot check is an assertion"
    );
}

#[test]
fn drawing_nothing_and_saying_nothing_is_refused() {
    // THE CENTRAL REFUSAL. Accepting an empty review would write a record that
    // says nothing happened, which is exactly the state the record exists to
    // distinguish from. There is no half-answer here to be permissive about.
    let mut g = two_ideas();
    let err = g
        .review_relations(node::DECISION, "dec:a", &[], None)
        .expect_err("must refuse");
    let msg = format!("{err}");
    assert!(msg.contains("nobody has opened"), "{msg}");
}

#[test]
fn a_note_is_a_real_answer_and_clears_the_finding() {
    // ⭐ The case the whole design is for: somebody looked, found nothing, and
    // said so. That idea is NOT a finding, and reporting it as one would punish
    // the only behaviour that makes the unlinked ones meaningful.
    let mut g = two_ideas();
    assert_eq!(unreviewed_gap(&g), Some(2));

    let out = g
        .review_relations(
            node::DECISION,
            "dec:a",
            &[],
            Some("searched; nearest were dec:b and dec:c; no real relation to either"),
        )
        .expect("review");
    assert_eq!(out.state, ReviewState::ReviewedUnlinked);
    assert!(out.note.is_some());

    // dec:a is settled; dec:b is still nobody's business.
    assert_eq!(unreviewed_gap(&g), Some(1));
}

#[test]
fn a_blank_note_is_not_an_answer() {
    // Whitespace would clear the finding while recording no judgement at all —
    // the cheapest possible way to make the detector lie, so it is closed.
    let mut g = two_ideas();
    let err = g
        .review_relations(node::DECISION, "dec:a", &[], Some("   \n  "))
        .expect_err("must refuse");
    assert!(format!("{err}").contains("nobody has opened"));
}

#[test]
fn a_relation_with_no_reason_is_refused() {
    let mut g = two_ideas();
    let mut l = link(edge::CONTRADICTS, "dec:b");
    l.evidence = "  ".into();
    let err = g
        .review_relations(node::DECISION, "dec:a", &[l], None)
        .expect_err("must refuse");
    assert!(format!("{err}").contains("neither check nor overturn"));
}

#[test]
fn a_structural_edge_cannot_be_drawn_by_a_review() {
    // CONTAINS and SATISFIES are load-bearing design, and this call's premise
    // is that the author is still thinking. The error names the vocabulary that
    // IS available rather than only refusing.
    let mut g = two_ideas();
    let mut l = link(edge::CONTAINS, "dec:b");
    l.relation = edge::CONTAINS.into();
    let err = g
        .review_relations(node::DECISION, "dec:a", &[l], None)
        .expect_err("must refuse");
    let msg = format!("{err}");
    assert!(msg.contains("not a relation a review can draw"), "{msg}");
    assert!(
        msg.contains("ANTICIPATES"),
        "the error must name what IS allowed: {msg}"
    );
}

#[test]
fn one_bad_link_writes_none_of_them() {
    // A review is one judgement. Writing two edges and then refusing the third
    // would leave the design holding half of it, with the caller unable to tell
    // which half landed.
    let mut g = two_ideas();
    g.create_node(
        node::DECISION,
        "dec:c",
        Props::new()
            .set("name", "c")
            .set("decision", "x")
            .set("status", "proposed"),
    )
    .expect("c");

    let mut bad = link(edge::CONTRADICTS, "dec:c");
    bad.evidence = String::new();
    let err = g.review_relations(
        node::DECISION,
        "dec:a",
        &[link(edge::ANTICIPATES, "dec:b"), bad],
        None,
    );
    assert!(err.is_err());
    assert!(
        g.outgoing("dec:a", Some(edge::ANTICIPATES))
            .expect("e")
            .is_empty(),
        "the good link must not have been written before the bad one was judged"
    );
}

#[test]
fn direction_can_be_reversed_because_it_is_part_of_the_claim() {
    // "the older idea EVOLVES_INTO this one" and "this one EVOLVES_INTO the
    // older" are different claims, and nothing downstream can tell that one of
    // them is false. So the caller can say which way it runs.
    let mut g = two_ideas();
    let mut l = link(edge::EVOLVES_INTO, "dec:b");
    l.incoming = true;
    let out = g
        .review_relations(node::DECISION, "dec:a", &[l], None)
        .expect("review");
    assert_eq!(out.drawn, vec!["EVOLVES_INTO <- dec:b"]);
    assert!(
        g.outgoing("dec:a", Some(edge::EVOLVES_INTO))
            .expect("e")
            .is_empty()
    );
    assert_eq!(
        g.outgoing("dec:b", Some(edge::EVOLVES_INTO))
            .expect("e")
            .len(),
        1
    );
}

#[test]
fn re_reviewing_reports_what_was_already_there() {
    // Re-running must not read as having found something new, and must not
    // double the edge.
    let mut g = two_ideas();
    g.review_relations(
        node::DECISION,
        "dec:a",
        &[link(edge::ANTICIPATES, "dec:b")],
        None,
    )
    .expect("first");
    let out = g
        .review_relations(
            node::DECISION,
            "dec:a",
            &[link(edge::ANTICIPATES, "dec:b")],
            None,
        )
        .expect("second");
    assert!(out.drawn.is_empty());
    assert_eq!(out.already_present, vec!["ANTICIPATES -> dec:b"]);
    assert_eq!(
        g.outgoing("dec:a", Some(edge::ANTICIPATES))
            .expect("e")
            .len(),
        1
    );
}

#[test]
fn being_related_to_by_something_else_counts_as_connected() {
    // 🛑 Direction-blindness in the DETECTOR, deliberately opposite to the
    // direction-awareness in the write. An idea that something else contradicts
    // is just as reachable as one that contradicts something; asking only about
    // outgoing edges would report the second idea of every pair as an orphan
    // and demand a duplicate edge back to fix it.
    let mut g = two_ideas();
    g.review_relations(
        node::DECISION,
        "dec:a",
        &[link(edge::ANTICIPATES, "dec:b")],
        None,
    )
    .expect("review");
    assert_eq!(
        unreviewed_gap(&g),
        None,
        "both ends of the relation are connected; neither is a finding"
    );
}

#[test]
fn the_finding_is_one_question_not_one_per_idea() {
    // Per-node this would have fired 115 times on reflow2's own graph on the
    // day it shipped — every one correct, and the whole category filtered by
    // the end of the week.
    let g = two_ideas();
    let found: Vec<_> = g
        .detect_gaps()
        .expect("gaps")
        .into_iter()
        .filter(|x| x.gap_source == GapSource::UnreviewedIdeas)
        .collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].affected_ids.len(), 2, "one gap, naming both ideas");
    assert!(
        found[0].title.contains("2 of 2"),
        "a numerator with no denominator says almost nothing: {}",
        found[0].title
    );
}

#[test]
fn a_settled_decision_is_not_an_open_idea() {
    // The finding is about thinking that has not been connected, not about
    // every Decision ever written. Anything the owner has accepted, rejected or
    // superseded is out of scope by construction.
    let mut g = two_ideas();
    g.set_decision_status("dec:a", "accepted").expect("accept");
    assert_eq!(unreviewed_gap(&g), Some(1));
}

#[test]
fn a_decision_point_with_alternatives_is_asked_about_by_the_other_detector() {
    // A fork being weighed is not a thought nobody connected, and
    // `undecided_decision_point` already asks about it. Reporting it twice, in
    // two vocabularies, is how a gap list stops being read.
    let mut g = two_ideas();
    for alt in ["art:one", "art:two"] {
        g.register_alternative("dec:a", alt, alt, "designs/x.json")
            .expect("register");
    }
    assert_eq!(
        unreviewed_gap(&g),
        Some(1),
        "only dec:b remains — dec:a is a fork, not an orphan"
    );
}

#[test]
fn nothing_to_run_on_produces_no_finding_at_all() {
    // A detector that reports zero because it had NOTHING TO CHECK reads
    // exactly like one that ran clean. With no proposed decisions there is no
    // population, so the honest output is silence rather than "0 of 0".
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    assert_eq!(unreviewed_gap(&g), None);
}
