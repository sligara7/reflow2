//! A scoped answer is materially different from the unscoped one, and says so
//! when it is not.
//!
//! `req:a-scoped-answer-actually-narrows`, `dec:the-default-scope-depth-should-be-two`.
//!
//! # The measurement this exists because of
//!
//! Driving the built binary over all 56 Components of reflow2's own design at
//! the old default depth of 3: **every one returned 50-60 of the design's 83
//! gaps**, median 55, over regions of 595-903 nodes. The spread across all 56
//! was 50..60 — indistinguishable. `in_scope: 55, out_of_scope: 28` read as
//! "your part has 55 gaps" and said the same thing to every team about every
//! part. At depth 2 the same sweep returned 2-27 (median 4).
//!
//! # What is checked here, and what deliberately is not
//!
//! A unit test cannot reproduce a 56-component sweep, and a fixture tuned until
//! it shows a 50..60 spread would be a test written to match the answer. So
//! these probe the PROPERTIES that make the sweep's result impossible to hide:
//! that the default is the measured one, that the share is computed against the
//! only honest denominator, that a failure to narrow arrives in words, and that
//! the two notes cannot be confused for one another.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DEFAULT_SCOPE_DEPTH, DesignGraph, EpochType, SCOPE_IS_BARELY_NARROWER_AT};

/// A project with two segments, carrying THREE anchored gaps between them: two
/// the project reaches and one it does not.
///
/// Built by adding holes of a kind the detectors already find rather than by
/// tuning numbers until a threshold trips — the shape was probed against the
/// real detectors first, and the counts below are what they actually return.
fn program() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    for cmp in ["cmp:ground", "cmp:space"] {
        g.add_component(cmp, cmp, "a segment", None).unwrap();
        g.create_edge(
            edge::CONTAINS,
            node::PROJECT,
            "proj:p",
            node::COMPONENT,
            cmp,
            Props::new(),
        )
        .unwrap();
    }
    // A whole thread on ground: capability, its requirement, no hole.
    g.add_capability("cap:downlink", "cap:downlink", "receives a pass", None)
        .unwrap();
    g.allocate("cap:downlink", "cmp:ground").unwrap();
    g.add_requirement("req:pass", "req:pass", "every pass is received")
        .unwrap();
    g.satisfies("cap:downlink", "req:pass").unwrap();

    // Two capabilities serving no stated requirement, one on each segment —
    // `unmotivated_capability`, anchored, and INSIDE the project's reach.
    for (cap, cmp) in [("cap:transmit", "cmp:space"), ("cap:relay", "cmp:ground")] {
        g.add_capability(cap, cap, "does a thing nobody asked for", None)
            .unwrap();
        g.allocate(cap, cmp).unwrap();
    }

    // One capability allocated NOWHERE — anchored, and OUTSIDE the project's
    // reach, because nothing connects it. This is what keeps the project's
    // share below 1.0 and makes "out_of_scope" mean something.
    g.add_capability("cap:orphan", "cap:orphan", "allocated to nothing", None)
        .unwrap();
    g.add_requirement("req:orphan", "req:orphan", "must hold")
        .unwrap();
    g.satisfies("cap:orphan", "req:orphan").unwrap();
    g
}

#[test]
fn the_default_depth_is_the_one_the_measurement_chose() {
    // The constant IS the fix. A test that only exercised behaviour would pass
    // just as happily against the old 3, which is the value that produced the
    // defect, so the number itself is asserted.
    assert_eq!(
        DEFAULT_SCOPE_DEPTH, 2,
        "depth 3 returned 50-60 of 83 gaps for every one of 56 components — \
         indistinguishable answers. Depth 1 was rejected because it stops short of \
         the requirements a component's capabilities satisfy."
    );
}

#[test]
fn the_share_is_computed_against_findings_that_could_have_been_in_scope() {
    // The denominator is the whole point. `total` includes unanchored findings
    // — statements about the design as a whole — which can never fall inside
    // ANY region, so dividing by it would flatter every scoped answer by a
    // constant and make a region look narrower than it is.
    let g = program();
    let scoped = g
        .detect_gaps_in_scope("cmp:ground", DEFAULT_SCOPE_DEPTH)
        .unwrap();

    let anchored = scoped.in_scope + scoped.out_of_scope;
    let expected = if anchored == 0 {
        0.0
    } else {
        scoped.in_scope as f64 / anchored as f64
    };
    assert!(
        (scoped.share_of_anchored - expected).abs() < 1e-9,
        "share must be in_scope / (in_scope + out_of_scope), not in_scope / total: \
         got {} with in_scope {} out_of_scope {} unanchored {} total {}",
        scoped.share_of_anchored,
        scoped.in_scope,
        scoped.out_of_scope,
        scoped.unanchored,
        scoped.total,
    );
    assert!(
        scoped.unanchored == 0 || expected != scoped.in_scope as f64 / scoped.total as f64,
        "with unanchored findings present the two denominators must actually differ, \
         or this probe proves nothing"
    );
}

#[test]
fn a_scope_that_swallows_the_design_says_so_in_words() {
    // Scoping to the Project reaches everything, so it is the honest way to
    // produce a "narrowing" that narrows nothing without tuning a fixture until
    // it misbehaves.
    let g = program();
    let whole = g
        .detect_gaps_in_scope("proj:p", DEFAULT_SCOPE_DEPTH)
        .unwrap();

    assert!(
        whole.share_of_anchored > SCOPE_IS_BARELY_NARROWER_AT,
        "scoping to the Project should hold most anchored findings; got {}",
        whole.share_of_anchored
    );
    let note = whole
        .narrowing_note
        .expect("an answer holding most of the design must SAY it is not a narrowing");
    assert!(
        note.contains("BARELY NARROWER"),
        "the note has to name the problem, not hint at it: {note}"
    );
    assert!(
        note.contains("50%"),
        "and must state the threshold it fired at, so a reader can disagree with it: {note}"
    );
    assert!(
        note.contains("depth"),
        "and must name the remedy — a smaller depth: {note}"
    );
}

#[test]
fn the_two_notes_are_separate_fields_because_they_say_opposite_things() {
    // `note` means the region was too SMALL to mean anything; `narrowing_note`
    // means it was too LARGE. One field carrying either would have to be read
    // before it could be understood.
    let mut g = program();
    g.add_epoch("epoch:island", "an island", EpochType::Milestone, 1)
        .unwrap();

    let vacuous = g
        .detect_gaps_in_scope("epoch:island", DEFAULT_SCOPE_DEPTH)
        .unwrap();
    assert_eq!(vacuous.region_size, 1, "an epoch is an island in the walk");
    assert!(
        vacuous.note.is_some() && vacuous.narrowing_note.is_none(),
        "a vacuous region carries the vacuity note and NOT the narrowing note"
    );

    let whole = g
        .detect_gaps_in_scope("proj:p", DEFAULT_SCOPE_DEPTH)
        .unwrap();
    assert!(
        whole.narrowing_note.is_some() && whole.note.is_none(),
        "a region that swallowed the design carries the narrowing note and NOT the \
         vacuity note"
    );
}

#[test]
fn structural_defects_get_the_same_self_check_as_gaps() {
    // Both detectors return `Scoped`, and a self-check that only one of them
    // performed would be the sort of half-applied rule that made the vacuity
    // note need repairing twice.
    let g = program();
    let scoped = g
        .detect_defects_in_scope("proj:p", DEFAULT_SCOPE_DEPTH)
        .unwrap();

    let anchored = scoped.in_scope + scoped.out_of_scope;
    if anchored > 0 {
        let expected = scoped.in_scope as f64 / anchored as f64;
        assert!(
            (scoped.share_of_anchored - expected).abs() < 1e-9,
            "defects must compute the same share as gaps"
        );
        assert_eq!(
            scoped.narrowing_note.is_some(),
            expected > SCOPE_IS_BARELY_NARROWER_AT,
            "and fire the note on the same rule"
        );
    }
}
