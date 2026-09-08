//! Re-calling a constructor must not erase what the design already knew
//! (BL-183 — BL-46 and BL-166's shape, a third and general time).
//!
//! # The defect, stated once
//!
//! Every `add_*` helper names a **subset** of its node type's properties and
//! writes through create-or-**replace**. So calling one with an id that already
//! exists — which is exactly what the **revise-design** skill prescribes for
//! changing a node's text — silently resets every property the helper does not
//! name back to its schema default. The write returns success. Nothing reports
//! a downgrade.
//!
//! It is invisible until a property has been moved **off** its default, which
//! is why it survived three encounters: on a freshly-captured node the reset is
//! a no-op, and the one time it mattered the value that vanished was a `status`
//! nobody re-read.
//!
//! # Why this is worse than losing a field
//!
//! The properties these helpers omit are the ones that carry the **user's
//! word**:
//!
//! - `Requirement.status` is where certainty comes from (`dec:certainty-derived`)
//!   — every move off `proposed` records the user confirming, deferring or
//!   dropping. Resetting it forges their signature in reverse.
//! - `Decision.status` back to `proposed` re-opens a settled fork: a proposed
//!   Decision with alternatives is an open decision point `detect_gaps`
//!   reports, so a settled choice comes back as a question the user already
//!   answered — the precise failure `dec:reopen-supersedes` exists to prevent,
//!   arriving through a code path instead of a choice.
//! - `Interface.designation` is how the design says which contracts are the
//!   **published boundaries** (`req:key-interfaces`), and computations read it.
//! - `Artifact.checksum` / `last_confirmed_at` is BL-166 exactly, which was
//!   supposed to be fixed.
//!
//! Each case below sets a property off its default, re-calls the constructor
//! the way a reviser would, and asserts the design still knows what it knew.
//! **Written before the fix**: every one of them failed first.
//!
//! # The scope of this file was drawn on a NAME, and that let one through
//!
//! Until 2026-09-07 every case here was an `add_*` constructor, and the note on
//! `re_adding_an_artifact_does_not_erase_its_confirmation` said `link_artifact`
//! "was fixed for this in BL-166". It was fixed for every field a caller could
//! OMIT and not for `name`, which was mandatory — so the one thing a re-linker
//! could not say was "leave the name alone", and every re-link rewrote it.
//!
//! The class is not "calls beginning with add_". It is **any write path that
//! builds a node from a partial property set**, and membership had been decided
//! by how the call was spelled. That is why the linker's cases now live here.

use reflow2_core::DesignGraph;
use reflow2_core::Value;
use reflow2_core::nodes::node;

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open in-memory graph")
}

/// One property of a node, as a string, or `<absent>`.
fn prop(g: &DesignGraph, node_type: &str, id: &str, key: &str) -> String {
    g.get_node(node_type, id)
        .expect("get_node")
        .expect("node exists")
        .properties
        .get(key)
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => format!("{other:?}"),
        })
        .unwrap_or_else(|| "<absent>".to_string())
}

// ---------------------------------------------------------------------------
// The user's word.
// ---------------------------------------------------------------------------

/// The one that files the row: an accepted Decision must not come back as
/// `proposed` because someone corrected a typo in its text.
#[test]
fn revising_a_decision_does_not_reopen_it() {
    let mut g = graph();
    g.add_decision("dec:settled", "Settled", "We chose A.", Some("Because A."))
        .expect("decision");
    g.set_decision_status("dec:settled", "accepted")
        .expect("accept");

    // The revise-design move: same id, corrected prose.
    g.add_decision(
        "dec:settled",
        "Settled",
        "We chose A (corrected).",
        Some("Because A."),
    )
    .expect("revise");

    assert_eq!(
        prop(&g, node::DECISION, "dec:settled", "status"),
        "accepted",
        "a settled fork must not reopen because its wording changed"
    );
    assert_eq!(
        prop(&g, node::DECISION, "dec:settled", "decision"),
        "We chose A (corrected).",
        "and the revision must still have happened"
    );
}

/// Certainty is derived from `Requirement.status`, and every move off
/// `proposed` is the user's word. A reworded requirement must not silently
/// un-confirm itself.
#[test]
fn rewording_a_requirement_does_not_un_confirm_it() {
    let mut g = graph();
    g.add_requirement("req:one", "First", "The system shall do the thing.")
        .expect("requirement");
    g.set_requirement_status("req:one", "accepted")
        .expect("accept");
    g.set_provenance(node::REQUIREMENT, "req:one", "inferred")
        .expect("provenance");

    g.add_requirement(
        "req:one",
        "First",
        "The system shall do the thing, precisely.",
    )
    .expect("reword");

    assert_eq!(
        prop(&g, node::REQUIREMENT, "req:one", "status"),
        "accepted",
        "resetting this forges the user's signature in reverse (dec:certainty-derived)"
    );
    assert_eq!(
        prop(&g, node::REQUIREMENT, "req:one", "provenance"),
        "inferred",
        "how the requirement entered the graph is not changed by rewording it"
    );
}

/// A capability that is built and checked must not read as `planned` again
/// because its description was sharpened.
#[test]
fn rewording_a_capability_does_not_unbuild_it() {
    let mut g = graph();
    g.add_capability("cap:one", "Thing", "Does the thing.", None)
        .expect("capability");
    g.set_capability_status("cap:one", "verified")
        .expect("verify");

    g.add_capability("cap:one", "Thing", "Does the thing, reliably.", None)
        .expect("reword");

    assert_eq!(
        prop(&g, node::CAPABILITY, "cap:one", "status"),
        "verified",
        "a verified capability must not silently revert to planned"
    );
}

// ---------------------------------------------------------------------------
// Published boundaries and contracts.
// ---------------------------------------------------------------------------

/// `Interface` is the worst case in the audit — thirteen properties, one of
/// which says whether this is a **published boundary** that computations read
/// (`req:key-interfaces`), and the rest of which are the contract itself.
/// `set_interface_spec` documents that it can be called repeatedly as a spec
/// fills in; re-calling `add_interface` must not undo all of it.
#[test]
fn renaming_an_interface_does_not_discard_the_contract() {
    let mut g = graph();
    g.add_interface("ifc:api", "The API").expect("interface");
    g.set_interface_designation("ifc:api", "published")
        .expect("designation");
    g.set_interface_spec(
        "ifc:api",
        None,
        None,
        Some("json"),
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("spec");

    g.add_interface("ifc:api", "The Public API")
        .expect("rename");

    assert_eq!(
        prop(&g, node::INTERFACE, "ifc:api", "designation"),
        "published",
        "a published boundary must not stop being one because it was renamed"
    );
    assert_eq!(
        prop(&g, node::INTERFACE, "ifc:api", "payload_format"),
        "json",
        "the contract must survive a rename"
    );
    assert_eq!(
        prop(&g, node::INTERFACE, "ifc:api", "name"),
        "The Public API"
    );
}

// ---------------------------------------------------------------------------
// BL-166 again, from the other door.
// ---------------------------------------------------------------------------

/// `link_artifact` was fixed for this in BL-166. `add_artifact` — the bare
/// constructor the instructions offer as its sibling — was not, so the same
/// erasure is still reachable one call away.
#[test]
fn re_adding_an_artifact_does_not_erase_its_confirmation() {
    let mut g = graph();
    g.add_artifact("art:one", "one.rs", Some("code"), Some("src/one.rs"))
        .expect("artifact");
    g.set_artifact_checksum(
        "art:one",
        "sha256:abc",
        reflow2_core::DriftDisposition::BaselineEstablished,
        None,
        Some("2026-08-02"),
    )
    .expect("checksum");

    g.add_artifact("art:one", "one.rs", Some("code"), Some("src/lib/one.rs"))
        .expect("re-register");

    assert_eq!(
        prop(&g, node::ARTIFACT, "art:one", "checksum"),
        "sha256:abc",
        "BL-166's erasure must not be reachable through add_artifact either"
    );
    assert_eq!(
        prop(&g, node::ARTIFACT, "art:one", "location"),
        "src/lib/one.rs",
        "and the move must still have happened"
    );
}

// ---------------------------------------------------------------------------
// The project itself.
// ---------------------------------------------------------------------------

/// `add_project` omits `mode`, `objective` and `status`. Genesis sets all
/// three; re-running it, or renaming the project, must not blank the design's
/// own purpose.
#[test]
fn renaming_a_project_does_not_blank_its_objective() {
    let mut g = graph();
    g.add_project("proj:one", "Demo").expect("project");
    g.set_project_mode("proj:one", "rigid").expect("mode");

    g.add_project("proj:one", "Demo Mk II").expect("rename");

    assert_eq!(
        prop(&g, node::PROJECT, "proj:one", "mode"),
        "rigid",
        "governance mode is a decision, not a side effect of the name"
    );
}

// ---------------------------------------------------------------------------
// The general guarantee.
// ---------------------------------------------------------------------------

/// Creation is unaffected: a constructor called on an id that does NOT exist
/// still writes exactly what it was given, defaults and all. The fix must be
/// merge-on-revise, never merge-on-create — otherwise nothing could ever be
/// reset deliberately and the change would be a different bug.
#[test]
fn a_first_call_still_creates_from_scratch() {
    let mut g = graph();
    g.add_requirement("req:new", "New", "A statement.")
        .expect("requirement");

    assert_eq!(
        prop(&g, node::REQUIREMENT, "req:new", "status"),
        "proposed",
        "a brand-new requirement takes its schema default, not something inherited"
    );
    assert_eq!(
        prop(&g, node::REQUIREMENT, "req:new", "provenance"),
        "authored"
    );
}

/// A property the caller DOES name is still overwritten — merge must not
/// become "first write wins", which would make revision impossible.
#[test]
fn a_named_property_is_still_overwritten() {
    let mut g = graph();
    g.add_capability("cap:one", "Thing", "First description.", None)
        .expect("capability");

    g.add_capability("cap:one", "Renamed", "Second description.", None)
        .expect("revise");

    assert_eq!(prop(&g, node::CAPABILITY, "cap:one", "name"), "Renamed");
    assert_eq!(
        prop(&g, node::CAPABILITY, "cap:one", "description"),
        "Second description."
    );
}

/// An explicitly-passed optional still wins over the stored value — otherwise
/// "merge" would quietly become "you may never change this back".
#[test]
fn an_explicit_optional_still_overwrites() {
    let mut g = graph();
    g.add_capability("cap:one", "Thing", "Desc.", Some("realized"))
        .expect("capability");

    g.add_capability("cap:one", "Thing", "Desc.", Some("in_progress"))
        .expect("revise");

    assert_eq!(
        prop(&g, node::CAPABILITY, "cap:one", "status"),
        "in_progress",
        "passing a value explicitly must still set it"
    );
}

// ---------------------------------------------------------------------------
// The linker, which this file's own header believed was already safe.
// ---------------------------------------------------------------------------

/// `link_artifact` was the FIRST place this class was fixed (BL-166), and the
/// header above says so. The fix reached every field the caller could omit and
/// stopped at the one they could not: `name` was mandatory, so "leave the name
/// alone" was inexpressible and every re-link rewrote it to whatever the caller
/// happened to re-type.
///
/// That is the same shape as `set_verification_status` deleting `last_run_at`
/// on omission, fixed 2026-08-09 — and the same convention three sibling
/// setters state outright: omitting one leaves it alone.
///
/// **WHY THE CLASS GUARD MISSED IT.** This file's cases are all `add_*`
/// constructors. Membership was drawn on the NAME of the call rather than on
/// what it does — write a node from a partial property set — and `link_artifact`
/// does exactly that under a different name. A hand-drawn boundary around a
/// behavioural class is the thing that let a fixed defect stay half-fixed.
#[test]
fn re_linking_an_artifact_does_not_rename_it() {
    let mut g = graph();
    g.add_capability("cap:one", "One", "The first thing.", None)
        .expect("capability");
    g.add_capability("cap:two", "Two", "The second thing.", None)
        .expect("capability");

    let rich = "reconcile.rs — the as-built sweep, and what it refuses to \
                decide on the caller's behalf";
    g.link_artifact(reflow2_core::LinkArtifactOptions {
        artifact_id: "art:one".into(),
        name: Some(rich.into()),
        description: Some("Walks the tree and reports drift.".into()),
        location: Some("src/reconcile.rs".into()),
        artifact_type: Some("code".into()),
        content_ref: None,
        note_kind: None,
        target_type: node::CAPABILITY.into(),
        target_id: "cap:one".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })
    .expect("first link");

    // The ordinary second act: the same file also realizes another capability.
    // The caller has nothing to say about the name and should not have to.
    g.link_artifact(reflow2_core::LinkArtifactOptions {
        artifact_id: "art:one".into(),
        name: None,
        description: None,
        location: None,
        artifact_type: None,
        content_ref: None,
        note_kind: None,
        target_type: node::CAPABILITY.into(),
        target_id: "cap:two".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })
    .expect("second link");

    assert_eq!(
        prop(&g, node::ARTIFACT, "art:one", "name"),
        rich,
        "re-linking a file to a second target must not rename it"
    );
    assert_eq!(
        prop(&g, node::ARTIFACT, "art:one", "description"),
        "Walks the tree and reports drift.",
        "and the prose must survive too — it is the part nobody can reconstruct"
    );
    assert_eq!(
        prop(&g, node::ARTIFACT, "art:one", "location"),
        "src/reconcile.rs",
        "the fields BL-166 already protected must still be protected"
    );
}

/// The other direction, so the fix cannot quietly become "you may never rename".
/// An explicitly-passed name still wins, exactly as
/// `a_named_property_is_still_overwritten` requires of every constructor.
#[test]
fn an_explicit_name_still_renames_on_relink() {
    let mut g = graph();
    g.add_capability("cap:one", "One", "The first thing.", None)
        .expect("capability");
    let opts = |name: Option<String>| reflow2_core::LinkArtifactOptions {
        artifact_id: "art:one".into(),
        name,
        description: None,
        location: None,
        artifact_type: None,
        content_ref: None,
        note_kind: None,
        target_type: node::CAPABILITY.into(),
        target_id: "cap:one".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    };
    g.link_artifact(opts(Some("old.rs".into()))).expect("link");
    g.link_artifact(opts(Some("new.rs".into())))
        .expect("rename");

    assert_eq!(prop(&g, node::ARTIFACT, "art:one", "name"), "new.rs");
}

/// A FIRST link has nothing to preserve, so there the name is still required —
/// and the refusal says what would have worked rather than writing an unnamed
/// Artifact that every report would render blank.
#[test]
fn a_first_link_still_requires_a_name() {
    let mut g = graph();
    g.add_capability("cap:one", "One", "The first thing.", None)
        .expect("capability");
    let err = g
        .link_artifact(reflow2_core::LinkArtifactOptions {
            artifact_id: "art:missing".into(),
            name: None,
            description: None,
            location: None,
            artifact_type: None,
            content_ref: None,
            note_kind: None,
            target_type: node::CAPABILITY.into(),
            target_id: "cap:one".into(),
            completeness: None,
            conformance: None,
            provenance: None,
            fragment_id: None,
            checksum: None,
        })
        .expect_err("a first link with no name is refused");
    let msg = err.to_string();
    assert!(
        msg.contains("art:missing") && msg.contains("pass `name`"),
        "the refusal must name the artifact and what would have worked: {msg}"
    );
}

/// THE HOLE IN THE FIRST VERSION OF THE FIX, pinned so it cannot come back.
///
/// An Artifact made by `add_artifact` has no provenance Fragment, and the
/// Fragment's `title` is a REQUIRED property. So making `name` merely optional
/// was not enough: a re-link that omitted it produced no title and failed schema
/// validation on a field the caller had never mentioned. The fix is to RESOLVE
/// the name — the caller's if given, otherwise the one the design already holds
/// — and build the title from that.
///
/// Found by using the fix on this very change's own bookkeeping, which is the
/// only reason it was caught before the gate.
#[test]
fn re_linking_mints_a_missing_fragment_from_the_stored_name() {
    let mut g = graph();
    g.add_capability("cap:one", "One", "The first thing.", None)
        .expect("capability");
    // add_artifact, NOT link_artifact — so no Fragment exists yet.
    g.add_artifact("art:one", "reconcile.rs", Some("code"), Some("src/one.rs"))
        .expect("artifact");

    g.link_artifact(reflow2_core::LinkArtifactOptions {
        artifact_id: "art:one".into(),
        name: None,
        description: None,
        location: None,
        artifact_type: None,
        content_ref: None,
        note_kind: None,
        target_type: node::CAPABILITY.into(),
        target_id: "cap:one".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })
    .expect("a nameless re-link onto an artifact with no fragment must succeed");

    assert_eq!(
        prop(&g, node::ARTIFACT, "art:one", "name"),
        "reconcile.rs",
        "the stored name still stands"
    );
    assert_eq!(
        prop(&g, node::FRAGMENT, "frag:art:one", "title"),
        "Registered reconcile.rs",
        "and the fragment's required title was minted from it, not left empty"
    );
}
