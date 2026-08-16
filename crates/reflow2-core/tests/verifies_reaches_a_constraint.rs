//! Can a check say whether a LIMIT holds?
//!
//! `VERIFIES` was narrowed from `*` to an enumeration on 2026-08-08, and the
//! census that derived the list found five target types at 95/27/13/2/1 edges
//! and missed two carrying ONE edge each. The moment the wildcard became a list,
//! every unlisted-but-existing edge became unimportable — and dev_storyflow's
//! committed export, the design that PROPOSED the change, could not be
//! re-imported by the binary that wrote it for at least four days across thirty
//! export commits, while every status signal read green.
//!
//! `Constraint` is the half Anthony ruled back in on 2026-08-15
//! (`dec:should-a-verification-be-able-to-verify-a-constraint`): a Constraint is
//! a limit the design must respect, and asking whether a limit HOLDS is the most
//! natural thing to check about it. `DesignRule` was already legal, and a
//! Constraint is its sibling — the imposed limit beside the chosen rule.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;

fn graph_with_a_limit() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_constraint(
        "con:latency",
        "End-to-end latency",
        "the round trip stays under 200ms",
        None,
        None,
        None,
        None,
        None,
    )
    .expect("constraint");
    g.add_verification("ver:latency-gate", "the p99 latency check", None, None)
        .expect("verification");
    g
}

#[test]
fn a_check_can_verify_a_constraint() {
    let mut g = graph_with_a_limit();
    g.verifies("ver:latency-gate", node::CONSTRAINT, "con:latency")
        .expect("a check that measures whether a limit holds must be expressible");
}

/// COUNTERWEIGHT, and the reason this is a ruling rather than a loosening.
/// `Project` is the OTHER type the 2026-08-08 census missed, and it was
/// deliberately NOT admitted: a Verification that verifies a whole Project reads
/// as a modelling slip rather than a need, and nobody has ruled on it. If this
/// test ever starts passing, the enumeration has quietly become a wildcard again
/// and the question it was narrowed to make askable is unaskable once more.
#[test]
fn a_check_still_cannot_verify_a_whole_project() {
    let mut g = graph_with_a_limit();
    g.add_project("proj:x", "The whole thing").expect("project");
    assert!(
        g.verifies("ver:latency-gate", node::PROJECT, "proj:x")
            .is_err(),
        "Project was left out on purpose; admitting Constraint must not admit everything"
    );
}
