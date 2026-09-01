//! An edge's `evidence` is a claim about the world, and it goes stale.
//!
//! # The report
//!
//! bhome, 2026-08-31 (`art:bhome-genesis-feedback-2026-08-31`, part 7). Anthony
//! narrowed a climate requirement from desert-to-arctic to lower-48 city
//! climates. `propagate_change` returned 53 impacted nodes with an 8-node direct
//! ring, five of them RISKS edges pointing at the changed requirement — which
//! the report calls the best single moment of the session, because those five
//! objections had been written across three earlier conversations, hours apart,
//! and nothing else would have recalled them.
//!
//! But the five edges carried `evidence` written when the range WAS
//! desert-to-arctic — *"in the arctic, a tank of water at fish temperature is a
//! large continuous heat load"*. After the narrowing that evidence is WRONG, and
//! a reader following the edge is misled by prose that reads as current. Four of
//! five were repaired by hand, and only because:
//!
//! > the propagation happened to list them and I happened to remember what they
//! > said.
//!
//! # The asymmetry, which is the actual finding
//!
//! Node prose is governed three ways — every `add_*` warns on overwrite,
//! `reconcile_artifacts` catches design-vs-file drift, `change_axis_unstated`
//! catches an unstated axis. Edge prose had none of the three. An edge property
//! is a claim about the world in exactly the way a node property is, and only
//! one of the two was ever read.
//!
//! `fact:edge-prose-is-as-perishable-as-node-prose-and-nothing-reads-it`.
//!
//! # 🛑 What this check is NOT, and the tests hold the line
//!
//! It is a CRUDE LEXICAL CHECK and it says so in its own output. It cannot tell
//! a term that left the wording from a term that left the meaning, and it never
//! refuses a write — it is a warning attached to a propagation the caller asked
//! for.
//!
//! That restraint is deliberate and it is this project's own recent lesson: a
//! mis-tuned heuristic guard on this same surface fired 15 times with zero true
//! positives and was routinely pre-empted
//! (`fact:twelve-near-match-refusals-across-two-designs-and-not-one-was-a-duplicate`).
//! The difference here is that a warning which names its own crudeness costs a
//! glance, while a refusal costs a round trip and teaches the caller to
//! pre-empt it.
//!
//! So the counterweight test matters more than the positive one: an edge whose
//! prose is still wholly supported by the node it points at must stay SILENT.
//! If that ever fails, this check has become noise and should be removed rather
//! than tuned.

use reflow2_core::nodes::Props;
use reflow2_core::{DesignGraph, EpochType};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("in-memory graph")
}

/// The requirement BEFORE the narrowing.
const WIDE: &str = "The unit operates through the climate range from desert to arctic, without \
                    supplemental heating.";

/// The requirement AFTER it — "arctic" and "desert" are gone from it.
const NARROWED: &str = "The unit operates through the climate range of lower-48 city climates, \
                        without supplemental heating.";

/// Seeds the design as the reported session left it: the requirement was WIDE,
/// the change was recorded (which snapshots the prior state), and only then was
/// it narrowed. That order is the one `record_change` documents, and it is what
/// puts the prior text where this check can read it.
fn seed(g: &mut DesignGraph) {
    g.add_project("proj:bhome", "bhome").expect("project");
    g.add_requirement("req:climate-range", "Climate range", WIDE)
        .expect("requirement");
    g.add_capability(
        "cap:insulation",
        "Insulated envelope",
        "A sealed insulated envelope.",
        None,
    )
    .expect("capability");
}

/// Record the narrowing the way the loop prescribes: snapshot, then edit.
fn narrow_it(g: &mut DesignGraph) {
    g.add_epoch(
        "epoch:narrowing",
        "Climate narrowing",
        EpochType::Revision,
        1,
    )
    .expect("epoch");
    g.snapshot_node("epoch:narrowing", "Requirement", "req:climate-range")
        .expect("the prior state is captured before the edit");
    g.add_requirement("req:climate-range", "Climate range", NARROWED)
        .expect("the narrowed requirement replaces it");
}

/// ⭐ THE REPORTED CASE. The edge still argues about the arctic; the requirement
/// no longer mentions it.
#[test]
fn an_edge_whose_evidence_outlived_the_node_it_points_at_is_reported() {
    let mut g = graph();
    seed(&mut g);
    g.create_edge(
        "RISKS",
        "Capability",
        "cap:insulation",
        "Requirement",
        "req:climate-range",
        Props::new().set(
            "evidence",
            "In the arctic, a tank of water held at fish temperature is a large continuous \
                 heat load that the envelope cannot carry.",
        ),
    )
    .expect("the risk edge lands");
    narrow_it(&mut g);

    let report = g
        .stale_edge_evidence("req:climate-range")
        .expect("the check runs");

    assert!(
        report.coverage_note.is_none(),
        "a snapshot exists, so the check really ran: {report:?}"
    );
    assert_eq!(
        report.findings.len(),
        1,
        "the one stale edge is reported: {report:?}"
    );
    let found = &report.findings[0];
    assert_eq!(found.edge_type, "RISKS");
    assert_eq!(found.other_id, "cap:insulation");
    assert!(
        found.absent_terms.iter().any(|t| t == "arctic"),
        "the term the narrowing removed must be named — naming the EDGE alone would leave the \
         reader to diff two paragraphs by eye, which is the work this exists to save: {found:?}"
    );
}

/// 🛑 THE COUNTERWEIGHT, and the more important of the two. An edge whose prose
/// is still supported by the node must stay silent. A check that fires on
/// healthy edges is worse than no check: it trains the reader to skip it, which
/// is exactly how the near-match guard stopped being a check.
#[test]
fn an_edge_whose_evidence_still_holds_is_silent() {
    let mut g = graph();
    seed(&mut g);
    g.create_edge(
        "RISKS",
        "Capability",
        "cap:insulation",
        "Requirement",
        "req:climate-range",
        Props::new().set(
            "evidence",
            "Lower-48 city climates still demand an envelope that operates without \
                 supplemental heating.",
        ),
    )
    .expect("the risk edge lands");
    narrow_it(&mut g);

    let report = g
        .stale_edge_evidence("req:climate-range")
        .expect("the check runs");
    assert!(
        report.coverage_note.is_none(),
        "a snapshot exists, so the check really ran: {report:?}"
    );
    assert_eq!(
        report.edges_with_prose, 1,
        "it did look at the edge — an empty result over ZERO examined edges would prove nothing: \
         {report:?}"
    );
    assert!(
        report.findings.is_empty(),
        "an edge still supported by the node must not be reported: {report:?}"
    );
}

/// An edge carrying no prose at all has nothing to go stale, and must not be
/// reported as if it had. This is the `unstated` distinction `budget_report`
/// makes: absent is not the same fact as wrong.
#[test]
fn an_edge_with_no_prose_is_not_reported() {
    let mut g = graph();
    seed(&mut g);
    g.create_edge(
        "SATISFIES",
        "Capability",
        "cap:insulation",
        "Requirement",
        "req:climate-range",
        Props::new().build(),
    )
    .expect("the plain edge lands");
    narrow_it(&mut g);

    let report = g
        .stale_edge_evidence("req:climate-range")
        .expect("the check runs");
    assert!(
        report.findings.is_empty(),
        "an edge with no evidence has nothing to be stale: {report:?}"
    );
    assert_eq!(
        report.edges_with_prose, 0,
        "and it says it examined no prose, so the zero is not read as a clean bill: {report:?}"
    );
}

/// 🛑 SILENT, NOT CLEAN. With no snapshot there is no prior text, so an edge
/// that outlived the node is indistinguishable from one that never matched it.
/// Reporting that as "no findings" is the failure this project has a name for,
/// and it is the reason `coverage_note` exists.
#[test]
fn with_no_prior_state_the_check_says_it_could_not_run() {
    let mut g = graph();
    seed(&mut g);
    g.create_edge(
        "RISKS",
        "Capability",
        "cap:insulation",
        "Requirement",
        "req:climate-range",
        Props::new().set(
            "evidence",
            "In the arctic this envelope cannot carry the load.",
        ),
    )
    .expect("the risk edge lands");
    // Deliberately NO snapshot and no narrowing.

    let report = g
        .stale_edge_evidence("req:climate-range")
        .expect("the check runs");
    assert!(
        report.findings.is_empty(),
        "nothing can be concluded without prior text: {report:?}"
    );
    let note = report
        .coverage_note
        .as_deref()
        .expect("a zero with no prior state must SAY it is silent rather than clean");
    assert!(
        note.contains("silent") && note.contains("record_change"),
        "the note must say it is silent rather than clean, and name what would fix that: {note}"
    );
}
