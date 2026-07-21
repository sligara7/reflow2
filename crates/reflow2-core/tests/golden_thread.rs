//! End-to-end smoke test of the golden thread on an in-memory design graph.
//!
//! Builds the minimal traceability spine and reads it back:
//!
//! ```text
//!   Project ──CONTAINS──▶ Requirement
//!      │                      ▲
//!      ├──CONTAINS──▶ Capability ──SATISFIES──┘
//!      │                  │
//!      │             ALLOCATED_TO
//!      │                  ▼
//!      └──CONTAINS──▶ Component
//! ```
//!
//! This exercises schema-validated node/edge creation, typed constructors, node
//! read-back, and outgoing-edge scans — the primitives every coherence-loop
//! operation is built on.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{edge, node};

#[test]
fn golden_thread_round_trips() {
    let mut g = DesignGraph::open_in_memory().expect("open in-memory graph");

    // --- Intent (P0) ---
    g.add_project("proj:widget", "Widget Platform")
        .expect("create Project");
    g.add_requirement(
        "req:offline",
        "Works offline",
        "The device must operate with no network connection.",
    )
    .expect("create Requirement");

    // --- Function (P1) ---
    g.add_capability(
        "cap:local-cache",
        "Local caching",
        "Persist and serve data entirely on-device.",
        None,
    )
    .expect("create Capability");

    // --- Structure (P2) ---
    g.add_component(
        "cmp:cache-store",
        "Cache Store",
        "On-device key/value store.",
        None,
    )
    .expect("create Component");

    // --- The golden thread ---
    g.contains("proj:widget", node::REQUIREMENT, "req:offline")
        .expect("Project CONTAINS Requirement");
    g.contains("proj:widget", node::CAPABILITY, "cap:local-cache")
        .expect("Project CONTAINS Capability");
    g.contains("proj:widget", node::COMPONENT, "cmp:cache-store")
        .expect("Project CONTAINS Component");
    g.satisfies("cap:local-cache", "req:offline")
        .expect("Capability SATISFIES Requirement");
    g.allocate("cap:local-cache", "cmp:cache-store")
        .expect("Capability ALLOCATED_TO Component");

    // --- Read back: counts ---
    assert_eq!(g.count_nodes(node::PROJECT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::REQUIREMENT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), 1);
    assert_eq!(g.count_nodes(node::COMPONENT).unwrap(), 1);

    // --- Read back: a node's properties survived the round trip ---
    let req = g
        .get_node(node::REQUIREMENT, "req:offline")
        .unwrap()
        .expect("Requirement should exist");
    assert_eq!(req.properties["name"].as_str(), Some("Works offline"));
    assert_eq!(
        req.properties["statement"].as_str(),
        Some("The device must operate with no network connection.")
    );
    // schema default applied on create.
    assert_eq!(req.properties["status"].as_str(), Some("proposed"));

    // --- Read back: the thread is walkable from the Capability ---
    let sat = g
        .outgoing("cap:local-cache", Some(edge::SATISFIES))
        .unwrap();
    assert_eq!(sat.len(), 1);
    assert_eq!(sat[0].to_id, "req:offline");

    let alloc = g
        .outgoing("cap:local-cache", Some(edge::ALLOCATED_TO))
        .unwrap();
    assert_eq!(alloc.len(), 1);
    assert_eq!(alloc[0].to_id, "cmp:cache-store");

    // Project contains all three children.
    let contained = g.outgoing("proj:widget", Some(edge::CONTAINS)).unwrap();
    assert_eq!(contained.len(), 3);
}

#[test]
fn unknown_node_type_fails_loud() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let err = g.create_node("Nonsense", "x:1", reflow2_core::nodes::Props::new());
    assert!(
        err.is_err(),
        "unknown node type must be rejected, not silently accepted"
    );
}

#[test]
fn missing_required_property_fails_loud() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // Requirement requires `statement`; omit it.
    let err = g.create_node(
        node::REQUIREMENT,
        "req:bad",
        reflow2_core::nodes::Props::new().set("name", "No statement"),
    );
    assert!(err.is_err(), "missing required property must be rejected");
}

#[test]
fn integer_literal_is_accepted_for_a_float_property() {
    // BL-50: JSON has one number type, so every client writes `confidence: 1`
    // — the store's strict Value validation refused the bare integer.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:a", "A", "does a", None).unwrap();
    g.add_capability("cap:b", "B", "does b", None).unwrap();

    let e = g
        .create_edge(
            edge::DUPLICATES,
            node::CAPABILITY,
            "cap:a",
            node::CAPABILITY,
            "cap:b",
            reflow2_core::nodes::Props::new().set("confidence", 1i64),
        )
        .expect("an integer literal on a float property must be accepted");
    assert_eq!(
        e.properties.get("confidence"),
        Some(&dynograph_core::Value::Float(1.0)),
        "widened losslessly and stored as the schema's float"
    );

    // Widening must not bypass the schema's range check: confidence is [0, 1].
    let out_of_range = g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:b",
        node::CAPABILITY,
        "cap:a",
        reflow2_core::nodes::Props::new().set("confidence", 2i64),
    );
    assert!(
        out_of_range.is_err(),
        "a widened integer still faces the range check"
    );
}

#[test]
fn a_large_integer_is_not_lossily_widened_to_a_float() {
    // BL-58: near i64::MAX, `(i as f64) as i64` saturates back to i64::MAX, so
    // the old round-trip "exactness" check passed a value f64 cannot hold. A
    // small integer widens; i64::MAX must NOT be silently coerced — it stays an
    // Int and the store's strict validation then rejects it (fail loud).
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:a", "A", "does a", None).unwrap();
    g.add_capability("cap:b", "B", "does b", None).unwrap();

    // A small integer on a no-range float property widens losslessly.
    let ok = g
        .create_edge(
            edge::DEPENDS_ON,
            node::CAPABILITY,
            "cap:a",
            node::CAPABILITY,
            "cap:b",
            reflow2_core::nodes::Props::new().set("data_volume", 1000i64),
        )
        .expect("a small integer widens to a float");
    assert_eq!(
        ok.properties.get("data_volume"),
        Some(&dynograph_core::Value::Float(1000.0))
    );

    // i64::MAX cannot be represented exactly as f64 — it must not be widened,
    // so the strict store rejects the bare integer rather than storing a lie.
    let lossy = g.create_edge(
        edge::DEPENDS_ON,
        node::CAPABILITY,
        "cap:b",
        node::CAPABILITY,
        "cap:a",
        reflow2_core::nodes::Props::new().set("data_volume", i64::MAX),
    );
    assert!(
        lossy.is_err(),
        "i64::MAX must fail loud, not widen to an inexact float"
    );
}
