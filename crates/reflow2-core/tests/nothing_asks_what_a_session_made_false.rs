//! Capture is additive, and nothing asked what a session made FALSE.
//!
//! # The measurement this exists for
//!
//! `INVALIDATES` shipped on 2026-08-23 *with* its reader, deliberately, so the
//! marker would not become a comment nobody consults. Measured 2026-08-24, one
//! day later, with the tool served the whole time: **zero edges had ever been
//! drawn.** Not one, by anybody.
//!
//! The edge was reachable and unused, and the reason was structural rather than
//! careless. A design's vocabulary reaches real work only with three legs — a
//! TYPED TOOL, an INSTRUCTION that names it, and a COMPUTATION THAT NOTICES ITS
//! ABSENCE. #321 built the first and wired the READ side into `where-am-i`. No
//! skill, and nothing in the surface, ever told anyone to WRITE one, and nothing
//! anywhere noticed that none existed.
//!
//! # Why the question is session-sized
//!
//! Design-wide this graph carries 270 open observations, and a detector firing
//! on all of them is wallpaper — the failure a consumer abandoned reflow2 over.
//! Measured across all 639 ChangeEvents on reflow2's own graph: **71% touch no
//! open observation at all, and the median when one is touched is 1.** That is
//! the whole reason this asks about the events a session just wrote instead of
//! about the design.
//!
//! ⚠️ AND THE FIRST VERSION OF THAT NUMBER WAS WRONG IN THE FLATTERING
//! DIRECTION. It was measured at 78% before the computation read `subject_id`,
//! i.e. against the 56% of observations reachable by edge alone. Widening the
//! reach to 97% necessarily lengthened the shortlists, and the honest figures
//! are 71% silent, median 1, mean 4.3, p90 13, max 40. **A measurement taken
//! against a narrower implementation than the one that shipped is not evidence
//! about the one that shipped.**
//!
//! # What it must not become
//!
//! `dec:verification-freshness-not-a-gap` (accepted 2026-07-26, read before
//! this was written) rules that a stale-looking CHECK is a STANDING PROPERTY:
//! it would fire on every legitimate refactor, so it belongs on the
//! confirmation ledger and never in a nagging list. A TemporalFact is the other
//! thing — a DATED OBSERVATION, asserted once, true at a moment, re-derived by
//! nothing. Every assertion below turns on keeping those apart: observations
//! only, never checks, and never a gap source.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

/// A component, an observation about it, and a change that moved it.
///
/// The shape of the real failure: somebody measures a thing, somebody else
/// fixes the thing, and the measurement goes on describing a world that has
/// moved.
fn a_measurement_and_the_work_that_moved_it(g: &mut DesignGraph) {
    g.add_component("cmp:service", "service", "the served surface", None)
        .unwrap();
    fact(g, "fact:service-is-slow", Some("2026-08-21"), None);
    g.create_edge(
        edge::HAS_TEMPORAL_FACT,
        node::COMPONENT,
        "cmp:service",
        node::TEMPORAL_FACT,
        "fact:service-is-slow",
        Props::new(),
    )
    .unwrap();
    change(g, "chg:made-it-fast", &["cmp:service"]);
}

fn fact(g: &mut DesignGraph, id: &str, valid_from: Option<&str>, valid_to: Option<&str>) {
    fact_about(g, id, "cmp:service", valid_from, valid_to)
}

/// `subject_id` is REQUIRED on a TemporalFact — which is exactly why the
/// computation reads it as well as the subject edges.
fn fact_about(
    g: &mut DesignGraph,
    id: &str,
    subject_id: &str,
    valid_from: Option<&str>,
    valid_to: Option<&str>,
) {
    g.create_node(
        node::TEMPORAL_FACT,
        id,
        Props::new()
            .set("name", id)
            .set("subject_id", subject_id)
            .set("basis", "measured")
            .set_opt("valid_from", valid_from)
            .set_opt("valid_to", valid_to),
    )
    .unwrap();
}

fn change(g: &mut DesignGraph, id: &str, touched: &[&str]) {
    g.create_node(
        node::CHANGE_EVENT,
        id,
        Props::new()
            .set("name", id)
            .set("change_type", "defect_fix"),
    )
    .unwrap();
    for t in touched {
        g.create_edge(
            edge::CHANGED,
            node::CHANGE_EVENT,
            id,
            node::COMPONENT,
            t,
            Props::new(),
        )
        .unwrap();
    }
}

#[test]
fn work_that_moves_a_thing_surfaces_the_observation_about_it() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_measurement_and_the_work_that_moved_it(&mut g);

    let out = g.unclaimed_findings_near(&["chg:made-it-fast"]).unwrap();

    assert_eq!(out.candidates.len(), 1, "the observation must be surfaced");
    assert_eq!(out.candidates[0].finding_id, "fact:service-is-slow");
    assert_eq!(out.subjects_examined, 1);
    // The reason it is on the list travels with it. Being told WHAT to look at
    // without being told WHY is how a shortlist becomes something to dismiss.
    assert_eq!(out.candidates[0].reached_via, vec!["cmp:service"]);
    assert_eq!(out.candidates[0].valid_from.as_deref(), Some("2026-08-21"));
}

#[test]
fn an_observation_somebody_already_closed_is_not_raised_again() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_measurement_and_the_work_that_moved_it(&mut g);
    // valid_to set: somebody dated its end.
    fact(
        &mut g,
        "fact:service-is-slow",
        Some("2026-08-21"),
        Some("2026-08-24"),
    );

    let out = g.unclaimed_findings_near(&["chg:made-it-fast"]).unwrap();
    assert!(out.candidates.is_empty(), "a closed observation is settled");
}

#[test]
fn an_observation_somebody_already_claimed_is_not_raised_again() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_measurement_and_the_work_that_moved_it(&mut g);
    g.invalidates(
        node::CHANGE_EVENT,
        "chg:made-it-fast",
        node::TEMPORAL_FACT,
        "fact:service-is-slow",
        Some("made it fast"),
        Some("2026-08-24"),
    )
    .unwrap();

    let out = g.unclaimed_findings_near(&["chg:made-it-fast"]).unwrap();
    assert!(
        out.candidates.is_empty(),
        "the question has been answered; asking again is the nag this avoids"
    );
}

#[test]
fn a_check_is_never_raised_as_a_candidate() {
    // THE CASE THAT KEEPS THIS FROM REVERSING dec:verification-freshness-not-a-gap.
    // A stale-looking CHECK is a standing property that fires on every
    // legitimate refactor. Only DATED OBSERVATIONS belong here.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:service", "service", "the served surface", None)
        .unwrap();
    g.add_verification("ver:it-works", "it works", Some("test"), None, None)
        .unwrap();
    g.verifies("ver:it-works", node::COMPONENT, "cmp:service")
        .unwrap();
    g.set_verification_status("ver:it-works", "passing", Some("2026-08-01"), None)
        .unwrap();
    change(&mut g, "chg:made-it-fast", &["cmp:service"]);

    let out = g.unclaimed_findings_near(&["chg:made-it-fast"]).unwrap();
    assert!(
        out.candidates.is_empty(),
        "a Verification is a standing property, not a dated observation"
    );
}

#[test]
fn work_that_touched_nothing_anchored_says_so_rather_than_saying_nothing() {
    // A SHORT LIST AND A BLIND ONE LOOK IDENTICAL, and this is the difference.
    // `subjects_examined: 0` says the work reached no anchored ground; an empty
    // list with subjects examined says it reached ground and found nothing.
    let mut g = DesignGraph::open_in_memory().unwrap();
    change(&mut g, "chg:touched-nothing", &[]);

    let out = g.unclaimed_findings_near(&["chg:touched-nothing"]).unwrap();
    assert!(out.candidates.is_empty());
    assert_eq!(
        out.subjects_examined, 0,
        "the caller must be able to tell 'nothing to check' from 'checked, all clear'"
    );
}

#[test]
fn an_event_id_that_names_nothing_is_reported_not_skipped() {
    // A typo would otherwise return an empty shortlist — which reads exactly
    // like "your work retired nothing", the most reassuring answer available
    // and the one least likely to be questioned.
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_measurement_and_the_work_that_moved_it(&mut g);

    let out = g
        .unclaimed_findings_near(&["chg:made-it-fast", "chg:typo"])
        .unwrap();
    assert_eq!(out.unknown_events, vec!["chg:typo"]);
    assert_eq!(
        out.candidates.len(),
        1,
        "the events that DO exist are still answered"
    );
}

#[test]
fn one_observation_reached_by_two_subjects_is_listed_once() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_measurement_and_the_work_that_moved_it(&mut g);
    g.add_component("cmp:tools", "tools", "the tool layer", None)
        .unwrap();
    g.create_edge(
        edge::HAS_TEMPORAL_FACT,
        node::COMPONENT,
        "cmp:tools",
        node::TEMPORAL_FACT,
        "fact:service-is-slow",
        Props::new(),
    )
    .unwrap();
    change(&mut g, "chg:touched-both", &["cmp:service", "cmp:tools"]);

    let out = g.unclaimed_findings_near(&["chg:touched-both"]).unwrap();
    assert_eq!(
        out.candidates.len(),
        1,
        "one row per observation, not per path"
    );
    assert_eq!(
        out.candidates[0].reached_via,
        vec!["cmp:service", "cmp:tools"],
        "but every path that reached it is named"
    );
}

#[test]
fn it_reaches_an_observation_anchored_the_other_way_round() {
    // HAS_TEMPORAL_FACT points subject -> fact; ABOUT_ENTITY points fact ->
    // subject. Both are in live use, and reading only one would silently halve
    // the coverage while looking like it worked.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:service", "service", "the served surface", None)
        .unwrap();
    fact(&mut g, "fact:about-way", Some("2026-08-21"), None);
    g.create_edge(
        edge::ABOUT_ENTITY,
        node::TEMPORAL_FACT,
        "fact:about-way",
        node::COMPONENT,
        "cmp:service",
        Props::new(),
    )
    .unwrap();
    change(&mut g, "chg:made-it-fast", &["cmp:service"]);

    let out = g.unclaimed_findings_near(&["chg:made-it-fast"]).unwrap();
    assert_eq!(out.candidates.len(), 1);
    assert_eq!(out.candidates[0].finding_id, "fact:about-way");
}

#[test]
fn it_is_a_candidate_and_never_a_verdict() {
    // Nothing here infers that the observation is FALSE — only that the thing
    // it describes has moved and nobody has said either way. The judgement is
    // the author's, and `invalidates` is how they record it. Proof: the fact's
    // own properties are untouched by asking.
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_measurement_and_the_work_that_moved_it(&mut g);

    g.unclaimed_findings_near(&["chg:made-it-fast"]).unwrap();

    let n = g
        .get_node(node::TEMPORAL_FACT, "fact:service-is-slow")
        .unwrap()
        .unwrap();
    assert!(
        !n.properties.contains_key("valid_to"),
        "asking must never close anything"
    );
}

#[test]
fn asking_about_no_events_at_all_is_quiet_and_honest() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_measurement_and_the_work_that_moved_it(&mut g);

    let out = g.unclaimed_findings_near(&[]).unwrap();
    assert!(out.candidates.is_empty());
    assert_eq!(out.subjects_examined, 0);
    assert!(out.unknown_events.is_empty());
}
