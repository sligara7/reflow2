//! An edge to a node that does not exist is refused — **through every typed
//! helper**, not only through `create_edge`.
//!
//! `ver:edges-need-both-ends`. The friction that raised it, 2026-07-25:
//! `authored_by` accepted the contributor id `person:anthony`, which had never
//! been created. The edge was stored, `get_node` returned null for its target,
//! and a phantom `AUTHORED_BY` sat in the graph until somebody noticed it by eye.
//!
//! THE GUARD LANDED IN `create_edge` (graph.rs:297, 2026-07-28) AND WAS TESTED
//! THERE. `write_side.rs` already holds four good cases: a missing target is
//! refused, a missing source is refused, a refused edge leaves nothing behind,
//! a legitimate edge is still accepted, and an unknown edge type is still named
//! first. None of that is repeated here.
//!
//! WHAT WAS NOT TESTED, AND IS THE WHOLE POINT OF THIS FILE: every one of those
//! cases calls `g.create_edge(...)` directly, and the defect was in a TYPED
//! HELPER. Sixteen public helpers draw edges, and nothing proved they route
//! through the guarded path rather than reaching the engine another way. That is
//! not a hypothetical divergence — BL-183 found exactly this shape on the node
//! side, where 16 of 18 `add_*` constructors had drifted from the generic path
//! and each named only a subset of its type's properties. The helpers are where
//! the drift goes, because they are what callers actually use.
//!
//! So this is an AUDIT, not a sample: the table below is meant to hold every
//! typed helper that joins two nodes that must already exist. A helper missing
//! from it is untested, so add the row when you add the helper.
//!
//! DELIBERATELY OUT OF SCOPE, named so the omission is a choice and not an
//! oversight:
//!
//! - Constructors that create their own source node and draw an edge on the way
//!   (`add_decision`, `add_flow`, `add_resource`, `add_change_event`,
//!   `add_readiness`, `add_dimension_observation`). Only their TARGET can be
//!   missing, and `add_change_event` already carries its own all-or-nothing
//!   guarantee with tests.
//! - `create_edges`, the bulk form, whose all-or-nothing behaviour is
//!   `dec:bulk-is-all-or-nothing-with-per-item-findings` and is tested in
//!   `bulk.rs`.
//! - `gate_on`, which takes a struct rather than loose ids and validates a rung
//!   range first, so it is not uniform with the others.
//!
//! ## What the mutation check measured, which is not what it was expected to
//!
//! MUTATION-CHECKED by disabling the endpoint guard in `create_edge` and
//! re-running: **14 of the 16 failed, not 16.** `documents` and
//! `release_includes` still refused, because they carry their OWN existence
//! checks before delegating (`artifact.rs:519` and `:525`, `operate.rs:336`).
//!
//! That is not a defect and nothing here should "fix" it — belt and braces on two
//! helpers is strictly safer than one shared guard. But it is worth knowing
//! precisely, because it changes what this file proves: fourteen of these helpers
//! depend entirely on `create_edge`'s guard and would regress together if it
//! moved, while two would hold. So the audit is not uniform evidence, and a
//! future reader comparing the mutation result against the table should not read
//! the two survivors as a gap in the test.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{edge, node};
use reflow2_core::temporal::EpochType;

/// A design holding one real node of every type the table below needs, so the
/// only thing missing in each case is the endpoint under test.
fn populated() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:r", "R", "A real requirement.")
        .unwrap();
    g.add_capability("cap:c", "C", "A real capability", None)
        .unwrap();
    g.add_component("cmp:c", "Comp", "a real component", None)
        .unwrap();
    g.add_interface("ifc:i", "A real interface").unwrap();
    g.add_artifact("art:a", "a.rs", Some("code"), Some("src/a.rs"))
        .unwrap();
    g.add_verification("ver:v", "A real check", None, None)
        .unwrap();
    g.add_release("rel:r", "v1.0.0", Some("1.0.0"), None)
        .unwrap();
    g.add_environment("env:e", "A real environment", None, None)
        .unwrap();
    g.add_contributor("who:w", "somebody", None, None, None)
        .unwrap();
    g.add_constraint(
        "con:c",
        "A real budget",
        "Under a limit.",
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    g.add_epoch("epoch:one", "First", EpochType::Baseline, 1)
        .unwrap();
    g.add_epoch("epoch:two", "Second", EpochType::Revision, 2)
        .unwrap();
    g
}

/// The id no test creates. Any helper that accepts an edge to this has the
/// 2026-07-25 defect back.
const GHOST: &str = "ghost:does-not-exist";

/// Every typed helper that joins two nodes which must already exist, each
/// called with ONE endpoint replaced by [`GHOST`].
///
/// The closure returns whether the call was refused. A `String` label travels
/// with it so a failure names the helper rather than a row number.
#[allow(clippy::type_complexity)]
fn helpers() -> Vec<(&'static str, Box<dyn Fn(&mut DesignGraph) -> bool>)> {
    vec![
        // The one the friction was actually filed about.
        (
            "authored_by",
            Box::new(|g: &mut DesignGraph| {
                g.authored_by(node::REQUIREMENT, "req:r", GHOST, None, None)
                    .is_err()
            }),
        ),
        (
            "satisfies",
            Box::new(|g: &mut DesignGraph| g.satisfies("cap:c", GHOST).is_err()),
        ),
        (
            "allocate",
            Box::new(|g: &mut DesignGraph| g.allocate("cap:c", GHOST).is_err()),
        ),
        (
            "governed_by",
            Box::new(|g: &mut DesignGraph| {
                g.governed_by(node::CAPABILITY, "cap:c", node::DECISION, GHOST)
                    .is_err()
            }),
        ),
        (
            "provides",
            Box::new(|g: &mut DesignGraph| g.provides("cmp:c", GHOST).is_err()),
        ),
        (
            "consumes",
            Box::new(|g: &mut DesignGraph| g.consumes("cmp:c", GHOST).is_err()),
        ),
        (
            "contains",
            Box::new(|g: &mut DesignGraph| g.contains("proj:p", node::REQUIREMENT, GHOST).is_err()),
        ),
        (
            "contain_component",
            Box::new(|g: &mut DesignGraph| g.contain_component("cmp:c", GHOST).is_err()),
        ),
        (
            "realizes",
            Box::new(|g: &mut DesignGraph| {
                g.realizes("art:a", node::CAPABILITY, GHOST, None).is_err()
            }),
        ),
        (
            "documents",
            Box::new(|g: &mut DesignGraph| {
                g.documents("art:a", node::CAPABILITY, GHOST, None).is_err()
            }),
        ),
        (
            "verifies",
            Box::new(|g: &mut DesignGraph| g.verifies("ver:v", node::CAPABILITY, GHOST).is_err()),
        ),
        (
            "deploy_to",
            Box::new(|g: &mut DesignGraph| g.deploy_to("rel:r", GHOST, None).is_err()),
        ),
        (
            "require_resource",
            Box::new(|g: &mut DesignGraph| {
                g.require_resource(node::COMPONENT, "cmp:c", GHOST, None)
                    .is_err()
            }),
        ),
        (
            "release_includes",
            Box::new(|g: &mut DesignGraph| {
                g.release_includes("rel:r", node::CAPABILITY, GHOST, None)
                    .is_err()
            }),
        ),
        (
            "constrains",
            Box::new(|g: &mut DesignGraph| {
                g.constrains("con:c", node::CAPABILITY, GHOST, None, None, None)
                    .is_err()
            }),
        ),
        (
            "precedes",
            Box::new(|g: &mut DesignGraph| g.precedes("epoch:one", GHOST).is_err()),
        ),
    ]
}

#[test]
fn every_typed_helper_refuses_an_endpoint_that_does_not_exist() {
    // THE AUDIT. Each helper is called on its own graph so one refusal cannot
    // mask the next, and every failure is collected before asserting — a helper
    // that stores a phantom is a defect worth naming alongside its siblings
    // rather than hiding behind whichever one fails first.
    let mut accepted = Vec::new();
    for (name, call) in helpers() {
        let mut g = populated();
        if !call(&mut g) {
            accepted.push(name);
        }
    }
    assert!(
        accepted.is_empty(),
        "these typed helpers ACCEPTED an edge to a node that does not exist, so \
         each one stored a phantom endpoint the way authored_by did in the \
         2026-07-25 friction: {accepted:?}"
    );
}

#[test]
fn the_audit_covers_every_helper_that_draws_an_edge_between_existing_nodes() {
    // THE COUNTERWEIGHT THAT KEEPS THE AUDIT AN AUDIT. The table above is only
    // worth anything if it is complete, and a table is exactly the thing that
    // silently stops being complete when a helper is added. This is not a
    // substitute for reading the source — it is a floor, so the count cannot
    // quietly shrink.
    //
    // If you added a typed edge helper, add its row and raise this number. If
    // you removed one, lower it. Either way the change is deliberate and shows
    // up in the diff, which is the whole point.
    assert_eq!(
        helpers().len(),
        16,
        "the audit table has changed size — add or remove the row deliberately, \
         and do not lower this number to make a red test green"
    );
}

#[test]
fn a_refused_helper_call_leaves_nothing_behind() {
    // Refusal has to be atomic, not merely reported. `authored_by`'s original
    // defect was that the edge WAS stored; a helper that errors after writing
    // would pass the audit above and still leave the graph unwalkable.
    let mut g = populated();
    assert!(
        g.authored_by(node::REQUIREMENT, "req:r", GHOST, None, None)
            .is_err()
    );
    assert!(
        g.outgoing("req:r", Some(edge::AUTHORED_BY))
            .expect("read back")
            .is_empty(),
        "a refused authored_by must leave no edge on the source node"
    );
}

#[test]
fn the_helpers_still_draw_the_edge_when_both_ends_are_real() {
    // Without this, the cheapest way to pass every test above is to break all
    // sixteen helpers. Three are spot-checked across three different modules —
    // graph.rs, artifact.rs and operate.rs — because a guard added in one file's
    // helper says nothing about another's.
    let mut g = populated();
    g.satisfies("cap:c", "req:r")
        .expect("both ends real, so satisfies must write");
    g.realizes("art:a", node::CAPABILITY, "cap:c", None)
        .expect("both ends real, so realizes must write");
    g.deploy_to("rel:r", "env:e", None)
        .expect("both ends real, so deploy_to must write");
    assert_eq!(
        g.outgoing("cap:c", Some(edge::SATISFIES))
            .expect("read back")
            .len(),
        1,
        "the accepted edge must be readable"
    );
}
