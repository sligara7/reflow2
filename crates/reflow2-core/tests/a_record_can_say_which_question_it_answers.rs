//! A design record can name the Question it answered.
//!
//! THE SCHEMA ALREADY PROMISED THIS AND NOTHING DELIVERED IT. `Question.answer`
//! reads "What the user said, in their own words. The design nodes it produced
//! are linked separately" — and they were not linked at all, separately or
//! otherwise. Measured 2026-09-02: `describe_schema{from: Decision, to:
//! Question}` returned ZERO exact and ZERO half-exact matches.
//!
//! Three independent reports wanted this one edge: proj:chama's loop-debt
//! confusion (2026-09-02), api-boss's permanently-unanswerable Question
//! (2026-08-15), and the wording-amendment question left open the same day.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{AskedQuestion, DesignGraph};

/// Build a design holding one answered question whose gap is still open.
fn answered_question() -> (DesignGraph, String) {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:r", "R", "Must do x.").unwrap();
    g.add_capability("cap:x", "X", "does x", Some("realized"))
        .unwrap();
    let gap = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .find(|gap| !gap.affected_ids.is_empty())
        .expect("an anchored gap");
    g.record_asked_question(
        &gap.id,
        &gap.affected_ids,
        "Where should this live?",
        AskedQuestion::default(),
    )
    .unwrap();
    g.answer_question(&gap.id, "Not sure yet — park it.")
        .unwrap();
    let qid = format!("question:{}", gap.id.strip_prefix("gap:").unwrap());
    (g, qid)
}

#[test]
fn a_decision_can_name_the_question_it_answered() {
    let (mut g, qid) = answered_question();
    g.add_decision(
        "dec:deferred",
        "Deferred on purpose",
        "Recorded as an open decision rather than guessed.",
        None,
    )
    .unwrap();

    // THE EDGE THE SCHEMA ALREADY DESCRIBES. Direction reads as a sentence:
    // the RECORD answers the QUESTION.
    g.create_edge(
        edge::ANSWERS,
        node::DECISION,
        "dec:deferred",
        node::QUESTION,
        &qid,
        Props::new().set("note", "Captures the deferral the user asked for."),
    )
    .unwrap();

    let out = g.outgoing("dec:deferred", Some(edge::ANSWERS)).unwrap();
    assert_eq!(out.len(), 1, "the record names the question it answered");
    assert_eq!(out[0].to_id, qid);

    // And the Question can be asked who answered it — the direction that
    // makes the loop's claim computable rather than inferred.
    let inbound = g.incoming(&qid, Some(edge::ANSWERS)).unwrap();
    assert_eq!(inbound.len(), 1);
    assert_eq!(inbound[0].from_id, "dec:deferred");
}

#[test]
fn the_answer_side_is_open_because_an_answer_is_not_always_a_decision() {
    // An answer can land as a Requirement, a Capability, a status move. The
    // `from` side is deliberately not enumerated; the `to` side IS, so
    // describe_schema ranks this above the both-sides-wildcard pile rather
    // than burying it (fact:governed-by-is-the-governance-edge-and-describe-
    // schema-can-never-rank-it).
    let (mut g, qid) = answered_question();
    g.create_edge(
        edge::ANSWERS,
        node::REQUIREMENT,
        "req:r",
        node::QUESTION,
        &qid,
        Props::new(),
    )
    .unwrap();
    assert_eq!(g.outgoing("req:r", Some(edge::ANSWERS)).unwrap().len(), 1);
}

#[test]
fn an_answer_edge_pointing_at_something_that_is_not_a_question_is_refused() {
    // The `to` side is enumerated, and that is the half that must hold: an
    // ANSWERS edge to a Requirement would validate through a wildcard and
    // mean nothing.
    let (mut g, _qid) = answered_question();
    g.add_decision("dec:d", "D", "x", None).unwrap();
    let bad = g.create_edge(
        edge::ANSWERS,
        node::DECISION,
        "dec:d",
        node::REQUIREMENT,
        "req:r",
        Props::new(),
    );
    assert!(
        bad.is_err(),
        "ANSWERS must not connect a Decision to a Requirement"
    );
}

#[test]
fn the_loop_counts_answers_that_name_their_record_and_infers_nothing_from_the_rest() {
    let (mut g, qid) = answered_question();

    // BEFORE: the answer exists in the graph but nothing links it. The loop
    // must NOT call this unwritten — that was the defect fixed the same day.
    let before = g.loop_status().unwrap();
    assert_eq!(before.answered_with_open_gap, 1);
    assert_eq!(
        before.answered_naming_their_record, 0,
        "nothing names its record yet"
    );
    let line = before
        .next
        .iter()
        .find(|l| l.contains("answered question(s)"))
        .expect("the class gets a line")
        .clone();
    assert!(
        !line.contains("never reached the design") && !line.to_lowercase().contains("not written"),
        "absence of the edge is not evidence of missing work: {line:?}"
    );

    // AFTER: the record names the question. Now the loop can say so from a
    // lookup rather than an inference.
    g.add_decision("dec:deferred", "Deferred", "Parked on purpose.", None)
        .unwrap();
    g.create_edge(
        edge::ANSWERS,
        node::DECISION,
        "dec:deferred",
        node::QUESTION,
        &qid,
        Props::new(),
    )
    .unwrap();

    let after = g.loop_status().unwrap();
    assert_eq!(after.answered_with_open_gap, 1, "the gap is still open");
    assert_eq!(
        after.answered_naming_their_record, 1,
        "and now the answer names what it became"
    );
}

#[test]
fn a_design_written_before_the_edge_existed_is_not_reported_as_in_debt() {
    // THE MIXED-VINTAGE CASE AGENTS.md requires asking about: an additive
    // schema change leaves old graphs without the new edge. Such a design
    // reports 0 named records and MUST NOT thereby be told it owes anything
    // it does not.
    let (g, _qid) = answered_question();
    let s = g.loop_status().unwrap();
    assert_eq!(s.answered_naming_their_record, 0);
    let line = s
        .next
        .iter()
        .find(|l| l.contains("answered question(s)"))
        .unwrap();
    assert!(
        line.contains("NOT evidence"),
        "the line must decline the inference out loud: {line:?}"
    );
}
