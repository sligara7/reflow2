//! At genesis a design has no Components, so nothing CAN thread — and
//! `unthreaded_cluster` used to file that as a structural defect while
//! `concept_without_design` described the same state approvingly.
//!
//! `proj:chama` ran `detect_defects` ten minutes into its first design and got
//! **10 structural defects, 8 of them `unthreaded_cluster`** — every requirement
//! sitting with its satisfying capability as a 2–3 node island. That shape is
//! what the genesis skill INSTRUCTS ("do NOT create Components yet — leaving
//! structure unspecified is deliberate"), and `CONTAINS` is not a traceability
//! edge, so a design with no Components cannot thread by construction.
//!
//! `dec:idea-a-detector-can-read-the-phase-the-design-already-declares`, settled
//! 2026-09-02: report them under `swept.expected_at_this_phase` — visible and
//! counted, never silenced, the same shape `swept.parked` already established.

use reflow2_core::DesignGraph;

/// Genesis: intent captured, structure deliberately unspecified.
fn at_genesis() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    for (r, c) in [("a", "A"), ("b", "B"), ("c", "C")] {
        g.add_requirement(&format!("req:{r}"), r, "Must hold.")
            .unwrap();
        g.add_capability(&format!("cap:{r}"), c, "does it", Some("planned"))
            .unwrap();
        g.satisfies(&format!("cap:{r}"), &format!("req:{r}"))
            .unwrap();
    }
    g
}

#[test]
fn genesis_islands_are_reported_as_expected_at_this_phase_not_as_defects() {
    let sweep = at_genesis().detect_defects().unwrap();

    let clusters = sweep
        .defects
        .iter()
        .filter(|d| d.category.as_str() == "unthreaded_cluster")
        .count();
    assert_eq!(
        clusters, 0,
        "a design with zero Components cannot thread; filing that as a defect \
         tells a first-time user they did something wrong on their first day"
    );
    assert!(
        !sweep.swept.expected_at_this_phase.is_empty(),
        "VISIBLE AND COUNTED, never silenced — a vacuous zero is the failure \
         this bucket exists to avoid, not the goal"
    );
}

#[test]
fn once_structure_exists_the_rule_reports_normally_again() {
    // The exemption is the PHASE, not the rule. A design that has begun
    // declaring structure gets the ordinary answer back — otherwise this would
    // be a permanent silencer wearing a phase's clothes.
    let mut g = at_genesis();
    g.add_component("cmp:one", "One", "holds the ingest side", None)
        .unwrap();
    g.add_component("cmp:two", "Two", "holds the rest", None)
        .unwrap();
    g.allocate("cap:a", "cmp:one").unwrap();

    let sweep = g.detect_defects().unwrap();
    assert!(
        sweep.swept.expected_at_this_phase.is_empty(),
        "structure exists now, so the phase exemption must stop applying"
    );
}
