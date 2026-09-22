//! A promise you publish travels with the check that says whether it holds.
//!
//! `export_surface`'s stated contract is: what stays home is "requirements,
//! capabilities, decisions, VERIFICATIONS, history, provenance, and every
//! internal component and contract". Requirements were later carved out as a
//! deliberate opt-in exception (`req:publishable-promise`) after a real trial
//! found the promise had ended up asserted in a comment in the CONSUMER's build
//! file — on the wrong side of the seam. This is that same question for the
//! EVIDENCE, and it was reached by the same route: a real consumer.
//!
//! ⭐ WHY IT MATTERS, in this design's own terms. Delivery is computed as
//! SATISFIED and REALIZED and A PASSING CHECK. A consumer who receives a
//! published promise and no check cannot tell whether it currently HOLDS — so
//! the surface hands across a claim nothing can re-run. That is precisely the
//! defect this repository spent 2026-09-22 removing from the inside of its own
//! design (241 capabilities reading "verified", 21 with a check anything could
//! re-run), reproduced at the seam.
//!
//! 🛑 DERIVED, NOT A SECOND OPT-IN, and the reason is measured rather than
//! aesthetic. Interfaces and Requirements each travel on an explicit
//! `designation`. A third gate on Verification would reproduce the failure
//! measured the same day: reflow2 held 262 requirements and had marked ZERO
//! `published`, because the vocabulary existed and nothing asked. **The owner
//! already opted in when they published the TARGET.** A check travels if and
//! only if it VERIFIES a node that is already on the surface — the same derived
//! rule that carries the parts either side of a contract and the artifacts that
//! specify it.
//!
//! ⚠️ AND THE RISK THAT COMES WITH IT, stated because nothing checks it:
//! `Verification.findings` is free text about what a run found, and a surface
//! exports nodes AS STORED — trimming one would make it import as a lie. So a
//! findings field holding internal paths or ids leaves with the check. That is
//! bounded (only checks on PUBLISHED targets travel) and it is a real
//! disclosure an owner should know about.
//!
//! OBSERVED FAILING before the carve-out: the two "travels" tests failed and
//! the two "stays home" tests passed vacuously — which is the shape a carve-out's
//! tests take before the carve-out exists.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::node;

/// A design with one published promise, one published boundary, and one of
/// each kept internal — so what travels and what does not are both asserted.
fn a_design_that_publishes_something() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_project("proj:thing", "Thing").expect("project");

    g.add_requirement("req:promised", "Ordering is preserved", "it is preserved")
        .expect("promised requirement");
    g.set_requirement_designation("req:promised", "published")
        .expect("publish the promise");
    g.add_requirement("req:internal", "An internal need", "internal")
        .expect("internal requirement");

    g.add_interface("ifc:published", "the published boundary")
        .expect("interface");
    g.set_interface_designation("ifc:published", "published")
        .expect("publish the boundary");

    // One check per target, so each assertion below names exactly one node.
    g.add_verification("ver:promise", "the ordering check", None, None, None)
        .expect("promise check");
    g.verifies("ver:promise", node::REQUIREMENT, "req:promised")
        .expect("verifies the promise");
    g.set_verification_status("ver:promise", "passing", Some("2026-09-22"), None)
        .expect("passing");

    g.add_verification("ver:boundary", "the boundary check", None, None, None)
        .expect("boundary check");
    g.verifies("ver:boundary", node::INTERFACE, "ifc:published")
        .expect("verifies the boundary");

    g.add_verification("ver:internal", "an internal check", None, None, None)
        .expect("internal check");
    g.verifies("ver:internal", node::REQUIREMENT, "req:internal")
        .expect("verifies an internal requirement");
    g
}

fn surface_holds(g: &DesignGraph, id: &str) -> bool {
    g.export_surface()
        .expect("surface")
        .document
        .nodes
        .iter()
        .any(|n| n.node_id == id)
}

#[test]
fn the_check_on_a_published_promise_travels() {
    let g = a_design_that_publishes_something();
    assert!(
        surface_holds(&g, "ver:promise"),
        "a consumer receiving a promise and no check cannot tell whether it HOLDS. Delivery is \
         satisfied AND realized AND a passing check, so withholding the check hands across a \
         claim nothing can re-run — the same defect this design removed from its own insides \
         on 2026-09-22, reproduced at the seam"
    );
}

#[test]
fn the_check_on_a_published_boundary_travels_too() {
    let g = a_design_that_publishes_something();
    assert!(
        surface_holds(&g, "ver:boundary"),
        "a published Interface is a commitment exactly as a published Requirement is, and the \
         rule is the same for both: a check travels if it verifies something already on the \
         surface"
    );
}

/// The half that makes this a carve-out rather than a hole.
#[test]
fn a_check_on_an_internal_requirement_stays_home() {
    let g = a_design_that_publishes_something();
    assert!(
        !surface_holds(&g, "ver:internal"),
        "the target was never published, so its check is internal. This is DERIVED from the \
         target's designation, never from the check's own — which is why it needs no third \
         opt-in gate and cannot go unused the way `designation` did"
    );
    assert!(
        !surface_holds(&g, "req:internal"),
        "and the internal requirement itself still stays home, unchanged"
    );
}

/// Unchanged behaviour, asserted so the carve-out cannot quietly widen: a design
/// that publishes no boundary and no promise still publishes no checks.
#[test]
fn a_design_that_publishes_nothing_still_exports_no_checks() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_project("proj:thing", "Thing").expect("project");
    g.add_requirement("req:internal", "An internal need", "internal")
        .expect("requirement");
    g.add_verification("ver:internal", "an internal check", None, None, None)
        .expect("check");
    g.verifies("ver:internal", node::REQUIREMENT, "req:internal")
        .expect("verifies");
    assert!(!surface_holds(&g, "ver:internal"));
}

/// The disclosure is NAMED, not silent. A check travels as stored, findings and
/// all — measured on reflow2 itself, the travelling checks carried internal
/// build paths and internal ids in that field — so the report says which ones
/// left, and an owner can read it before handing the surface over.
#[test]
fn the_surface_names_the_checks_it_carried() {
    let g = a_design_that_publishes_something();
    let s = g.export_surface().expect("surface");
    assert_eq!(
        s.evidence,
        vec!["ver:boundary".to_string(), "ver:promise".to_string()],
        "the checks that travelled are named and sorted, so a disclosure is something an owner \
         reads here rather than discovers in somebody else's graph"
    );
}
