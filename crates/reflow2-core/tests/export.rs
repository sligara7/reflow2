//! BL-20 — the design as a portable document, and back.

use reflow2_core::nodes::node;
use reflow2_core::{DesignGraph, GraphExport, LinkArtifactOptions};

fn a_design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "Weather station").unwrap();
    g.add_requirement("req:offline", "Offline", "Must work without a network.")
        .unwrap();
    g.add_capability("cap:read", "Read sensors", "polls the sensors")
        .unwrap();
    g.add_component(
        "cmp:node",
        "Outdoor node",
        "the outdoor unit",
        Some("subsystem"),
    )
    .unwrap();
    g.satisfies("cap:read", "req:offline").unwrap();
    g.allocate("cap:read", "cmp:node").unwrap();
    g.contains("proj:p", node::REQUIREMENT, "req:offline")
        .unwrap();
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:read".into(),
        name: "reading.py".into(),
        location: Some("src/reading.py".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:read".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:abc".into()),
    })
    .unwrap();
    g.set_requirement_status("req:offline", "accepted").unwrap();
    g
}

/// The property the whole item rests on: a design survives the round trip.
#[test]
fn a_design_survives_export_and_import() {
    let original = a_design();
    let doc = original.export_graph().unwrap();
    assert!(doc.nodes.len() >= 5 && !doc.edges.is_empty());

    let mut restored = DesignGraph::open_in_memory().unwrap();
    let report = restored.import_graph(&doc).unwrap();
    assert_eq!(report.nodes_written, doc.nodes.len());
    assert_eq!(report.edges_written, doc.edges.len());
    assert!(
        report.skipped_edges.is_empty(),
        "a self-contained document must import whole, got {:?}",
        report.skipped_edges
    );

    // Exporting the restored graph gives the same document back.
    let again = restored.export_graph().unwrap();
    assert_eq!(again.nodes, doc.nodes, "nodes must round-trip exactly");
    assert_eq!(again.edges, doc.edges, "edges must round-trip exactly");

    // And the design still behaves the same — not just the same bytes.
    assert_eq!(
        restored.detect_gaps().unwrap().len(),
        original.detect_gaps().unwrap().len(),
        "a restored design must diagnose the same as the original"
    );
    let req = restored
        .get_node(node::REQUIREMENT, "req:offline")
        .unwrap()
        .unwrap();
    assert_eq!(req.properties["status"].as_str(), Some("accepted"));
    assert_eq!(
        req.properties["statement"].as_str(),
        Some("Must work without a network.")
    );
}

/// Deterministic output is what makes a backup directory diffable rather than a
/// pile of fresh blobs — a `HashMap`'s order is seeded per process, so an
/// unsorted export would rewrite itself every run.
#[test]
fn two_exports_of_an_unchanged_graph_are_byte_identical() {
    let g = a_design();
    let a = serde_json::to_string_pretty(&g.export_graph().unwrap()).unwrap();
    let b = serde_json::to_string_pretty(&g.export_graph().unwrap()).unwrap();
    assert_eq!(a, b);

    // Including across processes — the same graph rebuilt independently.
    let c = serde_json::to_string_pretty(&a_design().export_graph().unwrap()).unwrap();
    assert_eq!(a, c, "an identical design must serialize identically");

    // Property keys are sorted, not hash-ordered.
    let doc = g.export_graph().unwrap();
    let req = doc
        .nodes
        .iter()
        .find(|n| n.node_id == "req:offline")
        .unwrap();
    let keys: Vec<&String> = req.properties.keys().collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn the_export_records_which_reflow2_wrote_it() {
    let doc = a_design().export_graph().unwrap();
    assert!(doc.stamp.node_types >= 27, "{:?}", doc.stamp);
    assert!(!doc.stamp.reflow2_version.is_empty());
}

/// An edge whose endpoints are missing is named, never dropped quietly.
#[test]
fn an_edge_with_a_missing_endpoint_is_reported() {
    let mut doc: GraphExport = a_design().export_graph().unwrap();
    doc.nodes.retain(|n| n.node_id != "cmp:node");

    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g.import_graph(&doc).unwrap();
    assert_eq!(report.skipped_edges.len(), 1, "{:?}", report.skipped_edges);
    assert!(report.skipped_edges[0].contains("cmp:node"));
}

/// A document that fails validation leaves the graph untouched, not half-loaded.
#[test]
fn a_bad_document_imports_nothing() {
    let mut doc = a_design().export_graph().unwrap();
    doc.nodes.push(reflow2_core::ExportedNode {
        node_type: "NotAType".into(),
        node_id: "x:1".into(),
        properties: Default::default(),
    });

    let mut g = DesignGraph::open_in_memory().unwrap();
    assert!(
        g.import_graph(&doc).is_err(),
        "an unknown type must fail loud"
    );
    assert_eq!(
        g.export_graph().unwrap().nodes.len(),
        0,
        "a failed import must leave nothing behind"
    );
}
