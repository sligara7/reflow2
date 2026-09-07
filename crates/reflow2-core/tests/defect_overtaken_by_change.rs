//! A recorded defect whose subject moved after it was written, with nothing
//! saying whether that was the fix, gets asked about.
//!
//! The mirror of `fix_without_recorded_cause`. Written from
//! `fact:a-defect-fixed-but-never-closed-on-the-record-was-re-fixed-wrongly-a-
//! day-later`: a fix landed, drew no INVALIDATES, the fact went on reading as
//! open, and the next session trusted it and re-fixed the symptom a different
//! way. A stale open defect is a live instruction to do the wrong thing.
//!
//! Ordering needs two dates, and the detector says so rather than guessing: an
//! undated change is counted in the evidence and never treated as later.

use reflow2_core::detect::{GapCandidate, GapSource};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_artifact("art:detect", "detect.rs", None, None)
        .expect("artifact");
    g
}

fn defect(g: &mut DesignGraph, id: &str, subject: &str, valid_from: Option<&str>) {
    let mut props = Props::new()
        .set("name", id)
        .set("fact_type", "defect")
        .set("statement", "it fires on the wrong thing")
        .set("subject_id", subject);
    if let Some(d) = valid_from {
        props = props.set("valid_from", d);
    }
    g.create_node(node::TEMPORAL_FACT, id, props).expect("fact");
}

fn change_on(g: &mut DesignGraph, id: &str, target_type: &str, target: &str, at: Option<&str>) {
    change_of_kind(g, id, "defect_fix", target_type, target, at)
}

fn change_of_kind(
    g: &mut DesignGraph,
    id: &str,
    change_type: &str,
    target_type: &str,
    target: &str,
    at: Option<&str>,
) {
    let mut props = Props::new()
        .set("name", id)
        .set("change_type", change_type)
        .set("subject", "system");
    if let Some(d) = at {
        props = props.set("detected_at", d);
    }
    g.create_node(node::CHANGE_EVENT, id, props).expect("event");
    g.create_edge(
        edge::CHANGED,
        node::CHANGE_EVENT,
        id,
        target_type,
        target,
        Props::new(),
    )
    .expect("changed");
}

fn the_gaps(g: &DesignGraph) -> Vec<GapCandidate> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .filter(|x| x.gap_source == GapSource::DefectOvertakenByChange)
        .collect()
}

/// The case it was written from: the subject changed after the defect was
/// recorded, and nothing says whether that fixed it.
#[test]
fn a_later_dated_change_on_the_subject_asks_whether_it_was_the_fix() {
    let mut g = graph();
    defect(
        &mut g,
        "fact:defect-fires-wrong",
        "art:detect",
        Some("2026-09-04"),
    );
    change_on(
        &mut g,
        "chg:the-fix",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-05"),
    );

    let gaps = the_gaps(&g);
    assert_eq!(gaps.len(), 1);
    let gap = &gaps[0];
    assert_eq!(
        gap.affected_ids,
        vec!["chg:the-fix", "fact:defect-fires-wrong"],
        "anchored to the fact AND the change that may have fixed it"
    );
    assert!(
        gap.title.contains("1 later repair(s) touched"),
        "{}",
        gap.title
    );
    assert!(
        gap.description.contains("chg:the-fix"),
        "{}",
        gap.description
    );
}

/// INVALIDATES from the fix is the answer, and it clears the finding.
#[test]
fn a_fix_that_invalidates_the_fact_clears_it() {
    let mut g = graph();
    defect(
        &mut g,
        "fact:defect-fires-wrong",
        "art:detect",
        Some("2026-09-04"),
    );
    change_on(
        &mut g,
        "chg:the-fix",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-05"),
    );
    g.invalidates(
        node::CHANGE_EVENT,
        "chg:the-fix",
        node::TEMPORAL_FACT,
        "fact:defect-fires-wrong",
        Some("this was the fix"),
        Some("2026-09-05"),
    )
    .expect("invalidates");
    assert!(the_gaps(&g).is_empty());
}

/// A change BEFORE the defect was recorded cannot have fixed it.
#[test]
fn an_earlier_change_is_not_later() {
    let mut g = graph();
    defect(
        &mut g,
        "fact:defect-fires-wrong",
        "art:detect",
        Some("2026-09-04"),
    );
    change_on(
        &mut g,
        "chg:old",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-03"),
    );
    change_on(
        &mut g,
        "chg:same-day",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-04"),
    );
    assert!(
        the_gaps(&g).is_empty(),
        "same-day is not strictly later either"
    );
}

/// An undated change cannot be ordered. It is counted, never assumed later.
#[test]
fn an_undated_change_is_counted_not_assumed() {
    let mut g = graph();
    defect(
        &mut g,
        "fact:defect-fires-wrong",
        "art:detect",
        Some("2026-09-04"),
    );
    change_on(&mut g, "chg:undated", node::ARTIFACT, "art:detect", None);
    assert!(
        the_gaps(&g).is_empty(),
        "one undated change alone raises nothing"
    );

    change_on(
        &mut g,
        "chg:dated",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-06"),
    );
    let gaps = the_gaps(&g);
    assert_eq!(gaps.len(), 1);
    assert!(
        gaps[0]
            .evidence
            .contains("1 further change(s) on the same subject carry no detected_at"),
        "{}",
        gaps[0].evidence
    );
    assert!(!gaps[0].affected_ids.iter().any(|id| id == "chg:undated"));
}

/// Only a REPAIR can be the fix. A feature or a refactor that touched the
/// subject later is counted in the evidence and never offered as the answer —
/// over every later change, one hub subject on reflow2's own design carried 39
/// members, which is a question nobody can answer.
#[test]
fn a_later_change_that_is_not_a_repair_is_counted_not_offered() {
    let mut g = graph();
    defect(
        &mut g,
        "fact:defect-fires-wrong",
        "art:detect",
        Some("2026-09-04"),
    );
    change_of_kind(
        &mut g,
        "chg:feature",
        "new_feature",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-05"),
    );
    change_of_kind(
        &mut g,
        "chg:tidy",
        "refactor",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-05"),
    );
    assert!(
        the_gaps(&g).is_empty(),
        "two later non-repairs raise nothing"
    );

    change_on(
        &mut g,
        "chg:the-fix",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-06"),
    );
    let gaps = the_gaps(&g);
    assert_eq!(gaps.len(), 1);
    assert_eq!(
        gaps[0].affected_ids,
        vec!["chg:the-fix", "fact:defect-fires-wrong"]
    );
    assert!(
        gaps[0]
            .evidence
            .contains("2 later change(s) on the same subject were not repairs"),
        "{}",
        gaps[0].evidence
    );
}

/// A capability's defect is fixed by changing something that realizes it.
#[test]
fn a_change_on_an_artifact_realizing_the_subject_counts() {
    let mut g = graph();
    g.add_capability("cap:detect", "Detect", "finds gaps", Some("realized"))
        .expect("capability");
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:detect",
        node::CAPABILITY,
        "cap:detect",
        Props::new(),
    )
    .expect("realizes");
    defect(
        &mut g,
        "fact:defect-fires-wrong",
        "cap:detect",
        Some("2026-09-04"),
    );
    change_on(
        &mut g,
        "chg:the-fix",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-05"),
    );

    let gaps = the_gaps(&g);
    assert_eq!(gaps.len(), 1);
    assert!(gaps[0].affected_ids.iter().any(|id| id == "chg:the-fix"));
}

/// Closed and undated facts are out of scope, and say nothing.
#[test]
fn a_closed_or_undated_fact_is_skipped() {
    let mut g = graph();
    defect(&mut g, "fact:defect-undated", "art:detect", None);
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:defect-closed",
        Props::new()
            .set("name", "closed")
            .set("fact_type", "defect")
            .set("statement", "was wrong, then fixed")
            .set("subject_id", "art:detect")
            .set("valid_from", "2026-09-01")
            .set("valid_to", "2026-09-02"),
    )
    .expect("fact");
    change_on(
        &mut g,
        "chg:the-fix",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-05"),
    );
    assert!(the_gaps(&g).is_empty());
}

/// A further change on the same subject is a fresh question: the id moves.
#[test]
fn a_further_change_asks_again_under_a_new_id() {
    let mut g = graph();
    defect(
        &mut g,
        "fact:defect-fires-wrong",
        "art:detect",
        Some("2026-09-04"),
    );
    change_on(
        &mut g,
        "chg:first",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-05"),
    );
    let first = the_gaps(&g)[0].id.clone();
    change_on(
        &mut g,
        "chg:second",
        node::ARTIFACT,
        "art:detect",
        Some("2026-09-06"),
    );
    let second = the_gaps(&g)[0].id.clone();
    assert_ne!(first, second);
}
