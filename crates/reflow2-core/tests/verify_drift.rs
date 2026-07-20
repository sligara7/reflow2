//! The P4 reconcile (BL-30's M half): does the recorded outcome match what
//! the run actually reported? Pins the dangerous direction sorting first
//! (believed proven, actually broken), the persistent-gap loop, honest
//! rejection of nonsense outcomes, and partial-run semantics.

use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;
use reflow2_core::verify::{ObservedVerification, VerifyReconcileOptions};

fn verified_world() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.add_requirement("req:works", "It works", "The thing must work.")
        .expect("req");
    g.add_capability(
        "cap:core",
        "Core behaviour",
        "does the thing",
        Some("realized"),
    )
    .expect("cap");
    g.satisfies("cap:core", "req:works").expect("sat");
    g.add_verification("ver:core", "core test", Some("test"), Some("unit"))
        .expect("ver");
    g.verifies("ver:core", "Capability", "cap:core")
        .expect("verifies");
    g
}

fn obs(id: &str, outcome: &str) -> ObservedVerification {
    ObservedVerification {
        verification_id: id.to_string(),
        outcome: outcome.to_string(),
    }
}

#[test]
fn an_agreeing_outcome_is_not_drift() {
    let mut g = verified_world();
    g.set_verification_status("ver:core", "passing", None)
        .expect("status");
    let r = g
        .reconcile_verification(
            &[obs("ver:core", "passed")],
            &VerifyReconcileOptions::default(),
        )
        .expect("reconcile");
    assert!(r.findings.is_empty(), "{:?}", r.findings);
    assert_eq!(r.agreements, 1);
}

#[test]
fn believed_proven_actually_broken_is_found_and_lands_on_the_capability() {
    let mut g = verified_world();
    g.set_verification_status("ver:core", "passing", None)
        .expect("status");
    let r = g
        .reconcile_verification(
            &[obs("ver:core", "failed")],
            &VerifyReconcileOptions::default(),
        )
        .expect("reconcile");
    assert_eq!(r.findings.len(), 1);
    assert_eq!(r.findings[0].declared, "passing");
    assert_eq!(r.findings[0].observed, "failed");
    assert_eq!(r.propagation_seeds, vec!["cap:core"]);
}

#[test]
fn the_dangerous_direction_sorts_first() {
    let mut g = verified_world();
    g.add_verification("ver:aux", "aux test", None, None)
        .expect("ver");
    g.set_verification_status("ver:core", "passing", None)
        .expect("status");
    g.set_verification_status("ver:aux", "failing", None)
        .expect("status");
    // aux improved (failing→passed), core regressed (passing→failed).
    let r = g
        .reconcile_verification(
            &[obs("ver:aux", "passed"), obs("ver:core", "failed")],
            &VerifyReconcileOptions::default(),
        )
        .expect("reconcile");
    let order: Vec<&str> = r
        .findings
        .iter()
        .map(|f| f.verification_id.as_str())
        .collect();
    assert_eq!(
        order,
        ["ver:core", "ver:aux"],
        "broken-while-believed-proven first"
    );
}

#[test]
fn a_run_the_design_never_recorded_is_still_a_divergence() {
    let mut g = verified_world(); // status stays at the default, planned
    let r = g
        .reconcile_verification(
            &[obs("ver:core", "passed")],
            &VerifyReconcileOptions::default(),
        )
        .expect("reconcile");
    assert_eq!(
        r.findings.len(),
        1,
        "'planned' plus a real outcome is stale bookkeeping"
    );
    assert_eq!(r.findings[0].declared, "planned");
}

#[test]
fn nonsense_outcomes_are_rejected_by_name_and_the_batch_survives() {
    let mut g = verified_world();
    g.set_verification_status("ver:core", "passing", None)
        .expect("status");
    let r = g
        .reconcile_verification(
            &[
                obs("ver:core", "passed"),
                obs("ver:core", "gr33n"),
                obs("ver:ghost", "passed"),
            ],
            &VerifyReconcileOptions::default(),
        )
        .expect("reconcile");
    assert_eq!(r.agreements, 1, "the valid observation still processed");
    assert_eq!(r.rejected.len(), 1);
    assert!(r.rejected[0].contains("gr33n"));
    assert_eq!(r.unknown_ids, vec!["ver:ghost"]);
}

#[test]
fn a_partial_run_is_not_evidence_of_absence() {
    let mut g = verified_world();
    g.set_verification_status("ver:core", "passing", None)
        .expect("status");
    let r = g
        .reconcile_verification(&[], &VerifyReconcileOptions::default())
        .expect("reconcile");
    assert!(
        r.unobserved.is_empty(),
        "not exhaustive: silence is silence"
    );
    let r = g
        .reconcile_verification(
            &[],
            &VerifyReconcileOptions {
                exhaustive: true,
                ..Default::default()
            },
        )
        .expect("reconcile");
    assert_eq!(
        r.unobserved,
        vec!["ver:core"],
        "a passing claim the run never touched"
    );
}

/// The full loop: divergence → recorded event → persistent gap with
/// verification-appropriate advice → the human sets the status to what the
/// run said → the next run agrees → resolved, gap gone.
#[test]
fn the_divergence_nags_until_the_record_matches_a_real_run() {
    let mut g = verified_world();
    g.set_verification_status("ver:core", "passing", None)
        .expect("status");
    let r = g
        .reconcile_verification(
            &[obs("ver:core", "failed")],
            &VerifyReconcileOptions {
                record_events: true,
                detected_at: Some("2026-07-19T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .expect("reconcile");
    assert_eq!(r.recorded_events.len(), 1);

    let drift_gap = |g: &DesignGraph| {
        g.detect_gaps()
            .expect("gaps")
            .into_iter()
            .find(|c| c.gap_source == GapSource::UnresolvedDrift)
    };
    let gap = drift_gap(&g).expect("a recorded divergence must nag");
    assert!(
        gap.description.contains("set_verification_status"),
        "the advice must fit the drift kind: {}",
        gap.description
    );

    // The human writes down the truth; the check is genuinely failing now.
    g.set_verification_status("ver:core", "failing", None)
        .expect("status");
    let r2 = g
        .reconcile_verification(
            &[obs("ver:core", "failed")],
            &VerifyReconcileOptions::default(),
        )
        .expect("reconcile");
    assert_eq!(r2.resolved_events, r.recorded_events);
    assert!(
        drift_gap(&g).is_none(),
        "an answered divergence stops nagging"
    );
    // …and the honest status now raises the RIGHT signal instead.
    assert!(
        g.detect_gaps()
            .expect("gaps")
            .iter()
            .any(|c| c.gap_source == GapSource::FailingVerification),
        "a truthful failing status hands off to failing_verification"
    );
}

#[test]
fn a_new_divergence_pair_is_a_new_event_not_an_overwrite() {
    let mut g = verified_world();
    g.set_verification_status("ver:core", "passing", None)
        .expect("status");
    let opts = VerifyReconcileOptions {
        record_events: true,
        ..Default::default()
    };
    let r1 = g
        .reconcile_verification(&[obs("ver:core", "failed")], &opts)
        .expect("reconcile");
    // Same divergence re-observed: same event.
    let r1b = g
        .reconcile_verification(&[obs("ver:core", "failed")], &opts)
        .expect("reconcile");
    assert_eq!(r1.recorded_events, r1b.recorded_events);
    // The status moves, then diverges the OTHER way: a different event, and
    // the first one resolves because its divergence is gone.
    g.set_verification_status("ver:core", "failing", None)
        .expect("status");
    let r2 = g
        .reconcile_verification(&[obs("ver:core", "passed")], &opts)
        .expect("reconcile");
    assert_ne!(
        r1.recorded_events, r2.recorded_events,
        "the flapping history stays visible"
    );
    assert_eq!(r2.resolved_events, r1.recorded_events);
}
