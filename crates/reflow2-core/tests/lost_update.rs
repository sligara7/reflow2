//! A write can be made conditional on what the caller read.
//!
//! # Why this exists
//!
//! `req:a-write-cannot-silently-lose-someone-elses-work`, the `required`
//! obligation of `epoch:instruments-stop-overstating`. MEASURED FROM BOTH SIDES
//! OF ONE COLLISION on a shared graph: a worker read a node, ninety seconds
//! later another attached session wrote it, and the write returned a normal
//! success with the full node body and nothing unusual. **The winner was never
//! told.** The loser found out only because `record_change` happens to return
//! the snapshot it took — a diagnostic side-effect of an unrelated tool, not a
//! guard.
//!
//! # Why a refusal and not a fifth report
//!
//! The `revision` block already REPORTS what a write replaced. That is
//! detection, and it tells the loser afterwards while telling the winner
//! nothing. `rule:fix-it-properly-while-it-is-still-cheap` is why this is a
//! precondition instead: the requirement's own words are that the revision
//! block's hash "is exactly the raw material a compare-and-swap needs; nothing
//! consumes it yet".

use reflow2_core::nodes::Props;
use reflow2_core::{DesignGraph, node_content_hash};

fn graph_with_a_note() -> (DesignGraph, String) {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_decision(
        "dec:shared",
        "A decision two people will edit",
        "The first draft.",
        None,
    )
    .unwrap();
    let read = g.get_node("Decision", "dec:shared").unwrap().unwrap();
    let hash = node_content_hash(&read.properties);
    (g, hash)
}

/// The whole point: an edit written against a stale read is REFUSED.
#[test]
fn a_write_against_a_stale_read_is_refused_and_names_both_hashes() {
    let (mut g, my_hash) = graph_with_a_note();

    // Somebody else writes in between — an ordinary, correct, unguarded write.
    g.upsert_node(
        "Decision",
        "dec:shared",
        Props::new().set("decision", "Their careful rewrite, which must not vanish."),
    )
    .unwrap();

    // My edit, still holding the hash from before their write.
    let err = g
        .upsert_node_if_unchanged(
            "Decision",
            "dec:shared",
            Props::new().set("decision", "My edit, written against what I read."),
            &my_hash,
        )
        .expect_err("a stale write must be refused, not merged");

    let msg = format!("{err}");
    assert!(
        msg.contains("has changed since you read it"),
        "the refusal must say WHY: {msg}"
    );
    assert!(
        msg.contains(&my_hash),
        "it must name what the caller expected: {msg}"
    );
    assert!(
        msg.contains("Re-read"),
        "and name what would have worked: {msg}"
    );

    // …and the other person's work is still there. This is the assertion the
    // whole requirement is about: refusing is worthless if it refused too late.
    let now = g.get_node("Decision", "dec:shared").unwrap().unwrap();
    assert_eq!(
        now.properties.get("decision").unwrap().as_str(),
        Some("Their careful rewrite, which must not vanish.")
    );
}

/// The counterweight, and without it the guard could simply refuse everything
/// and still pass the test above.
#[test]
fn a_write_against_a_current_read_goes_through() {
    let (mut g, my_hash) = graph_with_a_note();

    g.upsert_node_if_unchanged(
        "Decision",
        "dec:shared",
        Props::new().set("decision", "My edit, and nobody raced me."),
        &my_hash,
    )
    .expect("an unraced write must not be refused");

    let now = g.get_node("Decision", "dec:shared").unwrap().unwrap();
    assert_eq!(
        now.properties.get("decision").unwrap().as_str(),
        Some("My edit, and nobody raced me.")
    );
    // The merge semantics of upsert survive the guard: what I did not pass is
    // still there.
    assert_eq!(
        now.properties.get("name").unwrap().as_str(),
        Some("A decision two people will edit")
    );
}

/// A DELETION is a mismatch, not an invitation to create it back. Somebody
/// removed the node while the caller was deciding, and silently resurrecting it
/// would undo their removal — which is the same class of silent loss, pointed
/// the other way.
#[test]
fn a_write_against_a_node_somebody_deleted_is_refused_rather_than_recreating_it() {
    let (mut g, my_hash) = graph_with_a_note();
    g.delete_node("Decision", "dec:shared").unwrap();

    let err = g
        .upsert_node_if_unchanged(
            "Decision",
            "dec:shared",
            Props::new().set("decision", "My edit, against a node that is gone."),
            &my_hash,
        )
        .expect_err("a write against a deleted node must be refused");

    let msg = format!("{err}");
    assert!(
        msg.contains("DELETED") || msg.contains("does not exist"),
        "the refusal must distinguish deletion from a racing edit: {msg}"
    );
    assert!(
        g.get_node("Decision", "dec:shared").unwrap().is_none(),
        "and it must not have resurrected the node"
    );
}

/// The hash is a fact about CONTENT, not about map ordering — otherwise a
/// caller's correct expectation would be refused at random.
#[test]
fn the_content_hash_is_stable_across_property_ordering() {
    let mut a = std::collections::HashMap::new();
    a.insert("z".to_string(), reflow2_core::Value::from("last"));
    a.insert("a".to_string(), reflow2_core::Value::from("first"));
    let mut b = std::collections::HashMap::new();
    b.insert("a".to_string(), reflow2_core::Value::from("first"));
    b.insert("z".to_string(), reflow2_core::Value::from("last"));

    assert_eq!(node_content_hash(&a), node_content_hash(&b));
}
