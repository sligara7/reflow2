//! A digest is a digest, whichever dialect it arrives in (BL-125).
//!
//! `canonical_checksum` exists because on 2026-07-25 four artifacts registered
//! from raw `sha256sum` output made the coherence gate report every one of them
//! as drifted while the bytes matched exactly — *"a false red on a gate whose
//! whole job is to be believed is worse than no gate."* That fix was applied to
//! the two WRITE sites and never to the comparison, so the same false red came
//! straight back through the read door: a bare hash passed to `link_artifact`
//! and the same bare hash passed to `reconcile_artifacts` reported
//! `checksum_change` on every artifact of a tree nobody had touched.
//!
//! It fails as a FALSE POSITIVE rather than an error, which is what makes it
//! expensive: the output is well-formed, carries correct `realizes` edges and
//! correct `propagation_seeds`, and says everything drifted. The natural
//! response — re-register everything — overwrites the baselines and hides it
//! for another cycle.
//!
//! The counterweight cases matter as much as the bug case. A fix that made
//! every comparison equal would pass the first three tests here and destroy the
//! detector, so genuine drift and a fingerprint that is NOT a bare hex digest
//! are both pinned.

use reflow2_core::LinkArtifactOptions;
use reflow2_core::drift::{DriftKind, ObservedArtifact, ReconcileOptions};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, node};

/// A golden thread with one registered artifact. `checksum` is written through
/// `link_artifact`, so it lands canonicalised exactly as a real caller's would.
fn thread_with_baseline(checksum: &str) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_requirement("req:live", "Live scores", "scores update live")
        .expect("req");
    g.add_capability("cap:score", "Scoring", "tracks the score", None)
        .expect("cap");
    g.satisfies("cap:score", "req:live").expect("satisfies");
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
        checksum: Some(checksum.into()),
    })
    .expect("link");
    g
}

fn observed(checksum: &str) -> ObservedArtifact {
    ObservedArtifact {
        artifact_id: "art:score".into(),
        present: true,
        checksum: Some(checksum.into()),
        realizes: None,
    }
}

const BARE: &str = "fb7da9167309360e6b2d3f5a4c8e1d0a9f3b6c2e5d8a1b4c7e0f3a6d9c2b5e8f";
const PREFIXED: &str = "sha256:fb7da9167309360e6b2d3f5a4c8e1d0a9f3b6c2e5d8a1b4c7e0f3a6d9c2b5e8f";
const OTHER: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// THE BUG. `link_artifact` canonicalises a bare digest on the way in, so the
/// stored baseline is prefixed; the caller then hands `reconcile_artifacts` the
/// same bare digest it computed, and before the fix every artifact reported
/// drift. This is the exact sequence a first-time user follows, because both
/// docstrings read as if a bare hash is fine — `reconcile`'s actively says
/// "compute the hashes yourself (any algorithm, used consistently)".
#[test]
fn a_bare_observed_digest_matches_a_prefixed_baseline() {
    let mut g = thread_with_baseline(BARE);
    let report = g
        .reconcile_artifacts(&[observed(BARE)], &ReconcileOptions::default())
        .expect("reconcile");

    assert!(
        report.findings.is_empty(),
        "the same digest in two dialects is not drift: {:?}",
        report.findings
    );
    assert_eq!(report.unchanged, 1);
    assert!(
        report.propagation_seeds.is_empty(),
        "a false drift would seed propagation from a file nobody touched"
    );
}

/// The mirror, and not hypothetical: `create_node` upserts without
/// canonicalising, and any graph written before the 2026-07-25 write-side fix
/// holds bare baselines. Such a graph must not start reporting drift the moment
/// a caller supplies the canonical form.
#[test]
fn a_prefixed_observed_digest_matches_a_bare_baseline() {
    let mut g = thread_with_baseline(PREFIXED);
    // Force the stored baseline back to the bare dialect, bypassing the
    // write-side canonicaliser the way an older graph or a raw upsert would.
    g.upsert_node(
        node::ARTIFACT,
        "art:score",
        Props::new().set("checksum", BARE),
    )
    .expect("upsert bare baseline");

    let report = g
        .reconcile_artifacts(&[observed(PREFIXED)], &ReconcileOptions::default())
        .expect("reconcile");

    assert!(
        report.findings.is_empty(),
        "a pre-canonicalisation baseline must still match its own digest: {:?}",
        report.findings
    );
    assert_eq!(report.unchanged, 1);
}

/// THE COUNTERWEIGHT. A fix that normalised everything into equality would
/// silence the detector and pass every case above. Two genuinely different
/// digests must still report, and must still name the design they affect.
#[test]
fn a_genuine_change_is_still_drift_across_dialects() {
    let mut g = thread_with_baseline(BARE);
    let report = g
        .reconcile_artifacts(&[observed(OTHER)], &ReconcileOptions::default())
        .expect("reconcile");

    assert_eq!(report.findings.len(), 1, "real drift must survive the fix");
    assert_eq!(report.findings[0].kind, DriftKind::ChecksumChange);
    assert_eq!(
        report.findings[0].realizes,
        vec!["cap:score"],
        "drift must still name the design node the file realizes"
    );
    assert_eq!(report.unchanged, 0);
}

/// One drift, one event — whichever dialect the observation arrived in. The
/// event id hashes the observed checksum as part of its identity, so leaving
/// the raw form in place would file the same divergence twice under two ids,
/// which is the same bug one layer down.
#[test]
fn the_drift_event_id_does_not_depend_on_the_dialect_observed() {
    let bare_id = {
        let mut g = thread_with_baseline(BARE);
        let report = g
            .reconcile_artifacts(
                &[observed(OTHER)],
                &ReconcileOptions {
                    record_events: true,
                    ..Default::default()
                },
            )
            .expect("reconcile");
        report.findings[0].event_id.clone().expect("event recorded")
    };

    let prefixed_id = {
        let mut g = thread_with_baseline(BARE);
        let report = g
            .reconcile_artifacts(
                &[observed(&format!("sha256:{OTHER}"))],
                &ReconcileOptions {
                    record_events: true,
                    ..Default::default()
                },
            )
            .expect("reconcile");
        report.findings[0].event_id.clone().expect("event recorded")
    };

    assert_eq!(
        bare_id, prefixed_id,
        "the same file at the same content must be one drift event, not two"
    );
}

/// The other counterweight: this normalises a KNOWN dialect, it does not police
/// the field. A fingerprint that is not a bare hex digest is left verbatim, so
/// a real difference between two algorithms' output still reports rather than
/// being massaged into agreement.
#[test]
fn a_non_hex_fingerprint_is_not_normalised_into_agreement() {
    let mut g = thread_with_baseline("blake3:zzzz");
    let report = g
        .reconcile_artifacts(&[observed("zzzz")], &ReconcileOptions::default())
        .expect("reconcile");

    assert_eq!(
        report.findings.len(),
        1,
        "an unknown prefix is not this fix's business to strip"
    );
    assert_eq!(report.findings[0].kind, DriftKind::ChecksumChange);
}
