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
