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
//!
//! ## LENGTH is a dialect too (BL-160)
//!
//! The same bug came back a third time in a second form, and reflow2's own
//! design is where it was caught: `tools/build_design_graph.py` registers
//! `hexdigest()[:16]` while an honest caller running `sha256sum` supplies all
//! 64, so on 2026-08-01 a full sweep of a provably clean tree reported **51
//! phantom drifts** in the same minute the coherence gate said `OK — design and
//! build agree`. The gate was right for the wrong reason: `reflow2_check.py`
//! carried a Python workaround truncating the observation to the registered
//! length. **The compensation lived in the wrong layer** — every consumer that
//! is not the gate (an agent driving `reconcile_artifacts` over MCP, another
//! project's CI, the coding agent this tool's own description tells to *"compute
//! the hashes yourself"*) hit the bug the gate was immune to.
//!
//! Two things follow from putting it in the core, and both are pinned below.
//! The comparison is a real PREFIX relationship, never a truncate-both-to-N:
//! two full digests that happen to share sixteen characters are still drift.
//! And it applies to the `sha256:` dialect only — a prefix rule let loose on an
//! arbitrary fingerprint would call `blake3:zz` and `blake3:zzzz` the same
//! thing, which is the "normalise everything into agreement" failure again.
//!
//! **A short baseline is a weak baseline, and that is a property of what was
//! registered, not of the comparison.** No minimum prefix length is imposed
//! here: the write side accepts a 16-char digest without complaint, so a read
//! side that refused to honour it would be the same write/read disagreement
//! this whole file exists to close.

use reflow2_core::drift::{DriftKind, ObservedArtifact, ReconcileOptions};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, node};
use reflow2_core::{DriftDisposition, LinkArtifactOptions};

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
        conformance: None,
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

/// The first sixteen characters of [`BARE`] — the length
/// `tools/build_design_graph.py` registers, and therefore the length most of
/// reflow2's own baselines are stored at.
const SHORT: &str = "fb7da9167309360e";
/// Sixteen characters that are NOT a prefix of [`BARE`]: it diverges in the
/// final character, so an observation truncated to this length still disagrees.
const SHORT_OTHER: &str = "fb7da9167309360f";
/// A full digest sharing exactly [`SHORT`]'s sixteen characters with [`BARE`]
/// and differing everywhere after. Two full digests, so neither is a prefix of
/// the other — the case a fixed truncate-to-16 would call agreement.
const NEAR: &str = "fb7da9167309360e000000000000000000000000000000000000000000000000";

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

// ---------------------------------------------------------------------------
// BL-160 — LENGTH is a dialect too
// ---------------------------------------------------------------------------

/// THE BUG, in the form that produced 51 phantom drifts on reflow2's own clean
/// tree. The baseline was registered at sixteen characters (which is what
/// `build_design_graph.py` writes); the caller hashes the file with `sha256sum`
/// and supplies all sixty-four. Nothing moved, and before the fix every one of
/// them reported `checksum_change`.
#[test]
fn a_full_digest_matches_a_truncated_baseline() {
    let mut g = thread_with_baseline(SHORT);
    let report = g
        .reconcile_artifacts(&[observed(BARE)], &ReconcileOptions::default())
        .expect("reconcile");

    assert!(
        report.findings.is_empty(),
        "a full digest whose prefix IS the baseline is not drift: {:?}",
        report.findings
    );
    assert_eq!(report.unchanged, 1);
    assert!(
        report.propagation_seeds.is_empty(),
        "a false drift would seed propagation from a file nobody touched"
    );
}

/// The mirror, and the direction `reflow2_check.py` used to take by hand: the
/// design holds a full digest and the observer supplies a truncated one. A fix
/// that only tolerated observed-longer-than-recorded would leave half the bug.
#[test]
fn a_truncated_observation_matches_a_full_baseline() {
    let mut g = thread_with_baseline(BARE);
    let report = g
        .reconcile_artifacts(&[observed(SHORT)], &ReconcileOptions::default())
        .expect("reconcile");

    assert!(
        report.findings.is_empty(),
        "a truncated observation of the same digest is not drift: {:?}",
        report.findings
    );
    assert_eq!(report.unchanged, 1);
}

/// THE COUNTERWEIGHT that keeps the length rule from being a way to stop
/// noticing. The relationship required is a real PREFIX, not equality after
/// truncating both sides to some fixed width: two full digests sharing their
/// first sixteen characters are two different digests, and the file really did
/// change.
#[test]
fn two_full_digests_sharing_a_prefix_are_still_drift() {
    let mut g = thread_with_baseline(BARE);
    let report = g
        .reconcile_artifacts(&[observed(NEAR)], &ReconcileOptions::default())
        .expect("reconcile");

    assert_eq!(
        report.findings.len(),
        1,
        "sharing sixteen characters is not being the same digest"
    );
    assert_eq!(report.findings[0].kind, DriftKind::ChecksumChange);
    assert_eq!(report.unchanged, 0);
}

/// The same counterweight from the short side: an observation truncated to the
/// baseline's own length that disagrees WITHIN that length is drift. Without
/// this, the rule could be implemented as "the shorter one wins, so stop
/// looking" and the tests above would all still pass.
#[test]
fn a_truncated_observation_that_differs_is_still_drift() {
    let mut g = thread_with_baseline(BARE);
    let report = g
        .reconcile_artifacts(&[observed(SHORT_OTHER)], &ReconcileOptions::default())
        .expect("reconcile");

    assert_eq!(report.findings.len(), 1, "a real disagreement must survive");
    assert_eq!(report.findings[0].kind, DriftKind::ChecksumChange);
}

/// The dialect boundary. Prefix tolerance is a fact about hex digests of one
/// algorithm, where a truncation is a genuine prefix of the whole. Applied to
/// an arbitrary fingerprint it becomes the "normalise everything into
/// agreement" failure the file's first counterweight already forbids — so
/// `blake3:zz` and `blake3:zzzz` stay two different fingerprints.
#[test]
fn a_prefix_of_a_non_sha256_fingerprint_is_not_agreement() {
    let mut g = thread_with_baseline("blake3:zzzz");
    let report = g
        .reconcile_artifacts(&[observed("blake3:zz")], &ReconcileOptions::default())
        .expect("reconcile");

    assert_eq!(
        report.findings.len(),
        1,
        "prefix tolerance belongs to the sha256 dialect, not to every string"
    );
    assert_eq!(report.findings[0].kind, DriftKind::ChecksumChange);
}

/// The degenerate end of the prefix rule, and the reason it is guarded rather
/// than left to `starts_with`: every string starts with the empty string, so a
/// baseline of `sha256:` with no digest behind it would otherwise agree with
/// whatever it was shown. A design can hold one — `create_node` upserts without
/// canonicalising — and it must read as a disagreement, never as a match.
#[test]
fn an_empty_digest_agrees_with_nothing() {
    let mut g = thread_with_baseline(BARE);
    g.upsert_node(
        node::ARTIFACT,
        "art:score",
        Props::new().set("checksum", "sha256:"),
    )
    .expect("upsert an empty digest");

    let report = g
        .reconcile_artifacts(&[observed(BARE)], &ReconcileOptions::default())
        .expect("reconcile");

    assert_eq!(
        report.findings.len(),
        1,
        "an empty baseline is not a baseline that matches everything"
    );
    assert_eq!(report.findings[0].kind, DriftKind::ChecksumChange);
}

/// The other half of the bug, and the one that bites a BL-157 sweep rather than
/// a gate. `set_artifact_checksum` guards `baseline_established` by asking
/// whether the value would MOVE an existing baseline — a **string** comparison,
/// so registering the full digest over a 16-char baseline of the same file was
/// refused as laundering a real drift. That is the bulk sweep of
/// `set_artifact_checksums` failing on every short-registered artifact for a
/// change that never happened.
#[test]
fn re_establishing_a_baseline_in_a_longer_dialect_is_not_moving_it() {
    let mut g = thread_with_baseline(SHORT);
    g.set_artifact_checksum(
        "art:score",
        BARE,
        DriftDisposition::BaselineEstablished,
        None,
        Some("2026-08-01"),
    )
    .expect("the same digest, said more precisely, is not a move");
}

/// THE COUNTERWEIGHT to that, and it is the one that matters: the guard exists
/// to stop `baseline_established` accepting a real change without answering
/// what the change meant. A genuinely different digest is still refused, and
/// the refusal still names the two dispositions that would be honest.
///
/// The baseline here is the FULL digest and the accept shares its first sixteen
/// characters, which is the same shape as the drift counterweight above —
/// deliberately, because it is the case a truncate-both-to-N rule would let
/// through. (Against a SHORT baseline, `NEAR` is a legitimate extension of it
/// and correctly accepted: a 16-char baseline only ever pinned 16 characters.
/// Writing this test that way first is how that was established rather than
/// assumed.)
#[test]
fn a_genuinely_different_digest_is_still_refused_as_a_first_baseline() {
    let mut g = thread_with_baseline(BARE);
    let err = g
        .set_artifact_checksum(
            "art:score",
            NEAR,
            DriftDisposition::BaselineEstablished,
            None,
            Some("2026-08-01"),
        )
        .expect_err("a different digest MOVES the baseline");
    let msg = format!("{err}");
    assert!(
        msg.contains("design_holds") && msg.contains("design_updated"),
        "the refusal must still name the disposition that is right: {msg}"
    );
}

/// Precision is never silently lost. When the two dialects agree, the LONGER
/// digest is what stays on the record: it contradicts nothing the shorter one
/// said and carries more of the evidence. It also makes the accept idempotent
/// ACROSS dialects — re-establishing with the short form afterwards keys off
/// the stored value, so one baseline is one claim rather than two.
#[test]
fn agreeing_dialects_keep_the_longer_digest_and_mint_one_claim() {
    let mut g = thread_with_baseline(SHORT);
    let (_, first) = g
        .set_artifact_checksum(
            "art:score",
            BARE,
            DriftDisposition::BaselineEstablished,
            None,
            Some("2026-08-01"),
        )
        .expect("register the full digest");

    let stored = g
        .get_node(node::ARTIFACT, "art:score")
        .expect("get")
        .expect("artifact")
        .properties
        .get("checksum")
        .and_then(|v| v.as_str())
        .expect("checksum")
        .to_string();
    assert_eq!(
        stored, PREFIXED,
        "the longer digest survives: it says everything the shorter one did"
    );

    let (_, second) = g
        .set_artifact_checksum(
            "art:score",
            SHORT,
            DriftDisposition::BaselineEstablished,
            None,
            Some("2026-08-01"),
        )
        .expect("the shorter form still agrees");
    assert_eq!(
        first, second,
        "one baseline is one claim, whichever dialect re-states it"
    );
    assert_eq!(
        g.get_node(node::ARTIFACT, "art:score")
            .expect("get")
            .expect("artifact")
            .properties
            .get("checksum")
            .and_then(|v| v.as_str()),
        Some(PREFIXED),
        "re-stating it more coarsely must not weaken the record"
    );
}
