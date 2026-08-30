//! A contract answers at the altitude you asked the question at.
//!
//! `undeclared_seam` asks *"do these two exact modules share a contract?"* and
//! could only ever ask it at module level. So a design that records its
//! DEPENDENCIES between modules and declares its CONTRACTS at the subsystem
//! boundary reads as having no contracts at all — measured on reflow2's own
//! design, 64 of 72 couplings undeclared, and
//! `fact:coupling-and-contract-are-recorded-in-vocabularies-that-never-meet`
//! showed the two sets were DISJOINT BY CONSTRUCTION, so the number could not
//! move however many contracts anyone wrote.
//!
//! Anthony, 2026-08-23: a contract should be *"defined at the lowest level that
//! actually defines the interface and the rest is rolled up"*, so that asking
//! at subsystem level answers yes while the graph still shows which two leaves
//! the contract actually sits between.
//!
//! Lifted to `subsystem` on the same design: 11 couplings, 13 contract pairs,
//! **nothing** undeclared.
//!
//! THREE PROPERTIES KEEP THAT HONEST, and they are what these pin:
//!
//! 1. **Both sets are lifted, never one.** Lifting couplings alone would leave
//!    the two vocabularies as disjoint as they were.
//! 2. **A zero says what it compared.** Every reply carries the raw count, so
//!    "0 undeclared" can never be read as "everything is contracted" when it
//!    means "everything visible AT THIS ALTITUDE is contracted".
//! 3. **Nothing is written back.** The roll-up is derived on every call. A
//!    stored edge between two subsystems would make the graph assert a contract
//!    nobody declared — `dec:views-are-projections`.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, GraphExport};

/// Two subsystems, each holding two modules. The DEPENDENCY is recorded between
/// modules; the CONTRACT is declared between two other modules. Nothing joins
/// them at module level — which is the shape the whole thing is about.
fn two_subsystems() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "A program").unwrap();
    for (id, level) in [
        ("sys:a", "subsystem"),
        ("sys:b", "subsystem"),
        ("cmp:a1", "component"),
        ("cmp:a2", "component"),
        ("cmp:b1", "component"),
        ("cmp:b2", "component"),
    ] {
        g.add_component(id, id, "a part", Some(level)).unwrap();
    }
    g.contain_component("sys:a", "cmp:a1").unwrap();
    g.contain_component("sys:a", "cmp:a2").unwrap();
    g.contain_component("sys:b", "cmp:b1").unwrap();
    g.contain_component("sys:b", "cmp:b2").unwrap();

    // The coupling: a1 depends on b1.
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        "cmp:a1",
        node::COMPONENT,
        "cmp:b1",
        Props::new(),
    )
    .unwrap();
    // The contract: declared between a DIFFERENT pair, a2 and b2.
    g.add_interface("ifc:x", "The contract").unwrap();
    g.provides("cmp:a2", "ifc:x").unwrap();
    g.consumes("cmp:b2", "ifc:x").unwrap();
    g
}

#[test]
fn at_module_level_the_two_vocabularies_never_meet() {
    // The state that made the finding unable to reach zero: one coupling, one
    // contract, and they are between different pairs.
    let g = two_subsystems();
    let r = g.seam_coverage(None).unwrap();
    assert_eq!(r.couplings, 1);
    assert_eq!(r.declared, 1);
    assert_eq!(
        r.covered, 0,
        "the coupling and the contract are not the same pair"
    );
    assert_eq!(r.uncovered.len(), 1);
    assert!(
        r.scope_note.contains("Module level, nothing lifted"),
        "the raw answer must say it lifted nothing: {}",
        r.scope_note
    );
}

#[test]
fn lifted_to_the_altitude_where_the_contract_lives_it_is_covered() {
    let g = two_subsystems();
    let r = g.seam_coverage(Some("subsystem")).unwrap();
    assert_eq!(r.couplings, 1, "a1->b1 lifts to sys:a <-> sys:b");
    assert_eq!(
        r.covered, 1,
        "and the a2/b2 contract lifts onto the same pair"
    );
    assert!(r.uncovered.is_empty());

    let seam = &r.covered_by[0];
    assert_eq!(seam.between, ("sys:a".into(), "sys:b".into()));
    assert!(
        seam.declared_at
            .iter()
            .any(|s| s.contains("cmp:a2") && s.contains("cmp:b2")),
        "answering 'yes' without naming WHERE it is declared is the half-answer that sends a \
         reader hunting: {:?}",
        seam.declared_at
    );
}

#[test]
fn a_coupling_inside_one_box_is_not_a_seam_at_that_altitude() {
    // Both sets are lifted, so a dependency between two modules of the SAME
    // subsystem disappears at subsystem altitude rather than becoming a
    // self-pair. Getting this wrong would invent a boundary inside a box.
    let mut g = two_subsystems();
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        "cmp:a1",
        node::COMPONENT,
        "cmp:a2",
        Props::new(),
    )
    .unwrap();
    assert_eq!(g.seam_coverage(None).unwrap().couplings, 2);
    let r = g.seam_coverage(Some("subsystem")).unwrap();
    assert_eq!(
        r.couplings, 1,
        "the a1<->a2 coupling is internal to sys:a and is not a seam one level up"
    );
}

#[test]
fn an_altitude_nothing_reaches_says_it_compared_nothing() {
    // THE VACUOUS ZERO, AND IT MUST ANNOUNCE ITSELF. No component sits at
    // `system`, so nothing lifts, and a bare `uncovered: []` here would read as
    // a clean bill for a question that was never asked.
    let g = two_subsystems();
    let r = g.seam_coverage(Some("system")).unwrap();
    assert_eq!(r.couplings, 0);
    assert!(r.uncovered.is_empty());
    assert!(
        r.scope_note.contains("reached no container at this level"),
        "a zero over an empty population must say the population was empty: {}",
        r.scope_note
    );
    assert!(
        r.scope_note.contains("lifted from 1"),
        "and it must still carry the raw count it did not compare: {}",
        r.scope_note
    );
}

#[test]
fn the_roll_up_writes_nothing_back() {
    // dec:views-are-projections. A stored edge between two subsystems would
    // make the graph assert a contract nobody declared, and it would then be
    // indistinguishable from one somebody did.
    let g = two_subsystems();
    let before: GraphExport = g.export_graph().unwrap();
    let _ = g.seam_coverage(Some("subsystem")).unwrap();
    let _ = g.seam_coverage(None).unwrap();
    let after: GraphExport = g.export_graph().unwrap();
    assert_eq!(
        before.content_hash, after.content_hash,
        "asking the question changed the design"
    );
}

/// The same shape as `two_subsystems`, with the containers named `sub:*`.
/// Nothing about a roll-up depends on how an id is spelled.
fn two_subsystems_named_sub() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "A program").unwrap();
    for (id, level) in [
        ("sub:a", "subsystem"),
        ("sub:b", "subsystem"),
        ("cmp:a1", "component"),
        ("cmp:a2", "component"),
        ("cmp:b1", "component"),
        ("cmp:b2", "component"),
    ] {
        g.add_component(id, id, "a part", Some(level)).unwrap();
    }
    g.contain_component("sub:a", "cmp:a1").unwrap();
    g.contain_component("sub:a", "cmp:a2").unwrap();
    g.contain_component("sub:b", "cmp:b1").unwrap();
    g.contain_component("sub:b", "cmp:b2").unwrap();
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        "cmp:a1",
        node::COMPONENT,
        "cmp:b1",
        Props::new(),
    )
    .unwrap();
    g.add_interface("ifc:x", "The contract").unwrap();
    g.provides("cmp:a2", "ifc:x").unwrap();
    g.consumes("cmp:b2", "ifc:x").unwrap();
    g
}

#[test]
fn a_container_is_found_by_its_level_not_by_how_its_id_is_spelled() {
    // REGRESSION, 2026-08-30. The walk up the spine picked a parent with
    // `p.starts_with("cmp:") || p.starts_with("sys:")` — a naming convention
    // standing in for the `level` declaration. Every module whose container was
    // named `sub:*` failed to lift and its pair was dropped from BOTH sets, so
    // the answer stayed silent instead of being wrong out loud.
    //
    // On reflow2's own design that hid 58 of 86 modules: the subsystem-level
    // answer compared 8 boundaries instead of 19 and reported 0 uncovered
    // instead of 6. Every test above this one used `sys:`-named containers, so
    // the suite shared the code's assumption and stayed green.
    let g = two_subsystems_named_sub();
    let r = g.seam_coverage(Some("subsystem")).unwrap();

    assert_eq!(r.couplings, 1, "a1->b1 must lift to sub:a <-> sub:b");
    assert_eq!(r.covered, 1, "and the a2/b2 contract onto the same pair");
    assert_eq!(
        r.covered_by[0].between,
        ("sub:a".into(), "sub:b".into()),
        "the lifted pair must name the sub:* containers"
    );
    assert!(
        !r.scope_note.contains("reached no"),
        "nothing was unreachable, so nothing may be reported dropped: {}",
        r.scope_note
    );
}

#[test]
fn a_dropped_pair_is_reported_as_a_pair_not_as_an_endpoint() {
    // `unreachable` increments once per PAIR and accumulates across BOTH the
    // coupling set and the contract set. It said "endpoint(s)" until
    // 2026-08-30, which reads as a count of orphaned components and sends a
    // reader looking for ones that do not exist.
    let g = two_subsystems();
    let r = g.seam_coverage(Some("system")).unwrap();
    assert!(
        r.scope_note.contains("coupling/contract pair(s)"),
        "a pair count must not be labelled an endpoint count: {}",
        r.scope_note
    );
}
