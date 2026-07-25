//! Published boundaries, and the severability they make computable.
//!
//! From the MOSA guidebook Anthony supplied (2026-07-25) and, independently,
//! from BL-45's system-of-systems thread: nothing marked an Interface as the
//! boundary others rely on, so "mine to change" and "published" were
//! indistinguishable. The designation only earns its keep if a computation reads
//! it (`dec:edge-orthogonality`), so these tests are mostly about the
//! computation: a change either stays behind the boundaries the design has
//! designated, or it does not, and the diagram gets no vote.
//!
//! The fixture is the satellite crosslink again: two vehicles' terminals talking
//! through a published optical contract, with an internal contract inside one
//! terminal for contrast.

use reflow2_core::nodes::node;
use reflow2_core::propagate::PropagateOptions;
use reflow2_core::{DesignGraph, Value};

/// A terminal with an internal contract, and a published crosslink to its peer.
fn constellation() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:sat", "Constellation").unwrap();

    for (id, name) in [
        ("cmp:terminal-a", "Terminal A"),
        ("cmp:terminal-b", "Terminal B"),
        ("cmp:pointing", "Pointing control"),
    ] {
        g.add_component(id, name, "part of a crosslink terminal", None)
            .unwrap();
    }

    // The published boundary between two vehicles: an ICD, in SE terms.
    g.add_interface("ifc:crosslink", "Optical crosslink")
        .unwrap();
    g.provides("cmp:terminal-a", "ifc:crosslink").unwrap();
    g.consumes("cmp:terminal-b", "ifc:crosslink").unwrap();

    // An internal contract inside terminal A — plumbing, not a promise.
    g.add_interface("ifc:pointing-api", "Pointing command API")
        .unwrap();
    g.provides("cmp:pointing", "ifc:pointing-api").unwrap();
    g.consumes("cmp:terminal-a", "ifc:pointing-api").unwrap();
    g
}

#[test]
fn an_interface_is_internal_until_someone_publishes_it() {
    // Publishing is a commitment. Defaulting to it would assert one nobody made
    // — the same reasoning that makes a new Decision `proposed`.
    let g = constellation();
    let ifc = g
        .get_node(node::INTERFACE, "ifc:crosslink")
        .unwrap()
        .unwrap();
    let designation = ifc.properties.get("designation").and_then(Value::as_str);
    assert!(
        designation.is_none() || designation == Some("internal"),
        "a fresh Interface must not claim to be published: {designation:?}"
    );
    assert!(
        g.published_interfaces().unwrap().is_empty(),
        "nothing is published until it is designated"
    );
}

#[test]
fn designating_a_boundary_is_loud_about_a_bad_value() {
    let mut g = constellation();
    let err = g.set_interface_designation("ifc:crosslink", "sort-of-public");
    let message = format!("{}", err.unwrap_err());
    assert!(
        message.contains("internal") && message.contains("published"),
        "the refusal must name the real options: {message}"
    );

    // And an unknown Interface fails loud rather than creating one.
    assert!(
        g.set_interface_designation("ifc:nope", "published")
            .is_err()
    );
}

#[test]
fn designating_preserves_everything_else() {
    let mut g = constellation();
    g.set_interface_designation("ifc:crosslink", "published")
        .unwrap();
    let ifc = g
        .get_node(node::INTERFACE, "ifc:crosslink")
        .unwrap()
        .unwrap();
    assert_eq!(ifc.properties["designation"], Value::from("published"));
    assert_eq!(
        ifc.properties["name"],
        Value::from("Optical crosslink"),
        "a designation change must not eat the node"
    );
    assert_eq!(
        g.published_interfaces().unwrap(),
        ["ifc:crosslink".to_string()].into_iter().collect()
    );
}

#[test]
fn a_change_that_reaches_the_peer_names_the_boundary_it_crossed() {
    // THE test. MOSA asks a program to "demonstrate severable modular
    // components"; this is that demonstration, computed: the change leaves
    // terminal A and lands on terminal B, so it is not contained — and the
    // report says WHICH contract carried it, because "you crossed a boundary" is
    // unactionable while "you crossed this one" tells you whom to talk to.
    let mut g = constellation();
    g.set_interface_designation("ifc:crosslink", "published")
        .unwrap();

    let radius = g
        .propagate_from(&["cmp:terminal-a"], PropagateOptions { max_depth: 4 })
        .unwrap();

    assert_eq!(
        radius.boundary_crossings,
        vec!["ifc:crosslink".to_string()],
        "the published contract must be named: {radius:?}"
    );
    let peer = radius
        .impacted
        .iter()
        .find(|n| n.node_id == "cmp:terminal-b")
        .expect("the peer is reachable");
    assert!(
        peer.crosses_published_boundary,
        "the peer is only reachable THROUGH the published contract"
    );
}

#[test]
fn a_change_behind_the_boundary_crosses_nothing() {
    // The severable case, which is the one worth being able to prove. A change
    // to the pointing control reaches terminal A through an INTERNAL contract
    // and stops: nothing published carried it.
    let mut g = constellation();
    g.set_interface_designation("ifc:crosslink", "published")
        .unwrap();

    let radius = g
        .propagate_from(&["cmp:pointing"], PropagateOptions { max_depth: 2 })
        .unwrap();

    assert!(
        radius.boundary_crossings.is_empty(),
        "an internal change must cross nothing published: {:?}",
        radius.boundary_crossings
    );
    let terminal = radius
        .impacted
        .iter()
        .find(|n| n.node_id == "cmp:terminal-a")
        .expect("reached through the internal API");
    assert!(!terminal.crosses_published_boundary);
}

#[test]
fn standing_on_a_boundary_is_not_crossing_it() {
    // Seeding the change ON the published interface — editing the contract
    // itself — must not report that the contract was crossed. You are changing
    // the promise, not passing through it, and conflating the two would make
    // every ICD edit look like a containment failure.
    let mut g = constellation();
    g.set_interface_designation("ifc:crosslink", "published")
        .unwrap();

    let radius = g
        .propagate_from(&["ifc:crosslink"], PropagateOptions { max_depth: 2 })
        .unwrap();
    assert!(
        radius.boundary_crossings.is_empty(),
        "the seed itself is not a crossing: {:?}",
        radius.boundary_crossings
    );
}

#[test]
fn the_summary_carries_the_crossings_too() {
    // propagate answers with a summary by default (BL-48/BL-49), so a finding
    // that only exists in the full dump is a finding most callers never see.
    let mut g = constellation();
    g.set_interface_designation("ifc:crosslink", "published")
        .unwrap();
    let summary = g
        .propagate_from(&["cmp:terminal-a"], PropagateOptions { max_depth: 4 })
        .unwrap()
        .summarize();
    assert_eq!(
        summary.boundary_crossings,
        vec!["ifc:crosslink".to_string()]
    );
}

#[test]
fn undesignating_a_boundary_takes_the_crossing_with_it() {
    // The designation is read live, never cached into the radius — a boundary
    // withdrawn stops being reported, because the computation follows the design
    // rather than remembering it.
    let mut g = constellation();
    g.set_interface_designation("ifc:crosslink", "published")
        .unwrap();
    g.set_interface_designation("ifc:crosslink", "internal")
        .unwrap();
    let radius = g
        .propagate_from(&["cmp:terminal-a"], PropagateOptions { max_depth: 4 })
        .unwrap();
    assert!(radius.boundary_crossings.is_empty());
}
