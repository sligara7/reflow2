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

#[test]
fn a_properly_modelled_contract_is_not_surprising() {
    // Two clusters joined only by a contract. The Interface is declared
    // structure — flagging it would penalise the modelling discipline
    // AGENTS.md asks for. The *components* it couples are still assessed.
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

    g.add_component("cmp:a", "A", "left").unwrap();
    g.add_component("cmp:b", "B", "right").unwrap();
    g.allocate("cap:a1", "cmp:a").unwrap();
    g.allocate("cap:b1", "cmp:b").unwrap();
    g.add_interface("ifc:link", "The contract").unwrap();
    g.provides("cmp:a", "ifc:link").unwrap();
    g.consumes("cmp:b", "ifc:link").unwrap();

    let s = g.surprising_connections().unwrap();
    assert!(
        !s.iter()
            .any(|c| c.from_id.starts_with("ifc:") || c.to_id.starts_with("ifc:")),
        "an Interface must never be reported as a surprising endpoint, got {s:?}"
    );
    // If the two components are genuinely distant, the coupling is reported
    // between *them*, explained by the contract it runs through.
    if let Some(c) = s.iter().find(|c| c.via.is_some()) {
        assert_eq!(c.via.as_deref(), Some("ifc:link"));
        assert!(c.reasons.contains(&"coupled through a shared contract"));
        assert!(c.from_id.starts_with("cmp:") && c.to_id.starts_with("cmp:"));
    }
}

#[test]
fn fragments_are_not_treated_as_parts_of_the_design() {
    // A sparse design where Leiden can only produce tiny communities. Every
    // edge would "bridge communities", which says nothing — so nothing fires.
    let mut g = DesignGraph::open_in_memory().unwrap();
    for c in ["cap:a", "cap:b", "cap:c", "cap:d"] {
        cap(&mut g, c);
    }
    dep(&mut g, "cap:a", "cap:b", 0.5);
    dep(&mut g, "cap:c", "cap:d", 0.5);
    dep(&mut g, "cap:b", "cap:c", 0.5);

    assert!(
        g.surprising_connections().unwrap().is_empty(),
        "pairs are not 'otherwise-distant parts of the design'"
    );
}

#[test]
fn provenance_nodes_stay_out_of_the_topology() {
    // Fragments and DriftEvents describe how the graph came to be, not how the
    // design is shaped. Leaving them in shifts communities and can be reported
    // as couplings in their own right.
    use reflow2_core::LinkArtifactOptions;
    use reflow2_core::drift::{ObservedArtifact, ReconcileOptions};

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
    dep(&mut g, "cap:a1", "cap:b1", 0.1);

    let before = g.surprising_connections().unwrap();

    // Register a file (creates an Artifact + a provenance Fragment) and record
    // drift against it (creates a DriftEvent joined by DEPENDS_ON).
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:a".into(),
        name: "a.rs".into(),
        location: Some("src/a.rs".into()),
        artifact_type: None,
        target_type: node::CAPABILITY.into(),
        target_id: "cap:a1".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:v1".into()),
    })
    .unwrap();
    g.reconcile_artifacts(
        &[ObservedArtifact {
            artifact_id: "art:a".into(),
            present: true,
            checksum: Some("sha256:v2".into()),
        }],
        &ReconcileOptions {
            record_events: true,
            ..Default::default()
        },
    )
    .unwrap();

    let after = g.surprising_connections().unwrap();
    assert_eq!(
        before.len(),
        after.len(),
        "bookkeeping must not change the topology: {before:?} vs {after:?}"
    );
    assert!(
        !after
            .iter()
            .any(|c| c.from_id.starts_with("drift:") || c.to_id.starts_with("drift:")),
        "a DriftEvent is not a design coupling"
    );
}

#[test]
fn detection_is_stable_across_runs_on_an_unchanged_graph() {
    // Guards against instability *within* a process (ordering that depends on
    // iteration of an unsorted collection, floating-point tie-breaks, etc.).
    //
    // It cannot catch the original bug, and that is worth stating: Rust seeds a
    // HashSet's hasher per **process**, so repeated calls inside one test see a
    // consistent order and pass either way. The real regression test for that
    // lives in `tools/smoke_mcp.py`, which runs the binary twice in separate
    // processes and compares. Keep both.
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
    dep(&mut g, "cap:a1", "cap:b1", 0.1);

    let fingerprint = |g: &DesignGraph| -> Vec<String> {
        let mut v: Vec<String> = g
            .surprising_connections()
            .unwrap()
            .iter()
            .map(|c| format!("{}|{}|{}", c.from_id, c.to_id, c.edge_type))
            .collect();
        v.sort();
        v
    };
    let first = fingerprint(&g);
    for _ in 0..8 {
        assert_eq!(
            first,
            fingerprint(&g),
            "same graph must give the same answer"
        );
    }

    let mut gap_ids = |g: &DesignGraph| -> Vec<String> {
        let mut v: Vec<String> = g
            .detect_gaps()
            .unwrap()
            .iter()
            .map(|c| c.id.clone())
            .collect();
        v.sort();
        v
    };
    let first_gaps = gap_ids(&g);
    for _ in 0..8 {
        assert_eq!(
            first_gaps,
            gap_ids(&g),
            "gap ids must be reproducible or a review cannot hold"
        );
    }
}
