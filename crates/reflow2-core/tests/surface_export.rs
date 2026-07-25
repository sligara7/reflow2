//! Publishing a surface: the contracts others may rely on, and nothing else.
//!
//! The first piece of `req:design-composes` that every architecture answer needs
//! — whatever composes, it composes through a published boundary rather than by
//! reaching into another system's internals — and the openness half of
//! `req:key-interfaces`.
//!
//! Two things are being pinned, and the second matters more. That internals stay
//! home, obviously. And that the document is HONEST ABOUT BEING PARTIAL: a
//! recipient cannot tell a small design from a heavily filtered one, so a surface
//! that did not say what it withheld would be the silent drop rule 6 forbids,
//! aimed at the person least able to detect it.

use reflow2_core::nodes::node;
use reflow2_core::{DesignGraph, LinkArtifactOptions};

/// Two vehicles talking through a published optical crosslink, with real
/// internals behind it: a requirement, a capability, a decision, an internal
/// contract and the component that serves it.
fn program() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:sat", "Constellation").unwrap();

    for (id, name) in [
        ("cmp:terminal-a", "Terminal A"),
        ("cmp:terminal-b", "Terminal B"),
        ("cmp:pointing", "Pointing control"),
    ] {
        g.add_component(id, name, "part of a crosslink terminal", None)
            .unwrap();
        g.contains("proj:sat", node::COMPONENT, id).unwrap();
    }

    // The published boundary, with a machine-readable contract behind it.
    g.add_interface("ifc:crosslink", "Optical crosslink")
        .unwrap();
    g.provides("cmp:terminal-a", "ifc:crosslink").unwrap();
    g.consumes("cmp:terminal-b", "ifc:crosslink").unwrap();
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:crosslink-icd".into(),
        name: "crosslink.proto".into(),
        location: Some("icd/crosslink.proto".into()),
        artifact_type: Some("spec".into()),
        target_type: node::INTERFACE.into(),
        target_id: "ifc:crosslink".into(),
        fragment_id: None,
        provenance: None,
        completeness: None,
        checksum: None,
    })
    .unwrap();
    g.set_interface_designation("ifc:crosslink", "published")
        .unwrap();

    // Internals: nobody outside is entitled to any of this.
    g.add_interface("ifc:pointing-api", "Pointing command API")
        .unwrap();
    g.provides("cmp:pointing", "ifc:pointing-api").unwrap();
    g.consumes("cmp:terminal-a", "ifc:pointing-api").unwrap();
    g.add_requirement(
        "req:secret-range",
        "Crosslink range",
        "The crosslink must close at 5,000 km.",
    )
    .unwrap();
    g.add_capability("cap:track", "Track the peer", "closed-loop pointing", None)
        .unwrap();
    g.satisfies("cap:track", "req:secret-range").unwrap();
    g.allocate("cap:track", "cmp:pointing").unwrap();
    g.add_decision(
        "dec:optics",
        "Off-the-shelf optics",
        "Buy rather than build.",
        Some("Schedule."),
    )
    .unwrap();
    g
}

#[test]
fn the_surface_carries_the_boundary_its_contract_and_both_sides() {
    let g = program();
    let surface = g.export_surface().unwrap();
    let ids: Vec<&str> = surface
        .document
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();

    assert_eq!(surface.published, vec!["ifc:crosslink".to_string()]);
    for expected in [
        "ifc:crosslink",     // the boundary
        "art:crosslink-icd", // what specifies it — the real ICD
        "cmp:terminal-a",    // the provider
        "cmp:terminal-b",    // the consumer
        "proj:sat",          // whose surface this is
    ] {
        assert!(ids.contains(&expected), "missing {expected} from {ids:?}");
    }
}

#[test]
fn nothing_internal_leaves() {
    let g = program();
    let surface = g.export_surface().unwrap();
    let ids: Vec<&str> = surface
        .document
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();

    for secret in [
        "req:secret-range", // a requirement is intent, not a contract
        "cap:track",        // how we meet it is nobody's business
        "dec:optics",       // least of all why we chose it
        "ifc:pointing-api", // an internal contract stays internal
        "cmp:pointing",     // and so does the part behind it
    ] {
        assert!(
            !ids.contains(&secret),
            "{secret} leaked into the surface: {ids:?}"
        );
    }
    assert!(
        surface.withheld_nodes >= 5,
        "and the withholding is counted: {surface:?}"
    );
}

#[test]
fn the_document_says_it_is_partial() {
    // THE test. A recipient cannot tell a small design from a filtered one, so
    // the count and the reason travel with it.
    let g = program();
    let surface = g.export_surface().unwrap();
    assert!(surface.note.contains("WITHHELD"), "{}", surface.note);
    assert!(
        surface.note.contains(&surface.withheld_nodes.to_string()),
        "the note must carry the number, not just the word: {}",
        surface.note
    );
    assert!(
        surface.note.contains("not a backup"),
        "and must say what this document is NOT, since it looks exactly like an export: {}",
        surface.note
    );
}

#[test]
fn an_undesignated_design_publishes_nothing_and_says_so_loudly() {
    // The dangerous case: an empty surface is indistinguishable from a design
    // with nothing in it, so someone could ship one believing they had shared
    // their boundary. Not refused — "prove I publish nothing" is legitimate — but
    // impossible to mistake.
    let mut g = program();
    g.set_interface_designation("ifc:crosslink", "internal")
        .unwrap();

    let surface = g.export_surface().unwrap();
    assert!(surface.published.is_empty());
    assert!(
        surface.note.starts_with("EMPTY SURFACE"),
        "{}",
        surface.note
    );
    assert!(
        surface.note.contains("set_interface_designation"),
        "and it must say how to fix it: {}",
        surface.note
    );
}

#[test]
fn a_dangling_edge_is_dropped_and_counted() {
    // Every edge to a withheld node goes: an edge pointing at a node the
    // recipient does not have is a phantom, and phantoms are what
    // `import_graph` reports rather than accepts.
    let g = program();
    let surface = g.export_surface().unwrap();
    let ids: Vec<&str> = surface
        .document
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();
    for e in &surface.document.edges {
        assert!(
            ids.contains(&e.from_id.as_str()) && ids.contains(&e.to_id.as_str()),
            "edge {} {} -> {} dangles",
            e.edge_type,
            e.from_id,
            e.to_id
        );
    }
    assert!(surface.withheld_edges > 0, "{surface:?}");
}

#[test]
fn the_surface_does_not_join_the_hash_chain() {
    // A derived view must not read as an ancestor of the full design, or
    // compare_designs would answer "does other descend from base?" with a
    // published surface (dec:export-hash-chain).
    let g = program();
    let surface = g.export_surface().unwrap();
    assert!(
        surface.document.prev_content_hash.is_none(),
        "a surface must not chain"
    );
    assert!(
        surface.document.content_hash.is_some(),
        "but it still fingerprints itself, so a recipient can tell it apart from an edit"
    );
    assert!(
        surface.document.stamp.is_some(),
        "and still says which reflow2 wrote it (req:survives-upgrade)"
    );
}

#[test]
fn the_surface_is_importable_by_the_other_side() {
    // The point of publishing: the recipient loads it into their own design and
    // gets the contract plus who is on each side — and nothing they should not
    // have. This is the composition step, done by hand, that req:design-composes
    // wants automated once its mechanism is decided.
    let g = program();
    let surface = g.export_surface().unwrap();

    let mut theirs = DesignGraph::open_in_memory().unwrap();
    let report = theirs.import_graph(&surface.document).unwrap();
    assert!(
        report.skipped_edges.is_empty(),
        "a surface must import cleanly, with no phantom edges: {report:?}"
    );
    assert!(
        theirs
            .get_node(node::INTERFACE, "ifc:crosslink")
            .unwrap()
            .is_some(),
        "they have the contract"
    );
    assert!(
        theirs
            .get_node(node::REQUIREMENT, "req:secret-range")
            .unwrap()
            .is_none(),
        "and not our intent"
    );
}
