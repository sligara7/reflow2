//! The axis existed and the door it mattered at was locked.
//!
//! `ChangeSubject {system, record}` was added to the schema and to
//! `add_change_event`, and then `record_change` — the composed CHANGE step, the
//! path a session is actually on when it snapshots a node before editing it or
//! accepts a drift — passed `None` unconditionally with no parameter a caller
//! could use to say otherwise.
//!
//! ⭐ THE REASONING IN THE CODE WAS HALF RIGHT, WHICH IS WHY IT SURVIVED.
//! The comment argued that this path must not INFER an axis, because the
//! mapping from `change_type` is not total — a `resync` is honestly either one.
//! That is correct and still holds. What it did not follow from is the
//! conclusion drawn: that the axis must therefore be ABSENT. Not-inferring and
//! not-askable are different properties, and treating them as one left 468 of
//! 593 change events on this project's own graph with no axis on them.
//!
//! `None` still means nobody said. What it no longer means is nobody could.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;
use reflow2_core::temporal::ChangeRecord;
use reflow2_core::{ChangeAction, ChangeSubject, ChangeType, EpochType};

fn graph_with_an_epoch_and_a_thing() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_epoch("epoch:e", "e", EpochType::Revision, 1)
        .expect("epoch");
    g.add_artifact(
        "art:thing",
        "thing.rs",
        Some("code"),
        Some("crates/x/src/thing.rs"),
    )
    .expect("artifact");
    g
}

fn a_change(id: &'static str, subject: Option<ChangeSubject>) -> ChangeRecord<'static> {
    ChangeRecord {
        epoch_id: "epoch:e",
        change_event_id: id,
        name: "accepted the on-disk state",
        change_type: ChangeType::Resync,
        subject,
        target_type: "Artifact",
        target_id: "art:thing",
        action: ChangeAction::Modified,
    }
}

fn subject_of(g: &DesignGraph, id: &str) -> Option<String> {
    g.get_node(node::CHANGE_EVENT, id)
        .expect("get")
        .expect("event")
        .properties
        .get("subject")
        .and_then(|v| v.as_str().map(str::to_string))
}

/// The half that was unreachable: a resync where nothing about the system moved
/// and only the design's knowledge of it did.
#[test]
fn the_composed_change_step_can_say_only_the_record_moved() {
    let mut g = graph_with_an_epoch_and_a_thing();
    g.record_change(a_change("chg:knowledge-moved", Some(ChangeSubject::Record)))
        .expect("record_change must accept the axis its own schema declares");

    assert_eq!(
        subject_of(&g, "chg:knowledge-moved").as_deref(),
        Some("record"),
        "the axis the caller stated must reach the stored event, not be dropped on the way"
    );
}

/// The same call on the other axis, so the test cannot pass by hardcoding one.
#[test]
fn the_composed_change_step_can_say_the_system_moved() {
    let mut g = graph_with_an_epoch_and_a_thing();
    g.record_change(a_change("chg:thing-moved", Some(ChangeSubject::System)))
        .expect("record_change");

    assert_eq!(subject_of(&g, "chg:thing-moved").as_deref(), Some("system"));
}

/// Silence stays available and stays silent. `record_change` must not start
/// guessing an axis from `change_type` now that it has somewhere to put one —
/// `Resync` is the exact type whose mapping is not total, so a graph that
/// answered "system" here would be asserting something nobody said.
#[test]
fn saying_nothing_is_still_a_true_answer_and_is_never_filled_in() {
    let mut g = graph_with_an_epoch_and_a_thing();
    g.record_change(a_change("chg:unstated", None))
        .expect("record_change");

    assert_eq!(
        subject_of(&g, "chg:unstated"),
        None,
        "absent must stay absent: inferring an axis from change_type is the failure this \
         field was added to avoid, and a default would reintroduce it"
    );
}
