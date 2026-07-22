//! BL-20 — the design as a portable document, and back.

use reflow2_core::nodes::node;
use reflow2_core::{DesignGraph, GraphExport, LinkArtifactOptions};

fn a_design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "Weather station").unwrap();
    g.add_requirement("req:offline", "Offline", "Must work without a network.")
        .unwrap();
    g.add_capability("cap:read", "Read sensors", "polls the sensors", None)
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

/// BL-19's backfill half, and the reason export/import is the migration path
/// rather than bespoke per-change code: importing applies the *current* schema's
/// defaults, so a document written before a property existed comes back with it.
///
/// Without this, a schema change leaves mixed-vintage nodes — detectors reading
/// `None` on old ones and a value on new ones, with no error and no marker.
#[test]
fn importing_an_old_document_backfills_new_defaults() {
    // A Requirement as an older reflow2 would have exported it: no `status`,
    // because the field was not being written yet.
    let mut doc = a_design().export_graph().unwrap();
    for n in &mut doc.nodes {
        if n.node_id == "req:offline" {
            n.properties.remove("status");
            n.properties.remove("priority");
        }
    }
    assert!(
        !doc.nodes
            .iter()
            .any(|n| n.node_id == "req:offline" && n.properties.contains_key("status")),
        "the document under test must genuinely lack the field"
    );

    let mut g = DesignGraph::open_in_memory().unwrap();
    g.import_graph(&doc).unwrap();

    let req = g
        .get_node(node::REQUIREMENT, "req:offline")
        .unwrap()
        .unwrap();
    assert_eq!(
        req.properties["status"].as_str(),
        Some("proposed"),
        "an old document must come back with the current schema's default, not a hole"
    );
    assert_eq!(req.properties["priority"].as_str(), Some("medium"));
    assert_eq!(
        req.properties["statement"].as_str(),
        Some("Must work without a network."),
        "and nothing it did carry may be lost in the process"
    );
}

// ---- Content hash + lineage chain (dec:export-hash-chain) -------------------

/// The export fingerprints its own content; the same design fingerprints the
/// same, and any content change moves it.
#[test]
fn the_export_carries_a_verifiable_content_hash() {
    let g = a_design();
    let doc = g.export_graph().unwrap();

    let hash = doc.content_hash.clone().expect("content_hash is set");
    assert!(
        hash.starts_with("sha256:") && hash.len() == 7 + 64,
        "{hash}"
    );
    assert_eq!(doc.verify_content_hash(), Some(true));
    assert_eq!(
        g.export_graph().unwrap().content_hash.unwrap(),
        hash,
        "an unchanged design hashes identically"
    );

    let mut g2 = DesignGraph::open_in_memory().unwrap();
    g2.import_graph(&doc).unwrap();
    g2.add_capability("cap:log", "Log readings", "writes them down", None)
        .unwrap();
    assert_ne!(
        g2.export_graph().unwrap().content_hash.unwrap(),
        hash,
        "a changed design hashes differently"
    );
}

/// The hash covers the design content only — the same design written by a
/// different build (different stamp) or claiming different ancestry must
/// fingerprint identically, because content identity is what the chain and
/// the diff reason about.
#[test]
fn the_content_hash_excludes_stamp_and_chain() {
    let g = a_design();
    let doc = g.export_graph().unwrap();
    let mut relabelled = doc.clone();
    relabelled.stamp.reflow2_version = "9.9.9".into();
    relabelled.prev_content_hash = Some("sha256:0000".into());

    assert_eq!(
        doc.compute_content_hash(),
        relabelled.compute_content_hash()
    );
}

/// Tampering is three-valued: a matching hash verifies, a mismatch is
/// reported, and a document that predates hashing is neither — absence of a
/// hash is not evidence of tampering.
#[test]
fn tampering_and_prehash_documents_are_distinguished() {
    let g = a_design();
    let mut doc = g.export_graph().unwrap();

    doc.nodes[0]
        .properties
        .insert("name".into(), reflow2_core::Value::from("edited by hand"));
    assert_eq!(doc.verify_content_hash(), Some(false));

    let report = DesignGraph::open_in_memory()
        .unwrap()
        .import_graph(&doc)
        .unwrap();
    let note = report
        .integrity_note
        .expect("a tampered document is said loudly");
    assert!(note.contains("content_hash"), "{note}");

    doc.content_hash = None; // pre-hashing document
    assert_eq!(doc.verify_content_hash(), None);
    let report = DesignGraph::open_in_memory()
        .unwrap()
        .import_graph(&doc)
        .unwrap();
    assert!(
        report.integrity_note.is_none(),
        "an unhashed document imports without accusation"
    );
}

/// The chain advances only when content changes — an unchanged design keeps
/// its predecessor's chain, which is what keeps unchanged exports
/// byte-identical.
#[test]
fn the_chain_advances_on_change_and_holds_still_otherwise() {
    let g = a_design();
    let mut first = g.export_graph().unwrap();
    first.prev_content_hash = Some("sha256:ancestor".into());

    // Unchanged content: the successor inherits the predecessor's own chain.
    let mut same = g.export_graph().unwrap();
    same.chain_after(&first);
    assert_eq!(same.prev_content_hash.as_deref(), Some("sha256:ancestor"));

    // Changed content: the chain advances to the predecessor's hash.
    let mut g2 = DesignGraph::open_in_memory().unwrap();
    g2.import_graph(&first).unwrap();
    g2.add_capability("cap:log", "Log readings", "writes them down", None)
        .unwrap();
    let mut changed = g2.export_graph().unwrap();
    changed.chain_after(&first);
    assert_eq!(
        changed.prev_content_hash,
        Some(first.compute_content_hash()),
        "a changed successor names its predecessor"
    );
}

/// A pre-hashing predecessor still has an identity — the chain can grow from
/// a file written before this feature existed.
#[test]
fn the_chain_grows_from_an_unhashed_predecessor() {
    let g = a_design();
    let mut old = g.export_graph().unwrap();
    old.content_hash = None;

    let mut g2 = DesignGraph::open_in_memory().unwrap();
    g2.import_graph(&old).unwrap();
    g2.add_capability("cap:log", "Log readings", "writes them down", None)
        .unwrap();
    let mut new = g2.export_graph().unwrap();
    new.chain_after(&old);
    assert_eq!(
        new.prev_content_hash,
        Some(old.compute_content_hash()),
        "the predecessor's identity is recomputed, not refused"
    );
}
