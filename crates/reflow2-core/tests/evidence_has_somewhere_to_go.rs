//! A check has somewhere to put what it FOUND, separate from what it IS.
//!
//! `req:a-finding-has-somewhere-to-put-its-evidence`.
//!
//! # The measurement that shaped this, taken before any code was written
//!
//! The requirement named its own precondition — *"THE LOAD-BEARING
//! UNCERTAINTY, to answer BEFORE building: would a new field alone change
//! anything?"* — and the corpus answered it: **no.**
//!
//! 164 Verifications, median `name` 76 words, 72 over 100, longest 654. And
//! `description` was ALREADY declared, fulltext, and the embedding field —
//! **used once in 164 nodes**. Not ignored: UNREACHABLE.
//! `add_verification(id, name, method, level)` had no parameter for it, so the
//! only route was raw `create_node` and essentially nobody took it. Everyone
//! wrote into `name` because `name` was the only string on offer.
//!
//! So a `findings` field alone would have become the SECOND unused field. The
//! reachability is half the fix, and these probes pin both halves.

use reflow2_core::DesignGraph;

fn graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g
}

/// The half that would have been missed: the constructor can reach the field.
/// Without this, `description` stays what it has been since the beginning — a
/// declared, fulltext, embedding field with one writer in 164 nodes.
#[test]
fn the_constructor_can_reach_description_at_all() {
    let mut g = graph();
    g.add_verification(
        "ver:short",
        "the schema merges",
        Some("test"),
        Some("unit"),
        Some("Runs tools/validate_schema.py against every schema/*.yaml and requires OK."),
    )
    .unwrap();

    let v = g.get_node("Verification", "ver:short").unwrap().unwrap();
    assert_eq!(
        v.properties.get("description").unwrap().as_str(),
        Some("Runs tools/validate_schema.py against every schema/*.yaml and requires OK."),
    );
    assert_eq!(
        v.properties.get("name").unwrap().as_str(),
        Some("the schema merges"),
        "and the name stays a label rather than absorbing the account"
    );
}

/// Evidence belongs to a RUN, so it is written with the outcome.
#[test]
fn a_run_records_what_it_found_beside_its_outcome() {
    let mut g = graph();
    g.add_verification("ver:x", "the guard refuses a stale write", None, None, None)
        .unwrap();

    g.set_verification_status(
        "ver:x",
        "passing",
        Some("2026-08-17"),
        Some("Mutation-checked: discarding the refusal fails exactly two of four probes."),
    )
    .unwrap();

    let v = g.get_node("Verification", "ver:x").unwrap().unwrap();
    assert_eq!(
        v.properties.get("status").unwrap().as_str(),
        Some("passing")
    );
    assert_eq!(
        v.properties.get("findings").unwrap().as_str(),
        Some("Mutation-checked: discarding the refusal fails exactly two of four probes.")
    );
}

/// Omitting `findings` LEAVES IT ALONE — the same contract `last_run_at`
/// carries one field over, and the same bug if it were got wrong. Re-marking a
/// check passing without restating the evidence must not erase the evidence.
#[test]
fn re_running_without_restating_the_evidence_keeps_it() {
    let mut g = graph();
    g.add_verification("ver:y", "a check", None, None, None)
        .unwrap();
    g.set_verification_status(
        "ver:y",
        "passing",
        Some("2026-08-01"),
        Some("42 cases, 0 failures"),
    )
    .unwrap();

    // A later run that records only the outcome.
    g.set_verification_status("ver:y", "passing", Some("2026-08-17"), None)
        .unwrap();

    let v = g.get_node("Verification", "ver:y").unwrap().unwrap();
    assert_eq!(
        v.properties.get("findings").unwrap().as_str(),
        Some("42 cases, 0 failures"),
        "omitting findings must LEAVE IT ALONE, never erase it"
    );
    assert_eq!(
        v.properties.get("last_run_at").unwrap().as_str(),
        Some("2026-08-17"),
        "…while what WAS supplied still moves"
    );
}

/// …and supplying it replaces it, or "leaves it alone" would be indistinguishable
/// from "cannot be updated".
#[test]
fn a_later_run_can_replace_the_evidence() {
    let mut g = graph();
    g.add_verification("ver:z", "a check", None, None, None)
        .unwrap();
    g.set_verification_status("ver:z", "passing", None, Some("first run: clean"))
        .unwrap();
    g.set_verification_status("ver:z", "failing", None, Some("second run: 3 regressions"))
        .unwrap();

    let v = g.get_node("Verification", "ver:z").unwrap().unwrap();
    assert_eq!(
        v.properties.get("findings").unwrap().as_str(),
        Some("second run: 3 regressions")
    );
    assert_eq!(
        v.properties.get("status").unwrap().as_str(),
        Some("failing")
    );
}

/// The three fields are three DIFFERENT things, and conflating any two is the
/// state this requirement exists to end: `name` labels, `description` explains
/// the check, `findings` reports the run.
#[test]
fn name_description_and_findings_are_all_preserved_independently() {
    let mut g = graph();
    g.add_verification(
        "ver:three",
        "label",
        Some("test"),
        Some("unit"),
        Some("what the check is"),
    )
    .unwrap();
    g.set_verification_status(
        "ver:three",
        "passing",
        Some("2026-08-17"),
        Some("what it found"),
    )
    .unwrap();

    let v = g.get_node("Verification", "ver:three").unwrap().unwrap();
    assert_eq!(v.properties.get("name").unwrap().as_str(), Some("label"));
    assert_eq!(
        v.properties.get("description").unwrap().as_str(),
        Some("what the check is"),
        "recording an outcome must not cost the account of what the check is"
    );
    assert_eq!(
        v.properties.get("findings").unwrap().as_str(),
        Some("what it found")
    );
}
