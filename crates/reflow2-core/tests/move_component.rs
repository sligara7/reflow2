//! Re-parenting a Component on the containment spine.
//!
//! **The defect these pin was found by using reflow2, not by reading it**
//! (2026-08-20). Re-decomposition — what adopting a brownfield system, acting
//! on a design review, or doing severability work all consist of — had no
//! operation. Asked in a user's own words, *"move a component to a different
//! parent, re-decompose"*, `find_tools` ranked `contain_component` top of 152,
//! and `contain_component` ADDS a parent and removes nothing. So the
//! discoverable route was also the wrong one: it leaves the old edge behind,
//! the spine stops being a tree, and `hierarchy_issues` reports
//! `multiple_parents` afterwards — the wrong end of the act.
//!
//! That is not a hypothetical. It happened to this project's own `cmp:identity`
//! the same day, because it had been wired straight to the Project *precisely
//! because* it had no subsystem, so giving it a real parent left it with two.
//! `moving_a_component_wired_to_the_project_detaches_that_too` is that case.
//!
//! WHAT IS DELIBERATELY NOT TESTED HERE, said rather than left to be found: the
//! level relation is REPORTED and never enforced, so there is no test that a
//! bad level is refused — `hierarchy_issues` is the authority on decomposition
//! defects, and giving one rule two homes gives it two homes that can disagree.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::temporal::EpochType;

/// A project, a pair of subsystems, and one component under the first.
fn spine() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.add_component("sys:a", "A", "one subsystem", Some("subsystem"))
        .expect("sys:a");
    g.add_component("sys:b", "B", "another subsystem", Some("subsystem"))
        .expect("sys:b");
    g.add_component("cmp:x", "X", "a part", Some("component"))
        .expect("cmp:x");
    g.contain_component("sys:a", "cmp:x").expect("initial nest");
    g
}

fn parents(g: &DesignGraph, child: &str) -> Vec<String> {
    let mut p: Vec<String> = g
        .incoming(child, Some(edge::CONTAINS))
        .expect("incoming")
        .into_iter()
        .map(|e| e.from_id)
        .collect();
    p.sort();
    p
}

#[test]
fn moving_detaches_the_old_parent_and_says_which() {
    let mut g = spine();
    let out = g.move_component("cmp:x", "sys:b").expect("move");

    // The whole point: ONE parent afterwards, not two.
    assert_eq!(parents(&g, "cmp:x"), vec!["sys:b".to_string()]);
    // And it never happens silently — the caller is told what came off.
    assert_eq!(out.detached, vec!["sys:a".to_string()]);
    assert_eq!(out.new_parent_id, "sys:b");
    assert!(!out.already_there);
    // Parent one level above child is the ordinary case and says nothing.
    assert!(out.level_note.is_none(), "{:?}", out.level_note);
}

#[test]
fn moving_a_component_wired_to_the_project_detaches_that_too() {
    // The `cmp:identity` case. A component with no subsystem is often wired
    // straight to the Project; if the move only looked for Component parents,
    // giving it a real one would leave it with two — creating the exact defect
    // this operation exists to prevent.
    let mut g = spine();
    g.add_component(
        "cmp:homeless",
        "Homeless",
        "no subsystem",
        Some("component"),
    )
    .expect("cmp");
    g.create_edge(
        edge::CONTAINS,
        node::PROJECT,
        "proj:1",
        node::COMPONENT,
        "cmp:homeless",
        Props::new(),
    )
    .expect("project contains");

    let out = g.move_component("cmp:homeless", "sys:a").expect("move");

    assert_eq!(parents(&g, "cmp:homeless"), vec!["sys:a".to_string()]);
    assert_eq!(out.detached, vec!["proj:1".to_string()]);
}

#[test]
fn a_component_that_had_no_parent_reports_nothing_detached() {
    // "Placed for the first time" and "moved" are different facts. Reporting
    // an empty `detached` rather than a bare success is what tells them apart —
    // a caller who believed they were moving something learns they were not.
    let mut g = spine();
    g.add_component("cmp:new", "New", "unplaced", Some("component"))
        .expect("cmp");

    let out = g.move_component("cmp:new", "sys:a").expect("move");

    assert!(out.detached.is_empty());
    assert_eq!(parents(&g, "cmp:new"), vec!["sys:a".to_string()]);
    // Nothing was detached, so there is no lost history to warn about.
    assert!(out.history_note.is_none());
}

#[test]
fn moving_somewhere_it_already_is_is_idempotent() {
    let mut g = spine();
    let out = g.move_component("cmp:x", "sys:a").expect("move");

    assert!(out.already_there);
    assert!(out.detached.is_empty());
    // Exactly one edge, not a duplicate.
    assert_eq!(parents(&g, "cmp:x"), vec!["sys:a".to_string()]);
}

#[test]
fn a_component_cannot_contain_itself() {
    let mut g = spine();
    let err = g.move_component("cmp:x", "cmp:x").expect_err("refused");
    assert!(format!("{err}").contains("cannot contain itself"), "{err}");
    // And the refusal changed nothing.
    assert_eq!(parents(&g, "cmp:x"), vec!["sys:a".to_string()]);
}

#[test]
fn an_unknown_endpoint_is_refused_before_anything_moves() {
    let mut g = spine();

    let err = g.move_component("cmp:x", "sys:nope").expect_err("refused");
    assert!(format!("{err}").contains("sys:nope"), "{err}");
    // The refusal must not have detached the real parent on its way to failing.
    assert_eq!(parents(&g, "cmp:x"), vec!["sys:a".to_string()]);

    let err = g.move_component("cmp:nope", "sys:b").expect_err("refused");
    assert!(format!("{err}").contains("cmp:nope"), "{err}");
}

#[test]
fn a_level_that_will_trip_hierarchy_issues_is_named_at_the_moment_of_the_move() {
    // The remedy has to be reachable from the message the reader actually
    // sees. hierarchy_issues would report this later; saying it here is the
    // difference between a defect found and a defect avoided.
    let mut g = spine();
    let out = g.move_component("sys:b", "sys:a").expect("move");
    let note = out
        .level_note
        .expect("same-level containment should be named");
    assert!(note.contains("level_mismatch"), "{note}");
    assert!(note.contains("hierarchy_issues"), "{note}");

    // Inverted containment is a different sentence, not the same one reused.
    let mut g = spine();
    let out = g.move_component("sys:a", "cmp:x").expect("move");
    let note = out
        .level_note
        .expect("inverted containment should be named");
    assert!(note.contains("inverted"), "{note}");
}

#[test]
fn detaching_a_parent_names_the_call_that_preserves_it() {
    // The old containment is design history. The operation does not snapshot
    // for the caller — that would decide for them which epoch it belongs to —
    // but it must not let the history go quietly either.
    let mut g = spine();
    let out = g.move_component("cmp:x", "sys:b").expect("move");
    let note = out.history_note.expect("a detached parent is lost history");
    assert!(note.contains("record_change"), "{note}");
    assert!(note.contains("cmp:x"), "{note}");
    // It names WHICH parent came off, not just that one did.
    assert!(note.contains("sys:a"), "{note}");
}

#[test]
fn an_earlier_snapshot_does_not_silence_the_history_note() {
    // THE BUG THIS PINS, found by using the operation rather than reading it:
    // the first version suppressed the note whenever the child had ANY
    // snapshot. A snapshot taken in an earlier epoch says nothing about
    // whether THIS move is recorded, so that rule went quiet for exactly the
    // long-lived node whose history is most worth keeping — silent in the
    // dangerous direction.
    let mut g = spine();
    g.add_epoch(
        "epoch:earlier",
        "Something else entirely",
        EpochType::Revision,
        1,
    )
    .expect("epoch");
    g.snapshot_node("epoch:earlier", node::COMPONENT, "cmp:x")
        .expect("snapshot");

    let out = g.move_component("cmp:x", "sys:b").expect("move");

    assert!(
        out.history_note.is_some(),
        "an unrelated earlier snapshot must not suppress the note"
    );
}
