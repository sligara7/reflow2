//! Registering the file that IS a check must give that check an executable form.
//!
//! THE DEFECT, met by hand on 2026-09-22 and then measured. `link_artifact`
//! ends in `self.realizes(...)` whatever the target type, so the discoverable
//! way to record a check's executable form — "name the file that IS the check",
//! which `detect_gaps`' own `quantity_check_without_executable_form` text tells
//! you to do — produced an Artifact REALIZING the Verification and ZERO
//! incoming IMPLEMENTS. `has_executable_form` reads IMPLEMENTS, so the check
//! still reported as having no form, and the tool's reply said nothing about a
//! second edge being owed.
//!
//! ⭐ AND REALIZES IS NOT MERELY INCOMPLETE HERE — IT IS THE WRONG EDGE, by the
//! schema's own words. `IMPLEMENTS`' extraction hint reads: "Not REALIZES (that
//! says a file implements a CAPABILITY; a check interrogates one rather than
//! providing it)." REALIZES reaches Verification only through `to: "*"`, and
//! the note on IMPLEMENTS warned about exactly this: "a wildcard would ACCEPT
//! this pair and never MODEL it". The wildcard accepted it for months.
//!
//! MEASURED on this design's own export the same day: 40 Verifications carried
//! a REALIZES from a registered file and no IMPLEMENTS — somebody did the
//! instructed thing and got a formless check — while 30 of the 31 that DID
//! have a form were drawn purely by `create_edge`, bypassing `link_artifact`
//! altogether. The route that worked was the one nothing pointed at.
//!
//! OBSERVED FAILING before the fix: `an_implements_edge_is_drawn` and
//! `the_wrong_edge_is_not_drawn` both failed, and
//! `registering_the_file_alone_makes_the_capability_runnable` failed on the
//! count — which is the whole user-visible consequence.

use reflow2_core::nodes::{edge, node};
use reflow2_core::{DesignGraph, LinkArtifactOptions};

fn options(target_type: &str, target_id: &str) -> LinkArtifactOptions {
    LinkArtifactOptions {
        artifact_id: "art:thing-test".into(),
        name: Some("thing_test.rs".into()),
        location: Some("tests/thing_test.rs".into()),
        description: None,
        artifact_type: Some("test".into()),
        content_ref: None,
        note_kind: None,
        target_type: target_type.into(),
        target_id: target_id.into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:deadbeef".into()),
    }
}

/// A capability with a passing check, and nothing yet naming what runs it.
fn a_design_with_a_check() -> DesignGraph {
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

#[test]
fn an_implements_edge_is_drawn() {
    let mut g = a_design_with_a_check();
    g.link_artifact(options(node::VERIFICATION, "ver:thing"))
        .expect("register the file that is the check");

    let implements = g
        .incoming("ver:thing", Some(edge::IMPLEMENTS))
        .expect("incoming IMPLEMENTS");
    assert_eq!(
        implements.len(),
        1,
        "registering the file that IS a check must give the check an executable form. \
         Without this the instructed route produces a check nothing can re-run, and the \
         caller is never told a second edge is owed"
    );
    assert_eq!(implements[0].from_id, "art:thing-test");
}

#[test]
fn the_wrong_edge_is_not_drawn() {
    let mut g = a_design_with_a_check();
    g.link_artifact(options(node::VERIFICATION, "ver:thing"))
        .expect("register the file that is the check");

    assert!(
        g.incoming("ver:thing", Some(edge::REALIZES))
            .expect("incoming REALIZES")
            .is_empty(),
        "REALIZES is the WRONG edge for this pair, not merely an incomplete one: the \
         schema's own hint on IMPLEMENTS says \"Not REALIZES (that says a file implements a \
         CAPABILITY; a check interrogates one rather than providing it)\". It was only ever \
         accepted here because REALIZES is declared `to: \"*\"`"
    );
}

/// The user-visible consequence, asserted end to end rather than as an edge:
/// doing the one instructed thing is enough.
#[test]
fn registering_the_file_alone_makes_the_capability_runnable() {
    let mut g = a_design_with_a_check();
    let before = g.verification_coverage().expect("coverage");
    assert_eq!(
        before.capabilities_verified_by_runnable_check, 0,
        "nothing runs the check yet"
    );

    g.link_artifact(options(node::VERIFICATION, "ver:thing"))
        .expect("register the file that is the check");

    let after = g.verification_coverage().expect("coverage");
    assert_eq!(
        after.capabilities_verified, 1,
        "unchanged — registering a file does not change whether the check passes"
    );
    assert_eq!(
        after.capabilities_verified_by_runnable_check, 1,
        "ONE call, the one the gap text asks for, and the check is now re-runnable. This is \
         the assertion that would have caught the defect: every other signal in the design \
         read green while the answer was wrong"
    );
}

/// The unchanged half. REALIZES is right for a capability, and this fix must
/// not disturb the 400-odd artifacts already registered that way.
#[test]
fn registering_a_file_against_a_capability_still_realizes_it() {
    let mut g = a_design_with_a_check();
    g.link_artifact(options(node::CAPABILITY, "cap:thing"))
        .expect("register a file against a capability");

    assert_eq!(
        g.incoming("cap:thing", Some(edge::REALIZES))
            .expect("incoming REALIZES")
            .len(),
        1,
        "a file realizing a CAPABILITY is exactly what REALIZES means, and is untouched"
    );
    assert!(
        g.incoming("cap:thing", Some(edge::IMPLEMENTS))
            .expect("incoming IMPLEMENTS")
            .is_empty(),
        "IMPLEMENTS is declared `to: Verification` only — it must not leak onto other targets"
    );
}
