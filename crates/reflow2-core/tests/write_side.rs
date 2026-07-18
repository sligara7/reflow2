//! Write-side coverage (WS-1..3) — can a user record what DETECT asks for?
//!
//! The audit found reflow2's read side running ahead of its write side:
//! `Verification`, `Release`, `Environment`, `Resource` and `Decision` were
//! counted by DETECT, listed by the report and classified by PROPAGATE, but had
//! no typed constructor. So the system raised gaps demanding exactly those types
//! and offered no way to answer.
//!
//! These tests assert the round trip that matters: the gap fires, the user
//! records the thing it asked for, and the gap closes.

use reflow2_core::LinkArtifactOptions;
use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{edge, node};
use reflow2_core::propagate::{ImpactDirection, PropagateOptions};

/// A design built as far as P3: intent, capability, component, and a real file.
fn built_thread() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_requirement("req:live", "Live scores", "scores update live")
        .expect("req");
    g.add_capability("cap:score", "Scoring", "tracks the score")
        .expect("cap");
    g.add_component("cmp:engine", "Score engine", "computes scores")
        .expect("cmp");
    g.satisfies("cap:score", "req:live").expect("satisfies");
    g.allocate("cap:score", "cmp:engine").expect("allocate");
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:score".into(),
        name: "Score.cs".into(),
        location: Some("src/Score.cs".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:score".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:aaa".into()),
    })
    .expect("link");
    g
}

fn gap_sources(g: &DesignGraph) -> Vec<GapSource> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .map(|c| c.gap_source)
        .collect()
}

// ---- WS-1 · Verification ---------------------------------------------------

#[test]
fn recording_a_verification_closes_the_gap_that_asked_for_one() {
    let mut g = built_thread();
    assert!(
        gap_sources(&g).contains(&GapSource::BuildWithoutVerification),
        "a built design with no checks must raise the gap first"
    );

    g.add_verification("ver:score", "Score unit tests", Some("test"), Some("unit"))
        .expect("verification");
    g.verifies("ver:score", node::CAPABILITY, "cap:score")
        .expect("verifies");

    let after = g.detect_gaps().expect("detect");
    let sources: Vec<GapSource> = after.iter().map(|c| c.gap_source).collect();
    assert!(
        !sources.contains(&GapSource::BuildWithoutVerification),
        "recording a verification must close the phase-level gap, got {sources:?}"
    );

    // The detector covers Capabilities *and* Artifacts: verifying the capability
    // clears it, and leaves the artifact's own coverage gap standing. That is
    // correct — a test proving the capability works does not prove this
    // particular file is the thing that does it.
    let unverified: Vec<&str> = after
        .iter()
        .filter(|c| c.gap_source == GapSource::UnverifiedCapability)
        .flat_map(|c| c.affected_ids.iter().map(String::as_str))
        .collect();
    assert!(
        !unverified.contains(&"cap:score"),
        "the verified capability must no longer be flagged, got {unverified:?}"
    );
    assert_eq!(
        unverified,
        vec!["art:score"],
        "the unverified artifact should still be flagged"
    );
}

#[test]
fn a_failing_check_reaches_the_requirement_behind_it() {
    let mut g = built_thread();
    g.add_verification("ver:score", "Score unit tests", Some("test"), Some("unit"))
        .expect("verification");
    g.verifies("ver:score", node::CAPABILITY, "cap:score")
        .expect("verifies");
    g.set_verification_status("ver:score", "failing", Some("2026-07-18T10:00:00Z"))
        .expect("status");

    // A failing check is a live signal: propagate from it to see what it means.
    let radius = g
        .propagate_from(&["ver:score"], PropagateOptions::default())
        .expect("propagate");
    let reached: Vec<&str> = radius.impacted.iter().map(|n| n.node_id.as_str()).collect();
    assert!(
        reached.contains(&"cap:score") && reached.contains(&"req:live"),
        "a failing check must reach the capability and the requirement, got {reached:?}"
    );
    let cap = radius
        .impacted
        .iter()
        .find(|n| n.node_id == "cap:score")
        .expect("capability impacted");
    assert_eq!(cap.direction, ImpactDirection::Upstream);
}

#[test]
fn setting_status_preserves_the_rest_of_the_check() {
    let mut g = built_thread();
    g.add_verification(
        "ver:score",
        "Score unit tests",
        Some("review"),
        Some("system"),
    )
    .expect("verification");
    let updated = g
        .set_verification_status("ver:score", "passing", None)
        .expect("status");

    assert_eq!(updated.properties["status"].as_str(), Some("passing"));
    assert_eq!(
        updated.properties["method"].as_str(),
        Some("review"),
        "updating the outcome must not erase what the check is"
    );
    assert_eq!(updated.properties["level"].as_str(), Some("system"));
}

#[test]
fn status_on_a_missing_verification_fails_loud() {
    let mut g = built_thread();
    assert!(
        g.set_verification_status("ver:ghost", "passing", None)
            .is_err(),
        "no silent create-on-update"
    );
}

// ---- WS-2 · Release / Environment / Resource -------------------------------

#[test]
fn recording_a_deployment_closes_the_deploy_operate_gap() {
    let mut g = built_thread();
    assert!(
        gap_sources(&g).contains(&GapSource::NoDeployOperate),
        "a design with no operate layer must raise the gap first"
    );

    g.add_release("rel:v1", "Scoreboard v1", Some("1.0.0"), Some("container"))
        .expect("release");
    g.add_environment(
        "env:prod",
        "Production",
        Some("production"),
        Some("us-west"),
    )
    .expect("environment");
    g.deploy_to("rel:v1", "env:prod", Some("active"))
        .expect("deploy");

    assert!(
        !gap_sources(&g).contains(&GapSource::NoDeployOperate),
        "recording a deployment must close the gap"
    );
}

#[test]
fn a_release_carries_its_deployment_status() {
    let mut g = built_thread();
    g.add_release("rel:v1", "Scoreboard v1", Some("1.0.0"), None)
        .expect("release");
    g.add_environment("env:prod", "Production", Some("production"), None)
        .expect("environment");
    g.deploy_to("rel:v1", "env:prod", Some("rolled_back"))
        .expect("deploy");

    let edges = g
        .outgoing("rel:v1", Some(edge::DEPLOYED_TO))
        .expect("outgoing");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].to_id, "env:prod");
    assert_eq!(
        edges[0].properties["status"].as_str(),
        Some("rolled_back"),
        "the declared-vs-actual axis must survive the round trip"
    );
}

#[test]
fn a_resource_dependency_is_recorded_with_its_criticality() {
    let mut g = built_thread();
    g.add_resource("res:db", "Scores database", Some("postgres"))
        .expect("resource");
    g.require_resource(node::COMPONENT, "cmp:engine", "res:db", Some("required"))
        .expect("requires");

    let edges = g
        .outgoing("cmp:engine", Some(edge::REQUIRES_RESOURCE))
        .expect("outgoing");
    assert_eq!(edges.len(), 1);
    assert_eq!(
        edges[0].properties["criticality"].as_str(),
        Some("required")
    );
}

#[test]
fn a_changed_resource_reaches_what_depends_on_it() {
    let mut g = built_thread();
    g.add_resource("res:db", "Scores database", Some("postgres"))
        .expect("resource");
    g.require_resource(node::COMPONENT, "cmp:engine", "res:db", Some("required"))
        .expect("requires");

    // Swapping the database should surface the component that needs it.
    let radius = g
        .propagate_from(&["res:db"], PropagateOptions::default())
        .expect("propagate");
    assert!(
        radius.impacted.iter().any(|n| n.node_id == "cmp:engine"),
        "a resource change must reach its dependents"
    );
}

#[test]
fn an_unknown_enum_value_fails_loud_rather_than_defaulting() {
    let mut g = built_thread();
    assert!(
        g.add_environment("env:x", "Nowhere", Some("moon_base"), None)
            .is_err(),
        "an invalid env_type must fail, not silently fall back to the default"
    );
}

// ---- WS-3 · Decision -------------------------------------------------------

#[test]
fn a_decision_can_be_recorded_and_linked_to_what_it_governs() {
    let mut g = built_thread();
    g.add_decision(
        "dec:store",
        "Use Postgres",
        "Scores are stored in Postgres, not in-memory.",
        Some("Scores must survive a restart; in-memory loses them."),
    )
    .expect("decision");
    g.governed_by(node::COMPONENT, "cmp:engine", node::DECISION, "dec:store")
        .expect("governed_by");

    let edges = g
        .outgoing("cmp:engine", Some(edge::GOVERNED_BY))
        .expect("outgoing");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].to_id, "dec:store");

    let stored = g
        .get_node(node::DECISION, "dec:store")
        .expect("get")
        .expect("present");
    assert!(
        stored.properties["rationale"].as_str().is_some(),
        "the reason is the point of recording the decision"
    );
}

#[test]
fn a_decision_reaches_what_it_governs_when_it_changes() {
    let mut g = built_thread();
    g.add_decision(
        "dec:store",
        "Use Postgres",
        "Postgres, not in-memory.",
        None,
    )
    .expect("decision");
    g.governed_by(node::COMPONENT, "cmp:engine", node::DECISION, "dec:store")
        .expect("governed_by");

    let radius = g
        .propagate_from(&["dec:store"], PropagateOptions::default())
        .expect("propagate");
    assert!(
        radius.impacted.iter().any(|n| n.node_id == "cmp:engine"),
        "revisiting a decision must surface what it shaped"
    );
}

#[test]
fn a_decision_without_its_content_fails_loud() {
    let mut g = built_thread();
    // `decision` is a required property; an empty design record is not useful.
    assert!(
        g.create_node(
            node::DECISION,
            "dec:empty",
            reflow2_core::nodes::Props::new().set("name", "Nameless")
        )
        .is_err(),
        "a Decision with no decision must fail loud"
    );
}
