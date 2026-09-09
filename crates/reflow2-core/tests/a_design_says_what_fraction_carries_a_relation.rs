//! *"Of my N things of kind X, how many carry relation R?"* — and a typo is
//! refused rather than answered.
//!
//! # The requirement
//!
//! `req:a-design-can-be-asked-what-fraction-of-a-kind-carries-a-relation`,
//! Anthony 2026-08-31: the core traceability question of systems engineering,
//! in the user's own words — *how many of my requirements are actually
//! verified?* Every session that needed it wrote its own script;
//! `fact:five-capabilities-...-one-query-shape-found-them-all` recorded five,
//! and the session that built this added six more in a single afternoon.
//!
//! # ⭐ The case that carries this file
//!
//! `a_typo_is_refused_rather_than_answered`. A misspelled node type would
//! otherwise return `0 of 0`, which is indistinguishable from a clean result —
//! the failure this project keeps meeting, most recently in
//! `untriaged_report`'s parking rule. Everything else here is arithmetic; that
//! one is the design.

use reflow2_core::relation_coverage::Direction;
use reflow2_core::{DesignGraph, nodes::Props, nodes::edge, nodes::node};

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_project("proj:p", "A project").expect("project");
    for i in 0..4 {
        g.add_requirement(
            &format!("req:r{i}"),
            &format!("Requirement {i}"),
            "something the design must do",
        )
        .expect("requirement");
    }
    // Two of the four are verified; the other two are not.
    for i in 0..2 {
        g.create_node(
            node::VERIFICATION,
            &format!("ver:v{i}"),
            Props::new()
                .set("name", format!("check {i}"))
                .set("status", "passing"),
        )
        .expect("verification");
        g.create_edge(
            edge::VERIFIES,
            node::VERIFICATION,
            &format!("ver:v{i}"),
            node::REQUIREMENT,
            &format!("req:r{i}"),
            Props::new(),
        )
        .expect("verifies");
    }
    g
}

#[test]
fn a_typo_is_refused_rather_than_answered() {
    // ⭐ THE ONE THAT MATTERS. `0 of 0 — 100%` about a kind that does not exist
    // reads exactly like good news, and a reader has no way to tell.
    let g = design();
    let err = g
        .relation_coverage("Requirment", edge::VERIFIES, Direction::Incoming)
        .expect_err("a misspelled node type was counted instead of refused");
    let msg = format!("{err}");
    assert!(
        msg.contains("Requirement"),
        "the refusal must point somewhere, not only say no: {msg}"
    );
}

#[test]
fn an_undeclared_edge_type_is_refused_too() {
    // The mirror, and it fails the other way round: every node would count as
    // MISSING the relation, so the answer would read as total absence — a
    // finding, rather than a typo.
    let g = design();
    let err = g
        .relation_coverage(node::REQUIREMENT, "VERIFYS", Direction::Incoming)
        .expect_err("an undeclared edge type was counted instead of refused");
    assert!(format!("{err}").contains("VERIFIES"), "{err}");
}

#[test]
fn it_answers_the_question_it_was_built_for() {
    let g = design();
    let r = g
        .relation_coverage(node::REQUIREMENT, edge::VERIFIES, Direction::Incoming)
        .expect("coverage");
    assert_eq!(r.population, 4);
    assert_eq!(r.with_relation, 2);
    assert_eq!(r.without, 2);
    assert_eq!(r.fraction, Some(0.5));
    assert_eq!(r.missing, vec!["req:r2".to_string(), "req:r3".to_string()]);
}

#[test]
fn direction_is_part_of_the_question_and_comes_back_with_the_answer() {
    // The same pair read the other way is a different question, and one that is
    // not a thing: requirements do not verify. It must answer 0, not 2.
    let g = design();
    let out = g
        .relation_coverage(node::REQUIREMENT, edge::VERIFIES, Direction::Outgoing)
        .expect("coverage");
    assert_eq!(out.with_relation, 0);
    assert_eq!(out.direction, "outgoing");
    let inc = g
        .relation_coverage(node::REQUIREMENT, edge::VERIFIES, Direction::Incoming)
        .expect("coverage");
    assert_eq!(inc.with_relation, 2);
    assert_eq!(inc.direction, "incoming");
}

#[test]
fn an_empty_population_reports_no_fraction_at_all() {
    // A fraction of nothing is not a fact. 0.0 and 1.0 are both claims the data
    // does not support, and either would sit in a report looking like one.
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_project("proj:p", "A project").expect("project");
    let r = g
        .relation_coverage(node::REQUIREMENT, edge::VERIFIES, Direction::Incoming)
        .expect("coverage");
    assert_eq!(r.population, 0);
    assert_eq!(r.fraction, None);
    assert!(
        r.note.contains("NOTHING TO EXAMINE") && r.note.contains("not a typo"),
        "an empty population must say which empty it is, and that the type WAS declared: {}",
        r.note
    );
}

#[test]
fn full_coverage_says_so_without_listing_nothing() {
    let mut g = design();
    for i in 2..4 {
        g.create_node(
            node::VERIFICATION,
            &format!("ver:v{i}"),
            Props::new()
                .set("name", format!("check {i}"))
                .set("status", "passing"),
        )
        .expect("verification");
        g.create_edge(
            edge::VERIFIES,
            node::VERIFICATION,
            &format!("ver:v{i}"),
            node::REQUIREMENT,
            &format!("req:r{i}"),
            Props::new(),
        )
        .expect("verifies");
    }
    let r = g
        .relation_coverage(node::REQUIREMENT, edge::VERIFIES, Direction::Incoming)
        .expect("coverage");
    assert_eq!(r.fraction, Some(1.0));
    assert!(r.missing.is_empty());
}
