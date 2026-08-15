//! A who-edge refused for a fixable reason says WHAT WOULD HAVE WORKED.
//!
//! # Why this exists
//!
//! `authored_by` and `owned_by` went straight to `create_edge`, whose endpoint
//! rule renders both of these identically as `Node not found: Contributor <id>`:
//!
//! - the id names nothing (a typo), and
//! - the id names a real node that is an **`Actor`**, not a `Contributor`.
//!
//! The second is the trap, because the node IS there. Told "not found", a caller
//! hunts for a wrong id and never suspects a wrong TYPE.
//!
//! Reported by dev_storyflow on 2026-08-15 **against the reporter itself**: rather
//! than retry, the session wrote the user's authorship into Decision prose about
//! ten times, which is the direct reason `what_next` had nothing to show in its
//! most important band. Its own conclusion — *"the failure mode of a rejected
//! typed call is not an error, it is prose."* The workaround reads better than the
//! edge would have, and silently drops the structure.
//!
//! The principle is not new here: `claim_region` already names its fix, the
//! missing-`seat` refusal has since 2026-07-30, and `get_node` refuses an unknown
//! node type rather than answering a confident `None`.

use reflow2_core::DesignGraph;

fn graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_decision(
        "dec:the-gate-uses-the-civil-date",
        "The gate uses the civil date",
        "The daily gate compares against the person's own calendar day, not UTC midnight.",
        None,
    )
    .expect("a decision to attribute");
    g
}

fn refusal(g: &mut DesignGraph, contributor_id: &str) -> String {
    g.authored_by(
        "Decision",
        "dec:the-gate-uses-the-civil-date",
        contributor_id,
        Some("author"),
        None,
    )
    .expect_err("a who-edge to a non-Contributor must be refused")
    .to_string()
}

/// THE REPORTED CASE. The node exists and is the wrong type.
#[test]
fn pointing_at_an_actor_says_it_is_a_type_error_and_not_a_missing_node() {
    let mut g = graph();
    g.create_node(
        "Actor",
        "actor:the-subscriber",
        reflow2_core::nodes::Props::new().set("name", "The subscriber"),
    )
    .expect("an Actor exists");

    let msg = refusal(&mut g, "actor:the-subscriber");
    assert!(
        msg.contains("EXISTS BUT IS AN Actor"),
        "the refusal must say the node is there and is the wrong type: {msg}"
    );
    assert!(
        msg.contains("add_contributor"),
        "and it must name the call that would have worked: {msg}"
    );
    assert!(
        !msg.contains("not found"),
        "'not found' is the false half — it sends the caller after a wrong id \
         when the id was right and the TYPE was wrong: {msg}"
    );
}

/// THE OTHER CASE, which must stay distinguishable from the first — two different
/// facts must not share one reply.
#[test]
fn pointing_at_nothing_says_nothing_is_there_and_names_the_fix() {
    let mut g = graph();
    let msg = refusal(&mut g, "who:nobody");
    assert!(
        msg.contains("no Contributor 'who:nobody' exists yet"),
        "a genuine absence must read as an absence: {msg}"
    );
    assert!(
        msg.contains("add_contributor"),
        "and still name the fix: {msg}"
    );
    assert!(
        !msg.contains("EXISTS BUT IS AN Actor"),
        "it must NOT claim a type error that did not happen: {msg}"
    );
}

/// `owned_by` fails the same way for the same reason, so it gets the same guard.
#[test]
fn owned_by_refuses_the_same_way() {
    let mut g = graph();
    g.create_node(
        "Actor",
        "actor:the-subscriber",
        reflow2_core::nodes::Props::new().set("name", "The subscriber"),
    )
    .expect("an Actor exists");

    let msg = g
        .owned_by(
            "Decision",
            "dec:the-gate-uses-the-civil-date",
            "actor:the-subscriber",
            None,
            None,
        )
        .expect_err("owned_by must refuse an Actor too")
        .to_string();
    assert!(
        msg.contains("EXISTS BUT IS AN Actor") && msg.contains("owned_by"),
        "{msg}"
    );
}

/// THE COUNTERWEIGHT. A real Contributor must still work — a guard that refused
/// everything would pass all three cases above and destroy the tool.
#[test]
fn a_real_contributor_still_records_authorship() {
    let mut g = graph();
    g.add_contributor(
        "who:ajs",
        "Anthony Sligar",
        Some("person"),
        Some("@ajs"),
        None,
    )
    .expect("a contributor");
    g.authored_by(
        "Decision",
        "dec:the-gate-uses-the-civil-date",
        "who:ajs",
        Some("author"),
        None,
    )
    .expect("a real Contributor must still be accepted");
}
