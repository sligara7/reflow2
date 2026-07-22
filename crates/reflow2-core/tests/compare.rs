//! Design-vs-design comparison (`compare_designs` / `compare_with_base`) —
//! the reconcile family's sibling, BL-71 rung c.
//!
//! The scenario these tests replay is the one that motivated the module: a
//! curated rebuild and an accumulated live graph disagreeing about what the
//! design *is*, caught at the time only by a node count dropping.

use reflow2_core::temporal::ChangeType;
use reflow2_core::{DesignGraph, GraphExport, LIVE_GRAPH_LABEL, compare_designs};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open in-memory graph")
}

/// A small design with one of everything the diff cares about: design nodes,
/// a supporting node, and edges.
fn seed(g: &mut DesignGraph) {
    g.add_project("proj:demo", "Demo").expect("project");
    g.add_requirement(
        "req:one",
        "First requirement",
        "The system does the first thing.",
    )
    .expect("requirement");
    // Status set explicitly so the changed-property assertions compare a
    // present value against a present value, not against a create-time default.
    g.set_requirement_status("req:one", "proposed")
        .expect("status");
    g.add_capability("cap:one", "First capability", "Does the first thing.", None)
        .expect("capability");
    g.satisfies("cap:one", "req:one").expect("satisfies");
    g.add_change_event("chg:seed", "Seeded the design", ChangeType::NewFeature)
        .expect("change event");
}

fn export(g: &DesignGraph) -> GraphExport {
    g.export_graph().expect("export")
}

#[test]
fn identical_documents_report_identical() {
    let mut g = graph();
    seed(&mut g);
    let doc = export(&g);

    let diff = compare_designs(&doc, &doc, "a.json", "b.json");

    assert!(diff.summary.identical);
    assert_eq!(diff.base, "a.json");
    assert_eq!(diff.other, "b.json");
    assert_eq!(diff.summary.nodes_unchanged, doc.nodes.len());
    assert_eq!(diff.summary.edges_unchanged, doc.edges.len());
    assert!(diff.design.added.is_empty());
    assert!(diff.design.removed.is_empty());
    assert!(diff.design.changed.is_empty());
    assert!(diff.supporting.added.is_empty());
    assert!(diff.provenance_note.is_none());
}

#[test]
fn added_removed_and_changed_are_directional_and_banded() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    // The "other" side: one requirement's status moves, one new capability
    // appears, and the change event is gone — mixed design and supporting
    // divergence, like the real 2026-07-21 one.
    g.set_requirement_status("req:one", "accepted")
        .expect("status");
    g.add_capability("cap:two", "Second capability", "Does a second thing.", None)
        .expect("capability");
    g.delete_node("ChangeEvent", "chg:seed")
        .expect("delete change event");
    let other = export(&g);

    let diff = compare_designs(&base, &other, "committed", "session");

    assert!(!diff.summary.identical);

    // Added is what `other` has that `base` does not — and it is design content.
    assert_eq!(diff.summary.design_added, 1);
    assert_eq!(diff.design.added[0].node_id, "cap:two");
    assert_eq!(diff.design.added[0].node_type, "Capability");
    assert_eq!(
        diff.design.added[0].name.as_deref(),
        Some("Second capability")
    );

    // The deleted change event is a supporting-layer removal, not a design one.
    assert_eq!(diff.summary.supporting_removed, 1);
    assert_eq!(diff.supporting.removed[0].node_id, "chg:seed");
    assert_eq!(diff.summary.design_removed, 0);

    // The status move is a property-level finding on the changed node.
    assert_eq!(diff.summary.design_changed, 1);
    let changed = &diff.design.changed[0];
    assert_eq!(changed.node_id, "req:one");
    assert!(changed.retyped_to.is_none());
    let status = changed
        .properties
        .iter()
        .find(|p| p.property == "status")
        .expect("status divergence reported");
    assert_eq!(
        status.base.as_ref().and_then(|v| v.as_str()),
        Some("proposed")
    );
    assert_eq!(
        status.other.as_ref().and_then(|v| v.as_str()),
        Some("accepted")
    );
}

#[test]
fn direction_reverses_with_the_arguments() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);
    g.add_capability("cap:two", "Second capability", "Does a second thing.", None)
        .expect("capability");
    let other = export(&g);

    let forward = compare_designs(&base, &other, "base", "other");
    let backward = compare_designs(&other, &base, "base", "other");

    assert_eq!(forward.summary.design_added, 1);
    assert_eq!(forward.summary.design_removed, 0);
    assert_eq!(backward.summary.design_added, 0);
    assert_eq!(backward.summary.design_removed, 1);
    assert_eq!(backward.design.removed[0].node_id, "cap:two");
}

#[test]
fn edge_divergence_is_reported_by_identity_and_properties() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    g.add_requirement("req:two", "Second requirement", "Another thing.")
        .expect("requirement");
    g.satisfies("cap:one", "req:two").expect("satisfies");
    let other = export(&g);

    let diff = compare_designs(&base, &other, "base", "other");

    // The new SATISFIES edge is added; nothing was removed or changed.
    assert!(
        diff.edges_added
            .iter()
            .any(|e| e.edge_type == "SATISFIES" && e.to_id == "req:two"),
        "new SATISFIES edge reported as added"
    );
    assert_eq!(diff.summary.edges_removed, 0);
    assert_eq!(diff.summary.edges_changed, 0);
}

#[test]
fn absent_and_present_properties_are_distinguished() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    // Give the other side a property the base never had, so the report must
    // distinguish "absent" from "present but different".
    g.upsert_node(
        "Capability",
        "cap:one",
        std::collections::HashMap::from([(
            "owner_claims_built".to_string(),
            reflow2_core::Value::Bool(true),
        )]),
    )
    .expect("upsert");
    let other = export(&g);

    let diff = compare_designs(&base, &other, "base", "other");

    let changed = &diff.design.changed[0];
    let d = changed
        .properties
        .iter()
        .find(|p| p.property == "owner_claims_built")
        .expect("new property reported");
    assert!(d.base.is_none(), "absent on base is None, not a default");
    assert!(d.other.is_some());
}

#[test]
fn compare_with_base_reads_the_live_graph_as_other() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    g.add_capability("cap:two", "Second capability", "Does a second thing.", None)
        .expect("capability");

    let diff = g.compare_with_base(&base, "committed").expect("compare");

    assert_eq!(diff.base, "committed");
    assert_eq!(diff.other, LIVE_GRAPH_LABEL);
    assert_eq!(diff.summary.design_added, 1);
    assert_eq!(diff.design.added[0].node_id, "cap:two");
}

#[test]
fn the_report_is_deterministic() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);
    g.add_capability("cap:two", "Second capability", "Does a second thing.", None)
        .expect("capability");
    g.set_requirement_status("req:one", "accepted")
        .expect("status");
    let other = export(&g);

    let a = compare_designs(&base, &other, "base", "other");
    let b = compare_designs(&base, &other, "base", "other");

    assert_eq!(
        serde_json::to_string(&a).expect("serialize"),
        serde_json::to_string(&b).expect("serialize"),
        "same documents, byte-identical report"
    );
}

#[test]
fn different_graph_ids_are_noted_not_judged() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);
    let mut other = base.clone();
    other.graph_id = "another-design".to_string();

    let diff = compare_designs(&base, &other, "base", "other");

    // Same contents, different graph id: identical nodes/edges, with the
    // context note present.
    assert!(diff.summary.identical);
    let note = diff.provenance_note.expect("graph id difference noted");
    assert!(note.contains("another-design"));
}

// ---- Ancestry through the lineage chain (dec:export-hash-chain) -------------

#[test]
fn ancestry_reads_the_lineage_chain_in_both_directions() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);
    g.add_capability("cap:two", "Second capability", "Does a second thing.", None)
        .expect("capability");
    let mut other = export(&g);
    other.chain_after(&base);

    let diff = compare_designs(&base, &other, "base", "other");
    assert_eq!(diff.ancestry, reflow2_core::DiffAncestry::OtherSucceedsBase);

    let reversed = compare_designs(&other, &base, "base", "other");
    assert_eq!(
        reversed.ancestry,
        reflow2_core::DiffAncestry::BaseSucceedsOther
    );
}

#[test]
fn two_successors_of_one_parent_read_as_siblings() {
    let mut g = graph();
    seed(&mut g);
    let parent = export(&g);

    g.add_capability("cap:two", "Second capability", "Does a second thing.", None)
        .expect("capability");
    let mut left = export(&g);
    left.chain_after(&parent);

    let mut g2 = graph();
    seed(&mut g2);
    g2.add_capability("cap:three", "Third capability", "Does a third thing.", None)
        .expect("capability");
    let mut right = g2.export_graph().expect("export");
    right.chain_after(&parent);

    let diff = compare_designs(&left, &right, "left", "right");
    assert_eq!(
        diff.ancestry,
        reflow2_core::DiffAncestry::SiblingsOfCommonParent,
        "the two-writer fork in its simplest form is named"
    );
}

#[test]
fn unrelated_records_are_unknown_not_guessed() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);
    let other = export(&g);

    let diff = compare_designs(&base, &other, "base", "other");
    assert_eq!(diff.ancestry, reflow2_core::DiffAncestry::Unknown);
}

#[test]
fn a_tampered_side_is_named_in_the_provenance_note() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);
    let mut other = export(&g);
    other.nodes[0]
        .properties
        .insert("name".into(), reflow2_core::Value::from("edited by hand"));

    let diff = compare_designs(&base, &other, "base", "other");
    let note = diff
        .provenance_note
        .expect("tampering is context the reader needs");
    assert!(
        note.contains("other does not match its own content_hash"),
        "{note}"
    );
}
