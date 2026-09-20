//! The unrealized-capability finding is already silent while nothing is built.
//!
//! flo2, 2026-09-19, reported that this source fires once per capability on a
//! young design — 17 times on their own graph — "each phrased as a thing to
//! answer, with no hint that 'nothing built yet' may simply be where the
//! project IS. A person reads seventeen of those as seventeen defects." The
//! promoted fix was one sentence in the body saying the phase is normal.
//!
//! ⭐ MEASURED 2026-09-20 WHILE BUILDING THAT SENTENCE, AND IT CHANGED THE
//! ANSWER: the detector returns early at `pop.artifacts == 0`, so a design with
//! nothing built raises NONE of these. A conditional sentence on that case is
//! unreachable code. flo2's seventeen were therefore raised on a design that
//! HAD started building, where "what gets built for this?" is the live question
//! the finding is for.
//!
//! AND THE OTHER OBVIOUS FIX IS RULED OUT IN THE SOURCE ITSELF, immediately
//! above the guard: "No threshold and no proportion — BL-5's lesson was that a
//! loud detector needs a different QUESTION, not a tuned number."
//!
//! So this file pins the behaviour rather than changing it. Both halves matter:
//! the silence while nothing is built is what keeps a first session from
//! meeting one finding per capability, and it is exactly what a sentence about
//! the phase would have been added to compensate for.

use reflow2_core::DesignGraph;

fn design_with_capabilities(n: usize) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_project("proj:g", "Greenhouse").expect("project");
    for i in 0..n {
        g.add_capability(
            &format!("cap:c{i}"),
            &format!("Capability {i}"),
            "Something the system does.",
            None,
        )
        .expect("capability");
        g.add_component(&format!("cmp:c{i}"), &format!("Part {i}"), "A part.", None)
            .expect("component");
        g.allocate(&format!("cap:c{i}"), &format!("cmp:c{i}"))
            .expect("allocate");
    }
    g
}

fn unrealized_bodies(g: &DesignGraph) -> Vec<String> {
    g.detect_gaps()
        .expect("gaps")
        .into_iter()
        .filter(|gap| gap.gap_source.as_str() == "unrealized_capability")
        .map(|gap| gap.description)
        .collect()
}

#[test]
fn while_nothing_is_built_the_finding_is_silent_rather_than_once_per_capability() {
    let g = design_with_capabilities(3);
    assert!(
        unrealized_bodies(&g).is_empty(),
        "a design that has not started building must not be asked what builds each capability;          that is the case a phase sentence would have been written to soften, and the silence          is the better answer"
    );
    // And the phase IS stated, once, by the aggregate that owns that question.
    let said_the_phase = g
        .detect_gaps()
        .expect("gaps")
        .into_iter()
        .any(|gap| gap.gap_source.as_str() == "design_without_build");
    assert!(
        said_the_phase,
        "the phase belongs to one aggregate finding, not to one finding per capability"
    );
}

#[test]
fn once_building_has_started_the_question_is_asked_per_capability() {
    let mut g = design_with_capabilities(3);
    g.add_artifact("art:pump", "pump.rs", Some("code"), Some("src/pump.rs"))
        .expect("artifact");
    g.realizes("art:pump", "Capability", "cap:c0", None, None)
        .expect("realizes");

    let bodies = unrealized_bodies(&g);
    assert!(
        !bodies.is_empty(),
        "once something is built, an unrealized capability is a live question"
    );
    for body in &bodies {
        assert!(
            body.contains("what actually gets built for it?"),
            "and it is asked as a question: {body}"
        );
    }
}
