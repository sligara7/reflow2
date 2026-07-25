//! Composing two designs at an ownership boundary.
//!
//! `dec:nested-graphs` option (c), decided 2026-07-25: a design is its own graph
//! when something is separately owned, released or shared, and designs link by
//! **mirroring** each other's published surface — because an edge cannot cross a
//! store, the schema validates both endpoints.
//!
//! The fixture is the satellite program split the way (c) says to split it: the
//! crosslink terminal and the ground segment are separately owned, so they are
//! separate designs, and they meet at one published contract.
//!
//! Three properties are pinned, and the third is the one that protects someone's
//! afternoon: the mirror carries the coordinate (whose design, which version,
//! when); ordinary local edges then work across the seam; and an id collision is
//! REFUSED rather than resolved, because upsert would otherwise overwrite your
//! design with somebody else's node.

use reflow2_core::nodes::node;
use reflow2_core::{DesignGraph, GraphExport, Value};

/// The space team's design: a terminal publishing an optical crosslink.
fn space_segment() -> GraphExport {
    // Two designs, two names. Until federation every graph claimed the same
    // graph_id, which is why mirroring needs `open_in_memory_as`: a mirror is
    // only meaningful between designs that can tell each other apart.
    let mut g = DesignGraph::open_in_memory_as("space").unwrap();
    g.add_project("proj:space", "Space segment").unwrap();
    g.add_component(
        "cmp:terminal",
        "Crosslink terminal",
        "the flight side",
        None,
    )
    .unwrap();
    g.add_interface("ifc:crosslink", "Optical crosslink")
        .unwrap();
    g.provides("cmp:terminal", "ifc:crosslink").unwrap();
    g.set_interface_designation("ifc:crosslink", "published")
        .unwrap();
    // Internals the ground team has no business seeing.
    g.add_requirement("req:power", "Power budget", "Under 40 W.")
        .unwrap();
    g.export_surface().unwrap().document
}

/// The ground team's design, which will consume that contract.
fn ground_segment() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory_as("ground").unwrap();
    g.add_project("proj:ground", "Ground segment").unwrap();
    g.add_component("cmp:gateway", "Gateway", "the ground side", None)
        .unwrap();
    g
}

#[test]
fn the_mirror_carries_the_coordinate() {
    // Which design, which version, when — the three facts that make a mirror a
    // dated claim about a version rather than an assumed-current truth.
    let surface = space_segment();
    let hash = surface
        .content_hash
        .clone()
        .expect("surfaces fingerprint themselves");
    let mut ground = ground_segment();

    let report = ground.mirror_surface(&surface, Some("2026-07-25")).unwrap();
    assert_eq!(report.mirror_of, "space");
    assert_eq!(report.mirror_content_hash.as_deref(), Some(hash.as_str()));
    assert!(report.collisions.is_empty(), "{report:?}");

    let mirrors = ground.mirrors().unwrap();
    assert_eq!(mirrors.len(), 1, "{mirrors:?}");
    assert_eq!(mirrors[0].project_id, "proj:space");
    assert_eq!(
        mirrors[0].mirror_content_hash.as_deref(),
        Some(hash.as_str())
    );
    assert_eq!(mirrors[0].mirrored_at.as_deref(), Some("2026-07-25"));
}

#[test]
fn mirrored_nodes_are_marked_foreign() {
    // "Imported reference nodes aren't marked foreign" was BL-45's finding. They
    // are now: provenance says how they got here, so nobody edits them believing
    // they are theirs.
    let mut ground = ground_segment();
    ground
        .mirror_surface(&space_segment(), Some("2026-07-25"))
        .unwrap();

    for id in ["ifc:crosslink", "cmp:terminal", "proj:space"] {
        let node = ground
            .scan_nodes(node::INTERFACE)
            .unwrap()
            .into_iter()
            .chain(ground.scan_nodes(node::COMPONENT).unwrap())
            .chain(ground.scan_nodes(node::PROJECT).unwrap())
            .find(|n| n.node_id == id)
            .unwrap_or_else(|| panic!("{id} should have been mirrored"));
        assert_eq!(
            node.properties.get("provenance").and_then(Value::as_str),
            Some("imported"),
            "{id} must say it came from elsewhere"
        );
    }
    // Our own project is untouched by the mirroring.
    let ours = ground
        .get_node(node::PROJECT, "proj:ground")
        .unwrap()
        .unwrap();
    assert!(!ours.properties.contains_key("mirror_of"));
}

#[test]
fn nothing_internal_arrives_with_the_mirror() {
    let mut ground = ground_segment();
    ground.mirror_surface(&space_segment(), None).unwrap();
    assert!(
        ground
            .get_node(node::REQUIREMENT, "req:power")
            .unwrap()
            .is_none(),
        "their power budget is not ours to hold"
    );
}

#[test]
fn the_seam_is_an_ordinary_local_edge() {
    // The whole reason mirroring beats a cross-store reference: once their
    // contract is a local node, our side consumes it with a normal edge, and the
    // golden thread, propagate and every detector work unchanged.
    let mut ground = ground_segment();
    ground.mirror_surface(&space_segment(), None).unwrap();

    ground.consumes("cmp:gateway", "ifc:crosslink").unwrap();

    let radius = ground
        .propagate_from(&["cmp:gateway"], Default::default())
        .unwrap();
    assert!(
        radius.impacted.iter().any(|n| n.node_id == "ifc:crosslink"),
        "impact must cross the seam: {radius:?}"
    );
    assert_eq!(
        radius.boundary_crossings,
        vec!["ifc:crosslink".to_string()],
        "and it must be reported AS a published-boundary crossing, because that is exactly what \
         changing our side of somebody else's contract is"
    );
}

#[test]
fn an_id_collision_is_refused_not_merged() {
    // THE test. import_graph is an upsert, so mirroring a surface whose ids
    // collide would silently overwrite local design with foreign nodes. A
    // collision is reported and skipped: two designs using one id for different
    // things is a naming conversation between owners, not a merge.
    let mut ground = ground_segment();
    // The ground team happens to have its own component with the same id.
    ground
        .add_component("cmp:terminal", "OUR ground terminal", "not theirs", None)
        .unwrap();

    let report = ground.mirror_surface(&space_segment(), None).unwrap();

    assert!(
        report.collisions.contains(&"cmp:terminal".to_string()),
        "the collision must be named: {report:?}"
    );
    assert!(report.note.contains("REFUSED"), "{}", report.note);
    let ours = ground
        .get_node(node::COMPONENT, "cmp:terminal")
        .unwrap()
        .unwrap();
    assert_eq!(
        ours.properties["name"].as_str(),
        Some("OUR ground terminal"),
        "our node survives untouched — this is the afternoon the guard protects"
    );
    assert_ne!(
        ours.properties.get("provenance").and_then(Value::as_str),
        Some("imported"),
        "and it is still ours, not relabelled as theirs"
    );
}

#[test]
fn an_edge_touching_a_collision_is_dropped_rather_than_rewired() {
    // Their PROVIDES edge pointed at their cmp:terminal. Ours is a different
    // thing with the same name, so pointing their edge at our node would
    // fabricate a relationship neither design asserted.
    let mut ground = ground_segment();
    ground
        .add_component("cmp:terminal", "OUR ground terminal", "not theirs", None)
        .unwrap();
    ground.mirror_surface(&space_segment(), None).unwrap();

    let provides = ground.outgoing("cmp:terminal", Some("PROVIDES")).unwrap();
    assert!(
        provides.is_empty(),
        "our component must not have inherited their contract: {provides:?}"
    );
}

#[test]
fn mirroring_a_design_into_itself_is_refused() {
    // Otherwise a filtered copy of your own design would overwrite the full one,
    // which is data loss wearing the clothes of a composition step.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:mine", "Mine").unwrap();
    let own_surface = g.export_surface().unwrap().document;

    let err = g.mirror_surface(&own_surface, None);
    let message = format!("{}", err.unwrap_err());
    assert!(
        message.contains("this graph") && message.contains("import_graph"),
        "the refusal must say why and what to use instead: {message}"
    );
}
