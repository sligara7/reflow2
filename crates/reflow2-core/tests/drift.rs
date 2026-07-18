//! As-built drift (SP-6b) — does the design still describe what was actually built?
//!
//! The other half of the failure the original Reflow never solved. `interface.rs`
//! covers "changed one side of a boundary, forgot the other"; this covers "changed
//! the code, and the systems-engineering layer never heard about it".

use reflow2_core::LinkArtifactOptions;
use reflow2_core::drift::{DriftKind, ObservedArtifact, ReconcileOptions};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;
use reflow2_core::propagate::{ImpactDirection, PropagateOptions};

/// A golden thread with one registered artifact carrying a checksum baseline.
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

fn observed(id: &str, present: bool, checksum: Option<&str>) -> ObservedArtifact {
    ObservedArtifact {
        artifact_id: id.into(),
        present,
        checksum: checksum.map(str::to_string),
    }
}

#[test]
fn an_unchanged_artifact_is_not_drift() {
    let mut g = built_thread();
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", true, Some("sha256:aaa"))],
            &ReconcileOptions::default(),
        )
        .expect("reconcile");

    assert!(report.findings.is_empty(), "matching hash is not drift");
    assert_eq!(report.unchanged, 1);
    assert!(report.propagation_seeds.is_empty());
}

#[test]
fn a_changed_file_is_drift_and_names_the_design_it_affects() {
    let mut g = built_thread();
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", true, Some("sha256:bbb"))],
            &ReconcileOptions::default(),
        )
        .expect("reconcile");

    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, DriftKind::ChecksumChange);
    assert_eq!(
        report.findings[0].realizes,
        vec!["cap:score"],
        "drift must name the design node the file realizes"
    );
    assert_eq!(report.propagation_seeds, vec!["cap:score"]);
}

#[test]
fn a_code_change_propagates_back_up_to_the_requirement() {
    // The whole point of SP-6b: an edit made in code reaches the systems-
    // engineering layer that justified it, instead of drifting silently.
    let mut g = built_thread();
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", true, Some("sha256:bbb"))],
            &ReconcileOptions::default(),
        )
        .expect("reconcile");

    let seeds: Vec<&str> = report
        .propagation_seeds
        .iter()
        .map(String::as_str)
        .collect();
    let radius = g
        .propagate_from(&seeds, PropagateOptions::default())
        .expect("propagate");

    let req = radius
        .impacted
        .iter()
        .find(|n| n.node_id == "req:live")
        .expect("the requirement behind the changed code must be reached");
    assert_eq!(
        req.direction,
        ImpactDirection::Upstream,
        "a code change reaches its requirement by walking *up* the thread"
    );
}

#[test]
fn a_recorded_drift_event_propagates_into_the_design() {
    // Seeding from the DriftEvent itself (not its artifact) must also reach the
    // design — DriftEvent →DEPENDS_ON→ Artifact →REALIZES→ Capability.
    let mut g = built_thread();
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", true, Some("sha256:bbb"))],
            &ReconcileOptions {
                record_events: true,
                detected_at: Some("2026-07-18T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .expect("reconcile");

    let event_id = report.findings[0].event_id.clone().expect("event recorded");
    assert_eq!(report.recorded_events, vec![event_id.clone()]);
    assert_eq!(g.count_nodes(node::DRIFT_EVENT).unwrap(), 1);

    let radius = g
        .propagate_from(&[&event_id], PropagateOptions::default())
        .expect("propagate");
    assert!(
        radius.impacted.iter().any(|n| n.node_id == "cap:score"),
        "the drift event must reach the capability, got {:?}",
        radius
            .impacted
            .iter()
            .map(|n| &n.node_id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn reconcile_does_not_write_unless_asked() {
    let mut g = built_thread();
    g.reconcile_artifacts(
        &[observed("art:score", true, Some("sha256:bbb"))],
        &ReconcileOptions::default(),
    )
    .expect("reconcile");

    assert_eq!(
        g.count_nodes(node::DRIFT_EVENT).unwrap(),
        0,
        "observing is not recording — no DriftEvent without record_events"
    );
}

#[test]
fn recording_the_same_drift_twice_does_not_duplicate() {
    let mut g = built_thread();
    let opts = ReconcileOptions {
        record_events: true,
        ..Default::default()
    };
    g.reconcile_artifacts(&[observed("art:score", true, Some("sha256:bbb"))], &opts)
        .expect("first");
    g.reconcile_artifacts(&[observed("art:score", true, Some("sha256:bbb"))], &opts)
        .expect("second");

    assert_eq!(
        g.count_nodes(node::DRIFT_EVENT).unwrap(),
        1,
        "the same unresolved divergence is one event, not one per scan"
    );
}

#[test]
fn a_deleted_file_is_the_most_severe_drift() {
    let mut g = built_thread();
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", false, None)],
            &ReconcileOptions::default(),
        )
        .expect("reconcile");

    assert_eq!(report.findings[0].kind, DriftKind::MissingArtifact);
    assert_eq!(report.findings[0].realizes, vec!["cap:score"]);
}

#[test]
fn an_unregistered_file_is_an_undocumented_addition() {
    let mut g = built_thread();
    let report = g
        .reconcile_artifacts(
            &[observed("art:ghost", true, Some("sha256:zzz"))],
            &ReconcileOptions::default(),
        )
        .expect("reconcile");

    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, DriftKind::UndocumentedAddition);
    assert!(
        report.findings[0].realizes.is_empty(),
        "it realizes nothing — that is the problem"
    );
}

#[test]
fn a_missing_baseline_is_surfaced_not_silently_passed() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_capability("cap:score", "Scoring", "tracks the score")
        .expect("cap");
    // Registered without a checksum — nothing to compare against later.
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:score".into(),
        name: "Score.cs".into(),
        location: Some("src/Score.cs".into()),
        artifact_type: None,
        target_type: node::CAPABILITY.into(),
        target_id: "cap:score".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })
    .expect("link");

    let report = g
        .reconcile_artifacts(
            &[observed("art:score", true, Some("sha256:aaa"))],
            &ReconcileOptions {
                record_events: true,
                ..Default::default()
            },
        )
        .expect("reconcile");

    assert_eq!(report.findings[0].kind, DriftKind::NoBaseline);
    assert_eq!(
        report.unchanged, 0,
        "unjudgeable must not count as unchanged"
    );
    assert!(
        report.findings[0].event_id.is_none(),
        "an observability gap is not a divergence — it must not become a DriftEvent"
    );
    assert_eq!(g.count_nodes(node::DRIFT_EVENT).unwrap(), 0);
}

#[test]
fn a_partial_scan_is_not_evidence_of_absence() {
    let mut g = built_thread();
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:other".into(),
        name: "Other.cs".into(),
        location: Some("src/Other.cs".into()),
        artifact_type: None,
        target_type: node::CAPABILITY.into(),
        target_id: "cap:score".into(),
        completeness: None,
        provenance: None,
        fragment_id: Some("frag:other".into()),
        checksum: Some("sha256:ccc".into()),
    })
    .expect("link other");

    // Only one of the two artifacts observed, and we do not claim completeness.
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", true, Some("sha256:aaa"))],
            &ReconcileOptions::default(),
        )
        .expect("reconcile");
    assert!(
        report.unobserved.is_empty(),
        "without `exhaustive`, an unlisted artifact is unknown, not missing"
    );

    // Now claim a full sweep: the unseen artifact must be surfaced.
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", true, Some("sha256:aaa"))],
            &ReconcileOptions {
                exhaustive: true,
                ..Default::default()
            },
        )
        .expect("reconcile exhaustive");
    assert_eq!(report.unobserved, vec!["art:other"]);
}

#[test]
fn accepting_a_change_updates_the_baseline_so_it_stops_reporting() {
    let mut g = built_thread();
    let opts = ReconcileOptions::default();
    let before = g
        .reconcile_artifacts(&[observed("art:score", true, Some("sha256:bbb"))], &opts)
        .expect("before");
    assert_eq!(before.findings.len(), 1);

    // The user reviewed the change and accepted it into the design.
    g.set_artifact_checksum("art:score", "sha256:bbb")
        .expect("accept");

    let after = g
        .reconcile_artifacts(&[observed("art:score", true, Some("sha256:bbb"))], &opts)
        .expect("after");
    assert!(
        after.findings.is_empty(),
        "an accepted change is the new baseline, not permanent drift"
    );
    assert_eq!(after.unchanged, 1);
}

#[test]
fn findings_are_ranked_most_severe_first() {
    let mut g = built_thread();
    for (id, target) in [("art:b", "cap:score"), ("art:c", "cap:score")] {
        g.link_artifact(LinkArtifactOptions {
            artifact_id: id.into(),
            name: format!("{id}.cs"),
            location: None,
            artifact_type: None,
            target_type: node::CAPABILITY.into(),
            target_id: target.into(),
            completeness: None,
            provenance: None,
            fragment_id: Some(format!("frag:{id}")),
            checksum: Some("sha256:base".into()),
        })
        .expect("link");
    }

    let report = g
        .reconcile_artifacts(
            &[
                observed("art:score", true, Some("sha256:changed")), // checksum_change
                observed("art:b", false, None),                      // missing (worst)
                observed("art:c", true, None),                       // no_baseline (least)
            ],
            &ReconcileOptions::default(),
        )
        .expect("reconcile");

    let kinds: Vec<DriftKind> = report.findings.iter().map(|f| f.kind).collect();
    assert_eq!(
        kinds,
        vec![
            DriftKind::MissingArtifact,
            DriftKind::ChecksumChange,
            DriftKind::NoBaseline
        ]
    );
}
