//! Step 4's first half (`dec:step-4-is-built-on-real-runs-with-no-threshold-and-unscored-good-news`):
//! a real run is fed back in the unit a test runner reports — the FILE — and
//! the design resolves each file to the checks that name it.
//!
//! Pins: both routes from a file to a check (a Verification's own `location`,
//! and an Artifact at that location that IMPLEMENTS it); one failing file
//! fails every check it covers, and a check covered by two files takes the
//! worst outcome; a file the design names no check for is reported, never
//! dropped silently; and the resolved run, fed through the reconcile with
//! recording on, is what makes a design's loop read as closed.

use reflow2_core::graph::DesignGraph;
use reflow2_core::loop_closure::LoopClosureState;
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::verify::{ObservedFile, VerifyReconcileOptions};

fn world() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Pump").expect("project");
    for (v, name) in [
        ("ver:flow", "flow rate test"),
        ("ver:seal", "seal test"),
        ("ver:both", "system test"),
    ] {
        g.add_verification(v, name, None, None, None).expect("ver");
        g.set_verification_status(v, "passing", None, None)
            .expect("status");
    }
    // Route 1: the check's own `location`.
    let mut props = Props::new();
    for (k, val) in &g
        .get_node(node::VERIFICATION, "ver:flow")
        .expect("read")
        .expect("present")
        .properties
    {
        props = props.set(k, val.clone());
    }
    g.create_node(
        node::VERIFICATION,
        "ver:flow",
        props.set("location", "tests/flow.rs"),
    )
    .expect("location");
    // Route 2: an Artifact at the file IMPLEMENTS the check.
    for (art, loc, vers) in [
        ("art:seal", "tests/seal.rs", vec!["ver:seal", "ver:both"]),
        ("art:flow", "./tests/flow.rs", vec!["ver:both"]),
    ] {
        g.create_node(
            node::ARTIFACT,
            art,
            Props::new().set("name", art).set("location", loc),
        )
        .expect("artifact");
        for v in vers {
            g.create_edge(
                edge::IMPLEMENTS,
                node::ARTIFACT,
                art,
                node::VERIFICATION,
                v,
                Props::new(),
            )
            .expect("implements");
        }
    }
    g
}

fn file(location: &str, outcome: &str) -> ObservedFile {
    ObservedFile {
        location: location.to_string(),
        outcome: outcome.to_string(),
    }
}

fn outcome_of(r: &reflow2_core::verify::ResolvedFiles, id: &str) -> Option<String> {
    r.observed
        .iter()
        .find(|o| o.verification_id == id)
        .map(|o| o.outcome.clone())
}

#[test]
fn both_routes_from_a_file_to_a_check_resolve() {
    let g = world();
    let r = g
        .resolve_observed_files(&[
            file("tests/flow.rs", "passed"),
            file("tests/seal.rs", "passed"),
        ])
        .expect("resolve");
    assert_eq!(outcome_of(&r, "ver:flow").as_deref(), Some("passed"));
    assert_eq!(outcome_of(&r, "ver:seal").as_deref(), Some("passed"));
    assert_eq!(outcome_of(&r, "ver:both").as_deref(), Some("passed"));
    assert!(r.unmapped_locations.is_empty());
}

#[test]
fn a_check_covered_by_two_files_takes_the_worst_outcome() {
    let g = world();
    let r = g
        .resolve_observed_files(&[
            file("tests/flow.rs", "passed"),
            file("tests/seal.rs", "failed"),
        ])
        .expect("resolve");
    assert_eq!(outcome_of(&r, "ver:flow").as_deref(), Some("passed"));
    assert_eq!(outcome_of(&r, "ver:both").as_deref(), Some("failed"));
}

#[test]
fn a_leading_dot_slash_names_the_same_file() {
    let g = world();
    let r = g
        .resolve_observed_files(&[file("./tests/seal.rs", "passed")])
        .expect("resolve");
    assert_eq!(outcome_of(&r, "ver:seal").as_deref(), Some("passed"));
}

#[test]
fn a_file_the_design_names_no_check_for_is_reported() {
    let g = world();
    let r = g
        .resolve_observed_files(&[file("tests/unrelated.rs", "failed")])
        .expect("resolve");
    assert!(r.observed.is_empty());
    assert_eq!(r.unmapped_locations, vec!["tests/unrelated.rs"]);
}

#[test]
fn a_run_fed_back_by_file_closes_the_loop_and_a_failure_lands() {
    let mut g = world();
    assert_eq!(
        g.loop_closure().expect("closure").state,
        LoopClosureState::NeverClosed
    );
    let r = g
        .resolve_observed_files(&[
            file("tests/flow.rs", "passed"),
            file("tests/seal.rs", "failed"),
        ])
        .expect("resolve");
    let report = g
        .reconcile_verification(
            &r.observed,
            &VerifyReconcileOptions {
                record_events: true,
                exhaustive: false,
                detected_at: Some("2026-09-22".into()),
            },
        )
        .expect("reconcile");
    // Believed passing, actually failed: the dangerous direction, recorded.
    assert_eq!(report.findings.len(), 2, "{:?}", report.findings);
    assert!(report.findings.iter().all(|f| f.observed == "failed"));
    assert_eq!(report.stamped.len(), 3);
    let c = g.loop_closure().expect("closure");
    assert_eq!(c.state, LoopClosureState::AFailureCameBack);
    assert_eq!(c.failures_fed_back, 2);
}
