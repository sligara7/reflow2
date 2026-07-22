//! Three-way merge (`merge_designs`) — BL-80, compare's write-side sibling.
//!
//! Replays the case table `dec:merge-conflict-semantics` decides: one-sided
//! changes taken, agreed changes taken, both-sides changes conflicted,
//! delete/modify retained-and-asked, edges symmetric with nodes, conflicts
//! carrying deterministic ids.

use reflow2_core::merge::{ConflictKind, MergeAction, MergeUnit, Source};
use reflow2_core::{DesignGraph, GraphExport, MergeProposal, Value, merge_designs};

fn g() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open in-memory graph")
}

/// A small common ancestor: a project, a requirement at `proposed`, a
/// capability, and the edge between them.
fn seed(g: &mut DesignGraph) {
    g.add_project("proj:demo", "Demo").expect("project");
    g.add_requirement("req:one", "First requirement", "Does the first thing.")
        .expect("requirement");
    g.set_requirement_status("req:one", "proposed")
        .expect("status");
    g.add_capability("cap:one", "First capability", "Does the first thing.", None)
        .expect("capability");
    g.satisfies("cap:one", "req:one").expect("satisfies");
}

fn seeded() -> DesignGraph {
    let mut g = g();
    seed(&mut g);
    g
}

fn ex(g: &DesignGraph) -> GraphExport {
    g.export_graph().expect("export")
}

fn merge(base: &GraphExport, ours: &GraphExport, theirs: &GraphExport) -> MergeProposal {
    merge_designs(base, ours, theirs, "base", "ours", "theirs")
}

/// Inject an edge property directly on an export — the graph helpers do not
/// expose editable edge properties, and the merge is a pure function over
/// exports. Clears the stale hash so no spurious tamper note appears.
fn set_edge_prop(doc: &mut GraphExport, ty: &str, from: &str, to: &str, k: &str, v: &str) {
    for e in &mut doc.edges {
        if e.edge_type == ty && e.from_id == from && e.to_id == to {
            e.properties
                .insert(k.to_string(), Value::String(v.to_string()));
        }
    }
    doc.content_hash = None;
}

fn remove_edge(doc: &mut GraphExport, ty: &str, from: &str, to: &str) {
    doc.edges
        .retain(|e| !(e.edge_type == ty && e.from_id == from && e.to_id == to));
    doc.content_hash = None;
}

#[test]
fn identical_inputs_merge_to_nothing() {
    let base = ex(&seeded());
    let m = merge(&base, &base, &base);
    assert!(m.summary.clean);
    assert_eq!(m.summary.conflicts, 0);
    assert_eq!(m.summary.auto_resolved, 0);
    assert!(m.conflicts.is_empty());
    assert!(m.auto.is_empty());
    // Every node and edge agreed.
    assert_eq!(m.summary.nodes_unchanged, base.nodes.len());
    assert_eq!(m.summary.edges_unchanged, base.edges.len());
    assert!(m.provenance_note.is_none());
}

#[test]
fn one_sided_change_is_taken_cleanly() {
    let base = ex(&seeded());
    let mut o = seeded();
    o.set_requirement_status("req:one", "accepted")
        .expect("status");
    let ours = ex(&o);
    let theirs = base.clone();

    let m = merge(&base, &ours, &theirs);
    assert!(
        m.summary.clean,
        "a one-sided change is a fast-forward, no conflict"
    );
    let a = m
        .auto
        .iter()
        .find(|a| a.property.as_deref() == Some("status"))
        .expect("status resolved");
    assert_eq!(a.unit, MergeUnit::Property);
    assert_eq!(a.action, MergeAction::Take);
    assert_eq!(a.source, Source::Ours);
}

#[test]
fn both_sides_same_change_is_agreed() {
    let base = ex(&seeded());
    let mut o = seeded();
    o.set_requirement_status("req:one", "accepted").expect("o");
    let mut t = seeded();
    t.set_requirement_status("req:one", "accepted").expect("t");

    let m = merge(&base, &ex(&o), &ex(&t));
    assert!(m.summary.clean);
    let a = m
        .auto
        .iter()
        .find(|a| a.property.as_deref() == Some("status"))
        .expect("status resolved");
    assert_eq!(a.source, Source::Agreed);
}

#[test]
fn both_sides_different_change_is_a_property_conflict() {
    let base = ex(&seeded());
    let mut o = seeded();
    o.set_requirement_status("req:one", "accepted").expect("o");
    let mut t = seeded();
    t.set_requirement_status("req:one", "deferred").expect("t");

    let m = merge(&base, &ex(&o), &ex(&t));
    assert!(!m.summary.clean);
    assert_eq!(m.conflicts.len(), 1);
    let c = &m.conflicts[0];
    assert_eq!(c.kind, ConflictKind::Property);
    assert_eq!(c.target, "req:one");
    assert_eq!(c.property.as_deref(), Some("status"));
    assert_eq!(c.base, Some(Value::String("proposed".into())));
    assert_eq!(c.ours, Some(Value::String("accepted".into())));
    assert_eq!(c.theirs, Some(Value::String("deferred".into())));
    assert!(c.id.starts_with("merge:"), "deterministic conflict id");
    assert!(c.question.contains("status"));
}

#[test]
fn delete_versus_modify_retains_and_asks() {
    // theirs deletes cap:one; ours changes its description. The node is kept,
    // and a conflict asks — deletion must be re-justified.
    let base = ex(&seeded());
    let mut o = seeded();
    o.add_capability(
        "cap:one",
        "First capability",
        "A CHANGED description.",
        None,
    )
    .expect("modify");
    let mut t = seeded();
    t.delete_node("Capability", "cap:one").expect("delete");

    let m = merge(&base, &ex(&o), &ex(&t));
    let c = m
        .conflicts
        .iter()
        .find(|c| c.kind == ConflictKind::DeleteModify)
        .expect("a delete/modify conflict");
    assert_eq!(c.target, "cap:one");
    assert_eq!(c.retained, Some(Source::Ours), "ours kept the changed node");
    assert!(c.question.to_lowercase().contains("delet"));
}

#[test]
fn delete_versus_modify_is_symmetric() {
    // ours deletes; theirs modifies → retained is theirs.
    let base = ex(&seeded());
    let mut o = seeded();
    o.delete_node("Capability", "cap:one").expect("delete");
    let mut t = seeded();
    t.add_capability(
        "cap:one",
        "First capability",
        "A CHANGED description.",
        None,
    )
    .expect("modify");

    let m = merge(&base, &ex(&o), &ex(&t));
    let c = m
        .conflicts
        .iter()
        .find(|c| c.kind == ConflictKind::DeleteModify)
        .expect("a delete/modify conflict");
    assert_eq!(c.retained, Some(Source::Theirs));
}

#[test]
fn delete_delete_is_clean() {
    let base = ex(&seeded());
    let mut o = seeded();
    o.delete_node("Capability", "cap:one").expect("o del");
    let mut t = seeded();
    t.delete_node("Capability", "cap:one").expect("t del");

    let m = merge(&base, &ex(&o), &ex(&t));
    assert!(
        m.summary.clean,
        "both deleting the same node is not a conflict"
    );
    let a = m
        .auto
        .iter()
        .find(|a| a.unit == MergeUnit::Node && a.target == "cap:one")
        .expect("node removal resolved");
    assert_eq!(a.action, MergeAction::Remove);
    assert_eq!(a.source, Source::Agreed);
}

#[test]
fn one_sided_add_is_taken() {
    let base = ex(&seeded());
    let mut o = seeded();
    o.add_capability("cap:two", "Second capability", "Does a second thing.", None)
        .expect("add");
    let ours = ex(&o);
    let theirs = base.clone();

    let m = merge(&base, &ours, &theirs);
    assert!(m.summary.clean);
    let a = m
        .auto
        .iter()
        .find(|a| a.unit == MergeUnit::Node && a.target == "cap:two")
        .expect("added node resolved");
    assert_eq!(a.action, MergeAction::Add);
    assert_eq!(a.source, Source::Ours);
}

#[test]
fn add_add_same_id_conflicts_only_on_differing_properties() {
    // Both add cap:two, same type and name, different descriptions.
    let base = ex(&seeded());
    let mut o = seeded();
    o.add_capability("cap:two", "Second capability", "Ours description.", None)
        .expect("o add");
    let mut t = seeded();
    t.add_capability("cap:two", "Second capability", "Theirs description.", None)
        .expect("t add");

    let m = merge(&base, &ex(&o), &ex(&t));
    let c = m
        .conflicts
        .iter()
        .find(|c| c.target == "cap:two" && c.property.as_deref() == Some("description"))
        .expect("description conflict");
    assert_eq!(c.kind, ConflictKind::Property);
    // The agreed name is not a conflict.
    assert!(
        !m.conflicts
            .iter()
            .any(|c| c.property.as_deref() == Some("name")),
        "the agreed name property must not conflict"
    );
}

#[test]
fn add_add_different_types_is_a_node_type_conflict() {
    let base = ex(&seeded());
    let mut o = seeded();
    o.add_capability("dup:x", "A capability", "As a capability.", None)
        .expect("o add cap");
    let mut t = seeded();
    t.add_component("dup:x", "A component", "Serves as a component.", None)
        .expect("t add cmp");

    let m = merge(&base, &ex(&o), &ex(&t));
    let c = m
        .conflicts
        .iter()
        .find(|c| c.target == "dup:x")
        .expect("a conflict on the duplicated id");
    assert_eq!(c.kind, ConflictKind::NodeType);
    assert_eq!(c.ours, Some(Value::String("Capability".into())));
    assert_eq!(c.theirs, Some(Value::String("Component".into())));
}

#[test]
fn edges_get_the_same_three_way_rule() {
    // base carries an edge property; ours and theirs change it differently.
    let mut base = ex(&seeded());
    set_edge_prop(&mut base, "SATISFIES", "cap:one", "req:one", "weight", "1");
    let mut ours = base.clone();
    set_edge_prop(&mut ours, "SATISFIES", "cap:one", "req:one", "weight", "2");
    let mut theirs = base.clone();
    set_edge_prop(
        &mut theirs,
        "SATISFIES",
        "cap:one",
        "req:one",
        "weight",
        "3",
    );

    let m = merge(&base, &ours, &theirs);
    let c = m
        .conflicts
        .iter()
        .find(|c| c.kind == ConflictKind::EdgeProperty)
        .expect("edge property conflict");
    assert_eq!(c.property.as_deref(), Some("weight"));
    assert_eq!(c.base, Some(Value::String("1".into())));
}

#[test]
fn edge_remove_versus_modify_retains_and_asks() {
    let mut base = ex(&seeded());
    set_edge_prop(&mut base, "SATISFIES", "cap:one", "req:one", "weight", "1");
    // ours changes the edge; theirs removes it.
    let mut ours = base.clone();
    set_edge_prop(&mut ours, "SATISFIES", "cap:one", "req:one", "weight", "2");
    let mut theirs = base.clone();
    remove_edge(&mut theirs, "SATISFIES", "cap:one", "req:one");

    let m = merge(&base, &ours, &theirs);
    let c = m
        .conflicts
        .iter()
        .find(|c| c.kind == ConflictKind::EdgeRemoveModify)
        .expect("edge remove/modify conflict");
    assert_eq!(c.retained, Some(Source::Ours), "ours kept the changed edge");
}

#[test]
fn one_sided_edge_removal_of_unchanged_is_clean() {
    let base = ex(&seeded());
    // theirs removes the satisfies edge; ours leaves it untouched.
    let ours = base.clone();
    let mut theirs = base.clone();
    remove_edge(&mut theirs, "SATISFIES", "cap:one", "req:one");

    let m = merge(&base, &ours, &theirs);
    assert!(
        m.summary.clean,
        "removing an unchanged edge on one side is clean"
    );
    let a = m
        .auto
        .iter()
        .find(|a| a.unit == MergeUnit::Edge && a.action == MergeAction::Remove)
        .expect("edge removal resolved");
    assert_eq!(a.source, Source::Theirs);
}

#[test]
fn the_same_three_documents_always_merge_identically() {
    // Determinism, including conflict ids — the rerere key must be stable.
    let base = ex(&seeded());
    let mut o = seeded();
    o.set_requirement_status("req:one", "accepted").expect("o");
    let mut t = seeded();
    t.set_requirement_status("req:one", "deferred").expect("t");
    let ours = ex(&o);
    let theirs = ex(&t);

    let first = merge(&base, &ours, &theirs);
    let second = merge(&base, &ours, &theirs);
    assert_eq!(first, second);
    assert_eq!(first.conflicts[0].id, second.conflicts[0].id);
}
