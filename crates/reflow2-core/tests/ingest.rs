//! INGEST — freeform input → graph, via the mock LLM backend.
//!
//! Each extraction pass tags its prompt with a `[pass:NAME]` marker, so the
//! scriptable mock returns per-pass canned JSON by matching that marker.

use reflow2_core::nodes::{edge, node};
use reflow2_core::{DesignGraph, IngestOptions, IngestStatus, MockLlmBackend};

const BRIEF: &str = "Build a widget that serves reads fast and works offline.";

/// A mock scripted for a full, clean extraction.
fn full_mock() -> MockLlmBackend {
    MockLlmBackend::new()
        .on_contains(
            "[pass:project_intent]",
            r#"{"project":{"id":"proj:w","name":"Widget","objective":"ship it","mode":"flexible"}}"#,
        )
        .on_contains(
            "[pass:requirements]",
            r#"{"requirements":[{"id":"req:lat","name":"Latency","statement":"under 200ms","priority":"high"}]}"#,
        )
        .on_contains(
            "[pass:constraints]",
            r#"{"constraints":[{"id":"con:off","name":"Offline","statement":"no network","category":"operational"}]}"#,
        )
        .on_contains(
            "[pass:capabilities]",
            r#"{"capabilities":[{"id":"cap:cache","name":"Caching","description":"serve reads on-device"}]}"#,
        )
        .on_contains(
            "[pass:discovery]",
            r#"{"components":true,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains(
            "[pass:components]",
            r#"{"components":[{"id":"cmp:store","name":"Store","purpose":"kv store","allocated_capability_ids":["cap:cache"]}]}"#,
        )
        .on_contains(
            "[pass:satisfies]",
            r#"{"satisfies":[{"capability_id":"cap:cache","requirement_id":"req:lat"}]}"#,
        )
}

#[test]
fn full_ingest_builds_a_golden_thread_from_text() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(BRIEF, &IngestOptions::default(), &full_mock())
        .unwrap();

    assert_eq!(report.status, IngestStatus::Ok, "clean run: {report:?}");
    assert!(report.pass_errors.is_empty());
    assert!(report.dropped_edges.is_empty());

    // Nodes: Fragment + Project + Requirement + Constraint + Capability + Component.
    assert_eq!(g.count_nodes(node::PROJECT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::REQUIREMENT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::CONSTRAINT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), 1);
    assert_eq!(g.count_nodes(node::COMPONENT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::FRAGMENT).unwrap(), 1);
    assert_eq!(report.nodes_created, 6);

    // Edges: the golden thread the passes wired.
    let sat = g.outgoing("cap:cache", Some(edge::SATISFIES)).unwrap();
    assert_eq!(sat.len(), 1);
    assert_eq!(sat[0].to_id, "req:lat");
    let alloc = g.outgoing("cap:cache", Some(edge::ALLOCATED_TO)).unwrap();
    assert_eq!(alloc.len(), 1);
    assert_eq!(alloc[0].to_id, "cmp:store");

    // Provenance: the Fragment YIELDED every created entity (5 non-fragment nodes).
    let yielded = g
        .outgoing(&report.fragment_id, Some(edge::YIELDED))
        .unwrap();
    assert_eq!(yielded.len(), 5);
    assert_eq!(yielded[0].properties["action"].as_str(), Some("created"));

    // Extracted properties survived integration.
    let req = g.get_node(node::REQUIREMENT, "req:lat").unwrap().unwrap();
    assert_eq!(req.properties["priority"].as_str(), Some("high"));
    let proj = g.get_node(node::PROJECT, "proj:w").unwrap().unwrap();
    assert_eq!(proj.properties["mode"].as_str(), Some("flexible"));
}

#[test]
fn discovery_gate_suppresses_phase_two_when_absent() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // Components declared absent: the components pass must not run even though a
    // rule for it exists.
    let mock = MockLlmBackend::new()
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains("[pass:project_intent]", r#"{"project":{"id":"proj:w","name":"W"}}"#)
        .on_contains("[pass:requirements]", r#"{"requirements":[{"id":"req:a","name":"A","statement":"s"}]}"#)
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains("[pass:capabilities]", r#"{"capabilities":[{"id":"cap:a","name":"C","description":"d"}]}"#)
        .on_contains("[pass:satisfies]", r#"{"satisfies":[]}"#)
        .on_contains("[pass:components]", r#"{"components":[{"id":"cmp:x","name":"X","purpose":"p"}]}"#);

    let report = g.ingest(BRIEF, &IngestOptions::default(), &mock).unwrap();

    assert_eq!(g.count_nodes(node::COMPONENT).unwrap(), 0);
    assert!(
        !mock
            .calls()
            .iter()
            .any(|c| c.prompt.contains("[pass:components]")),
        "the components pass must not run when discovery says absent"
    );
    assert_eq!(report.status, IngestStatus::Ok);
}

#[test]
fn a_failed_pass_is_enveloped_and_siblings_survive() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // No `requirements` rule and no default → that pass runs dry and errors,
    // but every other pass still lands.
    let mock = MockLlmBackend::new()
        .on_contains("[pass:project_intent]", r#"{"project":{"id":"proj:w","name":"W"}}"#)
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains("[pass:capabilities]", r#"{"capabilities":[{"id":"cap:a","name":"C","description":"d"}]}"#)
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        );

    let report = g.ingest(BRIEF, &IngestOptions::default(), &mock).unwrap();

    assert_eq!(report.status, IngestStatus::Partial);
    assert!(report.pass_errors.iter().any(|e| e.pass == "requirements"));
    assert_eq!(
        g.count_nodes(node::REQUIREMENT).unwrap(),
        0,
        "failed pass yields nothing"
    );
    assert_eq!(
        g.count_nodes(node::CAPABILITY).unwrap(),
        1,
        "sibling pass survived"
    );
}

#[test]
fn phantom_edge_is_dropped_not_written() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // satisfies references a requirement id that was never created.
    let mock = MockLlmBackend::new()
        .on_contains("[pass:project_intent]", r#"{"project":{"id":"proj:w","name":"W"}}"#)
        .on_contains("[pass:requirements]", r#"{"requirements":[{"id":"req:real","name":"R","statement":"s"}]}"#)
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains("[pass:capabilities]", r#"{"capabilities":[{"id":"cap:a","name":"C","description":"d"}]}"#)
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains("[pass:satisfies]", r#"{"satisfies":[{"capability_id":"cap:a","requirement_id":"req:ghost"}]}"#);

    let report = g.ingest(BRIEF, &IngestOptions::default(), &mock).unwrap();

    assert_eq!(report.status, IngestStatus::Partial);
    assert_eq!(report.dropped_edges.len(), 1);
    let dropped = &report.dropped_edges[0];
    assert_eq!(dropped.edge_type, "SATISFIES");
    assert_eq!(dropped.to_id, "req:ghost");
    assert!(dropped.reason.contains("req:ghost"));
    // No phantom edge was written.
    assert!(
        g.outgoing("cap:a", Some(edge::SATISFIES))
            .unwrap()
            .is_empty()
    );
}
