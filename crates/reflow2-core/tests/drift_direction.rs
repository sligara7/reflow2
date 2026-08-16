//! Directional drift — which way the design and the build diverge.
//!
//! From the field, 2026-07-24: reflow2 running on storyflow reported that its
//! docs "consistently understate what's built". That is the failure the
//! predecessor died of — design well, implement, iterate several cycles, and the
//! stated requirements quietly stop matching the built product.
//!
//! A checksum cannot see it. A file that grew a whole subsystem and a file with
//! a typo fixed produce the same `checksum_change`, so understatement is
//! invisible exactly where it is largest. These tests pin the distinction, and
//! the first one is written to fail if the implementation only ever answered
//! "something changed".

use reflow2_core::DesignGraph;
use reflow2_core::artifact::LinkArtifactOptions;
use reflow2_core::drift::{DriftDirection, DriftKind, ObservedArtifact, ReconcileOptions};
use reflow2_core::nodes::node;

/// One artifact the design records as realizing a single capability, while the
/// codebase has grown two more.
fn grown() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    for (id, name) in [
        ("cap:parse", "Parse"),
        ("cap:render", "Render"),
        ("cap:cache", "Cache"),
    ] {
        g.add_capability(id, name, "does it", Some("realized"))
            .unwrap();
    }
    // The design records ONE realizes edge. The file, in reality, does three.
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:engine".into(),
        name: "engine.rs".into(),
        location: Some("src/engine.rs".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:parse".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:baseline".into()),
    })
    .unwrap();
    g
}

fn observed_with(realizes: Option<Vec<&str>>, checksum: &str) -> ObservedArtifact {
    ObservedArtifact {
        artifact_id: "art:engine".into(),
        present: true,
        checksum: Some(checksum.into()),
        realizes: realizes.map(|v| v.into_iter().map(str::to_string).collect()),
    }
}

#[test]
fn a_grown_file_reports_understatement_not_merely_change() {
    // THE test. Asserting "drift was detected" would pass here AND on a typo
    // fix, and would prove nothing about direction — which is the whole point.
    // So this asserts the direction and the specific unrecorded capabilities.
    let mut g = grown();
    let report = g
        .reconcile_artifacts(
            &[observed_with(
                Some(vec!["cap:parse", "cap:render", "cap:cache"]),
                "sha256:baseline",
            )],
            &ReconcileOptions::default(),
        )
        .unwrap();

    let f = report
        .findings
        .iter()
        .find(|f| f.direction.is_some())
        .expect("direction must be assessed when the caller supplies realizes");
    assert_eq!(f.direction, Some(DriftDirection::Understated));
    assert_eq!(f.kind, DriftKind::Understated);
    assert_eq!(
        f.unrecorded,
        vec!["cap:cache".to_string(), "cap:render".to_string()],
        "it must name WHAT the design fails to record — telling someone their \
         design is stale without telling them what to fix is why it stays stale"
    );
    assert!(f.unbuilt.is_empty());
}

#[test]
fn a_typo_fix_is_not_understatement() {
    // The other half of the distinction. Same file, same checksum change, but
    // the capabilities still match: this must NOT report a direction, or the
    // signal is as useless as the one it replaces.
    let mut g = grown();
    let report = g
        .reconcile_artifacts(
            &[observed_with(Some(vec!["cap:parse"]), "sha256:new")],
            &ReconcileOptions::default(),
        )
        .unwrap();

    assert!(
        report.findings.iter().all(|f| f.direction.is_none()),
        "capabilities agree, so there is no direction to report: {:?}",
        report.findings
    );
}

#[test]
fn a_design_claiming_more_than_exists_is_overstatement_and_ranks_higher() {
    let mut g = grown();
    g.realizes("art:engine", node::CAPABILITY, "cap:render", None, None)
        .unwrap();
    g.realizes("art:engine", node::CAPABILITY, "cap:cache", None, None)
        .unwrap();

    let report = g
        .reconcile_artifacts(
            &[observed_with(Some(vec!["cap:parse"]), "sha256:v2")],
            &ReconcileOptions::default(),
        )
        .unwrap();

    let f = report
        .findings
        .iter()
        .find(|f| f.direction.is_some())
        .expect("a direction");
    assert_eq!(f.direction, Some(DriftDirection::Overstated));
    assert_eq!(
        f.unbuilt,
        vec!["cap:cache".to_string(), "cap:render".to_string()]
    );
    // Overstatement outranks a plain checksum change: a design that claims
    // something absent is the one someone plans against.
    assert_eq!(
        report.findings.first().map(|f| f.kind),
        Some(DriftKind::Overstated),
        "it must sort above the checksum change: {:?}",
        report.findings
    );
}

#[test]
fn disagreement_in_both_directions_reports_diverged() {
    let mut g = grown();
    g.realizes("art:engine", node::CAPABILITY, "cap:render", None, None)
        .unwrap();

    // Design says parse+render; reality says parse+cache.
    let report = g
        .reconcile_artifacts(
            &[observed_with(
                Some(vec!["cap:parse", "cap:cache"]),
                "sha256:v2",
            )],
            &ReconcileOptions::default(),
        )
        .unwrap();

    let f = report
        .findings
        .iter()
        .find(|f| f.direction.is_some())
        .expect("a direction");
    assert_eq!(f.direction, Some(DriftDirection::Diverged));
    assert_eq!(f.unrecorded, vec!["cap:cache".to_string()]);
    assert_eq!(f.unbuilt, vec!["cap:render".to_string()]);
}

#[test]
fn understatement_is_found_even_when_the_bytes_never_moved() {
    // A design can be wrong from the day it was written. Tying direction to
    // checksum_change would miss exactly the long-lived, never-touched files
    // where understatement quietly accumulates.
    let mut g = grown();
    let report = g
        .reconcile_artifacts(
            &[observed_with(
                Some(vec!["cap:parse", "cap:render"]),
                "sha256:baseline",
            )],
            &ReconcileOptions::default(),
        )
        .unwrap();

    let f = report
        .findings
        .iter()
        .find(|f| f.direction == Some(DriftDirection::Understated))
        .expect("an unchanged file can still be understated");
    assert_eq!(f.unrecorded, vec!["cap:render".to_string()]);
}

#[test]
fn not_assessing_is_not_evidence_of_agreement() {
    // `realizes: None` means "I did not look", which must stay distinct from
    // "I looked and they match" — otherwise every legacy caller silently starts
    // asserting alignment it never checked.
    let mut g = grown();
    let report = g
        .reconcile_artifacts(
            &[observed_with(None, "sha256:whatever")],
            &ReconcileOptions::default(),
        )
        .unwrap();
    assert!(report.findings.iter().all(|f| f.direction.is_none()));
    assert!(report.findings.iter().all(|f| f.unrecorded.is_empty()));
}

#[test]
fn an_understatement_records_as_an_undocumented_addition() {
    // It is the same condition as a file the design never heard of, one level
    // down. Mapping it onto the existing schema value keeps the vocabulary
    // honest and avoids a schema change nobody would thank us for (BL-94).
    let mut g = grown();
    let report = g
        .reconcile_artifacts(
            &[observed_with(
                Some(vec!["cap:parse", "cap:render"]),
                "sha256:v2",
            )],
            &ReconcileOptions {
                record_events: true,
                detected_at: Some("2026-07-24".into()),
                ..Default::default()
            },
        )
        .unwrap();

    let f = report
        .findings
        .iter()
        .find(|f| f.direction == Some(DriftDirection::Understated))
        .expect("understated");
    let event_id = f.event_id.as_ref().expect("recorded");
    let ev = g.get_node(node::DRIFT_EVENT, event_id).unwrap().unwrap();
    assert_eq!(
        ev.properties.get("drift_type").and_then(|v| v.as_str()),
        Some("undocumented_addition")
    );
}
