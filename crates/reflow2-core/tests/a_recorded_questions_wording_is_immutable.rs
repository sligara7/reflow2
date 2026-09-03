//! A Question's recorded wording cannot be edited, and that is deliberate.
//!
//! `dec:idea-a-recorded-questions-wording-can-be-corrected`, settled 2026-09-02:
//! a Question records WHAT WAS ASKED, and the `answer` stored beside it was
//! given to those exact words. Editing them afterwards would make the record a
//! claim about what somebody wishes they had asked.
//!
//! THIS TEST EXISTS BECAUSE OF AGENTS.md RULE 8. The tool description and the
//! schema now both assert "no tool edits this field" — a claim about the code,
//! written the same day the rule requiring such claims to be asserted was.

use reflow2_core::nodes::node;
use reflow2_core::{AskedQuestion, DesignGraph};

fn answered() -> (DesignGraph, String, String) {
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
    let qid = format!("question:{}", gap.id.strip_prefix("gap:").unwrap());
    (g, gap.id, qid)
}

#[test]
fn answering_a_question_never_rewrites_what_was_asked() {
    let (mut g, gap_id, qid) = answered();
    let before = g
        .get_node(node::QUESTION, &qid)
        .unwrap()
        .expect("the question")
        .properties
        .get("question")
        .cloned();

    g.answer_question(&gap_id, "It belongs in the ingest service.")
        .unwrap();

    let after = g
        .get_node(node::QUESTION, &qid)
        .unwrap()
        .expect("still there");
    assert_eq!(
        after.properties.get("question").cloned(),
        before,
        "the answer is stored beside the words it was given to; those words do not move"
    );
    assert!(
        after.properties.contains_key("answer"),
        "the answer IS recorded — immutability is about the question, not the reply"
    );
}

#[test]
fn withdrawing_a_question_preserves_its_wording_rather_than_erasing_it() {
    // `withdraw_question` is what an agent reaches for when the phrasing is
    // wrong. Its description now says outright that it is NOT an edit tool, so
    // the record it leaves must still carry what was asked.
    let (mut g, gap_id, qid) = answered();
    let before = g
        .get_node(node::QUESTION, &qid)
        .unwrap()
        .unwrap()
        .properties
        .get("question")
        .cloned();

    g.withdraw_question(&gap_id).unwrap();

    let after = g
        .get_node(node::QUESTION, &qid)
        .unwrap()
        .expect("kept in the graph, not deleted");
    assert_eq!(
        after.properties.get("question").cloned(),
        before,
        "withdrawing records a different fact; it does not rewrite the wording"
    );
}
