//! "Verified" splits into a check something can RE-RUN and a check somebody TYPED.
//!
//! MEASURED 2026-09-22 on reflow2's own design: 312 Verifications, **311
//! reading `passing`, 282 with no executable form, 46 never run**, and the
//! coverage line reported "241/288 capability(ies) verified" with no hint that
//! most of that rests on an assertion nothing can re-check. A reader meeting
//! that number concludes the design is thoroughly verified. The code underneath
//! it is genuinely at 85% line coverage — the TESTS are real; the RECORD of
//! them is what was not.
//!
//! That gap is what an outside reader called "an AI coding project", and the
//! half the criticism lands on is this one
//! (`fact:the-verification-record-is-mostly-unfalsifiable-and-the-ai-coding-project-critique-lands-on-that-half`).
//!
//! ⭐ THE FIX IS A COUNT, NOT A JUDGEMENT. reflow2 does not decide whether a
//! check is any good — `dec:non-goal-reflow2-does-not-judge-whether-a-check-is-meaningful`
//! stands, and this does not touch it. It reports a fact it already holds:
//! whether anything IMPLEMENTS the check, which is exactly what
//! `has_executable_form` has meant all along in the loop digest. The number
//! stops conflating "somebody ran this" with "somebody said this".
//!
//! WHY THE ASSERTIONS READ THE SERIALIZED FORM: so this compiled against the
//! code before the field existed and failed on the assertion rather than on the
//! compiler. Observed failing that way first.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, LinkArtifactOptions};

/// A capability with a PASSING check that nothing implements — the shape 282
/// of this design's 312 checks are actually in.
fn a_design_with_a_claimed_check() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_capability("cap:thing", "Does the thing", "it does the thing", None)
        .expect("capability");
    g.add_verification("ver:thing", "the thing is checked", None, None, None)
        .expect("verification");
    g.verifies("ver:thing", node::CAPABILITY, "cap:thing")
        .expect("verifies");
    g.set_verification_status("ver:thing", "passing", None, None)
        .expect("passing");
    g
}

/// Give a check an executable form: register the file and draw the IMPLEMENTS
/// edge from it to the Verification.
///
/// 🛑 BOTH HALVES ARE NEEDED, AND THAT IS ITSELF THE FINDING. `link_artifact`
/// draws REALIZES and only REALIZES — whatever the target type. So the
/// discoverable route ("register the file that is the check") produces an
/// artifact pointing at the Verification and NO executable form, which is
/// exactly what `has_executable_form`, `quantity_check_without_executable_form`
/// and the count below all read. Nothing in the tool's own reply says a second
/// edge is owed; `detect_gaps` says so later, in prose, to whoever happens to
/// read that finding. That is a plausible cause of the 282 and it is recorded
/// here because this test is where it was met.
///
/// `LinkArtifactOptions` has no `Default`, so every field is named once here
/// rather than at each call.
fn give_the_check_an_executable_form(g: &mut DesignGraph, verification_id: &str) {
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:thing-test".into(),
        name: Some("thing_test.rs".into()),
        location: Some("tests/thing_test.rs".into()),
        description: None,
        artifact_type: Some("test".into()),
        content_ref: None,
        note_kind: None,
        target_type: node::VERIFICATION.into(),
        target_id: verification_id.into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:deadbeef".into()),
    })
    .expect("register the file that runs the check");
    g.create_edge(
        edge::IMPLEMENTS,
        node::ARTIFACT,
        "art:thing-test",
        node::VERIFICATION,
        verification_id,
        Props::new(),
    )
    .expect("the file IMPLEMENTS the check");
}

fn coverage(g: &DesignGraph) -> serde_json::Value {
    serde_json::to_value(g.verification_coverage().expect("coverage")).expect("serializable")
}

#[test]
fn a_passing_check_nothing_implements_counts_as_verified_but_not_as_runnable() {
    let g = a_design_with_a_claimed_check();
    let v = coverage(&g);

    assert_eq!(
        v["capabilities_verified"], 1,
        "the existing count is unchanged — this split must not move the number \
         anyone has been reading"
    );
    assert_eq!(
        v["capabilities_verified_by_runnable_check"], 0,
        "a passing check that NOTHING implements is a claim, not a run. Counting it as \
         runnable is how 311 passing checks read as a verified design while 282 of them \
         could not be re-checked by anything"
    );
}

#[test]
fn implementing_the_check_makes_it_runnable() {
    let mut g = a_design_with_a_claimed_check();
    give_the_check_an_executable_form(&mut g, "ver:thing");

    let v = coverage(&g);
    assert_eq!(
        v["capabilities_verified"], 1,
        "still verified — implementing a check does not change whether it passes"
    );
    assert_eq!(
        v["capabilities_verified_by_runnable_check"], 1,
        "once something IMPLEMENTS the check, the claim is re-checkable and counts as such. \
         This is the same signal `has_executable_form` already reports per check in the loop \
         digest, rolled up to the capability"
    );
}

/// The bar is a COUNT, never a verdict: a check that is implemented but NOT
/// passing must not count as runnable-verified either, or the split would
/// quietly re-introduce the defect it exists to remove — counting the existence
/// of a test rather than its result.
#[test]
fn an_implemented_check_that_is_not_passing_counts_as_neither() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_capability("cap:thing", "Does the thing", "it does the thing", None)
        .expect("capability");
    g.add_verification("ver:thing", "the thing is checked", None, None, None)
        .expect("verification");
    g.verifies("ver:thing", node::CAPABILITY, "cap:thing")
        .expect("verifies");
    g.set_verification_status("ver:thing", "failing", None, None)
        .expect("failing");
    give_the_check_an_executable_form(&mut g, "ver:thing");

    let v = coverage(&g);
    assert_eq!(
        v["capabilities_verified"], 0,
        "a failing check is not verification — unchanged behaviour, asserted here so the \
         split cannot regress it"
    );
    assert_eq!(
        v["capabilities_verified_by_runnable_check"], 0,
        "runnable is a NARROWING of verified, never a second way to be verified"
    );
}
