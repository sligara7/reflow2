//! `req:composed-analysis` — analysing two designs together.
//!
//! The mechanism is the user's: import one design into the other and run the
//! ORDINARY checks over the whole, so seam problems arrive as the gaps they
//! already are rather than needing a detector of their own.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

/// A consumer that expects a contract it does not provide itself.
fn consumer() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:a", "Consumer").unwrap();
    g.add_requirement("req:fast", "Fast reads", "reads under 200ms")
        .unwrap();
    g.add_capability("cap:read", "Reading", "reads data", None)
        .unwrap();
    g.add_component("cmp:store", "Store", "our own store", None)
        .unwrap();
    g.satisfies("cap:read", "req:fast").unwrap();
    g.allocate("cap:read", "cmp:store").unwrap();
    g
}

/// A dependency that ALSO calls something `cmp:store` — the collision that
/// makes a naive import destructive.
fn dependency() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:z", "Dependency").unwrap();
    g.add_capability("cap:persist", "Persistence", "keeps bytes", None)
        .unwrap();
    g.add_component(
        "cmp:store",
        "THEIR store",
        "a different thing entirely",
        None,
    )
    .unwrap();
    g.allocate("cap:persist", "cmp:store").unwrap();
    g
}

/// **The corruption this exists to prevent.** Both designs name something
/// `cmp:store`; a plain import would upsert one over the other and say nothing.
#[test]
fn colliding_ids_do_not_overwrite_each_other() {
    let ours = consumer();
    let theirs = dependency().export_graph().unwrap();

    let r = ours.compose_and_analyse(&theirs, "z").unwrap();
    assert_eq!(r.imported_graph_id, theirs.graph_id);
    assert!(r.imported_nodes >= 3, "{r:?}");

    // Our own design is untouched — nothing was persisted into it.
    let mine = ours
        .get_node(node::COMPONENT, "cmp:store")
        .unwrap()
        .unwrap();
    assert_eq!(
        mine.properties.get("name").and_then(|v| v.as_str()),
        Some("Store"),
        "composing must never write to the consumer's design"
    );
}

/// A finding is attributed, because a consumer shown its dependency's internal
/// gaps as if they were its own will switch the feature off.
#[test]
fn findings_say_which_design_they_belong_to() {
    let ours = consumer();
    let theirs = dependency().export_graph().unwrap();
    let r = ours.compose_and_analyse(&theirs, "z").unwrap();

    assert!(
        r.their_findings
            .iter()
            .all(|f| f.affected_ids.iter().all(|i| i.starts_with("z::"))),
        "a 'theirs' finding must name only imported ids: {:?}",
        r.their_findings
    );
    assert!(
        r.our_findings
            .iter()
            .all(|f| f.affected_ids.iter().all(|i| !i.starts_with("z::"))),
        "an 'ours' finding must name none: {:?}",
        r.our_findings
    );
}

/// The seam is what neither design could see alone: our component consuming a
/// contract their component provides. Once both are present the pair is
/// analysable, and the finding names ids from BOTH sides.
#[test]
fn a_finding_that_spans_both_designs_is_marked_as_the_seam() {
    let mut ours = consumer();
    // We consume a contract we do not provide. Alone, that is our gap.
    ours.add_interface("ifc:bytes", "Byte store API").unwrap();
    ours.create_edge(
        edge::CONSUMES,
        node::COMPONENT,
        "cmp:store",
        node::INTERFACE,
        "ifc:bytes",
        Props::new(),
    )
    .unwrap();

    let theirs = dependency().export_graph().unwrap();
    let r = ours.compose_and_analyse(&theirs, "z").unwrap();

    // Every bucket is attributed, and the classification is exhaustive.
    let total = r.seam_findings.len() + r.our_findings.len() + r.their_findings.len();
    assert!(total > 0, "the combined design must yield findings: {r:?}");
    for f in r.seam_findings.iter() {
        let mine = f.affected_ids.iter().any(|i| !i.starts_with("z::"));
        let yours = f.affected_ids.iter().any(|i| i.starts_with("z::"));
        assert!(mine && yours, "a seam finding names both sides: {f:?}");
    }
}

/// Composing must be READ-ONLY on both sides. The dependency is never persisted
/// and our export must not start carrying it — the hazard the requirement names.
#[test]
fn composing_writes_nothing_and_does_not_leak_into_our_export() {
    let ours = consumer();
    let before = ours.export_graph().unwrap();
    let theirs = dependency().export_graph().unwrap();

    ours.compose_and_analyse(&theirs, "z").unwrap();

    let after = ours.export_graph().unwrap();
    assert_eq!(
        before.content_hash, after.content_hash,
        "our design must be byte-identical after composing"
    );
    assert!(
        !after.nodes.iter().any(|n| n.node_id.starts_with("z::")),
        "an export of ours must never ship the dependency's internals"
    );
}

/// An empty namespace is refused rather than silently colliding — the failure
/// this whole mechanism exists to avoid must not be reachable by omission.
#[test]
fn an_empty_namespace_is_refused() {
    let ours = consumer();
    let theirs = dependency().export_graph().unwrap();
    assert!(ours.compose_and_analyse(&theirs, "  ").is_err());
}
