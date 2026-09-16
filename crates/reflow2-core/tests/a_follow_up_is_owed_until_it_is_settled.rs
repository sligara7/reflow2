//! A follow-up captured with one word stays owed until somebody settles it.
//!
//! The `/log-issue` skill is a door: a dated observation on the thing it is
//! about, in the person's own words, with no cause demanded. The counter-
//! argument recorded beside the idea is that a capture nobody ever revisits is
//! a to-do list by another name — the objection that killed a Task node type
//! (`dec:idea-a-one-word-capture-for-something-to-come-back-to`). This is the
//! other half: `loop_status` lists every open follow-up at the boundary, and
//! stops listing one only when it is SETTLED — closed by a record that answered
//! it (INVALIDATES), or lapsed with a `valid_to`. Anthony, 2026-09-16, from a
//! beamline: "I just want to go to that project and type /log-issue ... so
//! that it gets included in the design as something to follow up on."
//!
//! Pinned from both sides: the open one is owed and named, and each way of
//! settling it clears it; an ordinary finding is never mistaken for one.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};
use std::collections::HashMap;

fn design_with_a_service() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:beamline", "Beamline").unwrap();
    g.add_component("cmp:queueservice", "queueservice", "runs the queue", None)
        .unwrap();
    g
}

fn follow_up(g: &mut DesignGraph, id: &str, subject: &str, words: &str, on: &str) {
    g.create_node(
        node::TEMPORAL_FACT,
        id,
        Props::new()
            .set("name", words)
            .set("statement", words)
            .set("subject_id", subject)
            .set("fact_type", "follow_up")
            .set("basis", "measured")
            .set("valid_from", on),
    )
    .unwrap();
}

#[test]
fn an_open_follow_up_is_owed_and_named() {
    let mut g = design_with_a_service();
    follow_up(
        &mut g,
        "fact:follow-up-queueservice-misbehaving",
        "cmp:queueservice",
        "walked past the beamline; queueservice was having an issue — look later",
        "2026-09-16",
    );
    let s = g.loop_status().unwrap();
    assert_eq!(s.follow_ups_open, 1);
    assert_eq!(
        s.follow_ups[0].fact_id,
        "fact:follow-up-queueservice-misbehaving"
    );
    assert_eq!(s.follow_ups[0].subject_id, "cmp:queueservice");
    assert_eq!(s.follow_ups[0].captured_on.as_deref(), Some("2026-09-16"));
    assert!(
        s.next
            .iter()
            .any(|l| l.contains("1 follow-up(s) captured and never revisited")),
        "the boundary read must say it in words, not only in a count: {:?}",
        s.next
    );
    assert!(!s.clean, "an open follow-up is debt the loop owes");
}

#[test]
fn a_record_that_answered_it_settles_it() {
    let mut g = design_with_a_service();
    follow_up(
        &mut g,
        "fact:follow-up-queue",
        "cmp:queueservice",
        "queue stalls on restart",
        "2026-09-16",
    );
    g.add_change_event(
        "chg:queue-restart-fixed",
        "queue restart fixed",
        reflow2_core::temporal::ChangeType::NewFeature,
        Some(reflow2_core::temporal::ChangeSubject::System),
        Some("the stall was a missing wait"),
        None,
        Some("2026-09-17"),
    )
    .unwrap();
    g.create_edge(
        edge::INVALIDATES,
        node::CHANGE_EVENT,
        "chg:queue-restart-fixed",
        node::TEMPORAL_FACT,
        "fact:follow-up-queue",
        HashMap::<String, reflow2_core::Value>::new(),
    )
    .unwrap();
    let s = g.loop_status().unwrap();
    assert_eq!(
        s.follow_ups_open, 0,
        "answered means settled: {:?}",
        s.follow_ups
    );
}

#[test]
fn a_lapsed_follow_up_with_a_valid_to_is_settled() {
    let mut g = design_with_a_service();
    follow_up(
        &mut g,
        "fact:follow-up-old",
        "cmp:queueservice",
        "old note",
        "2026-09-01",
    );
    g.upsert_node(
        node::TEMPORAL_FACT,
        "fact:follow-up-old",
        Props::new().set("valid_to", "2026-09-10"),
    )
    .unwrap();
    assert_eq!(g.loop_status().unwrap().follow_ups_open, 0);
}

#[test]
fn an_ordinary_finding_is_not_a_follow_up() {
    let mut g = design_with_a_service();
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:measured-latency",
        Props::new()
            .set("name", "latency measured")
            .set("statement", "p50 latency 120 ms")
            .set("subject_id", "cmp:queueservice")
            .set("fact_type", "finding")
            .set("basis", "measured")
            .set("valid_from", "2026-09-16"),
    )
    .unwrap();
    let s = g.loop_status().unwrap();
    assert_eq!(
        s.follow_ups_open, 0,
        "a finding is not owed a revisit: {:?}",
        s.follow_ups
    );
}

#[test]
fn follow_ups_come_back_oldest_first() {
    let mut g = design_with_a_service();
    follow_up(
        &mut g,
        "fact:follow-up-b",
        "cmp:queueservice",
        "second",
        "2026-09-16",
    );
    follow_up(
        &mut g,
        "fact:follow-up-a",
        "cmp:queueservice",
        "first",
        "2026-09-10",
    );
    let s = g.loop_status().unwrap();
    let ids: Vec<&str> = s.follow_ups.iter().map(|f| f.fact_id.as_str()).collect();
    assert_eq!(ids, vec!["fact:follow-up-a", "fact:follow-up-b"]);
}
