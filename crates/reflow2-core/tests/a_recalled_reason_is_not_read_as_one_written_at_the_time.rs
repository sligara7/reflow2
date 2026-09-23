//! A reason given years after a change is recorded as recalled, and nothing
//! reads it as evidence it is not.
//!
//! The `why` skill interviews a system's designer about the changes in its git
//! history (`dec:idea-a-change-justification-skill-walks-the-commit-history-and-asks-why`).
//! Its brainstorm named three ways that record could lie, and each has a test
//! here:
//!
//! 1. MEMORY IS A RECONSTRUCTION. A reason recalled in 2026 about a 2021
//!    revert must say so, or it reads with the authority of one written in
//!    2021 — `rationale_basis`.
//! 2. HISTORY TOLD TODAY IS NOT A CHECK OF THE SYSTEM TODAY. One interview
//!    session must not flip every feature it touched from Unexamined to
//!    Confirmed in the confirmation ledger.
//! 3. BACKWARD RECORDS, FORWARD RULES. An old fix recorded now must not raise
//!    the checks written for new work — by the mechanism the design already
//!    uses for a backlog (parking under one accepted ruling), not a new one.

use reflow2_core::LinkArtifactOptions;
use reflow2_core::confirm::ConfirmationState;
use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::temporal::{
    ChangeAction, ChangeEventRecord, ChangeSubject, ChangeType, RationaleBasis, Repair,
};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open")
}

/// A feature that is built and registered, and that nobody has checked.
fn built_feature() -> DesignGraph {
    let mut g = graph();
    g.add_project("proj:1", "Reports").expect("project");
    g.add_capability("cap:export", "Report export", "exports results", None)
        .expect("cap");
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:export".into(),
        name: Some("export".into()),
        description: None,
        location: Some("src/export".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:export".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:aaa".into()),
        content_ref: None,
        note_kind: None,
    })
    .expect("link");
    g
}

fn record<'a>(
    id: &'a str,
    change_type: ChangeType,
    basis: Option<RationaleBasis>,
    repair: Option<&'a Repair>,
) -> ChangeEventRecord<'a> {
    ChangeEventRecord {
        id,
        name: id,
        change_type,
        subject: Some(ChangeSubject::System),
        summary: Some("moved the export button from the toolbar to the File menu"),
        rationale: Some("people hit it by accident when printing"),
        detected_at: Some("2020-07-14"),
        repair,
        rationale_basis: basis,
        commits: Some("3f2a9c1d, 77b01e2"),
    }
}

fn prop(g: &DesignGraph, id: &str, key: &str) -> Option<String> {
    g.get_node(node::CHANGE_EVENT, id)
        .expect("read")
        .expect("present")
        .properties
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

#[test]
fn the_record_says_how_its_reason_is_known_and_which_commits_it_was() {
    let mut g = graph();
    g.add_change_event_record(&record(
        "chg:recalled",
        ChangeType::ScopeChange,
        Some(RationaleBasis::Recalled),
        None,
    ))
    .expect("write");
    assert_eq!(
        prop(&g, "chg:recalled", "rationale_basis").as_deref(),
        Some("recalled")
    );
    assert_eq!(
        prop(&g, "chg:recalled", "commits").as_deref(),
        Some("3f2a9c1d, 77b01e2")
    );
    assert_eq!(
        prop(&g, "chg:recalled", "detected_at").as_deref(),
        Some("2020-07-14"),
        "the change is dated when it HAPPENED; who recalled it and when is AUTHORED_BY"
    );

    // The old constructor still writes no basis at all — absent means nobody
    // said, and is never defaulted to `contemporaneous`.
    g.add_change_event(
        "chg:plain",
        "plain",
        ChangeType::Refactor,
        None,
        None,
        None,
        None,
    )
    .expect("write");
    assert_eq!(prop(&g, "chg:plain", "rationale_basis"), None);
}

#[test]
fn a_recalled_change_is_counted_apart_and_never_confirms_a_capability() {
    let mut g = built_feature();
    for (id, basis) in [
        ("chg:told-later", RationaleBasis::Recalled),
        ("chg:nobody-knows", RationaleBasis::Unknown),
    ] {
        g.add_change_event_record(&record(id, ChangeType::ScopeChange, Some(basis), None))
            .expect("write");
        g.changed(id, node::CAPABILITY, "cap:export", ChangeAction::Modified)
            .expect("changed");
    }

    let ledger = g.confirmation_ledger().expect("ledger");
    let claim = &ledger.claims[0];
    assert_eq!(
        claim.state,
        ConfirmationState::Unexamined,
        "an interview about 2020 is not a check of the feature against its code today"
    );
    assert_eq!((claim.design_edits, claim.recalled_changes), (0, 2));

    // A change recorded at the time IS the design moving on the record, as
    // before — and so is one that says nothing about its basis.
    g.add_change_event_record(&record(
        "chg:at-the-time",
        ChangeType::ScopeChange,
        Some(RationaleBasis::Contemporaneous),
        None,
    ))
    .expect("write");
    g.changed(
        "chg:at-the-time",
        node::CAPABILITY,
        "cap:export",
        ChangeAction::Modified,
    )
    .expect("changed");
    let ledger = g.confirmation_ledger().expect("ledger");
    let claim = &ledger.claims[0];
    assert_eq!(claim.state, ConfirmationState::Confirmed);
    assert_eq!((claim.design_edits, claim.recalled_changes), (1, 2));
}

#[test]
fn an_unmarked_change_still_counts_as_a_design_edit() {
    // No behaviour moves for a design that never recorded a basis: absent is
    // not `recalled`.
    let mut g = built_feature();
    g.add_change_event(
        "chg:old",
        "old",
        ChangeType::ScopeChange,
        None,
        None,
        None,
        None,
    )
    .expect("write");
    g.changed(
        "chg:old",
        node::CAPABILITY,
        "cap:export",
        ChangeAction::Modified,
    )
    .expect("changed");
    let claim = &g.confirmation_ledger().expect("ledger").claims[0];
    assert_eq!(claim.state, ConfirmationState::Confirmed);
    assert_eq!((claim.design_edits, claim.recalled_changes), (1, 0));
}

#[test]
fn the_repair_tally_says_how_much_of_its_unstated_count_is_recalled_history() {
    let mut g = graph();
    g.add_change_event_record(&record(
        "chg:old-fix",
        ChangeType::DefectFix,
        Some(RationaleBasis::Recalled),
        None,
    ))
    .expect("write");
    g.add_change_event(
        "chg:new-fix",
        "new",
        ChangeType::DefectFix,
        None,
        None,
        None,
        None,
    )
    .expect("write");
    let corrected = Repair::CorrectedCause;
    g.add_change_event_record(&record(
        "chg:old-fix-with-disposition",
        ChangeType::DefectFix,
        Some(RationaleBasis::Recalled),
        Some(&corrected),
    ))
    .expect("write");

    let r = g.repair_report().expect("report");
    assert_eq!(r.repairs, 3);
    assert_eq!(r.corrected_cause, 1);
    assert_eq!(
        (r.unstated, r.unstated_recalled),
        (2, 1),
        "a recalled fix with no disposition is still unstated — only counted apart"
    );
    assert!(
        r.note.contains("1 are fixes recalled after the fact"),
        "the note says so in words: {}",
        r.note
    );
}

#[test]
fn a_design_with_no_recalled_history_reads_exactly_as_before() {
    let mut g = graph();
    g.add_change_event(
        "chg:fix",
        "fix",
        ChangeType::DefectFix,
        None,
        None,
        None,
        None,
    )
    .expect("write");
    let r = g.repair_report().expect("report");
    assert_eq!(r.unstated_recalled, 0);
    assert!(!r.note.contains("recalled"), "{}", r.note);
}

/// The recipe the `why` skill writes, end to end: an old fix, recalled, with
/// its axis stated and parked under the project's one accepted ruling that
/// history predates its forward-only rules. Neither rule for new work fires;
/// a fix recorded today still does.
#[test]
fn the_why_recipe_keeps_old_fixes_out_of_the_rules_for_new_work() {
    let mut g = graph();
    g.add_change_event_record(&record(
        "chg:2021-revert",
        ChangeType::DefectFix,
        Some(RationaleBasis::Recalled),
        None,
    ))
    .expect("write");
    g.add_decision(
        "dec:history-predates-the-forward-rules",
        "Changes recalled from before this project came under design control are history",
        "the forward-only rules bind from when they exist",
        Some("history is recorded, not re-litigated"),
    )
    .expect("decision");
    g.set_decision_status("dec:history-predates-the-forward-rules", "accepted")
        .expect("accepted");
    g.create_edge(
        edge::GOVERNED_BY,
        node::CHANGE_EVENT,
        "chg:2021-revert",
        node::DECISION,
        "dec:history-predates-the-forward-rules",
        Props::new().set("ruling", "parks"),
    )
    .expect("parks");

    let gaps = g.detect_gaps().expect("detect");
    let fired = |src: GapSource| gaps.iter().any(|x| x.gap_source == src);
    assert!(!fired(GapSource::FixWithoutRecordedCause));
    assert!(!fired(GapSource::ChangeAxisUnstated));

    g.add_change_event_record(&ChangeEventRecord {
        rationale_basis: Some(RationaleBasis::Contemporaneous),
        detected_at: Some("2026-09-23"),
        ..record("chg:today", ChangeType::DefectFix, None, None)
    })
    .expect("write");
    let gap = g
        .detect_gaps()
        .expect("detect")
        .into_iter()
        .find(|x| x.gap_source == GapSource::FixWithoutRecordedCause)
        .expect("a fix made today still owes its cause");
    assert_eq!(gap.affected_ids, vec!["chg:today"]);
}
