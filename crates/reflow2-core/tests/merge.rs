//! Three-way merge (`merge_designs`) — BL-80, compare's write-side sibling.
//!
//! Replays the case table `dec:merge-conflict-semantics` decides: one-sided
//! changes taken, agreed changes taken, both-sides changes conflicted,
//! delete/modify retained-and-asked, edges symmetric with nodes, conflicts
//! carrying deterministic ids.

use std::collections::BTreeMap;

use reflow2_core::merge::{ConflictKind, MergeAction, MergeError, MergeUnit, Resolution, Source};
use reflow2_core::{DesignGraph, GraphExport, MergeProposal, Value, merge_designs, resolve_merge};

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

// --- The apply rung: resolve conflicts and commit the merged design ---------

fn node_prop(doc: &GraphExport, id: &str, key: &str) -> Option<String> {
    doc.nodes
        .iter()
        .find(|n| n.node_id == id)
        .and_then(|n| n.properties.get(key))
        .and_then(|v| v.as_str().map(str::to_string))
}

fn has_node(doc: &GraphExport, id: &str) -> bool {
    doc.nodes.iter().any(|n| n.node_id == id)
}

fn live_status(g: &DesignGraph, id: &str) -> Option<String> {
    g.get_node("Requirement", id)
        .expect("get_node")
        .and_then(|n| {
            n.properties
                .get("status")
                .and_then(|v| v.as_str().map(str::to_string))
        })
}

#[test]
fn resolve_a_clean_merge_takes_each_side_and_adds() {
    let base = ex(&seeded());
    let mut o = seeded();
    o.set_requirement_status("req:one", "accepted").expect("o");
    let ours = ex(&o);
    let mut t = seeded();
    t.add_capability("cap:two", "Second capability", "Does a second thing.", None)
        .expect("t add");
    let theirs = ex(&t);

    let merged = resolve_merge(&base, &ours, &theirs, &BTreeMap::new()).expect("clean merge");
    assert_eq!(
        node_prop(&merged, "req:one", "status").as_deref(),
        Some("accepted")
    );
    assert!(has_node(&merged, "cap:two"), "theirs' addition is carried");
    // The merged document descends from ours.
    assert_eq!(
        merged.prev_content_hash,
        Some(ours.effective_content_hash())
    );
}

#[test]
fn resolve_a_property_conflict_each_way() {
    let base = ex(&seeded());
    let mut o = seeded();
    o.set_requirement_status("req:one", "accepted").expect("o");
    let ours = ex(&o);
    let mut t = seeded();
    t.set_requirement_status("req:one", "deferred").expect("t");
    let theirs = ex(&t);

    let cid = merge(&base, &ours, &theirs).conflicts[0].id.clone();

    for (choice, expected) in [
        (Resolution::Ours, "accepted"),
        (Resolution::Theirs, "deferred"),
        (Resolution::Base, "proposed"),
    ] {
        let mut res = BTreeMap::new();
        res.insert(cid.clone(), choice);
        let merged = resolve_merge(&base, &ours, &theirs, &res).expect("resolved");
        assert_eq!(
            node_prop(&merged, "req:one", "status").as_deref(),
            Some(expected),
            "resolution {choice:?} takes the {expected} value"
        );
    }
}

#[test]
fn resolve_delete_modify_keeps_or_deletes_on_the_decision() {
    // ours changes cap:one; theirs deletes it → retained is ours.
    let base = ex(&seeded());
    let mut o = seeded();
    o.add_capability("cap:one", "First capability", "CHANGED.", None)
        .expect("o modify");
    let ours = ex(&o);
    let mut t = seeded();
    t.delete_node("Capability", "cap:one").expect("t del");
    let theirs = ex(&t);

    let cid = merge(&base, &ours, &theirs)
        .conflicts
        .iter()
        .find(|c| c.kind == ConflictKind::DeleteModify)
        .expect("delete/modify")
        .id
        .clone();

    // Keep ours' changed node.
    let mut keep = BTreeMap::new();
    keep.insert(cid.clone(), Resolution::Ours);
    let merged = resolve_merge(&base, &ours, &theirs, &keep).expect("keep");
    assert!(has_node(&merged, "cap:one"));
    assert_eq!(
        node_prop(&merged, "cap:one", "description").as_deref(),
        Some("CHANGED.")
    );

    // Accept the deletion.
    let mut drop = BTreeMap::new();
    drop.insert(cid, Resolution::Theirs);
    let merged = resolve_merge(&base, &ours, &theirs, &drop).expect("drop");
    assert!(!has_node(&merged, "cap:one"), "the deletion was accepted");
}

#[test]
fn resolve_refuses_an_unresolved_conflict() {
    let base = ex(&seeded());
    let mut o = seeded();
    o.set_requirement_status("req:one", "accepted").expect("o");
    let mut t = seeded();
    t.set_requirement_status("req:one", "deferred").expect("t");

    let err = resolve_merge(&base, &ex(&o), &ex(&t), &BTreeMap::new()).unwrap_err();
    match err {
        MergeError::Unresolved(ids) => assert!(ids.iter().any(|i| i.starts_with("merge:"))),
        other => panic!("expected Unresolved, got {other:?}"),
    }
}

#[test]
fn resolve_refuses_a_resolution_that_names_no_conflict() {
    // A clean merge plus a bogus resolution — the stale/typo decision is surfaced.
    let base = ex(&seeded());
    let mut o = seeded();
    o.set_requirement_status("req:one", "accepted").expect("o");
    let ours = ex(&o);
    let theirs = base.clone();

    let mut res = BTreeMap::new();
    res.insert("merge:deadbeefdeadbeef".to_string(), Resolution::Ours);
    let err = resolve_merge(&base, &ours, &theirs, &res).unwrap_err();
    match err {
        MergeError::UnknownResolutions(ids) => {
            assert_eq!(ids, vec!["merge:deadbeefdeadbeef".to_string()])
        }
        other => panic!("expected UnknownResolutions, got {other:?}"),
    }
}

#[test]
fn apply_merge_commits_the_merge_into_the_live_graph() {
    // ours is the live graph: base + a status change. theirs adds a node and
    // deletes another. A clean merge applied end to end.
    let base = ex(&seeded());
    let mut g = seeded(); // this is ours, live
    g.set_requirement_status("req:one", "accepted")
        .expect("ours");

    let mut t = seeded();
    t.add_capability("cap:two", "Second capability", "Does a second thing.", None)
        .expect("t add");
    t.delete_node("Capability", "cap:one").expect("t del");
    let theirs = ex(&t);

    let report = g
        .apply_merge(&base, &theirs, &BTreeMap::new())
        .expect("clean apply");

    // ours' change survived, theirs' addition landed, theirs' deletion took.
    assert_eq!(live_status(&g, "req:one").as_deref(), Some("accepted"));
    assert!(g.get_node("Capability", "cap:two").expect("get").is_some());
    assert!(g.get_node("Capability", "cap:one").expect("get").is_none());
    assert!(report.nodes_added >= 1, "cap:two added");
    assert!(report.nodes_removed >= 1, "cap:one removed");
    assert!(
        report.edges_removed >= 1,
        "the satisfies edge went with cap:one"
    );
}

#[test]
fn apply_merge_refuses_until_conflicts_are_decided() {
    let base = ex(&seeded());
    let mut g = seeded(); // ours
    g.set_requirement_status("req:one", "accepted")
        .expect("ours");
    let mut t = seeded();
    t.set_requirement_status("req:one", "deferred").expect("t");
    let theirs = ex(&t);

    // No decision → refused, and nothing was written.
    let err = g.apply_merge(&base, &theirs, &BTreeMap::new()).unwrap_err();
    assert!(format!("{err}").contains("unresolved"));
    assert_eq!(
        live_status(&g, "req:one").as_deref(),
        Some("accepted"),
        "ours untouched"
    );

    // Decide theirs → the live graph takes it.
    let cid = {
        let ours = ex(&g);
        merge(&base, &ours, &theirs).conflicts[0].id.clone()
    };
    let mut res = BTreeMap::new();
    res.insert(cid, Resolution::Theirs);
    g.apply_merge(&base, &theirs, &res).expect("resolved apply");
    assert_eq!(live_status(&g, "req:one").as_deref(), Some("deferred"));
}
