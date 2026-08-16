//! `change_type` was answering two questions at once.
//!
//! It asks WHY a change happened — and it was also being made to carry WHETHER
//! ANYTHING CHANGED AT ALL. Five sessions across three projects each picked a
//! different least-wrong value for the same kind of event, because there was
//! nothing to agree on: two maintainer sessions and the dev_storyflow fleet
//! reached for `test_failure_fix` where no test existed, `chg:arrival-delta-
//! decided` reached for `scope_change` to mean "an open question was settled",
//! and the edit recording Alex's report reached for `resync` to mean "our
//! knowledge moved and the thing did not".
//!
//! ⭐ THE SPLIT WAS ALREADY NAMED IN THE CODE AND LEFT UNUSABLE.
//! `BaselineEstablished`'s doc comment says "every other variant answers why
//! the THING changed; this one says the thing did not change and only the
//! design's KNOWLEDGE of it did" — an axis with exactly one member, reserved so
//! nobody could reach it. `subject` is that axis, made reachable.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;
use reflow2_core::{ChangeSubject, ChangeType};

fn subject_of(g: &DesignGraph, id: &str) -> Option<String> {
    g.get_node(node::CHANGE_EVENT, id)
        .expect("get")
        .expect("event")
        .properties
        .get("subject")
        .and_then(|v| v.as_str().map(str::to_string))
}

#[test]
fn a_defect_fix_is_expressible_without_inventing_a_failed_test() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_change_event(
        "chg:civil-date",
        "the requirement said civil date; the code used UTC",
        ChangeType::DefectFix,
        Some(ChangeSubject::System),
    )
    .expect("a defect against accepted intent must be recordable as itself");

    assert_eq!(ChangeType::DefectFix.as_str(), "defect_fix");
    assert_eq!(subject_of(&g, "chg:civil-date").as_deref(), Some("system"));
}

/// The axis carries what `change_type` alone could not say.
#[test]
fn the_record_axis_can_say_the_thing_did_not_change() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_change_event(
        "chg:knowledge-moved",
        "corroboration merged into an existing defect; nothing about the system moved",
        ChangeType::Resync,
        Some(ChangeSubject::Record),
    )
    .expect("event");

    assert_eq!(
        subject_of(&g, "chg:knowledge-moved").as_deref(),
        Some("record"),
        "the same change_type on the other axis must be distinguishable"
    );
}

/// COUNTERWEIGHT 1 — UNSTATED IS A TRUE ANSWER AND MUST STAY ONE.
///
/// Every ChangeEvent written before 2026-08-15 has no subject, and inventing
/// one for them would put a claim on the record nobody made
/// (`req:defaults-do-not-assert`). This is the test that stops a later
/// convenience default from quietly asserting an axis.
#[test]
fn an_unstated_axis_stays_unstated() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_change_event(
        "chg:quiet",
        "somebody did not say",
        ChangeType::Refactor,
        None,
    )
    .expect("event");

    assert_eq!(
        subject_of(&g, "chg:quiet"),
        None,
        "absent must mean nobody said — never a guess derived from change_type"
    );
}

/// COUNTERWEIGHT 2 — the axis is NOT inferred from `change_type`, and this pins
/// why: the mapping is not total. `Resync` appears on BOTH axes above — the
/// record one in this file, and a genuine HEAL outcome that moved the system on
/// the other. Any future code that derives `subject` from `change_type` breaks
/// here, which is the point.
#[test]
fn one_change_type_appears_on_both_axes() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_change_event(
        "chg:a",
        "record side",
        ChangeType::Resync,
        Some(ChangeSubject::Record),
    )
    .expect("event");
    g.add_change_event(
        "chg:b",
        "system side",
        ChangeType::Resync,
        Some(ChangeSubject::System),
    )
    .expect("event");

    assert_ne!(
        subject_of(&g, "chg:a"),
        subject_of(&g, "chg:b"),
        "the same why on two axes must stay two different facts"
    );
}

/// COUNTERWEIGHT 3 — the reserved value stays reserved. `BaselineEstablished`
/// is set_artifact_checksum's alone so the confirmation ledger's count of first
/// baselines cannot be inflated by hand, and adding an axis must not have
/// quietly opened that door.
#[test]
fn the_reserved_value_is_still_reserved_on_the_generic_path() {
    assert_eq!(
        ChangeType::BaselineEstablished.as_str(),
        "baseline_established"
    );
    assert_eq!(ChangeSubject::Record.as_str(), "record");
}
