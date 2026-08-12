//! Re-mirroring one design is a REFRESH, not a second stranger arriving.
//!
//! # The defect, found by running the tool twice
//!
//! Measured 2026-08-12 on the reflow2 ↔ flo2 trial. The first mirror was clean:
//! 13 nodes, 19 edges, zero collisions, pinned to a content hash. Then the far
//! side moved the way a real partner moves — one boundary renamed, its `medium`
//! corrected — and the second mirror produced:
//!
//! ```text
//! mirrored_nodes         1
//! mirrored_edges         0
//! collisions            12      <- everything from the FIRST mirror
//! mirror_content_hash  null
//! ```
//!
//! Twelve of thirteen ids refused as collisions **with the previous mirror of
//! the same design**. The refusal is correct and protective for a genuine
//! cross-design clash — *"two designs using one id for different things is a
//! naming conversation between their owners"* — and it cannot tell that case
//! from this one.
//!
//! The resulting state is quietly wrong, which is worse than a plain failure:
//! the stale node stays with its old values, the host's own edge still points at
//! it, and **`mirrors` still reports the FIRST hash** — so after a failed
//! refresh the staleness register reads FRESH. The whole value of
//! `mirror_content_hash` is the claim that *"a newer surface with a different
//! hash means this mirror is stale"*.
//!
//! ⇒ **Re-mirroring one `mirror_of` is a different operation from mirroring a
//! new design.** One is "replace what I hold of theirs, at a new pin"; the other
//! is "add a stranger, and refuse anything that clashes". Only the second was
//! implemented, and a federation of independently-moving projects is MADE of
//! the first.
//!
//! # The question this had to settle first
//!
//! What happens to a host edge whose foreign target is gone from the new
//! surface? reflow2 refuses dangling edges outright, so a replace-style refresh
//! cannot simply delete.
//!
//! **It REFUSES and names what would break.** The host chose to consume that
//! boundary; removing it silently is exactly the class `dec:ask-not-repair`
//! forbids, and a refusal that names both the node and the edges pointing at it
//! is the two-sided-accept shape `set_artifact_checksum` already uses. A partner
//! withdrawing a contract you depend on is a conversation, not a cleanup.

use reflow2_core::nodes::node;
use reflow2_core::{DesignGraph, GraphExport};

/// Their published surface: a project, a component, and the boundary we consume.
fn their_surface(hash: &str, boundary: &str) -> GraphExport {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:them", "Their design").unwrap();
    g.add_component(
        "cmp:theirs",
        "Their service",
        "serves it",
        Some("subsystem"),
    )
    .unwrap();
    g.contains("proj:them", node::COMPONENT, "cmp:theirs")
        .unwrap();
    g.add_interface(boundary, "The boundary").unwrap();
    g.provides("cmp:theirs", boundary).unwrap();
    g.set_interface_designation(boundary, "published").unwrap();
    let mut doc = g.export_graph().unwrap();
    doc.graph_id = "them".to_string();
    doc.content_hash = Some(hash.to_string());
    doc
}

/// A host that has mirrored them once and drawn its own edge into the boundary.
fn host_that_mirrored_once() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:us", "Our design").unwrap();
    g.add_component(
        "cmp:ours",
        "Our service",
        "consumes theirs",
        Some("subsystem"),
    )
    .unwrap();
    g.contains("proj:us", node::COMPONENT, "cmp:ours").unwrap();
    g.mirror_surface(
        &their_surface("sha256:v1", "ifc:boundary"),
        Some("2026-08-12"),
    )
    .unwrap();
    g
}

// THE DEFECT CASE. The same design, mirrored again after it moved, must REFRESH
// — not collide with its own previous mirror.
#[test]
fn re_mirroring_the_same_design_refreshes_instead_of_colliding() {
    let mut g = host_that_mirrored_once();
    let r = g
        .mirror_surface(
            &their_surface("sha256:v2", "ifc:boundary"),
            Some("2026-08-13"),
        )
        .expect("a refresh of a design we already hold");

    assert!(
        r.collisions.is_empty(),
        "a design must not collide with its own previous mirror: {:?}",
        r.collisions
    );
    assert!(r.refreshed, "this is a refresh, and the report must say so");
}

// 🛑 THE ONE THAT MADE THE FAILURE DANGEROUS: the pin must advance, because a
// staleness register that reads FRESH after a failed refresh is worse than none.
#[test]
fn the_pin_advances_so_the_staleness_register_stops_lying() {
    let mut g = host_that_mirrored_once();
    assert_eq!(
        g.mirrors().unwrap()[0].mirror_content_hash.as_deref(),
        Some("sha256:v1"),
        "precondition: pinned to v1"
    );

    g.mirror_surface(
        &their_surface("sha256:v2", "ifc:boundary"),
        Some("2026-08-13"),
    )
    .unwrap();

    let m = g.mirrors().unwrap();
    assert_eq!(m.len(), 1, "one design mirrored, one entry — not two");
    assert_eq!(
        m[0].mirror_content_hash.as_deref(),
        Some("sha256:v2"),
        "the pin must move to what we now hold"
    );
    assert_eq!(m[0].mirrored_at.as_deref(), Some("2026-08-13"));
}

// The refreshed content must actually be theirs-as-of-now, not the old copy
// left in place beside a new arrival.
#[test]
fn refreshed_nodes_carry_the_new_values_and_the_old_ones_are_gone() {
    let mut g = host_that_mirrored_once();
    // They rename the boundary.
    g.mirror_surface(
        &their_surface("sha256:v2", "ifc:boundary-renamed"),
        Some("2026-08-13"),
    )
    .expect("nothing of ours points at the old boundary in this fixture");

    assert!(
        g.get_node(node::INTERFACE, "ifc:boundary-renamed")
            .unwrap()
            .is_some(),
        "the new boundary must be here"
    );
    assert!(
        g.get_node(node::INTERFACE, "ifc:boundary")
            .unwrap()
            .is_none(),
        "the withdrawn boundary must NOT linger — that is the stale-node half of the defect"
    );
}

// 🛑 THE SETTLED QUESTION. A boundary we CONSUME cannot be removed from under
// us: the refresh REFUSES and names both the node and our edge, because a
// partner withdrawing a contract you depend on is a conversation, not a
// cleanup (dec:ask-not-repair).
#[test]
fn a_refresh_that_would_withdraw_something_we_consume_is_refused_and_names_it() {
    let mut g = host_that_mirrored_once();
    g.consumes("cmp:ours", "ifc:boundary").unwrap();

    let err = g
        .mirror_surface(
            &their_surface("sha256:v2", "ifc:boundary-renamed"),
            Some("2026-08-13"),
        )
        .expect_err("withdrawing a boundary we consume must be refused");

    let msg = err.to_string();
    assert!(
        msg.contains("ifc:boundary"),
        "the refusal must name the boundary that would go: {msg}"
    );
    assert!(
        msg.contains("cmp:ours"),
        "and name what of ours points at it: {msg}"
    );
}

// AND THE REFUSAL MUST CHANGE NOTHING. A half-applied refresh would leave the
// graph in a state neither side described.
#[test]
fn a_refused_refresh_leaves_the_mirror_exactly_as_it_was() {
    let mut g = host_that_mirrored_once();
    g.consumes("cmp:ours", "ifc:boundary").unwrap();
    let _ = g.mirror_surface(
        &their_surface("sha256:v2", "ifc:boundary-renamed"),
        Some("2026-08-13"),
    );

    assert!(
        g.get_node(node::INTERFACE, "ifc:boundary")
            .unwrap()
            .is_some(),
        "the boundary we consume must still be here after a refusal"
    );
    assert!(
        g.get_node(node::INTERFACE, "ifc:boundary-renamed")
            .unwrap()
            .is_none(),
        "and nothing from the refused document may have landed"
    );
    assert_eq!(
        g.mirrors().unwrap()[0].mirror_content_hash.as_deref(),
        Some("sha256:v1"),
        "the pin must NOT advance on a refusal — that is the lie this whole file is about"
    );
}

// COUNTERWEIGHT, and the one that keeps refresh from eating the host: a
// DIFFERENT design's ids still collide and are still refused. Refresh is scoped
// to the design being re-mirrored and touches nothing else.
#[test]
fn a_different_design_still_collides_and_is_still_refused() {
    let mut g = host_that_mirrored_once();
    let mut other = their_surface("sha256:other", "ifc:boundary");
    other.graph_id = "someone-else".to_string();

    let r = g.mirror_surface(&other, Some("2026-08-13")).unwrap();
    assert!(
        !r.collisions.is_empty(),
        "two designs using one id is still a naming conversation, not a refresh"
    );
    assert!(!r.refreshed, "a stranger is not a refresh");
}

// COUNTERWEIGHT: our own nodes are never touched by a refresh, however much
// churn the far side has.
#[test]
fn our_own_design_survives_a_refresh_untouched() {
    let mut g = host_that_mirrored_once();
    g.mirror_surface(
        &their_surface("sha256:v2", "ifc:boundary"),
        Some("2026-08-13"),
    )
    .unwrap();

    assert!(g.get_node(node::COMPONENT, "cmp:ours").unwrap().is_some());
    assert!(g.get_node(node::PROJECT, "proj:us").unwrap().is_some());
}
