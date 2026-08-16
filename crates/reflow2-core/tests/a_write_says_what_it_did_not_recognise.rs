//! The schema does not describe everything a project may want to record, and
//! that is deliberate — the store is a property BAG so a design can hold what
//! reflow2 never anticipated. What was missing is that a caller could not tell
//! an EXTENSION from a TYPO, because the reply was identical either way.
//!
//! MEASURED 2026-08-16: `enforcement: "advisory"` was written to a DesignRule,
//! accepted, stored, and echoed back. The schema declares no such property —
//! the real field is `enforced`, a bool — so the write succeeded and meant
//! nothing. Only an unrelated gap firing exposed it.
//!
//! THE ASYMMETRY IS THE ARGUMENT: an unknown TOOL ARGUMENT is refused in 134
//! places across the served surface, and an EDGE to a missing node is refused
//! through sixteen typed helpers with atomicity and a size floor. The props
//! bag — exactly where a property name lands — said nothing at all.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, node};
use std::collections::HashMap;

fn props(pairs: &[(&str, &str)]) -> HashMap<String, reflow2_core::Value> {
    let mut p = Props::new();
    for &(k, v) in pairs {
        p = p.set(k, v);
    }
    p.into()
}

/// THE CASE, and it is the exact write that exposed this.
#[test]
fn an_undeclared_property_is_named() {
    let g = DesignGraph::open_in_memory().unwrap();
    let found = g.undeclared_properties(
        node::DESIGN_RULE,
        &props(&[
            ("name", "A rule"),
            ("statement", "it holds"),
            ("enforcement", "advisory"),
        ]),
    );
    assert_eq!(
        found,
        vec!["enforcement".to_string()],
        "the invented field must be named; `enforced` is the real one"
    );
}

/// COUNTERWEIGHT, and the one that decides whether this is a report or noise:
/// an ordinary write says NOTHING. A signal that fires on correct work is one
/// people stop reading, which is the failure mode this project keeps naming.
#[test]
fn a_write_using_only_declared_properties_is_silent() {
    let g = DesignGraph::open_in_memory().unwrap();
    let found = g.undeclared_properties(
        node::DESIGN_RULE,
        &props(&[("name", "A rule"), ("statement", "it holds")]),
    );
    assert!(found.is_empty(), "{found:?}");
}

/// The real field must not be mistaken for the invented one — this is the pair
/// that is one edit apart, and reporting `enforced` would make the check
/// actively misleading.
#[test]
fn the_correctly_spelled_property_is_not_reported() {
    let g = DesignGraph::open_in_memory().unwrap();
    let found = g.undeclared_properties(
        node::DESIGN_RULE,
        &props(&[("name", "A rule"), ("statement", "it holds")]),
    );
    assert!(!found.contains(&"enforced".to_string()), "{found:?}");
}

/// COUNTERWEIGHT 2: an unknown NODE TYPE reports nothing rather than every key.
/// That case is already refused loudly by the write itself, and answering "all
/// of them are undeclared" would bury the real error under noise.
#[test]
fn an_unknown_node_type_does_not_report_every_property() {
    let g = DesignGraph::open_in_memory().unwrap();
    let found = g.undeclared_properties("NotAType", &props(&[("anything", "at all")]));
    assert!(
        found.is_empty(),
        "an unknown type is the write's error to report, not this one's: {found:?}"
    );
}

/// Several unrecognised names come back together and in a stable order, so the
/// reply is diffable and a caller fixing them does not need several rounds.
#[test]
fn every_unrecognised_name_is_returned_sorted() {
    let g = DesignGraph::open_in_memory().unwrap();
    let found = g.undeclared_properties(
        node::DESIGN_RULE,
        &props(&[
            ("statement", "it holds"),
            ("zebra", "1"),
            ("name", "A rule"),
            ("alpha", "2"),
        ]),
    );
    assert_eq!(found, vec!["alpha".to_string(), "zebra".to_string()]);
}

/// AND THE WRITE STILL SUCCEEDS. This reports; it never refuses. The property
/// bag is a capability, not an oversight, and a fix that started rejecting
/// unknown keys would break the thing it was meant to make honest.
#[test]
fn the_undeclared_property_is_still_stored() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.create_node(
        node::DESIGN_RULE,
        "rule:x",
        props(&[
            ("name", "A rule"),
            ("statement", "it holds"),
            ("enforcement", "advisory"),
        ]),
    )
    .unwrap();

    let stored = g.get_node(node::DESIGN_RULE, "rule:x").unwrap().unwrap();
    assert_eq!(
        stored
            .properties
            .get("enforcement")
            .and_then(|v| v.as_str()),
        Some("advisory"),
        "reporting must not become refusing"
    );
}
