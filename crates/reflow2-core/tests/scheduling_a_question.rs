//! Scheduling the RESOLUTION of a gap into an increment.
//!
//! Anthony, 2026-08-20: *"add gaps that need to be closed to increments/epochs."*
//! The design carried 89 open gaps, 85 never put to anybody — a list that
//! endures and is skimmed is durable and useless at the same time. Scheduling
//! is what turns a standing list into work with a moment attached.
//!
//! ⭐ IT SCHEDULES THE QUESTION, NOT THE GAP, and that is the whole design.
//! **A gap is not a node.** Gaps are recomputed from the condition on every
//! run — which is why one carried the same id all day, and why it vanishes when
//! the condition is actually fixed rather than needing to be closed by hand.
//! That property is worth keeping, and it means there is nothing to hang a
//! schedule on. But `gap_to_prompt` already mints a durable `question:<id>` the
//! moment a gap is put to somebody, and `open_questions` already tracks it
//! until answered. So the durable node already existed; only the edge did not.
//!
//! ⭐ DELIVERY IS ANSWERING, and nothing could stand in for it. There is no
//! artifact to look for and no check to run: the whole content of closing a gap
//! is that the person whose judgement it needed gave one. `answer_question` is
//! the only thing that sets that, so delivery stays COMPUTED — the same rule
//! that governs every other scheduled item.
//!
//! 🛑 AND A WITHDRAWN QUESTION IS NOT OUTSTANDING. `outstanding` means "nobody
//! has said whether this was deferred or discontinued". Somebody said. Letting
//! it fall through would ask again, on every run, about a question already
//! taken off the table — the same false reading that made a delivered epoch
//! look unfinished until it was fixed.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, node};
use reflow2_core::temporal::{EpochType, ScheduleOutcome};

/// An arrived epoch with one asked question scheduled into it.
///
/// The id convention matters and is not decorative: `answer_question` takes a
/// GAP id and resolves it to `question:<same suffix>`. A fixture that names the
/// node anything else tests the schedule edge and nothing else — which is how
/// the first draft of these passed three cases while the interesting three
/// failed.
fn scheduled_question() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.create_node(
        node::QUESTION,
        "question:abc",
        Props::new()
            .set("question", "Where does packaging belong?")
            .set("gap_id", "gap:abc")
            .set("status", "asked"),
    )
    .expect("question");
    g.plan_epoch("epoch:1", "An increment", EpochType::Milestone, 1)
        .expect("epoch");
    g.schedule_for(
        node::QUESTION,
        "question:abc",
        node::DESIGN_EPOCH,
        "epoch:1",
        "expected",
        None,
    )
    .expect("schedule");
    g.set_epoch_status("epoch:1", "arrived").expect("arrived");
    g
}

fn outcome(g: &DesignGraph) -> ScheduleOutcome {
    let delta = g.arrival_delta("epoch:1").expect("delta");
    delta
        .items
        .first()
        .expect("one scheduled item")
        .outcome
        .clone()
}

#[test]
fn a_question_can_be_scheduled_into_an_increment() {
    let g = scheduled_question();
    let delta = g.arrival_delta("epoch:1").expect("delta");
    assert_eq!(delta.items.len(), 1);
    assert_eq!(delta.items[0].item_type, node::QUESTION);
    assert_eq!(delta.items[0].item_id, "question:abc");
}

#[test]
fn an_unanswered_question_is_outstanding() {
    // Correctly so: it is still pointed at an arrived moment and nobody has
    // said whether it moved or was dropped. This is the case `outstanding`
    // exists for.
    let g = scheduled_question();
    assert_eq!(outcome(&g), ScheduleOutcome::Outstanding);
}

#[test]
fn answering_the_question_delivers_it() {
    // THE CASE THE WHOLE THING IS FOR. Closing a gap produces no file and runs
    // no check; the answer IS the delivery, and it is computed from the record
    // `answer_question` writes rather than asserted beside it.
    let mut g = scheduled_question();
    assert_eq!(outcome(&g), ScheduleOutcome::Outstanding);

    g.answer_question("gap:abc", "Make it a subsystem — it is real and finished.")
        .expect("answered");

    assert_eq!(outcome(&g), ScheduleOutcome::Delivered);
}

#[test]
fn a_withdrawn_question_is_discontinued_not_outstanding() {
    // 🛑 `outstanding` means "NOBODY HAS SAID which of deferred or
    // discontinued this is". Somebody said. Reporting it as outstanding would
    // put the question again, every run, about something already taken off the
    // table — a false statement about a settled matter, which is the exact
    // shape of the delivery defect fixed the same week.
    let mut g = scheduled_question();
    g.withdraw_question("gap:abc").expect("withdrawn");
    assert_eq!(outcome(&g), ScheduleOutcome::Discontinued);
}

#[test]
fn a_question_needs_no_artifact_and_no_check_to_be_delivered() {
    // Stated as its own case because the two other schedulable types both
    // require evidence on disk or a passing check. A Question requires neither,
    // and that is not a loosening — answering is a strictly harder thing to
    // fake than either, since only the user's own word sets it.
    let mut g = scheduled_question();
    g.answer_question("gap:abc", "yes").expect("answered");
    assert_eq!(outcome(&g), ScheduleOutcome::Delivered);

    let n = g
        .get_node(node::QUESTION, "question:abc")
        .expect("get")
        .expect("present");
    assert_eq!(
        n.properties.get("status").and_then(|v| v.as_str()),
        Some("answered")
    );
}

#[test]
fn a_question_with_no_status_reads_as_asked_not_as_settled() {
    // Absent reads as the schema default. A Question written before this
    // existed must never be mistaken for a settled one — a silent promotion to
    // `delivered` on the day of an upgrade is the worst possible direction.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.create_node(
        node::QUESTION,
        "question:old-gap",
        Props::new()
            .set("question", "?")
            .set("gap_id", "gap:old-gap"),
    )
    .expect("question");
    g.plan_epoch("epoch:1", "An increment", EpochType::Milestone, 1)
        .expect("epoch");
    g.schedule_for(
        node::QUESTION,
        "question:old-gap",
        node::DESIGN_EPOCH,
        "epoch:1",
        "expected",
        None,
    )
    .expect("schedule");
    g.set_epoch_status("epoch:1", "arrived").expect("arrived");

    assert_eq!(outcome(&g), ScheduleOutcome::Outstanding);
}
