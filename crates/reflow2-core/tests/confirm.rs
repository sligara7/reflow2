//! The confirmation ledger (BL-35) — when was each design claim last checked
//! against reality, and what was the answer?
//!
//! The founding observation, from the erosion trials: an eroded design and a
//! genuinely coherent one both reported *quiet*. The ledger's whole job is to
//! make "nobody looked" distinguishable from "somebody looked and said it
//! holds" — and to report claim history without judging it
//! (`dec:report-dont-judge`: five design_holds with zero design edits is the
//! erosion signature, and the ledger makes it *legible*, never a verdict).

use reflow2_core::DriftDisposition;
use reflow2_core::LinkArtifactOptions;
use reflow2_core::confirm::ConfirmationState;
use reflow2_core::drift::{ObservedArtifact, ReconcileOptions};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;
use reflow2_core::temporal::{ChangeAction, ChangeRecord, ChangeType, EpochType};

/// A golden thread with one registered artifact carrying a checksum baseline.
fn built_thread() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_requirement("req:live", "Live scores", "scores update live")
        .expect("req");
    g.add_capability("cap:score", "Scoring", "tracks the score", None)
        .expect("cap");
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

fn observed(id: &str, checksum: &str) -> ObservedArtifact {
    ObservedArtifact {
        artifact_id: id.into(),
        present: true,
        checksum: Some(checksum.into()),
    }
}

fn recording() -> ReconcileOptions {
    ReconcileOptions {
        record_events: true,
        ..ReconcileOptions::default()
    }
}

#[test]
fn a_built_but_never_checked_claim_is_unexamined_not_confirmed() {
    // The state the original reflow died in: artifacts exist, baselines exist,
    // and *nothing has ever been checked*. That must not read as health.
    let g = built_thread();
    let ledger = g.confirmation_ledger().expect("ledger");

    assert_eq!(ledger.claims.len(), 1);
    let claim = &ledger.claims[0];
    assert_eq!(claim.capability_id, "cap:score");
    assert_eq!(claim.state, ConfirmationState::Unexamined);
    assert_eq!(claim.artifacts, vec!["art:score"]);
    assert_eq!(
        (ledger.drifting, ledger.confirmed, ledger.unexamined),
        (0, 0, 1)
    );
}

#[test]
fn a_capability_with_nothing_built_is_absent_by_design() {
    // "Nothing is built yet" is unrealized_capability's question, not a
    // confirmation question — the ledger must not double-report it.
    let mut g = built_thread();
    g.add_capability("cap:paper", "Paper-only", "designed, unbuilt", None)
        .expect("cap");
    let ledger = g.confirmation_ledger().expect("ledger");

    assert!(
        ledger.claims.iter().all(|c| c.capability_id != "cap:paper"),
        "an artifact-less capability has no confirmation claim to report"
    );
}

#[test]
fn an_unanswered_drift_is_the_actionable_drifting_state() {
    let mut g = built_thread();
    let report = g
        .reconcile_artifacts(&[observed("art:score", "sha256:bbb")], &recording())
        .expect("reconcile");
    assert_eq!(
        report.recorded_events.len(),
        1,
        "the divergence is recorded"
    );

    let ledger = g.confirmation_ledger().expect("ledger");
    let claim = &ledger.claims[0];
    assert_eq!(claim.state, ConfirmationState::Drifting);
    assert_eq!(claim.drift_events, 1);
    assert_eq!(claim.unresolved_drift_events, 1);
    assert_eq!(
        (ledger.drifting, ledger.confirmed, ledger.unexamined),
        (1, 0, 0)
    );
}

#[test]
fn an_answered_drift_is_confirmed_and_the_claim_kind_is_counted() {
    let mut g = built_thread();
    g.reconcile_artifacts(&[observed("art:score", "sha256:bbb")], &recording())
        .expect("reconcile");

    // The user reviewed the change: no design meaning, a fix restoring intent.
    g.set_artifact_checksum(
        "art:score",
        "sha256:bbb",
        DriftDisposition::DesignHolds {
            change_type: ChangeType::TestFailureFix,
        },
        None,
        Some("2026-07-20T10:00:00Z"),
    )
    .expect("accept");

    let ledger = g.confirmation_ledger().expect("ledger");
    let claim = &ledger.claims[0];
    assert_eq!(claim.state, ConfirmationState::Confirmed);
    assert_eq!(claim.unresolved_drift_events, 0, "the accept answered it");
    assert_eq!(claim.design_holds_claims, 1);
    assert_eq!(claim.design_updated_claims, 0);
    assert_eq!(claim.last_claim_at.as_deref(), Some("2026-07-20T10:00:00Z"));
}

#[test]
fn a_design_updated_accept_is_a_different_claim_than_design_holds() {
    // The two dispositions are different confirmation histories and must not
    // blur: one says "the code moved, the design already covered it", the
    // other says "the design moved with the code, here is the event".
    let mut g = built_thread();
    g.add_epoch("epoch:fix", "Fix 1", EpochType::Revision, 1)
        .expect("epoch");
    g.record_change(ChangeRecord {
        epoch_id: "epoch:fix",
        change_event_id: "chg:widen",
        name: "Scoring widened",
        change_type: ChangeType::RequirementCreep,
        target_type: node::CAPABILITY,
        target_id: "cap:score",
        action: ChangeAction::Modified,
    })
    .expect("record");
    g.set_artifact_checksum(
        "art:score",
        "sha256:bbb",
        DriftDisposition::DesignUpdated {
            change_event_id: "chg:widen",
        },
        None,
        None,
    )
    .expect("accept");

    let ledger = g.confirmation_ledger().expect("ledger");
    let claim = &ledger.claims[0];
    assert_eq!(claim.state, ConfirmationState::Confirmed);
    assert_eq!(claim.design_updated_claims, 1);
    assert_eq!(claim.design_holds_claims, 0);
    assert!(
        claim.design_edits >= 1,
        "the design moving is on the record against the capability itself"
    );
}

#[test]
fn the_erosion_signature_is_legible_in_the_counts() {
    // Cycle after cycle of "the design still holds" with zero design edits is
    // exactly how a design erodes into fiction. The ledger does not judge it
    // (a stable design under cosmetic churn looks identical) — but the counts
    // must make the pattern visible so a human can.
    let mut g = built_thread();
    for (i, sum) in ["sha256:bbb", "sha256:ccc", "sha256:ddd"]
        .iter()
        .enumerate()
    {
        g.reconcile_artifacts(
            &[observed("art:score", sum)],
            &ReconcileOptions {
                detected_at: Some(format!("2026-07-2{i}T00:00:00Z")),
                ..recording()
            },
        )
        .expect("reconcile");
        g.set_artifact_checksum(
            "art:score",
            sum,
            DriftDisposition::DesignHolds {
                change_type: ChangeType::TestFailureFix,
            },
            None,
            Some(&format!("2026-07-2{i}T01:00:00Z")),
        )
        .expect("accept");
    }

    let ledger = g.confirmation_ledger().expect("ledger");
    let claim = &ledger.claims[0];
    assert_eq!(claim.state, ConfirmationState::Confirmed);
    assert_eq!(claim.drift_events, 3, "three divergences, none overwritten");
    assert_eq!(claim.design_holds_claims, 3);
    assert_eq!(claim.design_updated_claims, 0);
    assert_eq!(
        claim.design_edits, 0,
        "the signature: claims without motion"
    );
    assert_eq!(
        claim.last_claim_at.as_deref(),
        Some("2026-07-22T01:00:00Z"),
        "the newest dated claim wins"
    );
}

#[test]
fn artifacts_reach_a_capability_through_its_allocated_component_too() {
    // BL-38's other P3 shape: the file realizes a Component, and the
    // capability is ALLOCATED_TO it. Confirmation must see through that hop
    // or component-realized capabilities read as unexamined forever.
    let mut g = built_thread();
    g.add_capability("cap:render", "Rendering", "draws the board", None)
        .expect("cap");
    g.add_component("cmp:ui", "UI", "the interface", None)
        .expect("cmp");
    g.allocate("cap:render", "cmp:ui").expect("allocate");
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:ui".into(),
        name: "Ui.cs".into(),
        location: Some("src/Ui.cs".into()),
        artifact_type: Some("code".into()),
        target_type: node::COMPONENT.into(),
        target_id: "cmp:ui".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:eee".into()),
    })
    .expect("link");

    let ledger = g.confirmation_ledger().expect("ledger");
    let claim = ledger
        .claims
        .iter()
        .find(|c| c.capability_id == "cap:render")
        .expect("component-allocated capability appears in the ledger");
    assert_eq!(claim.artifacts, vec!["art:ui"]);
    assert_eq!(claim.state, ConfirmationState::Unexamined);
}

#[test]
fn states_do_not_bleed_between_capabilities() {
    // One drifting capability must not paint its neighbours; the rollup counts
    // are per-claim sums, not a graph-wide mood.
    let mut g = built_thread();
    g.add_capability("cap:render", "Rendering", "draws the board", None)
        .expect("cap");
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:ui".into(),
        name: "Ui.cs".into(),
        location: Some("src/Ui.cs".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:render".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:eee".into()),
    })
    .expect("link");

    g.reconcile_artifacts(&[observed("art:score", "sha256:bbb")], &recording())
        .expect("reconcile");

    let ledger = g.confirmation_ledger().expect("ledger");
    assert_eq!(
        (ledger.drifting, ledger.confirmed, ledger.unexamined),
        (1, 0, 1)
    );
    let by_id = |id: &str| {
        ledger
            .claims
            .iter()
            .find(|c| c.capability_id == id)
            .expect("claim")
    };
    assert_eq!(by_id("cap:score").state, ConfirmationState::Drifting);
    assert_eq!(by_id("cap:render").state, ConfirmationState::Unexamined);
}
