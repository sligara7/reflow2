//! Graph report (SYNTHESIZE) — aggregates the deterministic analyses into one
//! "what should I look at?" artifact.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, Dimension};

fn dep(g: &mut DesignGraph, from: &str, to: &str, w: f64) {
    g.create_edge(
        edge::DEPENDS_ON,
        node::CAPABILITY,
        from,
        node::CAPABILITY,
        to,
        Props::new().set("weight", w),
    )
    .unwrap();
}

#[test]
fn report_aggregates_every_analysis_and_renders_markdown() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // Intent + two capability clusters allocated to two components.
    g.create_node(
        node::REQUIREMENT,
        "req:r",
        Props::new()
            .set("name", "R")
            .set("statement", "s")
            .set("status", "accepted"),
    )
    .unwrap();
    for c in ["cap:a1", "cap:a2", "cap:b1", "cap:b2"] {
        g.add_capability(c, c, "does a thing", None).unwrap();
    }
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();
    g.satisfies("cap:a1", "req:r").unwrap();
    for (c, comp) in [
        ("cap:a1", "cmp:a"),
        ("cap:a2", "cmp:a"),
        ("cap:b1", "cmp:b"),
        ("cap:b2", "cmp:b"),
    ] {
        g.allocate(c, comp).unwrap();
    }
    dep(&mut g, "cap:a1", "cap:a2", 0.9);
    dep(&mut g, "cap:b1", "cap:b2", 0.9);
    dep(&mut g, "cap:a1", "cap:b1", 0.1); // the surprising bridge
    // A declining quality dimension.
    g.add_dimension_observation(
        "o1",
        node::COMPONENT,
        "cmp:a",
        Dimension::Maintainability,
        0.9,
        "e01",
        None,
    )
    .unwrap();
    g.add_dimension_observation(
        "o2",
        node::COMPONENT,
        "cmp:a",
        Dimension::Maintainability,
        0.5,
        "e02",
        None,
    )
    .unwrap();

    let r = g.graph_report().unwrap();

    // Snapshot.
    assert!(r.node_counts.contains(&(node::CAPABILITY, 4)));
    assert!(r.node_counts.contains(&(node::COMPONENT, 2)));
    assert!(r.total_nodes >= 7);

    // Every analysis is represented.
    assert!(r.gap_count > 0 && !r.top_gaps.is_empty());
    let alloc = r.allocation.as_ref().expect("components exist");
    assert_eq!(alloc.component_count, 2);
    assert!(alloc.modularity > 0.9);
    assert_eq!(r.surprising.len(), 1);
    assert_eq!(r.surprising[0].from_id, "cap:a1");
    assert_eq!(r.surprising[0].to_id, "cap:b1");
    assert_eq!(r.declining.len(), 1);
    assert_eq!(r.declining[0].target_id, "cmp:a");
    assert_eq!(r.declining[0].dimension, Dimension::Maintainability);

    // Markdown renders each section.
    let md = r.to_markdown();
    for section in [
        "# Design graph report",
        "## Snapshot",
        "## Top gaps",
        "## Allocation health",
        "## Surprising couplings",
        "## Quality drift",
    ] {
        assert!(md.contains(section), "missing section: {section}");
    }
    assert!(md.contains("cmp:a"));
    assert!(md.contains("maintainability"));
}

#[test]
fn an_empty_graph_reports_empty() {
    let g = DesignGraph::open_in_memory().unwrap();
    let r = g.graph_report().unwrap();
    assert_eq!(r.total_nodes, 0);
    assert_eq!(r.gap_count, 0);
    assert!(r.allocation.is_none());
    assert!(r.to_markdown().contains("Empty graph"));
}
