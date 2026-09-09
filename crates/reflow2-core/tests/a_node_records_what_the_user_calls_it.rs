//! A node records what the USER calls it, and recording a second word never
//! costs the first.
//!
//! # Why this exists
//!
//! reflow2's vocabulary is deliberately abstract so it spans a house, a
//! spacecraft and a service. That abstraction leaves every user translating
//! their field's nouns into it — and one of them wrote a private translator
//! skill to do the translating, which put the mapping outside the design where
//! nothing could read it, check it, or carry it to the next session
//! (`dec:idea-a-users-domain-vocabulary-has-no-route-into-se-vocabulary`).
//!
//! Anthony's own framing is that every element of a design is a black box with
//! inner functions and interfaces (`req:recursive-black-box-decomposition`,
//! accepted). The frame is domain-neutral on purpose; the words a user brings
//! to it are not. This is where those words go.
//!
//! # What is pinned, and why the merge is the load-bearing part
//!
//! **The merge.** A replacing setter here would be this codebase's most-repeated
//! defect — a write path that builds a node from a partial property set and
//! erases what the caller did not name — arriving in the one place where the
//! value erased is a person's own vocabulary. It has been fixed four times
//! elsewhere (BL-46, BL-166, BL-183, `set_verification_status`), so it is pinned
//! here before it can happen a fifth.

use reflow2_core::{DesignGraph, Value, nodes::node};

fn graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_capability(
        "cap:answer-a-question",
        "Answer a question against the record set",
        "takes a question and returns matching records",
        None,
    )
    .expect("capability");
    g
}

fn aliases_of(g: &DesignGraph, id: &str) -> Vec<String> {
    match g
        .get_node(node::CAPABILITY, id)
        .expect("read")
        .expect("node exists")
        .properties
        .get("aliases")
    {
        Some(Value::List(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

/// The user's word reaches the stored property.
#[test]
fn a_users_own_word_for_a_thing_is_recorded() {
    let mut g = graph();
    g.record_alias(
        node::CAPABILITY,
        "cap:answer-a-question",
        &["query".to_string()],
    )
    .expect("record");
    assert_eq!(aliases_of(&g, "cap:answer-a-question"), vec!["query"]);
}

/// THE LOAD-BEARING ONE. A second word never costs the first.
#[test]
fn recording_a_second_word_does_not_cost_the_first() {
    let mut g = graph();
    g.record_alias(
        node::CAPABILITY,
        "cap:answer-a-question",
        &["query".to_string()],
    )
    .expect("first");
    g.record_alias(
        node::CAPABILITY,
        "cap:answer-a-question",
        &["lookup".to_string(), "search request".to_string()],
    )
    .expect("second");

    assert_eq!(
        aliases_of(&g, "cap:answer-a-question"),
        vec!["query", "lookup", "search request"],
        "terms MERGE, in the order they were learned — a replacing setter here would erase \
         somebody's own vocabulary, which is this codebase's most-repeated defect arriving in \
         the worst possible place"
    );
}

/// Recording a term twice is a no-op, not a growing list.
#[test]
fn the_same_word_recorded_twice_is_not_repeated() {
    let mut g = graph();
    for _ in 0..3 {
        g.record_alias(
            node::CAPABILITY,
            "cap:answer-a-question",
            &["query".to_string(), "QUERY".to_string()],
        )
        .expect("record");
    }
    assert_eq!(
        aliases_of(&g, "cap:answer-a-question"),
        vec!["query"],
        "case-insensitively the same word is the same word"
    );
}

/// Nothing else about the node moves — including the name, which is the
/// design's word and is NOT what an alias replaces.
#[test]
fn recording_an_alias_renames_nothing() {
    let mut g = graph();
    let before = g
        .get_node(node::CAPABILITY, "cap:answer-a-question")
        .expect("read")
        .expect("exists");
    g.record_alias(
        node::CAPABILITY,
        "cap:answer-a-question",
        &["query".to_string()],
    )
    .expect("record");
    let after = g
        .get_node(node::CAPABILITY, "cap:answer-a-question")
        .expect("read")
        .expect("exists");

    for key in ["name", "description", "status"] {
        assert_eq!(
            before.properties.get(key),
            after.properties.get(key),
            "`{key}` must be untouched: an alias records what the user calls the thing, it does \
             not rename the thing"
        );
    }
}

/// Passing no terms is REFUSED rather than treated as a clear. Erasing
/// somebody's vocabulary should not be reachable by passing nothing.
#[test]
fn passing_no_terms_is_refused_rather_than_clearing() {
    let mut g = graph();
    g.record_alias(
        node::CAPABILITY,
        "cap:answer-a-question",
        &["query".to_string()],
    )
    .expect("first");
    let err = g
        .record_alias(node::CAPABILITY, "cap:answer-a-question", &[])
        .expect_err("an empty list is refused");
    assert!(
        err.to_string().contains("does NOT clear"),
        "the refusal must say that omitting terms is not how you clear them: {err}"
    );
    assert_eq!(
        aliases_of(&g, "cap:answer-a-question"),
        vec!["query"],
        "and nothing was lost"
    );
}

/// A type that names a STATEMENT rather than a thing is refused, and the
/// refusal says which types accept it and why.
#[test]
fn a_type_that_names_no_thing_is_refused_with_the_reason() {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_constraint(
        "con:dose",
        "Dose limit",
        "Stay under the damage threshold.",
        None,
        None,
        None,
        None,
        None,
    )
    .expect("constraint");
    let err = g
        .record_alias(node::CONSTRAINT, "con:dose", &["exposure cap".to_string()])
        .expect_err("Constraint does not accept aliases");
    let msg = err.to_string();
    assert!(
        msg.contains("Requirement") && msg.contains("Flow"),
        "the refusal must name the five that DO accept it: {msg}"
    );
}
