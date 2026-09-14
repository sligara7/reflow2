//! A prohibition living in prose — "must never", "is not allowed to" — gets
//! noticed, once, as a practice.
//!
//! Written from the 2026-09-14 measurement of reflow2's own design: the
//! capture-intent routing table sends such a sentence to a Constraint, NOT a
//! Requirement, and the design held ≈61 of them in prose (9 of 10 sampled
//! genuine) against 8 Constraint nodes. The row had been added because eleven
//! prohibitions were left as Requirements — instruction present, and nothing
//! noticing it being skipped.
//!
//! Aggregate on purpose: per node this is the BL-73 wallpaper.

use reflow2_core::detect::{GapCandidate, GapSource};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open")
}

fn req(g: &mut DesignGraph, id: &str, statement: &str) {
    g.add_requirement(id, id, statement).expect("req");
}

fn the_gaps(g: &DesignGraph) -> Vec<GapCandidate> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .filter(|x| x.gap_source == GapSource::ProhibitionInProse)
        .collect()
}

/// The case it was written from: a prohibition inside a requirement's prose,
/// with no Constraint anywhere.
#[test]
fn a_must_never_inside_a_requirement_is_noticed() {
    let mut g = graph();
    req(
        &mut g,
        "req:no-hang",
        "A flag carries the answer, and a non-tty must never hang.",
    );
    req(&mut g, "req:plain", "The export names its version.");

    let gaps = the_gaps(&g);
    assert_eq!(gaps.len(), 1, "{gaps:#?}");
    let gap = &gaps[0];
    assert_eq!(gap.affected_ids, vec!["req:no-hang".to_string()]);
    assert!(
        gap.evidence.contains("1 live node")
            && gap.evidence.contains("2 such node(s) were scanned"),
        "numerator AND denominator: {}",
        gap.evidence
    );
    assert!(
        gap.evidence.contains("req:no-hang (\"must never\")"),
        "the matched phrase rides into the evidence: {}",
        gap.evidence
    );
}

/// ONE finding for the practice, not one per sentence.
#[test]
fn many_prohibitions_are_one_finding() {
    let mut g = graph();
    req(&mut g, "req:a", "It must never hang.");
    req(
        &mut g,
        "req:b",
        "The report is not allowed to become telemetry.",
    );
    g.add_capability(
        "cap:c",
        "cap:c",
        "Something-unnamed must never read as nothing-here.",
        None,
    )
    .expect("cap");
    let gaps = the_gaps(&g);
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].affected_ids.len(), 3);
    assert!(
        gaps[0].title.starts_with("3 prohibition(s)"),
        "{}",
        gaps[0].title
    );
}

/// Filing it as a Constraint that binds the node is the answer, and the
/// finding goes quiet for that node.
#[test]
fn a_constraint_that_binds_the_node_clears_it() {
    let mut g = graph();
    req(&mut g, "req:no-hang", "A non-tty must never hang.");
    assert_eq!(the_gaps(&g).len(), 1);

    g.add_constraint(
        "con:no-hang",
        "no hang",
        "A non-tty must never hang.",
        None,
        None,
        None,
        None,
        None,
    )
    .expect("constraint");
    g.create_edge(
        edge::CONSTRAINS,
        node::CONSTRAINT,
        "con:no-hang",
        node::REQUIREMENT,
        "req:no-hang",
        Props::new(),
    )
    .expect("constrains");
    assert!(
        the_gaps(&g).is_empty(),
        "bound by a Constraint, so it has a home"
    );
}

/// Deliberate exclusion, pinned: "shall not" is the requirement idiom, not
/// house law. An SE reader files "the system shall not exceed…" as a
/// Requirement, and a false neighbour is worse than a missing one.
#[test]
fn shall_not_is_the_requirement_idiom_and_is_not_matched() {
    let mut g = graph();
    req(
        &mut g,
        "req:limit",
        "The system shall not exceed 200 ms at the boundary.",
    );
    assert!(
        the_gaps(&g).is_empty(),
        "a shall-clause is a requirement, correctly filed"
    );
}

/// Deliberate exclusion, pinned: a brainstormed idea's "must never" is a
/// musing. The brainstorm skill's own rule is that detectors do not run over
/// ideas, and nudging one to firm up teaches the user that thinking out loud
/// has a cost.
#[test]
fn a_proposed_decisions_musing_is_not_scanned() {
    let mut g = graph();
    g.add_decision(
        "dec:idea",
        "OPEN — what if?",
        "Maybe the export must never be written by hand.",
        None,
    )
    .expect("idea");
    assert!(
        the_gaps(&g).is_empty(),
        "proposed = an idea, and ideas are not nudged"
    );

    g.set_decision_status("dec:idea", "accepted")
        .expect("accept");
    assert_eq!(
        the_gaps(&g).len(),
        1,
        "once ruled, its prohibition is intent and is scanned"
    );
}

/// Deliberate exclusion, pinned: a "we never…" in a DesignRule is HOME by the
/// routing table's other row, and a Constraint is the home itself.
#[test]
fn a_rule_and_a_constraint_are_the_homes_and_are_not_flagged() {
    let mut g = graph();
    g.add_design_rule(
        "rule:main",
        "nothing lands on main",
        "Work is not allowed to land on main directly.",
        None,
        Some(true),
    )
    .expect("rule");
    g.add_constraint(
        "con:one",
        "one writer",
        "A second writer is never allowed on the store.",
        None,
        None,
        None,
        None,
        None,
    )
    .expect("constraint");
    assert!(the_gaps(&g).is_empty(), "{:#?}", the_gaps(&g));
}

/// Deliberate exclusion, pinned: a record of what happened or was checked is
/// not intent. "The one direction this must never fail in" on a test is a
/// check, and it was in the measured sample.
#[test]
fn records_and_checks_are_not_intent_and_are_not_scanned() {
    let mut g = graph();
    g.create_node(
        node::VERIFICATION,
        "ver:x",
        Props::new()
            .set("name", "the count is never truncated")
            .set("description", "the one direction this must never fail in"),
    )
    .expect("ver");
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:x",
        Props::new()
            .set("name", "measured")
            .set("fact_type", "finding")
            .set(
                "statement",
                "the report must never become telemetry — and it did not",
            )
            .set("subject_id", "ver:x"),
    )
    .expect("fact");
    assert!(the_gaps(&g).is_empty());
}

/// Deliberate exclusion, pinned: the rationale is the why, not the what.
#[test]
fn a_prohibition_quoted_in_a_rationale_is_reasoning_not_intent() {
    let mut g = graph();
    g.add_decision(
        "dec:r",
        "use one store",
        "One store for every branch.",
        Some("because a second must never diverge from it"),
    )
    .expect("dec");
    g.set_decision_status("dec:r", "accepted").expect("accept");
    assert!(
        the_gaps(&g).is_empty(),
        "the rationale field is not scanned"
    );
}

/// Emphasis is common in this design's prose — "MUST NEVER" in capitals is
/// the same prohibition.
#[test]
fn matching_is_case_insensitive() {
    let mut g = graph();
    req(
        &mut g,
        "req:caps",
        "SOMETHING-UNNAMED MUST NEVER READ AS NOTHING-HERE.",
    );
    assert_eq!(the_gaps(&g).len(), 1);
}

/// Parking is the standing way to say "known, not now".
#[test]
fn a_parked_node_is_skipped() {
    let mut g = graph();
    req(&mut g, "req:no-hang", "A non-tty must never hang.");
    g.add_decision("dec:park", "park it", "parked", None)
        .expect("ruling");
    g.set_decision_status("dec:park", "accepted")
        .expect("accept");
    g.governed_by(
        node::REQUIREMENT,
        "req:no-hang",
        node::DECISION,
        "dec:park",
        Some("parks"),
        None,
    )
    .expect("park");
    assert!(the_gaps(&g).is_empty());
}

/// An empty design has not declined to file its prohibitions; it has not got
/// there. Silence, not a "0 of 0".
#[test]
fn nothing_to_scan_is_silence() {
    let g = graph();
    assert!(the_gaps(&g).is_empty());
}
