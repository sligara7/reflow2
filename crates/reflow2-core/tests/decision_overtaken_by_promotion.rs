//! An open question the design has already answered by building it gets asked
//! about.
//!
//! The same shape as `defect_overtaken_by_change` one type over, and simpler:
//! one edge and one status, with no dates to order and no artifact walk.
//!
//! Written from the 2026-09-14 sweep of reflow2's own design: of seven ideas
//! whose answers had already shipped, five carried the `EVOLVES_INTO` edge that
//! the brainstorm discipline draws on promotion — and nothing anywhere read it.
//! The other two carried no edge at all, which no detector can reach; that half
//! is the instruction's to fix, and the finding says so rather than reporting a
//! number that sounds complete.

use reflow2_core::detect::{GapCandidate, GapSource};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open")
}

/// A brainstormed question, recorded the way the skill records one.
fn idea(g: &mut DesignGraph, id: &str) {
    g.add_decision(id, id, "OPEN — should we?", None)
        .expect("decision");
    assert_eq!(
        status_of(g, id).as_deref(),
        Some("proposed"),
        "add_decision must land at proposed, or this whole file is testing the wrong state"
    );
}

fn status_of(g: &DesignGraph, id: &str) -> Option<String> {
    g.get_node(node::DECISION, id).expect("get").and_then(|n| {
        n.properties
            .get("status")
            .and_then(|v| v.as_str().map(str::to_string))
    })
}

fn promoted_into(g: &mut DesignGraph, from: &str, to_type: &str, to: &str) {
    g.create_edge(
        edge::EVOLVES_INTO,
        node::DECISION,
        from,
        to_type,
        to,
        Props::new().set("evidence", "the idea became this"),
    )
    .expect("evolves_into");
}

fn requirement(g: &mut DesignGraph, id: &str, status: &str) {
    g.add_requirement(id, id, "it must hold").expect("req");
    g.set_requirement_status(id, status).expect("status");
}

fn the_gaps(g: &DesignGraph) -> Vec<GapCandidate> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .filter(|x| x.gap_source == GapSource::DecisionOvertakenByPromotion)
        .collect()
}

/// The case it was written from: the idea was promoted, the promotion was
/// accepted, and the question it came from still reads open.
#[test]
fn an_idea_promoted_into_an_accepted_requirement_is_asked_about() {
    let mut g = graph();
    idea(&mut g, "dec:idea-units");
    requirement(&mut g, "req:units", "accepted");
    promoted_into(&mut g, "dec:idea-units", node::REQUIREMENT, "req:units");

    let gaps = the_gaps(&g);
    assert_eq!(gaps.len(), 1, "{gaps:#?}");
    assert!(
        gaps[0].affected_ids.contains(&"dec:idea-units".to_string())
            && gaps[0].affected_ids.contains(&"req:units".to_string()),
        "the finding must anchor to BOTH the open question and what answered it: {:?}",
        gaps[0].affected_ids
    );
    // The population, not just the count — the numerator alone says nothing.
    assert!(
        gaps[0].evidence.contains("1 of 1 "),
        "evidence must carry its denominator: {}",
        gaps[0].evidence
    );
}

/// The close: the owner rules, and the question stops being offered.
#[test]
fn closing_the_decision_clears_it() {
    let mut g = graph();
    idea(&mut g, "dec:idea-units");
    requirement(&mut g, "req:units", "accepted");
    promoted_into(&mut g, "dec:idea-units", node::REQUIREMENT, "req:units");
    assert_eq!(the_gaps(&g).len(), 1);

    g.set_decision_status("dec:idea-units", "accepted")
        .expect("close");
    assert!(the_gaps(&g).is_empty(), "a ruled question must go quiet");
}

/// The promotion has to have LANDED. A requirement still at `proposed` means
/// the idea was written down as intent, not that the question was answered.
#[test]
fn a_promotion_that_has_not_landed_yet_is_not_an_answer() {
    let mut g = graph();
    idea(&mut g, "dec:idea-units");
    g.add_requirement("req:units", "req:units", "it must hold")
        .expect("req");
    promoted_into(&mut g, "dec:idea-units", node::REQUIREMENT, "req:units");
    assert!(
        the_gaps(&g).is_empty(),
        "a proposed requirement is intent, not a settlement"
    );
}

/// Deliberate exclusion, pinned so it cannot be quietly widened: what the idea
/// became was itself put down, which REOPENS the question rather than settling
/// it.
#[test]
fn a_dropped_promotion_does_not_settle_the_question() {
    let mut g = graph();
    idea(&mut g, "dec:idea-units");
    requirement(&mut g, "req:units", "dropped");
    promoted_into(&mut g, "dec:idea-units", node::REQUIREMENT, "req:units");
    assert!(
        the_gaps(&g).is_empty(),
        "an idea whose promotion was dropped is open again, not answered"
    );
}

/// ChangeEvent carries no `status` and is counted on its existence: the change
/// was recorded as having happened. This is the class the hand sweep of
/// 2026-09-14 MISSED — three ideas that shipped as one recorded change and were
/// never closed, found only when the definition was measured before shipping.
#[test]
fn an_idea_that_shipped_as_a_recorded_change_is_asked_about() {
    let mut g = graph();
    idea(&mut g, "dec:idea-bulk-check");
    g.create_node(
        node::CHANGE_EVENT,
        "chg:surface-fixes",
        Props::new()
            .set("name", "four surface fixes")
            .set("change_type", "new_feature")
            .set("subject", "system"),
    )
    .expect("event");
    promoted_into(
        &mut g,
        "dec:idea-bulk-check",
        node::CHANGE_EVENT,
        "chg:surface-fixes",
    );
    assert_eq!(the_gaps(&g).len(), 1, "a shipped change answers the idea");
}

/// The other side of the status-free rule, and the reason it is an enumeration
/// rather than "anything without a status": a measurement ABOUT an idea is
/// evidence toward an answer, not the answer.
#[test]
fn a_measurement_is_evidence_not_an_answer() {
    let mut g = graph();
    idea(&mut g, "dec:idea-units");
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:units-measured",
        Props::new()
            .set("name", "how many quantities cross a seam")
            .set("fact_type", "measurement")
            .set("statement", "41 of 118")
            .set("subject_id", "dec:idea-units"),
    )
    .expect("fact");
    promoted_into(
        &mut g,
        "dec:idea-units",
        node::TEMPORAL_FACT,
        "fact:units-measured",
    );
    assert!(
        the_gaps(&g).is_empty(),
        "a fact informs the ruling; it does not make it"
    );
}

/// A rule in force has no status either, and its existence IS its adoption.
#[test]
fn an_idea_that_became_a_rule_in_force_is_asked_about() {
    let mut g = graph();
    idea(&mut g, "dec:idea-branch-then-pr");
    g.add_design_rule(
        "rule:branch-then-pr",
        "branch, then PR",
        "nothing lands on main directly",
        None,
        Some(true),
    )
    .expect("rule");
    promoted_into(
        &mut g,
        "dec:idea-branch-then-pr",
        node::DESIGN_RULE,
        "rule:branch-then-pr",
    );
    assert_eq!(the_gaps(&g).len(), 1);
}

/// Parking is the standing way to say "known, and not now" — honoured here as
/// everywhere else.
#[test]
fn a_parked_question_is_skipped() {
    let mut g = graph();
    idea(&mut g, "dec:idea-units");
    requirement(&mut g, "req:units", "accepted");
    promoted_into(&mut g, "dec:idea-units", node::REQUIREMENT, "req:units");

    g.add_decision("dec:park", "park the idea backlog", "parked", None)
        .expect("ruling");
    g.set_decision_status("dec:park", "accepted")
        .expect("accept");
    g.governed_by(
        node::DECISION,
        "dec:idea-units",
        node::DECISION,
        "dec:park",
        Some("parks"),
        None,
    )
    .expect("park");
    assert!(the_gaps(&g).is_empty(), "a parked question must go quiet");
}

/// Keyed on what the idea became, not only on the idea: a second promotion
/// landing is a fresh claim that the question is answered, so an acknowledgement
/// of the first must not silence it.
#[test]
fn a_second_promotion_moves_the_gap_id() {
    let mut g = graph();
    idea(&mut g, "dec:idea-units");
    requirement(&mut g, "req:units", "accepted");
    promoted_into(&mut g, "dec:idea-units", node::REQUIREMENT, "req:units");
    let first = the_gaps(&g)[0].id.clone();

    requirement(&mut g, "req:units-declared", "accepted");
    promoted_into(
        &mut g,
        "dec:idea-units",
        node::REQUIREMENT,
        "req:units-declared",
    );
    let gaps = the_gaps(&g);
    assert_eq!(gaps.len(), 1, "still one question, not two");
    assert_ne!(gaps[0].id, first, "the world moved; ask again");
}

/// The blind half, pinned so nobody mistakes silence for coverage: without the
/// edge there is nothing to read, and only the instruction reaches that case.
#[test]
fn an_answer_nobody_linked_back_is_out_of_reach() {
    let mut g = graph();
    idea(&mut g, "dec:idea-usage-log");
    requirement(&mut g, "req:usage-log", "accepted");
    assert!(
        the_gaps(&g).is_empty(),
        "no edge, no finding — this is the limit the evidence line states"
    );
}
