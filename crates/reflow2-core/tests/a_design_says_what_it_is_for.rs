//! `quality_target_unstated` — the design has never said what it is FOR.
//!
//! The attribute a system is built for decides WHICH GROUPING is right, and the
//! four disagree, so allocating without the answer silently picks performance
//! (`dec:idea-the-ility-chooses-the-allocation-graph`). This is the detector
//! that stops that happening quietly.
//!
//! THE CASE THAT CARRIES THE MOST WEIGHT IS THE MIDDLE ONE. A design that
//! weighed the question and has not committed is not the same as one nobody
//! ever asked, and reporting them identically is the failure
//! `Decision.no_relation_note` exists to prevent on this very node type.

use reflow2_core::{DesignGraph, GapScope, GapSource};

fn seeded() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    g.add_capability("cap:a", "Cap A", "Does A", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g
}

fn finding(g: &DesignGraph) -> Option<reflow2_core::GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == GapSource::QualityTargetUnstated)
}

#[test]
fn a_design_that_never_said_what_it_is_for_is_asked() {
    let g = seeded();
    let gap = finding(&g).expect("a design with capabilities must be asked what it is for");
    assert_eq!(gap.scope, GapScope::Project);
    assert!(gap.title.contains("unstated"), "got: {}", gap.title);
    assert!(
        gap.affected_ids.is_empty(),
        "one question, not one per node"
    );
}

#[test]
fn a_settled_target_ends_the_question() {
    let mut g = seeded();
    g.add_decision(
        "dec:for-reliability",
        "Built for reliability",
        "A lost reading cannot be recovered.",
        Some("Field units are unattended."),
    )
    .unwrap();
    g.set_quality_target("dec:for-reliability", "reliability")
        .unwrap();
    g.set_decision_status("dec:for-reliability", "accepted")
        .unwrap();

    assert!(
        finding(&g).is_none(),
        "the design has committed — nothing left to ask"
    );
}

#[test]
fn weighing_it_without_settling_reads_differently_from_never_asking() {
    // THE COUNTERWEIGHT THIS DETECTOR EXISTS FOR. Anthony, 2026-08-25: "a user
    // may not know at genesis, so should be able to defer". A deferral is a
    // real answer and must not be reported as silence.
    let mut g = seeded();
    g.add_decision(
        "dec:maybe-maintainability",
        "Leaning maintainability",
        "Probably maintainability, not yet settled.",
        None,
    )
    .unwrap();
    g.set_quality_target("dec:maybe-maintainability", "maintainability")
        .unwrap();
    // deliberately left `proposed`

    let gap = finding(&g).expect("still open — a leaning is not a commitment");
    assert!(
        gap.title.contains("still being weighed"),
        "a deferral must not read as never having been asked, got: {}",
        gap.title
    );
    assert!(
        gap.evidence.contains("maintainability"),
        "and it must say what is being weighed, got: {}",
        gap.evidence
    );

    let never = finding(&seeded()).unwrap();
    assert!(
        gap.severity < never.severity,
        "a question somebody is holding is less urgent than one nobody raised: {} vs {}",
        gap.severity,
        never.severity
    );
    assert_ne!(gap.title, never.title);
}

#[test]
fn a_rejected_target_is_history_and_not_a_position() {
    let mut g = seeded();
    g.add_decision(
        "dec:no",
        "Rejected idea",
        "We considered performance.",
        None,
    )
    .unwrap();
    g.set_quality_target("dec:no", "performance").unwrap();
    g.set_decision_status("dec:no", "rejected").unwrap();

    let gap = finding(&g).expect("a rejected target settles nothing");
    assert!(
        !gap.title.contains("still being weighed"),
        "a rejected target is not somebody leaning, got: {}",
        gap.title
    );
}

#[test]
fn a_design_that_has_not_said_what_it_does_is_not_asked_what_it_is_for() {
    // Wrong question at the wrong phase — `design_without_intent` owns this.
    let g = DesignGraph::open_in_memory().unwrap();
    assert!(finding(&g).is_none());
}

#[test]
fn the_finding_warns_against_the_one_wrong_way_to_defer() {
    // `acknowledge_gap` is aggregate-keyed, so accepting this once silences it
    // permanently and for every capability added afterwards — the trap measured
    // on `unreviewed_ideas` the same day
    // (`dec:idea-an-aggregate-acknowledgement-never-expires`). A deferral
    // accepted that way never comes back, which is the opposite of deferring.
    let gap = finding(&seeded()).unwrap();
    assert!(
        gap.description.contains("acknowledge_gap"),
        "the finding must name the wrong move, got: {}",
        gap.description
    );
    assert!(
        gap.description.contains("set_quality_target"),
        "and the right one"
    );
}

#[test]
fn an_unknown_axis_is_refused_rather_than_stored() {
    let mut g = seeded();
    g.add_decision("dec:x", "X", "...", None).unwrap();
    let err = g.set_quality_target("dec:x", "cheapness").unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("cheapness") && msg.contains("reliability"),
        "the refusal must name what was passed AND what would have worked, got: {msg}"
    );
}

#[test]
fn declaring_what_a_decision_is_for_does_not_rewrite_what_it_says() {
    let mut g = seeded();
    g.add_decision("dec:x", "X", "The decision text.", Some("The reasoning."))
        .unwrap();
    g.set_decision_status("dec:x", "accepted").unwrap();
    g.set_quality_target("dec:x", "security").unwrap();

    let n = g.get_node("Decision", "dec:x").unwrap().unwrap();
    let get = |k: &str| {
        n.properties
            .get(k)
            .and_then(reflow2_core::foundation::core::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(get("decision"), "The decision text.");
    assert_eq!(get("rationale"), "The reasoning.");
    assert_eq!(get("status"), "accepted", "status must survive");
    assert_eq!(get("quality_target"), "security");
}
