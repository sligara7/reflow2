//! A repair records WHICH KIND it was, a containment names what it stands in
//! for, and "what still rests on a patch?" becomes a question the graph answers.
//!
//! # The requirement, and the measurement that is its whole case
//!
//! `req:a-fix-says-whether-it-corrected-the-cause`, accepted on Anthony's own
//! words: *"this is my philosophy on how to build something — so ensuring that
//! it is built into the design of reflow2."* And the question he could not get
//! an answer to: *"I'm not sure how many fixes have been patches versus going
//! back to the drawing board."*
//!
//! Measured on this project's graph 2026-08-17: **472 ChangeEvents across
//! eleven change types, every one naming what MOVED and not one saying whether
//! it was the RIGHT fix.** A `test_failure_fix` is equally a root-cause rewrite
//! and a shim that made a red test green.
//!
//! The obvious hypothesis was refuted before anything was built: perhaps the
//! commonest fix type was just an unchosen default, since `test_failure_fix` is
//! what a `design_holds` drift accept defaults to. It was not — 55 of 101 were
//! auto-minted accepts and **all 55 carried written reasons**. The discipline
//! was there; the vocabulary was missing.
//!
//! # What these pin, clause by clause
//!
//! - **(a)** the disposition is recorded, and absent still means nobody said;
//! - **(b)** a containment names the correction it stands in for — enforced by
//!   the TYPE, so the bad state cannot be built rather than being rejected
//!   after the fact;
//! - **(c)** the count is reportable, *and the report cannot read as clean when
//!   it is merely silent* — which is the case that matters on day one, when
//!   every historical repair is unstated.

use reflow2_core::temporal::{ChangeRecord, Repair};
use reflow2_core::{ChangeAction, ChangeType, DesignGraph, EpochType, nodes::Props, nodes::node};

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_project("proj:p", "A project").expect("project");
    g.add_epoch("epoch:now", "Now", EpochType::Revision, 1)
        .expect("epoch");
    g.create_node(
        node::COMPONENT,
        "cmp:thing",
        Props::new()
            .set("name", "the thing")
            .set("purpose", "somewhere for a fix to land"),
    )
    .expect("component");
    g
}

fn record(g: &mut DesignGraph, id: &str, repair: Option<Repair>) {
    g.record_change(ChangeRecord {
        epoch_id: "epoch:now",
        change_event_id: id,
        name: "a fix",
        change_type: ChangeType::DefectFix,
        subject: None,
        target_type: node::COMPONENT,
        target_id: "cmp:thing",
        action: ChangeAction::Modified,
        repair,
    })
    .expect("record_change");
}

// ── (a) the disposition is recorded, and silence stays silence ──────────────

#[test]
fn a_repair_records_which_kind_it_was() {
    let mut g = design();
    record(&mut g, "chg:corrected", Some(Repair::CorrectedCause));
    let n = g
        .get_node(node::CHANGE_EVENT, "chg:corrected")
        .expect("read")
        .expect("node");
    let props: std::collections::HashMap<_, _> = n.properties.into_iter().collect();
    assert_eq!(
        props.get("repair").and_then(|v| v.as_str()),
        Some("corrected_cause")
    );
    assert!(
        !props.contains_key("stands_in_for"),
        "a correction stands in for nothing, so the field must be absent rather than empty"
    );
}

#[test]
fn saying_nothing_stays_saying_nothing() {
    // `req:defaults-do-not-assert`. Whether a fix reached its cause is a
    // judgement, and a default would record a claim nobody made — which is
    // exactly what makes the 472 historical events HONEST rather than missing.
    let mut g = design();
    record(&mut g, "chg:silent", None);
    let n = g
        .get_node(node::CHANGE_EVENT, "chg:silent")
        .expect("read")
        .expect("node");
    let props: std::collections::HashMap<_, _> = n.properties.into_iter().collect();
    assert!(
        !props.contains_key("repair"),
        "a disposition appeared that nobody stated: {props:?}"
    );
}

// ── (b) a containment names what it stands in for ───────────────────────────

#[test]
fn a_contained_symptom_carries_the_fix_it_stands_in_for() {
    let mut g = design();
    record(
        &mut g,
        "chg:patched",
        Some(Repair::ContainedSymptom {
            stands_in_for: String::from("the retry should be in the transport, not the caller"),
        }),
    );
    let n = g
        .get_node(node::CHANGE_EVENT, "chg:patched")
        .expect("read")
        .expect("node");
    let props: std::collections::HashMap<_, _> = n.properties.into_iter().collect();
    assert_eq!(
        props.get("repair").and_then(|v| v.as_str()),
        Some("contained_symptom")
    );
    assert_eq!(
        props.get("stands_in_for").and_then(|v| v.as_str()),
        Some("the retry should be in the transport, not the caller")
    );
}

// ── (c) the count is reportable, and cannot read as clean when it is silent ──

#[test]
fn the_report_splits_the_ledger_three_ways() {
    let mut g = design();
    record(&mut g, "chg:a", Some(Repair::CorrectedCause));
    record(&mut g, "chg:b", Some(Repair::CorrectedCause));
    record(
        &mut g,
        "chg:c",
        Some(Repair::ContainedSymptom {
            stands_in_for: String::from("rewrite the parser"),
        }),
    );
    record(&mut g, "chg:d", None);
    let r = g.repair_report().expect("report");
    assert_eq!(r.repairs, 4);
    assert_eq!(r.corrected_cause, 2);
    assert_eq!(r.contained_symptom, 1);
    assert_eq!(r.unstated, 1);
    assert_eq!(r.standing.len(), 1);
    assert_eq!(r.standing[0].stands_in_for, "rewrite the parser");
}

#[test]
fn a_silent_ledger_says_nobody_said_and_not_nothing_rests_on_a_patch() {
    // ⭐ THE CASE THAT MATTERS ON DAY ONE, and the reason `unstated` is a field
    // rather than a footnote. Reflow2's own graph has ~220 repairs and none of
    // them stated a disposition until this shipped. A report that listed the
    // (zero) standing patches and stopped would have answered "nothing rests on
    // a patch" — confidently, and falsely.
    let mut g = design();
    record(&mut g, "chg:a", None);
    record(&mut g, "chg:b", None);
    let r = g.repair_report().expect("report");
    assert_eq!(r.contained_symptom, 0);
    assert_eq!(r.unstated, 2);
    assert!(
        r.note.contains("NOBODY HAS SAID"),
        "an all-silent ledger read as an answer: {}",
        r.note
    );
    assert!(
        !r.note.to_lowercase().contains("complete"),
        "a silent ledger claimed completeness: {}",
        r.note
    );
}

#[test]
fn an_empty_design_says_it_had_nothing_to_examine() {
    // `req:a-report-says-what-it-swept-and-whether-its-checks-ran`: HAD NOTHING
    // TO EXAMINE and EXAMINED AND FOUND NOTHING must not read alike.
    let g = design();
    let r = g.repair_report().expect("report");
    assert_eq!(r.repairs, 0);
    assert!(
        r.note.contains("NOTHING TO EXAMINE"),
        "an empty design read as a clean one: {}",
        r.note
    );
}

#[test]
fn a_complete_ledger_says_so_and_only_then() {
    let mut g = design();
    record(&mut g, "chg:a", Some(Repair::CorrectedCause));
    record(
        &mut g,
        "chg:b",
        Some(Repair::ContainedSymptom {
            stands_in_for: String::from("do it properly next increment"),
        }),
    );
    let r = g.repair_report().expect("report");
    assert_eq!(r.unstated, 0);
    assert!(r.note.contains("complete"), "note: {}", r.note);
}

#[test]
fn a_change_that_is_not_a_repair_is_not_counted() {
    // The denominator is shared with `fix_without_recorded_cause` on purpose:
    // two definitions of "what counts as a fix" would drift, and the gap and
    // this report would then disagree about the same question.
    let mut g = design();
    g.record_change(ChangeRecord {
        epoch_id: "epoch:now",
        change_event_id: "chg:feature",
        name: "a feature",
        change_type: ChangeType::NewFeature,
        subject: None,
        target_type: node::COMPONENT,
        target_id: "cmp:thing",
        action: ChangeAction::Added,
        repair: None,
    })
    .expect("record_change");
    let r = g.repair_report().expect("report");
    assert_eq!(r.repairs, 0, "a new feature was counted as a repair");
}
