//! What a snapshot still holds of a node's prior state — whole, and FIELD BY
//! FIELD.
//!
//! The field half exists because the whole-node answer was reported to callers
//! as if it were the only one, and that made a revising write say "no snapshot
//! holds the state it replaced" about a value sitting in a snapshot verbatim.
//! Reported from the field 2026-08-21 by a session that checked the snapshot by
//! hand rather than believing the message.
//!
//! These probe the CORE function directly. The served behaviour is pinned in
//! `reflow2-mcp/tests/a_revising_write_says_whether_the_state_survived.rs`; this
//! file covers the contract that function offers to any caller, including the
//! inputs the MCP layer happens never to send.

use std::collections::HashMap;

use dynograph_core::Value;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, node};
use reflow2_core::temporal::{ChangeAction, ChangeRecord, ChangeType, EpochType};

/// A decision, snapshotted at its original state.
fn snapshotted() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_decision("dec:x", "A choice", "the original text", Some("why"))
        .expect("decision");
    g.add_epoch("epoch:e", "E", EpochType::Revision, 1)
        .expect("epoch");
    snapshot(&mut g, "epoch:e", "chg:c");
    g
}

/// Take a snapshot of `dec:x` at the given epoch.
fn snapshot(g: &mut DesignGraph, epoch_id: &str, change_event_id: &str) {
    g.record_change(ChangeRecord {
        epoch_id,
        change_event_id,
        name: "revising",
        change_type: ChangeType::ScopeChange,
        subject: None,
        target_type: node::DECISION,
        target_id: "dec:x",
        action: ChangeAction::Modified,
    })
    .expect("record_change");
}

fn props(pairs: &[(&str, &str)]) -> HashMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), Value::from(*v)))
        .collect()
}

#[test]
fn a_field_the_snapshot_holds_is_found_even_when_the_whole_state_moved_on() {
    // The reported sequence, at the core: the node has moved on since the
    // snapshot, so no whole-state match exists — but `decision` has not moved,
    // and that is the question the caller actually needs answered.
    let g = snapshotted();
    let cov = g
        .prior_state_coverage(
            "dec:x",
            "sha256:a-hash-no-snapshot-has",
            &["decision".to_string()],
            &props(&[("decision", "the original text")]),
        )
        .expect("coverage");

    assert!(cov.whole.is_none(), "no snapshot matches the whole state");
    assert!(
        cov.by_field
            .get("decision")
            .is_some_and(|s| s.contains("dec:x")),
        "but `decision`'s prior value IS held, and the snapshot is named: {:?}",
        cov.by_field
    );
}

#[test]
fn a_different_value_in_the_snapshot_is_not_preservation() {
    // 🛑 THE TRAP THIS MUST NOT FALL INTO. The snapshot HAS a `decision`. It is
    // simply not the one being replaced. Matching on the key rather than the
    // value would reassure a caller that something destroyed is recoverable —
    // the worst answer this function can give.
    let g = snapshotted();
    let cov = g
        .prior_state_coverage(
            "dec:x",
            "sha256:irrelevant",
            &["decision".to_string()],
            &props(&[("decision", "an INTERMEDIATE value nobody snapshotted")]),
        )
        .expect("coverage");

    assert!(
        cov.by_field.is_empty(),
        "the snapshot holds a different `decision`, so it preserves nothing about this one: {:?}",
        cov.by_field
    );
}

#[test]
fn a_field_absent_from_the_prior_state_is_never_reported_as_preserved() {
    // 🛑 ABSENT IS NOT A MATCH, and this is the probe that makes the guard
    // reachable. The MCP layer only ever asks about fields that were present
    // before the write, so from there this input cannot occur — but this is a
    // public function and a caller with no such invariant gets a straight
    // answer rather than a `None == None` coincidence reported as evidence.
    let g = snapshotted();
    let cov = g
        .prior_state_coverage(
            "dec:x",
            "sha256:irrelevant",
            &["a_field_nobody_ever_set".to_string()],
            &props(&[("decision", "the original text")]),
        )
        .expect("coverage");

    assert!(
        cov.by_field.is_empty(),
        "neither the prior state nor the snapshot has this field; two absences are not a \
         preserved value: {:?}",
        cov.by_field
    );
}

#[test]
fn the_whole_state_answer_still_works_and_covers_everything() {
    let g = snapshotted();
    let before = g
        .get_node(node::DECISION, "dec:x")
        .expect("get")
        .expect("present");
    let hash = reflow2_core::node_content_hash(&before.properties);

    let cov = g
        .prior_state_coverage(
            "dec:x",
            &hash,
            &["decision".to_string()],
            &before.properties,
        )
        .expect("coverage");

    assert!(
        cov.whole.is_some_and(|s| s.contains("dec:x")),
        "the untouched node matches its own snapshot exactly"
    );
    assert!(
        cov.by_field.contains_key("decision"),
        "and the field half agrees rather than contradicting it"
    );
}

#[test]
fn a_snapshot_of_another_node_is_never_consulted() {
    let mut g = snapshotted();
    g.add_decision("dec:other", "Another", "the original text", None)
        .expect("decision");

    // `dec:other` has the SAME `decision` text, and no snapshot of its own.
    let cov = g
        .prior_state_coverage(
            "dec:other",
            "sha256:irrelevant",
            &["decision".to_string()],
            &props(&[("decision", "the original text")]),
        )
        .expect("coverage");

    assert!(
        cov.by_field.is_empty() && cov.whole.is_none(),
        "an identical value under a DIFFERENT node's snapshot preserves nothing here: {cov:?}"
    );
}

#[test]
fn the_answer_is_deterministic_across_several_holding_snapshots() {
    // Two snapshots can hold the same field value. Which one is named must not
    // depend on scan order, or the same call answers differently twice.
    let mut g = snapshotted();
    g.add_epoch("epoch:e2", "E2", EpochType::Revision, 2)
        .expect("epoch");
    snapshot(&mut g, "epoch:e2", "chg:c2");

    // 🛑 COMPARING TWO CALLS WOULD NOT TEST THIS, and that was this probe's
    // first version. Both calls walk the store in the same order inside one
    // process, so they agree whether or not any rule is applied — an assertion
    // that can only pass. The THIRD inert-determinism assertion caught in a
    // single day; the fix is always the same, assert the rule against something
    // outside the iteration.
    let held = g
        .prior_state_coverage(
            "dec:x",
            "sha256:irrelevant",
            &["decision".to_string()],
            &props(&[("decision", "the original text")]),
        )
        .expect("coverage")
        .by_field;

    let named = held.get("decision").expect("both snapshots hold it");
    let both = ["snap:epoch:e:dec:x", "snap:epoch:e2:dec:x"];
    assert!(
        both.contains(&named.as_str()),
        "the named snapshot is one of the two that exist: {named}"
    );
    assert_eq!(
        named,
        both.iter().min().expect("two ids"),
        "and it is the lexicographically smallest — a stated rule, not whichever the store \
         happened to yield last"
    );
}

#[test]
fn a_node_with_no_snapshot_at_all_reports_nothing_rather_than_erroring() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.create_node(
        node::DECISION,
        "dec:lonely",
        Props::new().set("name", "L").set("decision", "d"),
    )
    .expect("node");

    let cov = g
        .prior_state_coverage(
            "dec:lonely",
            "sha256:irrelevant",
            &["name".to_string()],
            &props(&[("name", "L")]),
        )
        .expect("coverage");

    assert!(cov.whole.is_none() && cov.by_field.is_empty());
}
