//! Step 4, the fork half (`cap:fork-alternatives`), on Anthony's rulings of
//! 2026-09-23: a settled decision says where going back to it would start from,
//! and re-opening it is one call that never un-accepts the original
//! (`dec:reopen-supersedes`).

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::verify::{ObservedVerification, VerifyReconcileOptions};

fn decision(g: &mut DesignGraph, id: &str, status: &str) {
    g.create_node(
        node::DECISION,
        id,
        Props::new()
            .set("name", id)
            .set("decision", "x")
            .set("status", status),
    )
    .expect("decision");
}

fn epoch(g: &mut DesignGraph, id: &str, seq: i64, checksum: Option<&str>) {
    g.create_node(
        node::DESIGN_EPOCH,
        id,
        Props::new()
            .set("name", id)
            .set("sequence", seq)
            .set("status", "arrived")
            .set_opt("checksum", checksum),
    )
    .expect("epoch");
}

/// A pump: an accepted choice pinned to v1, governing a capability, with a
/// change recorded at v2 and one at v0.
fn world() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Pump").expect("project");
    g.add_capability("cap:impeller", "Impeller", "moves water", Some("realized"))
        .expect("cap");
    decision(&mut g, "dec:centrifugal", "accepted");
    g.create_edge(
        edge::GOVERNED_BY,
        node::CAPABILITY,
        "cap:impeller",
        node::DECISION,
        "dec:centrifugal",
        Props::new(),
    )
    .expect("governed");
    epoch(&mut g, "epoch:v0", 10, None);
    epoch(&mut g, "epoch:v1", 20, Some("sha256:aaa"));
    epoch(&mut g, "epoch:v2", 30, Some("sha256:bbb"));
    g.pin_at_epoch(node::DECISION, "dec:centrifugal", "epoch:v1")
        .expect("pin");
    for (chg, at) in [("chg:old", "epoch:v0"), ("chg:new", "epoch:v2")] {
        g.create_node(
            node::CHANGE_EVENT,
            chg,
            Props::new()
                .set("name", chg)
                .set("change_type", "new_feature"),
        )
        .expect("chg");
        g.pin_at_epoch(node::CHANGE_EVENT, chg, at).expect("pin");
        g.create_edge(
            edge::CHANGED,
            node::CHANGE_EVENT,
            chg,
            node::CAPABILITY,
            "cap:impeller",
            Props::new(),
        )
        .expect("changed");
    }
    g
}

#[test]
fn a_pinned_decision_gives_its_epoch_address_and_what_changed_since() {
    let g = world();
    let f = g.fork_point("dec:centrifugal").expect("fork");
    let e = f.epoch.as_ref().expect("pinned");
    assert_eq!(e.epoch_id, "epoch:v1");
    assert_eq!(e.checksum.as_deref(), Some("sha256:aaa"));
    assert_eq!(f.governed, vec!["cap:impeller"]);
    let since: Vec<_> = f
        .changed_since
        .as_ref()
        .expect("orderable")
        .iter()
        .map(|c| c.change_event_id.as_str())
        .collect();
    assert_eq!(since, vec!["chg:new"], "only changes at LATER epochs");
}

#[test]
fn an_unpinned_decision_says_it_cannot_order_changes_rather_than_none_changed() {
    let mut g = world();
    decision(&mut g, "dec:loose", "accepted");
    let f = g.fork_point("dec:loose").expect("fork");
    assert!(f.epoch.is_none());
    assert!(f.changed_since.is_none(), "cannot tell, not nothing");
    assert!(f.note.contains("EARLIEST EVIDENCE"), "{}", f.note);
}

#[test]
fn the_fork_point_carries_the_bad_news_behind_the_decision() {
    let mut g = world();
    g.add_verification("ver:flow", "flow", None, None, None)
        .expect("ver");
    g.verifies("ver:flow", "Capability", "cap:impeller")
        .expect("verifies");
    g.set_verification_status("ver:flow", "passing", None, None)
        .expect("status");
    g.reconcile_verification(
        &[ObservedVerification {
            verification_id: "ver:flow".into(),
            outcome: "failed".into(),
        }],
        &VerifyReconcileOptions {
            record_events: true,
            exhaustive: false,
            detected_at: Some("2026-09-23".into()),
        },
    )
    .expect("reconcile");
    let f = g.fork_point("dec:centrifugal").expect("fork");
    assert!(f.doubt.iter().any(|e| e.evidence_id == "ver:flow"));
    assert_eq!(f.evidence["ver:flow"].kind, "failed_run");
}

#[test]
fn reopening_mints_a_proposed_decision_and_never_un_accepts_the_original() {
    let mut g = world();
    let r = g
        .reopen_decision(
            "dec:centrifugal",
            "dec:reopen-pump-type",
            "Which pump type, again?",
            "the flow test failed",
            Some("git:abc123:docs/design/reflow2.json"),
        )
        .expect("reopen");
    let orig = g
        .get_node(node::DECISION, "dec:centrifugal")
        .expect("read")
        .expect("present");
    assert_eq!(
        orig.properties.get("status").and_then(|v| v.as_str()),
        Some("accepted")
    );
    let new = g
        .get_node(node::DECISION, "dec:reopen-pump-type")
        .expect("read")
        .expect("present");
    assert_eq!(
        new.properties.get("status").and_then(|v| v.as_str()),
        Some("proposed")
    );
    assert!(
        g.outgoing("dec:reopen-pump-type", Some(edge::OBSOLETES))
            .expect("edges")
            .iter()
            .any(|e| e.to_id == "dec:centrifugal")
    );
    let road = r.road_taken.expect("the road taken is registered");
    assert_eq!(
        road.location.as_deref(),
        Some("git:abc123:docs/design/reflow2.json")
    );
    assert_eq!(
        g.alternatives_for("dec:reopen-pump-type")
            .expect("alts")
            .len(),
        1
    );
    let f = g.fork_point("dec:centrifugal").expect("fork");
    assert_eq!(f.reopened_by, vec!["dec:reopen-pump-type"]);
}

#[test]
fn only_a_settled_road_can_be_reopened() {
    let mut g = world();
    decision(&mut g, "dec:idea-open", "proposed");
    let err = g
        .reopen_decision("dec:idea-open", "dec:x", "x", "y", None)
        .expect_err("refused");
    assert!(format!("{err}").contains("only a settled road"), "{err}");
}

#[test]
fn the_same_question_is_not_reopened_twice_while_open() {
    let mut g = world();
    g.reopen_decision("dec:centrifugal", "dec:again-1", "again", "why", None)
        .expect("first");
    let err = g
        .reopen_decision("dec:centrifugal", "dec:again-2", "again", "why", None)
        .expect_err("refused");
    assert!(format!("{err}").contains("dec:again-1"), "{err}");
}

#[test]
fn without_an_address_the_road_taken_is_not_invented() {
    let mut g = world();
    let r = g
        .reopen_decision("dec:centrifugal", "dec:again", "again", "why", None)
        .expect("reopen");
    assert!(r.road_taken.is_none());
    assert!(r.note.contains("NOT registered"), "{}", r.note);
}
