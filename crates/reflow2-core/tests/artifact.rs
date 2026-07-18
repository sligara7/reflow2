//! Artifact linking — the write side that closes the loop on real code (SP-6).

use reflow2_core::nodes::{edge, node};
use reflow2_core::{DesignGraph, LinkArtifactOptions};

fn graph_with_capability() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:flight", "Ball flight", "Simulate trajectory.")
        .unwrap();
    g
}

#[test]
fn add_artifact_and_realizes_link() {
    let mut g = graph_with_capability();
    g.add_artifact("art:ball", "Ball.cs", Some("code"), Some("src/Ball.cs"))
        .unwrap();
    g.realizes("art:ball", node::CAPABILITY, "cap:flight", Some("partial"))
        .unwrap();

    let art = g.get_node(node::ARTIFACT, "art:ball").unwrap().unwrap();
    assert_eq!(art.properties["artifact_type"].as_str(), Some("code"));
    assert_eq!(art.properties["location"].as_str(), Some("src/Ball.cs"));

    // The capability now has an incoming REALIZES — exactly what DETECT looks for.
    let realizes = g.incoming("cap:flight", Some(edge::REALIZES)).unwrap();
    assert_eq!(realizes.len(), 1);
    assert_eq!(realizes[0].from_id, "art:ball");
    assert_eq!(
        realizes[0].properties["completeness"].as_str(),
        Some("partial")
    );
}

#[test]
fn link_artifact_creates_artifact_fragment_and_edges_with_provenance() {
    let mut g = graph_with_capability();
    let link = g
        .link_artifact(LinkArtifactOptions {
            artifact_id: "art:ball".to_string(),
            name: "Ball.cs".to_string(),
            location: Some("src/Ball.cs".to_string()),
            artifact_type: Some("code".to_string()),
            target_type: node::CAPABILITY.to_string(),
            target_id: "cap:flight".to_string(),
            completeness: None, // → default "complete"
            provenance: None,   // → default "authored"
            fragment_id: None,  // → "frag:art:ball"
            checksum: None,     // → no drift baseline recorded
        })
        .expect("link_artifact");

    assert_eq!(link.completeness, "complete");
    assert_eq!(link.provenance, "authored");
    assert_eq!(link.fragment_id, "frag:art:ball");

    // Artifact exists, REALIZES the capability.
    assert!(g.get_node(node::ARTIFACT, "art:ball").unwrap().is_some());
    assert_eq!(
        g.incoming("cap:flight", Some(edge::REALIZES))
            .unwrap()
            .len(),
        1
    );

    // Provenance lives on the Fragment, which YIELDED the Artifact.
    let frag = g
        .get_node(node::FRAGMENT, "frag:art:ball")
        .unwrap()
        .expect("provenance fragment");
    assert_eq!(frag.properties["provenance"].as_str(), Some("authored"));
    let yielded = g.outgoing("frag:art:ball", Some(edge::YIELDED)).unwrap();
    assert_eq!(yielded.len(), 1);
    assert_eq!(yielded[0].to_id, "art:ball");
}

#[test]
fn link_artifact_fails_loud_on_missing_target() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let err = g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:x".to_string(),
        name: "X".to_string(),
        location: None,
        artifact_type: None,
        target_type: node::CAPABILITY.to_string(),
        target_id: "cap:nope".to_string(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    });
    assert!(
        err.is_err(),
        "linking to a non-existent target must fail loud, not dangle"
    );
    // And it must not have left a stray Artifact behind for the caller to trip on.
    assert!(g.get_node(node::ARTIFACT, "art:x").unwrap().is_none());
}

#[test]
fn artifact_missing_name_fails_loud() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let err = g.create_node(
        node::ARTIFACT,
        "art:bad",
        reflow2_core::nodes::Props::new().set("artifact_type", "code"),
    );
    assert!(err.is_err(), "Artifact requires name; must be rejected");
}
