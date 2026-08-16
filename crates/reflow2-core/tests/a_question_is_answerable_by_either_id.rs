//! `answer_question` reaches a Question by EITHER identifier.
//!
//! WHY THIS EXISTS (StoryFlow fleet, api-boss `79e8e8d5`, 2026-08-15): the
//! lookup was pure string derivation — `question:{gap_id without its prefix}` —
//! and a single `get_node` on the result. So a Question stored under any other
//! id was PERMANENTLY unanswerable, and `open_questions` published its
//! `question_id` while its sibling refused to take one. The loop then went on
//! printing "follow it up rather than asking again" about a question it
//! structurally could not close, and a later seat re-asks the user something
//! they already ruled on — the exact failure that instruction exists to prevent.
//!
//! THE COUNTERWEIGHTS ARE THE POINT. Widening a lookup is how a precise tool
//! becomes a guessing one: the derived id must still WIN, an absent question
//! must still be refused rather than quietly invented, and a gap id must not
//! start matching some unrelated node that happens to share its name.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, node};

/// A Question exactly as `gap_to_prompt` writes it: id derived from the gap.
fn derived(g: &mut DesignGraph, gap: &str) -> String {
    let id = format!("question:{}", gap.strip_prefix("gap:").unwrap_or(gap));
    g.create_node(
        node::QUESTION,
        &id,
        Props::new()
            .set("question", "Does the thing hold?")
            .set("gap_id", gap),
    )
    .unwrap();
    id
}

/// A Question written by hand — the shape a fleet produces when it records a
/// standing question that never came from a detector.
fn hand_authored(g: &mut DesignGraph, id: &str, gap: &str) {
    g.create_node(
        node::QUESTION,
        id,
        Props::new()
            .set("question", "Should fleet ops be its own subproject?")
            .set("gap_id", gap),
    )
    .unwrap();
}

fn status_of(g: &DesignGraph, id: &str) -> String {
    g.get_node(node::QUESTION, id)
        .unwrap()
        .unwrap()
        .properties
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// COUNTERWEIGHT, and first on purpose: the path every existing caller takes is
/// untouched. A gap id still finds the question derived from it.
#[test]
fn a_gap_id_still_answers_the_question_derived_from_it() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let qid = derived(&mut g, "gap:abc123");

    assert!(g.answer_question("gap:abc123", "yes, do it").unwrap());
    assert_eq!(status_of(&g, &qid), "answered");
    assert_eq!(
        g.get_node(node::QUESTION, &qid)
            .unwrap()
            .unwrap()
            .properties
            .get("answer")
            .and_then(|v| v.as_str()),
        Some("yes, do it"),
        "the answer text is kept verbatim"
    );
}

/// THE CASE THE FIX EXISTS FOR: a Question whose id nothing can derive.
#[test]
fn a_hand_authored_question_is_answerable_by_its_own_id() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    hand_authored(
        &mut g,
        "q:fleet_ops_as_reflow2_subproject",
        "gap:fleet_governance_has_no_executable_model",
    );

    // The gap id cannot reach it — the derivation wants a different name.
    assert!(
        !g.answer_question("gap:fleet_governance_has_no_executable_model", "x")
            .unwrap(),
        "precondition: the derived id genuinely does not exist"
    );

    // Its own id does, and that id is what open_questions publishes.
    assert!(
        g.answer_question("q:fleet_ops_as_reflow2_subproject", "yes — settled")
            .unwrap()
    );
    assert_eq!(
        status_of(&g, "q:fleet_ops_as_reflow2_subproject"),
        "answered"
    );
}

/// COUNTERWEIGHT: when BOTH could match, the derived one wins. Otherwise a gap
/// id that happens to name a Question node would start answering the wrong
/// question, silently — a worse bug than the one being fixed.
#[test]
fn the_derived_question_wins_when_both_exist() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let qid = derived(&mut g, "gap:collision");
    hand_authored(&mut g, "gap:collision", "gap:collision");

    assert!(g.answer_question("gap:collision", "settled").unwrap());
    assert_eq!(
        status_of(&g, &qid),
        "answered",
        "the question the gap derives must be the one that moves"
    );
    assert_eq!(
        status_of(&g, "gap:collision"),
        "asked",
        "and the node merely SHARING the name must be untouched"
    );
}

/// COUNTERWEIGHT: an id matching nothing is still refused. `false` is what the
/// served tool turns into a refusal, and answering a question nobody asked must
/// never read as success.
#[test]
fn an_unknown_id_is_still_refused() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    derived(&mut g, "gap:real");

    assert!(!g.answer_question("gap:nope", "hello").unwrap());
    assert!(!g.answer_question("q:also-nope", "hello").unwrap());
}

/// Withdrawal shares the resolver, so it gains the same reach. Stated as a case
/// because the report flagged it as unverified by inspection.
#[test]
fn withdrawing_reaches_a_hand_authored_question_too() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    hand_authored(&mut g, "q:overtaken", "gap:whatever");

    assert!(g.withdraw_question("q:overtaken").unwrap());
    assert_eq!(status_of(&g, "q:overtaken"), "withdrawn");
}

/// The error path can say what EXISTS, not just that it missed — the round trip
/// the reporter had no way to shortcut.
#[test]
fn the_known_ids_are_reportable_for_a_useful_refusal() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    assert!(
        g.known_question_ids().unwrap().is_empty(),
        "a design with no questions says so"
    );

    let qid = derived(&mut g, "gap:abc");
    hand_authored(&mut g, "q:by-hand", "gap:xyz");

    let known = g.known_question_ids().unwrap();
    assert_eq!(known.len(), 2);
    assert!(known.contains(&qid));
    assert!(known.contains(&"q:by-hand".to_string()));
}
