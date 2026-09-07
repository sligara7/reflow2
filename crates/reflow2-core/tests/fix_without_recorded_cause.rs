//! A repair that recorded no cause gets asked about it.
//!
//! The third leg of the root-cause vocabulary. The skill says how to find a
//! cause, the rules say to write it down, and until this detector nothing
//! noticed when a fix landed with neither — so `req:a-fix-says-whether-it-
//! corrected-the-cause` was accepted and unenforceable, and the design could
//! not answer "how much of this is standing on a patch?".
//!
//! Aggregate, like `change_axis_unstated`, and forward-only by the graph's own
//! word for a deliberate state rather than by a date: a backlog of fixes made
//! before the rule is PARKED under one accepted ruling and counted, so the
//! finding keeps moving as new fixes land.

use reflow2_core::detect::{GapCandidate, GapSource};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open")
}

fn change(g: &mut DesignGraph, id: &str, change_type: &str) {
    g.create_node(
        node::CHANGE_EVENT,
        id,
        Props::new()
            .set("name", id)
            .set("change_type", change_type)
            .set("subject", "system"),
    )
    .expect("event");
}

fn defect_fact(g: &mut DesignGraph, id: &str) {
    g.create_node(
        node::TEMPORAL_FACT,
        id,
        Props::new()
            .set("name", id)
            .set("fact_type", "defect")
            .set("statement", "something was wrong")
            .set("subject_id", "art:thing")
            .set("valid_from", "2026-09-01"),
    )
    .expect("fact");
}

fn the_gap(g: &DesignGraph) -> Option<GapCandidate> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .find(|x| x.gap_source == GapSource::FixWithoutRecordedCause)
}

/// The case the detector exists for: fixes recorded, causes never written.
#[test]
fn fixes_joined_to_no_cause_are_asked_about_once_with_their_denominator() {
    let mut g = graph();
    change(&mut g, "chg:fix-a", "defect_fix");
    change(&mut g, "chg:fix-b", "test_failure_fix");
    change(&mut g, "chg:tidy", "refactor");

    let gap = the_gap(&g).expect("two uncaused fixes must be asked about");
    assert_eq!(gap.title, "2 of 2 recorded fix(es) are joined to no cause");
    assert_eq!(gap.affected_ids, vec!["chg:fix-a", "chg:fix-b"]);
    assert!(
        !gap.affected_ids.iter().any(|id| id == "chg:tidy"),
        "a refactor is not a repair and owes no cause"
    );
}

/// A fix that INVALIDATES its finding, or that a cause fact CAUSES, has its
/// cause on the record — by either of the two shapes the skill leaves behind.
#[test]
fn a_fix_joined_to_its_finding_or_its_cause_is_not_counted() {
    let mut g = graph();
    g.add_artifact("art:thing", "thing", None, None)
        .expect("artifact");
    defect_fact(&mut g, "fact:defect-one");
    defect_fact(&mut g, "fact:defect-two");
    change(&mut g, "chg:closes-one", "defect_fix");
    change(&mut g, "chg:caused-by-two", "test_failure_fix");
    change(&mut g, "chg:bare", "defect_fix");
    g.invalidates(
        node::CHANGE_EVENT,
        "chg:closes-one",
        node::TEMPORAL_FACT,
        "fact:defect-one",
        Some("the fix closed it"),
        Some("2026-09-02"),
    )
    .expect("invalidates");
    g.create_edge(
        edge::CAUSES,
        node::TEMPORAL_FACT,
        "fact:defect-two",
        node::CHANGE_EVENT,
        "chg:caused-by-two",
        Props::new(),
    )
    .expect("causes");

    let gap = the_gap(&g).expect("the bare fix is still uncaused");
    assert_eq!(gap.title, "1 of 3 recorded fix(es) are joined to no cause");
    assert_eq!(gap.affected_ids, vec!["chg:bare"]);
}

/// A CAUSES edge on an artifact the fix touched says nothing about THIS fix.
/// One old cause on a hub file must not launder every later repair of it.
#[test]
fn a_cause_on_a_touched_artifact_does_not_count_for_the_fix() {
    let mut g = graph();
    g.add_artifact("art:thing", "thing", None, None)
        .expect("artifact");
    defect_fact(&mut g, "fact:defect-old");
    g.create_edge(
        edge::CAUSES,
        node::ARTIFACT,
        "art:thing",
        node::TEMPORAL_FACT,
        "fact:defect-old",
        Props::new(),
    )
    .expect("causes");
    change(&mut g, "chg:later-fix", "defect_fix");
    g.create_edge(
        edge::CHANGED,
        node::CHANGE_EVENT,
        "chg:later-fix",
        node::ARTIFACT,
        "art:thing",
        Props::new(),
    )
    .expect("changed");

    let gap = the_gap(&g).expect("the later fix is still uncaused");
    assert_eq!(gap.affected_ids, vec!["chg:later-fix"]);
}

/// The backlog: fixes made before the rule are parked under one accepted
/// ruling, skipped, and counted — never silently dropped.
#[test]
fn a_parked_fix_is_skipped_and_counted() {
    let mut g = graph();
    change(&mut g, "chg:before-the-rule", "defect_fix");
    change(&mut g, "chg:after-the-rule", "defect_fix");
    g.add_decision(
        "dec:pre-rule-fixes-are-forward-only",
        "Fixes before 2026-09-06 predate the rule",
        "parked",
        Some("forward-only"),
    )
    .expect("decision");
    g.set_decision_status("dec:pre-rule-fixes-are-forward-only", "accepted")
        .expect("accepted");
    g.create_edge(
        edge::GOVERNED_BY,
        node::CHANGE_EVENT,
        "chg:before-the-rule",
        node::DECISION,
        "dec:pre-rule-fixes-are-forward-only",
        Props::new().set("ruling", "parks"),
    )
    .expect("parks");

    let gap = the_gap(&g).expect("the post-rule fix is still asked about");
    assert_eq!(gap.title, "1 of 2 recorded fix(es) are joined to no cause");
    assert_eq!(gap.affected_ids, vec!["chg:after-the-rule"]);
    assert!(
        gap.evidence.contains("1 fix(es) are PARKED"),
        "the parked backlog is counted in the evidence, not swept: {}",
        gap.evidence
    );
}

/// Aggregate: the id does not move when the membership does, so an
/// acknowledgement of the practice survives the next fix.
#[test]
fn the_finding_is_one_aggregate_whose_id_survives_a_new_fix() {
    let mut g = graph();
    change(&mut g, "chg:fix-a", "defect_fix");
    let before = the_gap(&g).expect("gap").id;
    change(&mut g, "chg:fix-b", "defect_fix");
    let after = the_gap(&g).expect("gap").id;
    assert_eq!(
        before, after,
        "an aggregate is keyed on the source, not the members"
    );
}

/// A design with no repairs recorded owes nothing here.
#[test]
fn a_design_with_no_fixes_is_silent() {
    let mut g = graph();
    change(&mut g, "chg:feature", "new_feature");
    assert!(the_gap(&g).is_none());
}
