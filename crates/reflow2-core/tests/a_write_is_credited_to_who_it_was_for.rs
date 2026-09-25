//! A caller names who it writes for, and every node it writes is credited to them.
//!
//! `req:a-session-names-the-person-it-writes-for-and-the-server-remembers-it`:
//! attribution only, on trust. The core half is two things — the graph records
//! every node written while a caller is recording (at `create_node`, the one
//! place every node write passes), and `credit_writes` draws one AUTHORED_BY
//! (role `author`) per written node. Nothing here grants authority: a credited
//! author is never an approver.

use reflow2_core::DesignGraph;
use reflow2_core::graph::{authored_roles, edge_has_role};
use reflow2_core::nodes::{Props, edge, node};

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:cure", "Cure").unwrap();
    g.add_contributor("who:sister", "Sister", Some("person"), None, None)
        .unwrap();
    g
}

fn authors_of(g: &DesignGraph, id: &str) -> Vec<(String, Vec<String>)> {
    g.outgoing(id, Some(edge::AUTHORED_BY))
        .unwrap()
        .into_iter()
        .map(|e| (e.to_id.clone(), authored_roles(&e)))
        .collect()
}

#[test]
fn every_node_written_while_recording_is_credited_once_as_author() {
    let mut g = design();
    g.begin_touch_log();
    g.add_requirement(
        "req:talk",
        "Talk freely",
        "Any part of health can be discussed",
    )
    .unwrap();
    // Written twice in one call: credited once.
    g.add_requirement("req:talk", "Talk freely", "Mental health to broken bones")
        .unwrap();
    g.create_node(
        node::DECISION,
        "dec:connector",
        Props::new()
            .set("name", "A connector")
            .set("decision", "Built like flo2"),
    )
    .unwrap();
    let touched = g.take_touch_log();
    assert_eq!(g.credit_writes(&touched, "who:sister").unwrap(), 2);
    for id in ["req:talk", "dec:connector"] {
        assert_eq!(
            authors_of(&g, id),
            vec![("who:sister".into(), vec!["author".into()])],
            "{id}"
        );
    }
}

#[test]
fn nothing_is_recorded_unless_a_caller_asked() {
    let mut g = design();
    g.add_requirement("req:quiet", "Quiet", "Written with no one recording")
        .unwrap();
    assert!(g.take_touch_log().is_empty());
    assert!(authors_of(&g, "req:quiet").is_empty());
}

#[test]
fn taking_the_log_stops_recording() {
    let mut g = design();
    g.begin_touch_log();
    g.add_requirement("req:a", "A", "first").unwrap();
    assert_eq!(g.take_touch_log().len(), 1);
    g.add_requirement("req:b", "B", "after the log was taken")
        .unwrap();
    assert!(
        g.take_touch_log().is_empty(),
        "a later caller's writes must not be credited to this one"
    );
}

#[test]
fn snapshots_and_contributors_are_never_credited() {
    let mut g = design();
    let touched = vec![
        (node::SNAPSHOT.to_string(), "snap:x".to_string()),
        (node::CONTRIBUTOR.to_string(), "who:sister".to_string()),
    ];
    assert_eq!(g.credit_writes(&touched, "who:sister").unwrap(), 0);
    assert!(
        authors_of(&g, "who:sister").is_empty(),
        "a person is not the author of themselves"
    );
}

#[test]
fn a_node_deleted_in_the_same_call_is_skipped_not_an_error() {
    let mut g = design();
    let touched = vec![(node::REQUIREMENT.to_string(), "req:gone".to_string())];
    assert_eq!(g.credit_writes(&touched, "who:sister").unwrap(), 0);
}

#[test]
fn an_unknown_contributor_is_refused_and_nothing_is_credited() {
    let mut g = design();
    g.begin_touch_log();
    g.add_requirement("req:x", "X", "y").unwrap();
    let touched = g.take_touch_log();
    let err = g
        .credit_writes(&touched, "who:nobody")
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("add_contributor"),
        "the refusal must say how to fix it: {err}"
    );
    assert!(authors_of(&g, "req:x").is_empty());
    assert!(g.require_writes_for("who:nobody").is_err());
    assert!(g.require_writes_for("who:sister").is_ok());
}

#[test]
fn crediting_an_author_never_makes_them_an_approver() {
    let mut g = design();
    g.begin_touch_log();
    g.add_requirement("req:x", "X", "y").unwrap();
    let touched = g.take_touch_log();
    g.credit_writes(&touched, "who:sister").unwrap();
    let e = g.outgoing("req:x", Some(edge::AUTHORED_BY)).unwrap();
    assert!(e.iter().all(|e| !edge_has_role(e, "approver")));
}

#[test]
fn an_existing_approval_is_kept_when_the_same_person_is_also_credited_as_author() {
    let mut g = design();
    g.add_requirement("req:x", "X", "y").unwrap();
    g.authored_by(
        node::REQUIREMENT,
        "req:x",
        "who:sister",
        Some("approver"),
        Some("2026-09-25"),
    )
    .unwrap();
    let touched = vec![(node::REQUIREMENT.to_string(), "req:x".to_string())];
    g.credit_writes(&touched, "who:sister").unwrap();
    let roles = &authors_of(&g, "req:x")[0].1;
    assert!(
        roles.contains(&"approver".to_string()) && roles.contains(&"author".to_string()),
        "{roles:?}"
    );
}
