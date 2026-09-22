//! A design that publishes CONTRACTS and publishes no INTENT is asked about it.
//!
//! ROOT-CAUSED 2026-09-22 against a real consumer. flo2 mirrors reflow2's
//! published surface, correctly, with `mirror_surface`. Measured on both sides
//! the same day:
//!
//!   · reflow2 holds **262 Requirements and 0 marked `published`**, and 22
//!     Interfaces of which 3 are published.
//!   · flo2's mirror holds exactly 22 nodes — 11 Artifacts, 7 Components,
//!     3 Interfaces, 1 Project — and **zero Requirements, Capabilities or
//!     Decisions**. The 3 Interfaces match reflow2's 3 published ones exactly,
//!     so the mirror faithfully carries what reflow2 chose to publish.
//!   · flo2's whole design touches that mirror through **one edge**.
//!
//! ⇒ So a consumer asking a DESIGN question — "why does it work this way",
//! "what may I rely on" — reads seven components and three contracts and
//! correctly concludes the mirror has nothing to say. The owner reported the
//! symptom as an agent that keeps needing to be reminded the link exists; an
//! agent that stops mentioning a resource which never answers anything is
//! behaving sensibly.
//!
//! ⭐ WHY THIS DETECTOR AND NOT A CI SCRIPT. The hole is not reflow2's. Any
//! design that declares a published boundary and publishes no promise hands
//! its consumers structure with no reasoning, and nothing anywhere asks about
//! it. `export_surface` reports how much it WITHHELD, and that number reads as
//! success — a 2026-08-12 trial recorded "93 nodes and 201 edges withheld as
//! internal" as proof the black box was real. It is equally the reading "this
//! design publishes almost nothing", and the two were indistinguishable.
//!
//! 🛑 IT ASKS, IT DOES NOT JUDGE. Which requirements are promises is the
//! owner's call and nobody else's — `set_requirement_designation`'s own words:
//! "publishing is a commitment". This fires on ZERO, never on "too few", and
//! it is silent on a design that publishes no contracts either, because a
//! design nobody consumes owes nobody a promise.
//!
//! OBSERVED FAILING before the detector existed: the first two tests failed,
//! the third passed vacuously.

use reflow2_core::DesignGraph;

fn design_with_a_published_contract() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_project("proj:thing", "Thing").expect("project");
    g.add_requirement("req:thing", "The thing holds", "it holds")
        .expect("requirement");
    g.add_interface("ifc:thing", "the thing's boundary")
        .expect("interface");
    g.set_interface_designation("ifc:thing", "published")
        .expect("published contract");
    g
}

fn fires(g: &DesignGraph) -> bool {
    g.detect_gaps()
        .expect("gaps")
        .iter()
        .any(|gap| gap.gap_source.as_str() == "published_surface_carries_no_intent")
}

#[test]
fn publishing_a_contract_and_no_promise_is_asked_about() {
    let g = design_with_a_published_contract();
    assert!(
        fires(&g),
        "a design that publishes a boundary and no requirement hands every consumer structure \
         with no reasoning. Measured on reflow2 2026-09-22: 262 requirements, 0 published, and a \
         real consumer's mirror carried 22 nodes with no intent in it at all"
    );
}

#[test]
fn publishing_one_promise_is_enough_to_go_quiet() {
    let mut g = design_with_a_published_contract();
    g.set_requirement_designation("req:thing", "published")
        .expect("published promise");
    assert!(
        !fires(&g),
        "this fires on ZERO and never on \"too few\". Which requirements are promises is the \
         owner's judgement — `publishing is a commitment` — so the detector asks once and then \
         stays out of it"
    );
}

/// The silence that has to be deliberate: a design publishing NO contracts owes
/// nobody a promise, and asking it for one would fire on every private design
/// in existence.
#[test]
fn a_design_that_publishes_nothing_is_not_asked() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_project("proj:thing", "Thing").expect("project");
    g.add_requirement("req:thing", "The thing holds", "it holds")
        .expect("requirement");
    g.add_interface("ifc:thing", "the thing's boundary")
        .expect("interface");
    assert!(
        !fires(&g),
        "no published boundary means no consumer, and a design nobody consumes owes nobody a \
         promise"
    );
}
