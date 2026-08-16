//! A REALIZES edge says whether anyone checked the artifact against what the
//! target requires — `dec:should-a-realizes-edge-say-that-nobody-checked-the-file-still-obeys-the-rule`.
//!
//! WHY THIS EXISTS, and it came from outside the project: a Requirement said the
//! calendar day is the person's CIVIL date. The code used UTC. The Capability
//! said `realized`. The checksum matched, so `reconcile_artifacts` was silent.
//! EVERY SIGNAL IN THE GRAPH WAS GREEN AND A USER FOUND THE BUG IN THE PRODUCT.
//! Nothing distinguished a file CHECKED AGAINST THE RULE from a file MERELY
//! HASHED, and the two produced identical reports.
//!
//! reflow2 cannot compute the answer — it reads a design, never a running
//! system. So every case here is about RECORDING what somebody knows, and about
//! the one number that makes the silence visible.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::node;

fn temp_graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("in-memory graph")
}

fn graph_with_one_link(conformance: Option<&str>) -> DesignGraph {
    let mut g = temp_graph();
    g.add_capability("cap:c", "a capability", "what it does", None)
        .expect("capability");
    g.add_artifact("art:a", "a.rs", Some("code"), Some("src/a.rs"))
        .expect("artifact");
    g.realizes(
        "art:a",
        node::CAPABILITY,
        "cap:c",
        Some("complete"),
        conformance,
    )
    .expect("realizes");
    g
}

#[test]
fn a_link_nobody_vouched_for_counts_as_unchecked() {
    // THE DEFAULT IS THE WHOLE POINT. Registering a file says it exists, never
    // that anyone checked it against the requirement.
    let g = graph_with_one_link(None);
    let t = g.conformance_tally().expect("tally");
    assert_eq!(t.total, 1);
    assert_eq!(t.unchecked, 1);
    assert_eq!(t.reviewed, 0);
    assert_eq!(t.verified, 0);
}

#[test]
fn a_link_someone_read_against_the_requirement_counts_as_reviewed() {
    let g = graph_with_one_link(Some("reviewed"));
    let t = g.conformance_tally().expect("tally");
    assert_eq!((t.total, t.unchecked, t.reviewed, t.verified), (1, 0, 1, 0));
}

#[test]
fn a_link_a_check_exercises_counts_as_verified() {
    let g = graph_with_one_link(Some("verified"));
    let t = g.conformance_tally().expect("tally");
    assert_eq!((t.total, t.unchecked, t.reviewed, t.verified), (1, 0, 0, 1));
}

#[test]
fn conformance_is_a_different_question_from_completeness() {
    // THE COUNTERWEIGHT THAT DEFINES THE FEATURE. A file can be COMPLETE — all
    // of it exists — and still never have been checked against what the target
    // requires. That is exactly the shipped-bug case: complete, hashed, green,
    // wrong. If these two ever collapse into one field, this fails.
    let g = graph_with_one_link(None);
    let edges = g.outgoing("art:a", Some("REALIZES")).expect("edges");
    let props = &edges[0].properties;
    assert_eq!(
        props.get("completeness").and_then(|v| v.as_str()),
        Some("complete")
    );
    let t = g.conformance_tally().expect("tally");
    assert_eq!(t.unchecked, 1, "complete must not imply checked");
}

#[test]
fn a_matching_checksum_still_leaves_the_link_unchecked() {
    // The other half of the same counterweight: the Artifact's checksum says
    // the file has not MOVED. It has never said the file still OBEYS anything.
    let mut g = temp_graph();
    g.add_capability("cap:c", "a capability", "what it does", None)
        .expect("capability");
    g.add_artifact("art:a", "a.rs", Some("code"), Some("src/a.rs"))
        .expect("artifact");
    g.realizes("art:a", node::CAPABILITY, "cap:c", Some("complete"), None)
        .expect("realizes");
    assert_eq!(g.conformance_tally().expect("tally").unchecked, 1);
}

#[test]
fn the_tally_names_which_links_are_unchecked_not_only_how_many() {
    // A bare number is alarming; a number with names is actionable.
    let g = graph_with_one_link(None);
    let t = g.conformance_tally().expect("tally");
    assert_eq!(t.unchecked_sample, vec!["art:a → cap:c".to_string()]);
}

#[test]
fn the_sample_is_capped_and_the_true_total_is_not() {
    // NO SILENT CAPS: the sample stops at ten, and `unchecked` keeps counting.
    // A truncated list that also truncated the count would under-report the
    // problem, which is the one direction this must never fail in.
    let mut g = temp_graph();
    g.add_capability("cap:c", "a capability", "what it does", None)
        .expect("capability");
    for i in 0..25 {
        let art = format!("art:{i:02}");
        g.add_artifact(&art, &art, Some("code"), Some(&format!("src/{i}.rs")))
            .expect("artifact");
        g.realizes(&art, node::CAPABILITY, "cap:c", None, None)
            .expect("realizes");
    }
    let t = g.conformance_tally().expect("tally");
    assert_eq!(t.total, 25);
    assert_eq!(t.unchecked, 25, "the COUNT must not be capped");
    assert_eq!(t.unchecked_sample.len(), 10, "the SAMPLE is capped at ten");
}

#[test]
fn a_mixed_design_reports_each_bucket_separately() {
    let mut g = temp_graph();
    g.add_capability("cap:c", "a capability", "what it does", None)
        .expect("capability");
    for (i, conf) in [None, Some("reviewed"), Some("verified"), Some("reviewed")]
        .into_iter()
        .enumerate()
    {
        let art = format!("art:{i}");
        g.add_artifact(&art, &art, Some("code"), Some(&format!("src/{i}.rs")))
            .expect("artifact");
        g.realizes(&art, node::CAPABILITY, "cap:c", None, conf)
            .expect("realizes");
    }
    let t = g.conformance_tally().expect("tally");
    assert_eq!((t.total, t.unchecked, t.reviewed, t.verified), (4, 1, 2, 1));
}

#[test]
fn a_design_with_no_realizing_links_reports_zero_rather_than_nothing() {
    // "0 of 0 unchecked" and "I did not look" must not be the same answer.
    let mut g = temp_graph();
    g.add_capability("cap:c", "a capability", "what it does", None)
        .expect("capability");
    let t = g.conformance_tally().expect("tally");
    assert_eq!(t.total, 0);
    assert_eq!(t.unchecked, 0);
    assert!(t.unchecked_sample.is_empty());
}

#[test]
fn the_link_tool_reports_the_conformance_it_wrote() {
    // A caller must be able to READ BACK what was stored rather than assume it
    // — and "you registered this and nobody has checked it" is the single most
    // useful thing to learn at the moment of registering.
    let mut g = temp_graph();
    g.add_capability("cap:c", "a capability", "what it does", None)
        .expect("capability");
    let link = g
        .link_artifact(reflow2_core::LinkArtifactOptions {
            artifact_id: "art:a".into(),
            name: "a.rs".into(),
            location: Some("src/a.rs".into()),
            artifact_type: Some("code".into()),
            target_type: node::CAPABILITY.into(),
            target_id: "cap:c".into(),
            completeness: None,
            conformance: None,
            provenance: None,
            fragment_id: None,
            checksum: None,
        })
        .expect("link");
    // `completeness` defaults to the optimistic `complete`; `conformance` must
    // NOT follow it there. Registering is not checking.
    assert_eq!(link.completeness, "complete");
    assert_eq!(link.conformance, "unchecked");
}

#[test]
fn the_evidence_report_carries_the_tally_so_the_number_is_reachable() {
    // dec:edge-orthogonality: a vocabulary distinction earns its keep only if a
    // COMPUTATION reads it. If this ever stops being wired into a served
    // report, the property has become decoration and this test says so.
    let g = graph_with_one_link(None);
    let report = g.evidence_report().expect("evidence report");
    assert_eq!(report.conformance.total, 1);
    assert_eq!(report.conformance.unchecked, 1);
}
