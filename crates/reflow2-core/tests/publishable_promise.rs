//! A published surface can carry a behavioural promise (`req:publishable-promise`).
//!
//! Found by a real cross-repo trial on 2026-07-28, not by review: the
//! dynograph-foundation session published its offer to the reflow2 session and
//! could not express the one commitment its consumer most needed — that
//! `open_rocksdb` fails loud rather than silently falling back to an in-memory
//! store. `export_surface` withheld every Requirement as internal, behavioural
//! commitments live in Requirements, so the "published surface" carried
//! structure and no promises. The promise survived only as a comment in the
//! CONSUMER's build file, on the wrong side of the seam, where the provider
//! would never see it change.
//!
//! The cases below are the ones that would have caught it, plus the ones that
//! stop the fix from becoming a leak.

use reflow2_core::DesignGraph;

/// A design with one published boundary, one promise about it, and one piece of
/// internal intent that must never travel.
fn fixture() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_project("prj:p", "Provider").unwrap();
    g.add_component("cmp:store", "store", "durable storage", None)
        .unwrap();
    g.add_interface("ifc:store-api", "StorageEngine API")
        .unwrap();
    g.set_interface_designation("ifc:store-api", "published")
        .unwrap();
    g.provides("cmp:store", "ifc:store-api").unwrap();

    // The promise. Exactly the shape the trial found unpublishable.
    g.add_requirement(
        "req:fail-loud",
        "A missing on-disk store fails loud, never falls back to memory",
        "Asked to use on-disk storage without the backend compiled in, the build refuses with a \
         named error rather than silently using an in-memory store. A silent fallback loses a \
         consumer's data with no error at all.",
    )
    .unwrap();

    // Internal intent. Must stay home whatever else happens.
    g.add_requirement(
        "req:internal-plan",
        "Rewrite the cache layer next quarter",
        "Nobody outside this project has any business reading this.",
    )
    .unwrap();
    g
}

#[test]
fn a_designated_promise_travels_with_the_surface() {
    let mut g = fixture();
    g.set_requirement_designation("req:fail-loud", "published")
        .unwrap();

    let surface = g.export_surface().expect("surface");
    let ids: Vec<&str> = surface
        .document
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();

    assert!(
        ids.contains(&"req:fail-loud"),
        "the published promise must be in the surface — carrying it is the whole point: {ids:?}"
    );
    assert!(
        ids.contains(&"ifc:store-api"),
        "the boundary it constrains must still be there: {ids:?}"
    );
}

#[test]
fn an_undesignated_requirement_is_still_withheld() {
    let mut g = fixture();
    g.set_requirement_designation("req:fail-loud", "published")
        .unwrap();

    let surface = g.export_surface().expect("surface");
    let ids: Vec<&str> = surface
        .document
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();

    // The load-bearing half of the fix. If publishing one requirement published
    // them all, this would be a data leak dressed as a feature.
    assert!(
        !ids.contains(&"req:internal-plan"),
        "internal intent must NOT leak just because a sibling requirement was published: {ids:?}"
    );
    assert!(
        surface.withheld_nodes > 0,
        "what stayed home must still be counted, not silently dropped"
    );
}

#[test]
fn nothing_is_published_by_default() {
    // Publishing is a commitment; a requirement written today must not become a
    // promise to a stranger tomorrow because a default was convenient.
    let g = fixture();
    let surface = g.export_surface().expect("surface");
    let ids: Vec<&str> = surface
        .document
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();

    assert!(
        !ids.contains(&"req:fail-loud") && !ids.contains(&"req:internal-plan"),
        "no requirement may be published without someone deliberately saying so: {ids:?}"
    );
}

#[test]
fn a_surface_with_no_promises_says_so_rather_than_going_quiet() {
    // "No promises stated" and "promises you cannot see" must never look alike.
    // This is the same false-green rule the trial turned up: silence must never
    // read as compatibility.
    let g = fixture();
    let surface = g.export_surface().expect("surface");
    let note = surface.note.to_lowercase();

    assert!(
        note.contains("no behavioural promises"),
        "a promise-free surface must SAY it is promise-free: {}",
        surface.note
    );
    assert!(
        note.contains("none stated") && note.contains("none exist"),
        "and must distinguish 'none stated' from 'none exist': {}",
        surface.note
    );
}

#[test]
fn a_surface_with_promises_counts_them() {
    let mut g = fixture();
    g.set_requirement_designation("req:fail-loud", "published")
        .unwrap();

    let surface = g.export_surface().expect("surface");
    assert!(
        surface.note.contains("1 behavioural promise"),
        "the note must count the promises, the way it counts the boundaries: {}",
        surface.note
    );
}

#[test]
fn designation_can_be_taken_back() {
    // A commitment withdrawn is a real act — deprecating a promise has to be
    // possible, or `published` would be a one-way door.
    let mut g = fixture();
    g.set_requirement_designation("req:fail-loud", "published")
        .unwrap();
    g.set_requirement_designation("req:fail-loud", "internal")
        .unwrap();

    let surface = g.export_surface().expect("surface");
    let ids: Vec<&str> = surface
        .document
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();
    assert!(
        !ids.contains(&"req:fail-loud"),
        "a withdrawn promise must stop travelling: {ids:?}"
    );
}

#[test]
fn designating_leaves_the_rest_of_the_requirement_alone() {
    // A spec completed by several people over time must lose nothing.
    let mut g = fixture();
    g.set_requirement_status("req:fail-loud", "accepted")
        .unwrap();
    g.set_requirement_designation("req:fail-loud", "published")
        .unwrap();

    let n = g
        .get_node("Requirement", "req:fail-loud")
        .unwrap()
        .expect("requirement");
    assert_eq!(
        n.properties.get("status").and_then(|v| v.as_str()),
        Some("accepted"),
        "designating must not reset status"
    );
    assert!(
        n.properties
            .get("statement")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("silently")),
        "designating must not lose the statement"
    );
}

#[test]
fn an_invented_designation_is_refused() {
    let mut g = fixture();
    let err = g
        .set_requirement_designation("req:fail-loud", "sort-of-public")
        .expect_err("an invented designation must be refused, not stored");
    let msg = err.to_string();
    assert!(
        msg.contains("internal") && msg.contains("published"),
        "the refusal must name the values that ARE allowed: {msg}"
    );
}

#[test]
fn designating_a_requirement_that_does_not_exist_is_an_error() {
    let mut g = fixture();
    g.set_requirement_designation("req:nope", "published")
        .expect_err("publishing a promise nobody wrote must fail rather than create one");
}
