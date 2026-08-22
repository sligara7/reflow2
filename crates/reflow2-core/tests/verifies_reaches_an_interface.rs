//! Can a check say whether a CONTRACT still holds?
//!
//! `Interface` is the third type found missing from `VERIFIES`' enumeration,
//! after the extraction hint under-enumerating and the 2026-08-08 census missing
//! a long tail. This one could appear in neither, because NO EDGE EXISTED TO
//! COUNT — an absence cannot show up in a survey of what is present.
//!
//! THE COST OF IT BEING ABSENT, measured on reflow2's own design 2026-08-22:
//! ALL 18 Interfaces carried ZERO incoming VERIFIES — not a low number, the
//! total absence a banned edge produces — which reads as "nobody wrote one"
//! when the truth was "nobody could". Three of them are PUBLISHED contracts.
//! `ifc:mcp-tools` is specified 9 of 9 axes, is the surface every user's agent
//! binds to, and could not carry a single piece of evidence that it still
//! offers what it says.
//!
//! AND THE CHECK ALREADY EXISTED. `tools/toolsnap.py` freezes every served tool
//! schema as a committed golden and fails CI when the live binary disagrees.
//! That is conformance of an implementation against a published contract — the
//! thing a check on an Interface asserts — and it had to hang off a Capability
//! instead, leaving "which of my contracts have no check?" unaskable. The same
//! argument DesignRule was admitted on, one node type over.
//!
//! WHAT A CHECK ON A CONTRACT MEANS, so it is not confused with a unit test: it
//! asserts the boundary STILL OFFERS WHAT IT SAYS — the operations, the payload
//! shape, the error model. A Capability's check asks whether the behaviour
//! works; a contract's asks whether the promise is still kept.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;

fn graph_with_a_contract() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:tools", "MCP tool surface")
        .expect("interface");
    g.add_verification(
        "ver:toolsnap",
        "served tool schemas match the committed goldens",
        None,
        None,
        None,
    )
    .expect("verification");
    g
}

#[test]
fn a_check_can_verify_a_contract() {
    let mut g = graph_with_a_contract();
    g.verifies("ver:toolsnap", node::INTERFACE, "ifc:tools")
        .expect("a check that a boundary still offers what it says must be expressible");
}

/// WIDENING MUST NOT HAVE DISTURBED WHAT WAS ALREADY LEGAL. The incident this
/// enumeration carries in its comments was a NARROWING that orphaned real data
/// and made a committed export unimportable for four days while every status
/// signal read green. Adding a target refuses less than before and can orphan
/// nothing — but that is an argument, and this is the check.
#[test]
fn the_targets_that_were_already_legal_still_are() {
    let mut g = graph_with_a_contract();
    g.add_capability("cap:x", "X", "does x", None).unwrap();
    g.add_requirement("req:x", "X", "shall x").unwrap();
    g.add_component("cmp:x", "X", "part", None).unwrap();
    g.add_artifact("art:x", "x.rs", Some("code"), Some("src/x.rs"))
        .unwrap();

    for (ty, id) in [
        (node::CAPABILITY, "cap:x"),
        (node::REQUIREMENT, "req:x"),
        (node::COMPONENT, "cmp:x"),
        (node::ARTIFACT, "art:x"),
    ] {
        g.verifies("ver:toolsnap", ty, id)
            .unwrap_or_else(|e| panic!("{ty} was legal before and must stay legal: {e}"));
    }
}

/// COUNTERWEIGHT, and the reason this is a ruling rather than a loosening.
/// `Project` is the type the 2026-08-08 census also missed and it was
/// deliberately NOT admitted — a Verification that verifies a whole Project
/// reads as a modelling slip rather than a need, and nobody has ruled on it.
/// If this test ever starts passing, the enumeration has quietly become a
/// wildcard again and the question it was narrowed to make askable is unaskable
/// once more.
#[test]
fn a_check_still_cannot_verify_a_whole_project() {
    let mut g = graph_with_a_contract();
    g.add_project("proj:x", "The whole thing").expect("project");
    assert!(
        g.verifies("ver:toolsnap", node::PROJECT, "proj:x").is_err(),
        "Project was left out on purpose; admitting Interface must not admit everything"
    );
}
