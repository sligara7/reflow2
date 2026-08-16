//! The ledger's missing word: **nothing moved** (BL-157, BL-158).
//!
//! Two findings, one hole. The artifact ledger could say *the code moved and
//! carried no design meaning* (`design_holds`) and *the code moved and the
//! design moved with it* (`design_updated`), and it had no way at all to say
//! *nothing moved*. Both halves of that gap were found by hitting them:
//!
//! - **BL-157.** `art:detect` was registered with no checksum. Giving it one
//!   required a disposition, both available answers presuppose a movement, and
//!   the least-wrong choice recorded a `refactor` of a file the session never
//!   touched — a change that never happened, written into the ledger that
//!   exists to keep the design free of exactly that.
//! - **BL-158.** An exhaustive sweep of all 107 registered artifacts reported
//!   106 unchanged and zero drift, and `loop_status` afterwards still said *"1
//!   built capability never checked against reality"*. Recording only
//!   divergence means a clean pass writes nothing, so the operator who checks
//!   everything and the operator who checks nothing produce identical graphs.
//!
//! The counterweights are the load-bearing half and are marked below. A new
//! disposition that meant "skip the question" would be a way to launder real
//! drift past `dec:two-sided-accept`, and a confirmation that recorded more
//! than was observed would turn a partial sweep into a false all-clear.

use reflow2_core::DriftDisposition;
use reflow2_core::LinkArtifactOptions;
use reflow2_core::confirm::{ConfirmationState, VerificationFreshness};
use reflow2_core::drift::{DriftKind, ObservedArtifact, ReconcileOptions};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;
use reflow2_core::temporal::ChangeType;

/// A capability realized by one artifact that has **no** checksum — the state
/// BL-157 was found in.
fn unbaselined() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
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
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })
    .expect("link");
    g
}

/// The same capability, but its artifact was registered **with** a checksum and
/// nothing has happened since — no drift, no accept, no claim of any kind.
///
/// This is the real BL-158 state, not a contrivance: it is where every artifact
/// registered by `link_artifact` starts, and where `cap:skill-triggers` sat in
/// reflow2's own design while `loop_status` asked, pass after clean pass, for a
/// check against reality that had already happened.
fn baselined() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
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
        conformance: None,
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
        realizes: None,
    }
}

fn recording(at: Option<&str>) -> ReconcileOptions {
    ReconcileOptions {
        record_events: true,
        exhaustive: false,
        detected_at: at.map(str::to_string),
    }
}

// ---- BL-157 · a first baseline is not an accept ----------------------------

#[test]
fn a_first_baseline_records_that_nothing_moved() {
    let mut g = unbaselined();
    let (art, event_id) = g
        .set_artifact_checksum(
            "art:score",
            "sha256:aaa",
            DriftDisposition::BaselineEstablished,
            None,
            Some("2026-08-01"),
        )
        .expect("first baseline");

    assert_eq!(art.properties["checksum"].as_str(), Some("sha256:aaa"));

    let ev = g
        .get_node(node::CHANGE_EVENT, &event_id)
        .unwrap()
        .expect("the claim is on the record");
    assert_eq!(
        ev.properties["change_type"].as_str(),
        Some("baseline_established"),
        "the whole point of BL-157: not `refactor`, not `test_failure_fix` — \
         the record moved and the code did not"
    );
    assert_eq!(
        ev.properties["detected_at"].as_str(),
        Some("2026-08-01"),
        "a claim is worth having only if it says when"
    );
}

#[test]
fn establishing_the_same_first_baseline_twice_mints_one_claim() {
    let mut g = unbaselined();
    let (_, first) = g
        .set_artifact_checksum(
            "art:score",
            "sha256:aaa",
            DriftDisposition::BaselineEstablished,
            None,
            Some("2026-08-01"),
        )
        .unwrap();
    // The SAME baseline again is a no-op, so a re-run of a sweep is safe and
    // must not pile up identical claims. Only one that would MOVE the baseline
    // is refused — see the counterweight below.
    let (_, again) = g
        .set_artifact_checksum(
            "art:score",
            "sha256:aaa",
            DriftDisposition::BaselineEstablished,
            None,
            Some("2026-08-02"),
        )
        .expect("re-establishing the same first baseline is idempotent");
    assert_eq!(first, again);

    let ledger = g.confirmation_ledger().unwrap();
    let claim = &ledger.claims[0];
    assert_eq!(claim.baseline_claims, 1);
}

/// **COUNTERWEIGHT — the one that decides whether this is a fix or an off
/// switch.** A baseline can only ever be *first*. Allowed over an existing one,
/// `baseline_established` would be a way to accept genuine drift without
/// answering what the change meant, which is the silent accept the erosion
/// trials produced and `dec:two-sided-accept` exists to forbid.
#[test]
fn a_baseline_cannot_be_established_over_one_that_already_exists() {
    let mut g = baselined();
    let err = g
        .set_artifact_checksum(
            "art:score",
            "sha256:bbb",
            DriftDisposition::BaselineEstablished,
            None,
            Some("2026-08-01"),
        )
        .expect_err("this would move a baseline, which is a real accept");
    let msg = err.to_string();
    assert!(msg.contains("already has a baseline"), "{msg}");
    assert!(
        msg.contains("design_holds") && msg.contains("design_updated"),
        "the refusal must name what to do instead: {msg}"
    );

    let art = g.get_node(node::ARTIFACT, "art:score").unwrap().unwrap();
    assert_eq!(
        art.properties["checksum"].as_str(),
        Some("sha256:aaa"),
        "a refused accept must not move the baseline it refused to accept"
    );
}

/// **COUNTERWEIGHT, the other direction.** An accept with nothing to accept is
/// the fiction BL-157 actually hit. Refused rather than reported, because the
/// report would arrive after the fiction was already written.
#[test]
fn an_accept_against_no_baseline_is_refused_and_names_the_disposition() {
    for disposition in [
        DriftDisposition::DesignHolds {
            change_type: ChangeType::Refactor,
        },
        DriftDisposition::DesignUpdated {
            change_event_id: "chg:whatever",
        },
    ] {
        let mut g = unbaselined();
        let err = g
            .set_artifact_checksum("art:score", "sha256:aaa", disposition, None, None)
            .expect_err("there is no movement to take a position on");
        let msg = err.to_string();
        assert!(msg.contains("no recorded checksum"), "{msg}");
        assert!(
            msg.contains("baseline_established"),
            "the refusal must name the disposition that IS right: {msg}"
        );
        assert!(
            !g.get_node(node::ARTIFACT, "art:score")
                .unwrap()
                .unwrap()
                .properties
                .contains_key("checksum"),
            "nothing is written by a refused accept"
        );
    }
}

#[test]
fn the_ledger_counts_a_first_baseline_apart_from_an_accept() {
    let mut g = unbaselined();
    g.set_artifact_checksum(
        "art:score",
        "sha256:aaa",
        DriftDisposition::BaselineEstablished,
        None,
        Some("2026-08-01"),
    )
    .unwrap();
    let ledger = g.confirmation_ledger().unwrap();
    let claim = &ledger.claims[0];

    assert_eq!(claim.baseline_claims, 1);
    assert_eq!(
        claim.design_holds_claims, 0,
        "a first baseline is not a claim that the design held against a change — \
         folding it in would report a judgement nobody made"
    );
    assert_eq!(claim.design_updated_claims, 0);
}

/// **COUNTERWEIGHT.** `last_claim_at` is read as *the newest accepted change to
/// the code this check covers*. A first baseline is not a change, so letting it
/// in would mark every passing check on the capability stale the moment someone
/// registered a checksum that had been missing all along — a fresh check
/// reported as rotten, by the report that exists to catch rotten checks.
#[test]
fn a_first_baseline_does_not_age_a_passing_check() {
    let mut g = unbaselined();
    g.add_verification("ver:score", "Score suite", None, None)
        .unwrap();
    g.verifies("ver:score", node::CAPABILITY, "cap:score")
        .unwrap();
    g.set_verification_status("ver:score", "passing", Some("2026-07-01"))
        .unwrap();

    // Established LONG after the check ran. Nothing about the code moved.
    g.set_artifact_checksum(
        "art:score",
        "sha256:aaa",
        DriftDisposition::BaselineEstablished,
        None,
        Some("2026-12-25"),
    )
    .unwrap();

    let ledger = g.confirmation_ledger().unwrap();
    let claim = &ledger.claims[0];
    assert_eq!(
        claim.last_claim_at, None,
        "a first baseline is not a change"
    );
    assert_ne!(
        claim.verification_freshness,
        VerificationFreshness::Stale,
        "registering a missing checksum must not age the check that covers it"
    );
}

// ---- BL-158 · a clean reconcile is a result --------------------------------

#[test]
fn a_clean_reconcile_records_what_it_confirmed() {
    let mut g = baselined();
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", "sha256:aaa")],
            &recording(Some("2026-08-01")),
        )
        .unwrap();

    assert!(report.findings.is_empty(), "nothing drifted");
    assert_eq!(report.unchanged, 1);
    assert_eq!(
        report.confirmed,
        vec!["art:score".to_string()],
        "the pass that found everything correct has to leave a trace, or it is \
         indistinguishable from the pass nobody ran"
    );

    let art = g.get_node(node::ARTIFACT, "art:score").unwrap().unwrap();
    assert_eq!(
        art.properties["last_confirmed_at"].as_str(),
        Some("2026-08-01")
    );
}

/// **THE PAYOFF, and the case BL-158 was filed from.** A clean sweep must be
/// able to clear the debt it just discharged. Before this, `unexamined` was
/// computed only from recorded divergences and accepts, so a capability whose
/// artifact never drifted could never leave the state — and `loop_status` went
/// on asking, forever, for a pass that had just been run.
#[test]
fn a_clean_reconcile_clears_unexamined() {
    let mut g = baselined();

    let before = g.confirmation_ledger().unwrap();
    assert_eq!(
        before.claims[0].state,
        ConfirmationState::Unexamined,
        "nobody has looked yet — this is where every registered artifact starts"
    );
    assert_eq!(before.unexamined, 1);
    assert_eq!(before.claims[0].confirmations, 0);

    g.reconcile_artifacts(
        &[observed("art:score", "sha256:aaa")],
        &recording(Some("2026-08-01")),
    )
    .unwrap();

    let after = g.confirmation_ledger().unwrap();
    assert_eq!(after.claims[0].confirmations, 1);
    assert_eq!(
        after.claims[0].last_confirmed_at.as_deref(),
        Some("2026-08-01")
    );
    assert_eq!(
        after.unexamined, 0,
        "THE BUG: this number did not move for a 107-artifact sweep that found \
         everything correct"
    );
    assert_eq!(after.claims[0].state, ConfirmationState::Confirmed);
}

/// **COUNTERWEIGHT.** Looking is still not writing. `record_events` off is the
/// caller saying *show me first*, and a confirmation is a write like any other.
#[test]
fn looking_confirms_nothing() {
    let mut g = baselined();
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", "sha256:aaa")],
            &ReconcileOptions {
                record_events: false,
                exhaustive: false,
                detected_at: Some("2026-08-01".into()),
            },
        )
        .unwrap();

    assert_eq!(report.unchanged, 1);
    assert!(report.confirmed.is_empty());
    assert!(
        !g.get_node(node::ARTIFACT, "art:score")
            .unwrap()
            .unwrap()
            .properties
            .contains_key("last_confirmed_at")
    );
    assert_eq!(
        g.confirmation_ledger().unwrap().unexamined,
        1,
        "a look that wrote nothing must not clear the debt either"
    );
}

/// **COUNTERWEIGHT — the honesty bar BL-158 set for its own fix.** A
/// confirmation records what was ACTUALLY observed. A sweep that looked at one
/// of two artifacts confirms one of two, and can never read as a full pass.
#[test]
fn a_partial_sweep_confirms_only_what_it_saw() {
    let mut g = baselined();
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:other".into(),
        name: "Other.cs".into(),
        location: Some("src/Other.cs".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:score".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:bbb".into()),
    })
    .unwrap();

    let report = g
        .reconcile_artifacts(
            &[observed("art:score", "sha256:aaa")],
            &recording(Some("2026-08-01")),
        )
        .unwrap();

    assert_eq!(report.confirmed, vec!["art:score".to_string()]);
    assert!(
        !g.get_node(node::ARTIFACT, "art:other")
            .unwrap()
            .unwrap()
            .properties
            .contains_key("last_confirmed_at"),
        "an artifact nobody looked at must not be confirmed by a sweep past it"
    );
}

/// **COUNTERWEIGHT.** A confirmation says the bytes matched. An artifact that
/// drifted is reported as drift and confirmed of nothing — otherwise the same
/// pass would both raise a divergence and certify the thing that diverged.
#[test]
fn a_drifted_artifact_is_never_confirmed() {
    let mut g = baselined();
    let report = g
        .reconcile_artifacts(
            &[observed("art:score", "sha256:MOVED")],
            &recording(Some("2026-08-01")),
        )
        .unwrap();

    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, DriftKind::ChecksumChange);
    assert!(report.confirmed.is_empty());
    assert!(
        !g.get_node(node::ARTIFACT, "art:score")
            .unwrap()
            .unwrap()
            .properties
            .contains_key("last_confirmed_at")
    );
}

/// A confirmation exists to answer *when*, and the core takes no clock. So an
/// undated pass cannot write one — and **says which artifacts it therefore
/// skipped**, rather than leaving the caller to notice that a count did not
/// move, which is the exact shape of the bug being fixed.
#[test]
fn an_undated_confirmation_is_skipped_out_loud() {
    let mut g = baselined();
    let report = g
        .reconcile_artifacts(&[observed("art:score", "sha256:aaa")], &recording(None))
        .unwrap();

    assert_eq!(report.unchanged, 1);
    assert!(report.confirmed.is_empty());
    assert_eq!(
        report.unconfirmed_undated,
        vec!["art:score".to_string()],
        "a dropped write the caller has to infer is the silent drop this \
         project forbids"
    );
    assert!(
        !g.get_node(node::ARTIFACT, "art:score")
            .unwrap()
            .unwrap()
            .properties
            .contains_key("last_confirmed_at")
    );
}

#[test]
fn a_later_sweep_moves_the_confirmation_forward() {
    let mut g = baselined();
    g.reconcile_artifacts(
        &[observed("art:score", "sha256:aaa")],
        &recording(Some("2026-08-01")),
    )
    .unwrap();
    g.reconcile_artifacts(
        &[observed("art:score", "sha256:aaa")],
        &recording(Some("2026-09-15")),
    )
    .unwrap();

    let ledger = g.confirmation_ledger().unwrap();
    assert_eq!(
        ledger.claims[0].last_confirmed_at.as_deref(),
        Some("2026-09-15"),
        "the ledger answers when it was LAST checked"
    );
    assert_eq!(
        ledger.claims[0].confirmations, 1,
        "confirmations count artifacts under the claim, not passes over them"
    );
}
