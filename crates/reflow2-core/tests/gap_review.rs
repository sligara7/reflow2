//! Reviewing a gap — accept it, with a reason, and move it out of the open list.
//!
//! From the 2026-07-18 blind trial: an agent recorded a Decision saying some
//! couplings were intentional, and the gaps did not clear. Its verdict was that
//! a gap list which can never reach zero teaches you to skim it — "the exact
//! failure mode the tool exists to prevent". These tests pin the fix, including
//! the parts that keep it honest: nothing is deleted, nothing is hidden, and an
//! acknowledgement expires when the thing it was about changes.

use reflow2_core::AskedQuestion;
use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{edge, node};

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
        .find(|r| r.gap_id == gap_id)
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
            .any(|r| r.gap_id == gap_id),
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
    let entries: Vec<_> = reviewed.iter().filter(|r| r.gap_id == gap_id).collect();
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
            .any(|r| r.gap_id == gap_id),
        "the review still applies even if one endpoint could not be linked"
    );
}

/// BL-6b retired `unexpected_coupling` as a gap. At least one trial had already
/// acknowledged one, so those reviews had to survive the change: an
/// acknowledgement whose detector no longer exists is reported as retired,
/// never silently dropped. A reviewed list that shrinks for reasons the user
/// cannot see is the dishonesty this whole split exists to avoid.
#[test]
fn an_acknowledgement_outliving_its_detector_is_reported_not_dropped() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();

    // Acknowledge a gap id no live detector will ever produce — exactly the
    // state a graph is left in when a detector is retired under it.
    let stale = "gap:deadbeefdeadbeef";
    g.acknowledge_gap(
        stale,
        &["cap:x".to_string()],
        "that coupling is the product",
    )
    .unwrap();

    let reviewed = g.reviewed_gaps().unwrap();
    let retired: Vec<_> = reviewed.iter().filter(|r| r.retired.is_some()).collect();
    assert_eq!(retired.len(), 1, "the orphaned review must still be listed");
    assert_eq!(retired[0].gap_id, stale);
    assert_eq!(retired[0].reason, "that coupling is the product");
    assert!(
        retired[0].gap.is_none(),
        "there is no live candidate to show, and none should be invented"
    );
    assert!(
        retired[0]
            .retired
            .as_deref()
            .unwrap()
            .contains("No current detector"),
        "the reason it is retired must be stated, got {:?}",
        retired[0].retired
    );

    // Withdrawing still works, so the user is not stuck with it.
    g.withdraw_gap_acknowledgement(stale).unwrap();
    assert!(
        g.reviewed_gaps().unwrap().iter().all(|r| r.gap_id != stale),
        "a withdrawn review leaves the list"
    );
}

// ---- BL-4 · questions outlive the session ----------------------------------

/// The blind trial: *"the graph has no memory that a question was asked and is
/// awaiting an answer — so on my next session I'd re-derive the same gaps and
/// re-ask the same questions, which is the stateless-agent problem reflow2 is
/// supposed to solve."* It worked around this by hand-maintaining a Markdown
/// file. This is the round trip that replaces it.
#[test]
fn an_asked_question_is_still_there_next_session() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_capability("cap:flight", "Ball flight", "sim")
        .unwrap();

    let gap_id = "gap:abc123";
    let affected = vec!["cap:flight".to_string()];
    g.record_asked_question(
        gap_id,
        &affected,
        "Which part should own ball flight?",
        AskedQuestion {
            context_setter: Some("You described ball flight but nothing owns it."),
            asked_at: Some("2026-07-18T10:00:00Z"),
            ..Default::default()
        },
    )
    .unwrap();

    let open = g.open_questions().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].gap_id, gap_id);
    assert_eq!(
        open[0].question, "Which part should own ball flight?",
        "the wording the user saw must survive, not just the fact of asking"
    );
    assert_eq!(open[0].asked_at, "2026-07-18T10:00:00Z");

    // Reachable from the design, not only from the gap.
    let asked_about = g
        .outgoing(&open[0].question_id, Some(edge::ASKS_ABOUT))
        .unwrap();
    assert_eq!(asked_about.len(), 1);
    assert_eq!(asked_about[0].to_id, "cap:flight");
}

#[test]
fn answering_closes_it_and_keeps_what_the_user_said() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.record_asked_question("gap:x", &[], "Where does it run?", AskedQuestion::default())
        .unwrap();

    assert!(g.answer_question("gap:x", "On a Raspberry Pi.").unwrap());
    assert!(
        g.open_questions().unwrap().is_empty(),
        "an answered question is no longer awaiting an answer"
    );

    let node = g.get_node(node::QUESTION, "question:x").unwrap().unwrap();
    assert_eq!(node.properties["status"].as_str(), Some("answered"));
    assert_eq!(
        node.properties["answer"].as_str(),
        Some("On a Raspberry Pi."),
        "the user's own words are kept"
    );
    assert_eq!(
        node.properties["question"].as_str(),
        Some("Where does it run?"),
        "and so is what they were asked"
    );
}

/// Re-asking must not quietly erase an answer already given — the failure mode
/// would be a session overwriting what the last one learned.
#[test]
fn re_recording_an_answered_question_does_not_lose_the_answer() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.record_asked_question("gap:x", &[], "Where does it run?", AskedQuestion::default())
        .unwrap();
    g.answer_question("gap:x", "On a Raspberry Pi.").unwrap();

    // A later session re-phrases the same gap and records it again.
    g.record_asked_question(
        "gap:x",
        &[],
        "What hardware does this run on?",
        AskedQuestion::default(),
    )
    .unwrap();

    let node = g.get_node(node::QUESTION, "question:x").unwrap().unwrap();
    assert_eq!(
        node.properties["status"].as_str(),
        Some("answered"),
        "it must not reopen"
    );
    assert_eq!(
        node.properties["answer"].as_str(),
        Some("On a Raspberry Pi."),
        "and the answer must survive the re-phrasing"
    );
    assert!(g.open_questions().unwrap().is_empty());
}

#[test]
fn a_question_can_be_withdrawn_and_answering_an_unknown_one_fails_loud() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.record_asked_question("gap:x", &[], "Still relevant?", AskedQuestion::default())
        .unwrap();

    assert!(g.withdraw_question("gap:x").unwrap());
    assert!(g.open_questions().unwrap().is_empty());
    assert!(
        g.get_node(node::QUESTION, "question:x").unwrap().is_some(),
        "withdrawn, not deleted — the past is not overwritten"
    );

    assert!(
        !g.answer_question("gap:never-asked", "…").unwrap(),
        "answering a question nobody asked reports false rather than inventing one"
    );
}
