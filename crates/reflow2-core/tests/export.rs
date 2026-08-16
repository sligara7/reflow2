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
        conformance: None,
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
    let stamp = doc
        .stamp
        .as_ref()
        .expect("an export reflow2 writes is stamped");
    assert!(stamp.node_types >= 27, "{stamp:?}");
    assert!(!stamp.reflow2_version.is_empty());
}

/// BL-87: the stamp is the sibling of `content_hash` — a hand-authored or
/// third-party document with no stamp imports, treated as unstamped and
/// reported, never refused. `import_graph` never gates on the stamp, so
/// requiring it at deserialization was pure friction (the BL-83b adopt dogfood
/// hit `missing field stamp` with no hint about the envelope).
#[test]
fn a_stampless_document_imports_and_is_reported_unstamped() {
    // Round-trip through JSON with the stamp stripped, exactly as a client that
    // hand-authored the envelope would send it.
    let doc = a_design().export_graph().unwrap();
    let mut value = serde_json::to_value(&doc).unwrap();
    value.as_object_mut().unwrap().remove("stamp");
    let stampless: GraphExport = serde_json::from_value(value).unwrap();
    assert!(stampless.is_unstamped());
    assert_eq!(stampless.reflow2_version(), "unstamped");

    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g.import_graph(&stampless).unwrap();
    assert_eq!(
        report.nodes_written,
        doc.nodes.len(),
        "the design still loads"
    );
    let note = report
        .provenance_note
        .expect("an unstamped import must be reported, not silent");
    assert!(note.contains("no stamp"), "{note}");

    // A stamped document imports with no provenance note.
    let mut g2 = DesignGraph::open_in_memory().unwrap();
    assert!(
        g2.import_graph(&doc).unwrap().provenance_note.is_none(),
        "a stamped document is not flagged"
    );
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
    relabelled.stamp.as_mut().unwrap().reflow2_version = "9.9.9".into();
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

// ---- The import_graph cluster: BL-117 / BL-118 / BL-138 --------------------
//
// All three come from real adopt passes by people who are not us, following the
// `adopt` skill's central instruction — *"build one export document and
// `import_graph` it once"* — and finding that the door the skill sends them to
// does not describe itself, refuses what the skill produces, and reports one
// fault per round trip.

/// BL-138. The skill's own instruction, followed literally: nodes and edges and
/// nothing else. It used to fail on `missing field 'graph_id'` — a field the
/// server already knows and `import_graph` never reads.
#[test]
fn a_hand_authored_document_needs_no_graph_id_or_stamp() {
    let doc: GraphExport = serde_json::from_str(
        r#"{"nodes":[{"node_type":"Requirement","node_id":"req:hand",
             "properties":{"name":"Hand-authored","statement":"Written by an agent"}}],
            "edges":[]}"#,
    )
    .expect("a minimal {nodes, edges} document must deserialize");
    assert!(doc.is_unidentified());
    assert!(doc.is_unstamped());

    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g.import_graph(&doc).expect("and it must import");
    assert_eq!(report.nodes_written, 1);
    assert!(
        g.get_node(node::REQUIREMENT, "req:hand").unwrap().is_some(),
        "the node actually landed"
    );
}

/// The edge list is optional too — a first adopt pass is usually nodes only.
#[test]
fn a_document_may_omit_the_edge_list_entirely() {
    let doc: GraphExport = serde_json::from_str(
        r#"{"nodes":[{"node_type":"Requirement","node_id":"req:only",
             "properties":{"name":"Only","statement":"No edges yet"}}]}"#,
    )
    .expect("omitting `edges` must not be a refusal");
    let mut g = DesignGraph::open_in_memory().unwrap();
    assert_eq!(g.import_graph(&doc).unwrap().nodes_written, 1);
}

/// BL-138's COUNTERWEIGHT, and the reason this is not simply "drop the field".
/// `mirror_surface` genuinely needs to know where a surface came from — it
/// records `mirror_of` and guards against mirroring a design into itself — so
/// it must still refuse an unidentified document, by name.
#[test]
fn mirror_surface_still_refuses_a_document_that_names_no_source() {
    let doc: GraphExport = serde_json::from_str(
        r#"{"nodes":[{"node_type":"Requirement","node_id":"req:x",
             "properties":{"name":"X","statement":"S"}}],"edges":[]}"#,
    )
    .unwrap();
    let mut g = DesignGraph::open_in_memory().unwrap();
    let err = g
        .mirror_surface(&doc, None)
        .expect_err("mirroring cannot record a provenance it was never given");
    let msg = err.to_string();
    assert!(msg.contains("graph_id"), "{msg}");
    assert!(
        msg.contains("import_graph"),
        "the message must name the operation that DOES accept it: {msg}"
    );
}

/// BL-118. Four faults used to cost four full edit-retry cycles, because
/// validation stopped at the first. Now one response names every one of them,
/// with its position, and still writes nothing.
#[test]
fn every_fault_in_the_document_is_reported_in_one_response() {
    let doc: GraphExport = serde_json::from_str(
        r#"{"nodes":[
             {"node_type":"Requirement","node_id":"req:ok",
              "properties":{"name":"Fine","statement":"Valid"}},
             {"node_type":"Requirement","node_id":"req:bad-status",
              "properties":{"name":"Bad","statement":"S","status":"not-a-status"}},
             {"node_type":"NoSuchType","node_id":"x:1","properties":{}},
             {"node_type":"Requirement","node_id":"req:missing-statement",
              "properties":{"name":"No statement"}}
           ],"edges":[]}"#,
    )
    .unwrap();
    let mut g = DesignGraph::open_in_memory().unwrap();
    let err = g.import_graph(&doc).expect_err("the document is invalid");
    let msg = err.to_string();

    // Every fault, not just the first — the whole point of the row.
    for expected in ["req:bad-status", "x:1", "req:missing-statement"] {
        assert!(msg.contains(expected), "missing {expected} from:\n{msg}");
    }
    // Positional, so a 9,000-line document can be navigated.
    assert!(
        msg.contains("nodes[1]") && msg.contains("nodes[3]"),
        "{msg}"
    );

    // ATOMICITY UNTOUCHED — the half the row said must not be touched, and the
    // half the reporting session praised. The VALID node must not have landed.
    assert!(
        g.get_node(node::REQUIREMENT, "req:ok").unwrap().is_none(),
        "a rejected import writes nothing at all"
    );
}

/// A bad EDGE is collected the same way, and reported with its own index.
#[test]
fn edge_faults_are_collected_beside_node_faults() {
    let doc: GraphExport = serde_json::from_str(
        r#"{"nodes":[
             {"node_type":"Requirement","node_id":"req:a",
              "properties":{"name":"A","statement":"S"}},
             {"node_type":"Requirement","node_id":"req:b",
              "properties":{"name":"B","statement":"S"}}
           ],
           "edges":[{"edge_type":"NO_SUCH_EDGE","from_id":"req:a","to_id":"req:b"}]}"#,
    )
    .unwrap();
    let mut g = DesignGraph::open_in_memory().unwrap();
    let msg = g.import_graph(&doc).expect_err("bad edge type").to_string();
    assert!(msg.contains("edges[0]"), "{msg}");
    assert!(msg.contains("NO_SUCH_EDGE"), "{msg}");
    assert!(
        g.get_node(node::REQUIREMENT, "req:a").unwrap().is_none(),
        "still all-or-nothing"
    );
}

/// The counterweight to BL-118: a VALID document is unaffected, and still
/// commits in one batch.
#[test]
fn a_valid_document_still_imports_whole() {
    let doc: GraphExport = serde_json::from_str(
        r#"{"nodes":[
             {"node_type":"Requirement","node_id":"req:a",
              "properties":{"name":"A","statement":"S"}},
             {"node_type":"Capability","node_id":"cap:a",
              "properties":{"name":"CapA","description":"D"}}
           ],
           "edges":[{"edge_type":"SATISFIES","from_id":"cap:a","to_id":"req:a"}]}"#,
    )
    .unwrap();
    let mut g = DesignGraph::open_in_memory().unwrap();
    let r = g.import_graph(&doc).expect("valid");
    assert_eq!((r.nodes_written, r.edges_written), (2, 1));
    assert!(r.skipped_edges.is_empty());
}
