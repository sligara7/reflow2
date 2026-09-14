//! Approving a node you authored no longer erases that you authored it.
//!
//! Written from flo2's report of 2026-09-14 (twice in one day): AUTHORED_BY is
//! one edge per (node, contributor) — the role was not in the store's identity
//! for the edge — so the settling call REPLACED the author edge it found there,
//! silently, and the setter reported what it wrote rather than what it
//! displaced. `dec:design-authorship-identity` had promised the edge "is past
//! tense and never changes".
//!
//! Fixed at the cause: `roles` is a SET with a date per role, `authored_by`
//! merges, readers tolerate the legacy single `role`, and legacy edges are
//! normalised on import and on open.

use reflow2_core::graph::{
    DesignGraph, authored_roles, edge_has_role, normalize_authored_by_props,
};
use reflow2_core::nodes::{Props, edge, node};

fn graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_contributor("who:a", "Anthony", None, None, None)
        .expect("contributor");
    g.add_decision("dec:x", "one store", "One store for every branch.", None)
        .expect("decision");
    g
}

fn the_edge(g: &DesignGraph) -> reflow2_core::foundation::store::StoredEdge {
    let edges = g
        .outgoing("dec:x", Some(edge::AUTHORED_BY))
        .expect("outgoing");
    assert_eq!(
        edges.len(),
        1,
        "one edge per (node, contributor): {edges:#?}"
    );
    edges.into_iter().next().unwrap()
}

/// The case it was written from: write it in August, sign it off in September,
/// and both facts survive with their own dates.
#[test]
fn approving_what_you_authored_keeps_both_roles_and_both_dates() {
    let mut g = graph();
    g.authored_by(
        node::DECISION,
        "dec:x",
        "who:a",
        Some("author"),
        Some("2026-08-04"),
    )
    .expect("author");
    g.authored_by(
        node::DECISION,
        "dec:x",
        "who:a",
        Some("approver"),
        Some("2026-09-14"),
    )
    .expect("approve");

    let e = the_edge(&g);
    assert_eq!(authored_roles(&e), vec!["author", "approver"]);
    assert_eq!(
        e.properties.get("authored_at").and_then(|v| v.as_str()),
        Some("2026-08-04")
    );
    assert_eq!(
        e.properties.get("approved_at").and_then(|v| v.as_str()),
        Some("2026-09-14")
    );
    assert!(
        !e.properties.contains_key("role"),
        "the single-valued slot is gone: {e:#?}"
    );
}

/// Order does not matter, and the edge is byte-identical either way — so an
/// export does not churn on which act was recorded first.
#[test]
fn the_roles_are_a_set_in_canonical_order_whatever_the_write_order() {
    let mut a = graph();
    a.authored_by(node::DECISION, "dec:x", "who:a", Some("approver"), None)
        .unwrap();
    a.authored_by(node::DECISION, "dec:x", "who:a", Some("author"), None)
        .unwrap();
    let mut b = graph();
    b.authored_by(node::DECISION, "dec:x", "who:a", Some("author"), None)
        .unwrap();
    b.authored_by(node::DECISION, "dec:x", "who:a", Some("approver"), None)
        .unwrap();
    assert_eq!(the_edge(&a).properties, the_edge(&b).properties);
    assert_eq!(authored_roles(&the_edge(&a)), vec!["author", "approver"]);
}

/// Saying the same thing twice records it once.
#[test]
fn the_same_role_twice_is_one_role() {
    let mut g = graph();
    g.authored_by(
        node::DECISION,
        "dec:x",
        "who:a",
        Some("approver"),
        Some("2026-09-01"),
    )
    .unwrap();
    g.authored_by(
        node::DECISION,
        "dec:x",
        "who:a",
        Some("approver"),
        Some("2026-09-14"),
    )
    .unwrap();
    let e = the_edge(&g);
    assert_eq!(authored_roles(&e), vec!["approver"]);
    assert_eq!(
        e.properties.get("approved_at").and_then(|v| v.as_str()),
        Some("2026-09-14"),
        "a repeated act carries the latest date"
    );
}

/// No role means author, as the old schema default did.
#[test]
fn no_role_means_author() {
    let mut g = graph();
    g.authored_by(node::DECISION, "dec:x", "who:a", None, None)
        .unwrap();
    assert_eq!(authored_roles(&the_edge(&g)), vec!["author"]);
}

/// A role outside the vocabulary is refused, not stored — the enum used to
/// live on the schema property; it now lives at the typed door.
#[test]
fn an_unknown_role_is_refused() {
    let mut g = graph();
    let err = g
        .authored_by(node::DECISION, "dec:x", "who:a", Some("owner"), None)
        .unwrap_err();
    assert!(err.to_string().contains("owner"), "{err}");
    assert!(
        g.outgoing("dec:x", Some(edge::AUTHORED_BY))
            .unwrap()
            .is_empty()
    );
}

/// THE MIGRATION CASE. A legacy edge — single `role`, `acted_at` — written the
/// old way, then approved: the author survives, and the legacy slots are gone.
#[test]
fn a_legacy_author_edge_survives_a_new_approval() {
    let mut g = graph();
    g.create_edge(
        edge::AUTHORED_BY,
        node::DECISION,
        "dec:x",
        node::CONTRIBUTOR,
        "who:a",
        Props::new()
            .set("role", "author")
            .set("acted_at", "2026-08-04"),
    )
    .expect("legacy edge");
    // The reader understands the legacy shape before anything touches it.
    let legacy = the_edge(&g);
    assert!(edge_has_role(&legacy, "author"));
    assert!(!edge_has_role(&legacy, "approver"));

    g.authored_by(
        node::DECISION,
        "dec:x",
        "who:a",
        Some("approver"),
        Some("2026-09-14"),
    )
    .unwrap();
    let e = the_edge(&g);
    assert_eq!(authored_roles(&e), vec!["author", "approver"]);
    assert_eq!(
        e.properties.get("authored_at").and_then(|v| v.as_str()),
        Some("2026-08-04")
    );
    assert_eq!(
        e.properties.get("approved_at").and_then(|v| v.as_str()),
        Some("2026-09-14")
    );
    assert!(!e.properties.contains_key("role") && !e.properties.contains_key("acted_at"));
}

/// The migration itself: legacy edges move to the set shape, once, and a
/// second run finds nothing to do.
#[test]
fn migrate_rewrites_legacy_edges_once_and_is_idempotent() {
    let mut g = graph();
    g.add_requirement("req:y", "req:y", "it holds").unwrap();
    for (n, ty, role) in [
        ("dec:x", node::DECISION, "approver"),
        ("req:y", node::REQUIREMENT, "author"),
    ] {
        g.create_edge(
            edge::AUTHORED_BY,
            ty,
            n,
            node::CONTRIBUTOR,
            "who:a",
            Props::new().set("role", role).set("acted_at", "2026-08-01"),
        )
        .unwrap();
    }
    assert_eq!(g.migrate_authored_by_roles().unwrap(), 2);
    assert_eq!(g.migrate_authored_by_roles().unwrap(), 0, "idempotent");
    let e = the_edge(&g);
    assert_eq!(authored_roles(&e), vec!["approver"]);
    assert_eq!(
        e.properties.get("approved_at").and_then(|v| v.as_str()),
        Some("2026-08-01")
    );
    assert!(!e.properties.contains_key("role"));
}

/// A legacy export is normalised on import, so a consumer's old record loads
/// into the set shape rather than carrying two shapes in one design.
#[test]
fn a_legacy_export_is_normalised_on_import() {
    let mut src = graph();
    src.create_edge(
        edge::AUTHORED_BY,
        node::DECISION,
        "dec:x",
        node::CONTRIBUTOR,
        "who:a",
        Props::new()
            .set("role", "approver")
            .set("acted_at", "2026-08-01"),
    )
    .unwrap();
    let doc = src.export_graph().expect("export");
    // Prove the document really carries the legacy shape.
    let raw = doc
        .edges
        .iter()
        .find(|e| e.edge_type == edge::AUTHORED_BY)
        .unwrap();
    assert!(raw.properties.contains_key("role"));

    let mut dst = DesignGraph::open_in_memory().unwrap();
    dst.import_graph(&doc).expect("import");
    let e = dst
        .outgoing("dec:x", Some(edge::AUTHORED_BY))
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(authored_roles(&e), vec!["approver"]);
    assert_eq!(
        e.properties.get("approved_at").and_then(|v| v.as_str()),
        Some("2026-08-01")
    );
    assert!(!e.properties.contains_key("role"));
}

/// The readers that make the approver signature load-bearing still see it
/// when the set carries both roles.
#[test]
fn an_approver_who_also_authored_still_counts_as_the_approver() {
    let mut g = graph();
    g.authored_by(node::DECISION, "dec:x", "who:a", Some("author"), None)
        .unwrap();
    g.authored_by(node::DECISION, "dec:x", "who:a", Some("approver"), None)
        .unwrap();
    // Left `proposed` on purpose: an assigned open decision is what loop_status
    // reports, and it reports it only off the approver role.
    let status = g.loop_status_for(Some("who:a")).expect("loop_status");
    assert_eq!(status.unsettled_assigned_decisions, 1, "{status:#?}");
}

/// Normalisation never touches an edge already in the set shape.
#[test]
fn normalising_the_set_shape_changes_nothing() {
    let mut props = std::collections::HashMap::new();
    props.insert(
        "roles".to_string(),
        reflow2_core::foundation::core::Value::List(vec![
            reflow2_core::foundation::core::Value::String("author".into()),
        ]),
    );
    let before = props.clone();
    assert!(!normalize_authored_by_props(&mut props));
    assert_eq!(props, before);
}

/// An OLDER binary whose schema still declares `role: default author` injects
/// that default on any write. Found beside a set, it is not a claim — merging
/// it would mint a phantom author on every edge such a binary touched.
#[test]
fn a_stray_legacy_role_beside_the_set_is_dropped_not_merged() {
    use reflow2_core::foundation::core::Value;
    let mut props = std::collections::HashMap::new();
    props.insert(
        "roles".to_string(),
        Value::List(vec![Value::String("approver".into())]),
    );
    props.insert("role".to_string(), Value::String("author".into()));
    props.insert("acted_at".to_string(), Value::String("2026-09-01".into()));
    assert!(
        normalize_authored_by_props(&mut props),
        "there was something to drop"
    );
    assert!(!props.contains_key("role") && !props.contains_key("acted_at"));
    assert_eq!(
        props.get("roles"),
        Some(&Value::List(vec![Value::String("approver".into())])),
        "the set is the record; the stray default did not become an author"
    );
}
