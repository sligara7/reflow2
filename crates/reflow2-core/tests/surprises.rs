//! Surprising-connections detector — coupling edges bridging distant Leiden
//! communities (graph-analysis.md, mined from graphify).

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn cap(g: &mut DesignGraph, id: &str) {
    g.add_capability(id, id, "does a thing").unwrap();
}

fn dep(g: &mut DesignGraph, from: &str, to: &str, weight: f64) {
    g.create_edge(
        edge::DEPENDS_ON,
        node::CAPABILITY,
        from,
        node::CAPABILITY,
        to,
        Props::new().set("weight", weight),
    )
    .unwrap();
}

#[test]
fn a_lone_edge_bridging_two_communities_is_surprising() {
    // Two tightly-coupled triangles joined by a single weak bridge.
    let mut g = DesignGraph::open_in_memory().unwrap();
    for c in ["cap:a1", "cap:a2", "cap:a3", "cap:b1", "cap:b2", "cap:b3"] {
        cap(&mut g, c);
    }
    dep(&mut g, "cap:a1", "cap:a2", 0.9);
    dep(&mut g, "cap:a2", "cap:a3", 0.9);
    dep(&mut g, "cap:a1", "cap:a3", 0.9);
    dep(&mut g, "cap:b1", "cap:b2", 0.9);
    dep(&mut g, "cap:b2", "cap:b3", 0.9);
    dep(&mut g, "cap:b1", "cap:b3", 0.9);
    dep(&mut g, "cap:a1", "cap:b1", 0.1); // the lone bridge

    let s = g.surprising_connections().unwrap();
    assert_eq!(s.len(), 1, "only the bridge is surprising");
    let c = &s[0];
    assert_eq!(c.from_id, "cap:a1");
    assert_eq!(c.to_id, "cap:b1");
    assert_ne!(c.from_community, c.to_community);
    assert!(c.reasons.contains(&"bridges separate communities"));
    assert!(c.reasons.contains(&"sole bridge between these communities"));
}

#[test]
fn a_single_cohesive_cluster_has_no_surprises() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    for c in ["cap:a1", "cap:a2", "cap:a3"] {
        cap(&mut g, c);
    }
    dep(&mut g, "cap:a1", "cap:a2", 0.9);
    dep(&mut g, "cap:a2", "cap:a3", 0.9);
    dep(&mut g, "cap:a1", "cap:a3", 0.9);

    assert!(g.surprising_connections().unwrap().is_empty());
}

#[test]
fn a_peripheral_node_reaching_a_hub_across_communities_is_flagged() {
    // Community A: triangle + a leaf hanging off a1. Community B: a star hub.
    // The leaf bridges to the hub → low-degree node reaches a high-degree one.
    let mut g = DesignGraph::open_in_memory().unwrap();
    for c in [
        "cap:a1", "cap:a2", "cap:a3", "cap:leaf", "cap:hub", "cap:b1", "cap:b2", "cap:b3", "cap:b4",
    ] {
        cap(&mut g, c);
    }
    dep(&mut g, "cap:a1", "cap:a2", 0.9);
    dep(&mut g, "cap:a2", "cap:a3", 0.9);
    dep(&mut g, "cap:a1", "cap:a3", 0.9);
    dep(&mut g, "cap:a1", "cap:leaf", 0.9); // leaf sits in A, low degree
    dep(&mut g, "cap:hub", "cap:b1", 0.9);
    dep(&mut g, "cap:hub", "cap:b2", 0.9);
    dep(&mut g, "cap:hub", "cap:b3", 0.9);
    dep(&mut g, "cap:hub", "cap:b4", 0.9);
    dep(&mut g, "cap:leaf", "cap:hub", 0.1); // peripheral → hub bridge

    let s = g.surprising_connections().unwrap();
    let bridge = s
        .iter()
        .find(|c| c.from_id == "cap:leaf" && c.to_id == "cap:hub")
        .expect("the leaf→hub bridge is a surprising connection");
    assert!(
        bridge.reasons.contains(&"peripheral node reaches a hub"),
        "reasons were {:?}",
        bridge.reasons
    );
}
