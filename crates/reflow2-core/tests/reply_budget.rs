//! A reply that will not fit is still an answer — and says what it left out.
//!
//! The defect these pin, measured on reflow2's own design on 2026-08-23:
//! unscoped `detect_gaps` returned 79,566 characters of JSON, the harness
//! refused the call outright, and what the session saw was a wall of *harness*
//! text. So the documented first move of a session could not be made at all on
//! a mature design, and the tool that knew how to narrow never got to say so.
//!
//! `cap:bounded-reads` had promised since 2026-07-25 that a read which would not
//! fit answers with a bounded page and says what it left out. It was `verified`
//! — of `scan_nodes`, which is what `ver:bounded-reads` drives — and silent
//! about the call the whole loop orbits. These tests are the other half of that
//! claim.
//!
//! The property under all of them: **a shorter answer is never a quieter one.**
//! `count` and `by_source` describe every open gap in every tier, so the reply
//! can shrink without the design ever looking healthier than it is.

use reflow2_core::detect::{AFFECTED_CAP, ReplyDetail};
use reflow2_core::nodes::{Props, node};
use reflow2_core::{DesignGraph, GapCandidate};

/// `n` requirements nothing satisfies — `n` gaps, each carrying real prose.
///
/// The one component, capability and satisfied requirement are not decoration:
/// on a design with nothing allocated anywhere the phase detectors yield a
/// single "concept without design" rollup and every per-requirement gap stays
/// silent, so a fixture without them measures nothing.
fn design_with_gaps(n: usize) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "A project").unwrap();
    g.add_component("cmp:c", "A component", "does things", None)
        .unwrap();
    g.add_capability("cap:c", "Do the thing", "it does the thing", None)
        .unwrap();
    g.allocate("cap:c", "cmp:c").unwrap();
    g.add_requirement(
        "req:anchor",
        "The thing happens",
        "The system shall do the thing.",
    )
    .unwrap();
    g.satisfies("cap:c", "req:anchor").unwrap();
    for i in 0..n {
        g.add_requirement(
            &format!("req:{i}"),
            &format!("Requirement number {i}"),
            "The system shall do the thing this requirement is about, to the stated tolerance.",
        )
        .unwrap();
    }
    g
}

#[test]
fn an_answer_that_fits_is_returned_whole() {
    let g = design_with_gaps(3);
    let report = g.detect_gaps_within(30_000).unwrap();

    assert_eq!(report.budget.detail, ReplyDetail::Full);
    assert_eq!(report.budget.listed, report.count);
    assert!(
        report.budget.note.is_none(),
        "nothing was withheld, so there must be nothing to say about withholding: {:?}",
        report.budget.note
    );
    assert!(
        report.items.iter().all(|r| !r.gap.description.is_empty()),
        "the full tier keeps the prose"
    );
}

#[test]
fn a_shorter_answer_is_never_a_quieter_one() {
    let g = design_with_gaps(60);
    let every_gap = g.detect_gaps().unwrap().len();

    // Small enough that the prose cannot survive (29,651 chars), large enough
    // that all 63 titles do (15,525).
    let report = g.detect_gaps_within(20_000).unwrap();

    assert_eq!(report.budget.detail, ReplyDetail::TitlesOnly);
    assert_eq!(
        report.count, every_gap,
        "the COUNT is what says how much is open, and it is never budgeted away"
    );
    assert_eq!(
        report.items.len(),
        every_gap,
        "at this tier every gap is still listed — only what is said about each one went"
    );
    assert_eq!(
        report.by_source.values().sum::<usize>(),
        every_gap,
        "the counts by kind cover every gap, not only the listed ones"
    );
    assert!(
        report.items.iter().all(|r| r.gap.description.is_empty()
            && r.gap.evidence.is_empty()
            && r.gap.affected_ids.is_empty()),
        "the prose and the id lists are what this tier withholds"
    );
    assert!(
        report.items.iter().all(|r| !r.gap.title.is_empty()),
        "a row with no title would be a gap that had gone quiet"
    );

    let note = report.budget.note.expect("withholding must be stated");
    assert!(note.contains("WITHHELD TO FIT"), "{note}");
    assert!(
        note.contains("`scope`"),
        "the note is where reflow2 finally gets to suggest narrowing: {note}"
    );
}

#[test]
fn the_prose_is_dropped_from_every_row_or_none() {
    let g = design_with_gaps(60);
    let report = g.detect_gaps_within(20_000).unwrap();
    let with_prose = report
        .items
        .iter()
        .filter(|r| !r.gap.description.is_empty())
        .count();
    assert!(
        with_prose == 0 || with_prose == report.items.len(),
        "{with_prose} of {} rows kept their prose — a half-explained list teaches the reader \
         that the unexplained half matters less, which is a judgement nothing here made",
        report.items.len()
    );
}

#[test]
fn a_row_that_lists_no_affected_ids_still_says_how_many_there_are() {
    // One rollup gap over many nodes: 30 proposed Decisions related to nothing.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "A project").unwrap();
    for i in 0..30 {
        g.create_node(
            node::DECISION,
            &format!("dec:idea-{i}"),
            Props::new()
                .set("name", format!("IDEA — number {i}"))
                .set("decision", "An idea recorded as an idea.")
                .set("status", "proposed"),
        )
        .unwrap();
    }

    let report = g.detect_gaps_within(30_000).unwrap();
    let rollup = report
        .items
        .iter()
        .find(|r| r.affected_total >= 30)
        .expect("the rollup gap over the unreviewed ideas");

    assert_eq!(report.budget.detail, ReplyDetail::Full);
    assert_eq!(
        rollup.gap.affected_ids.len(),
        AFFECTED_CAP,
        "even a full reply caps the id list — a gap enumerating every node it touches is not \
         communicating, it is padding"
    );
    assert_eq!(
        rollup.affected_withheld,
        Some(rollup.affected_total - AFFECTED_CAP)
    );
    assert!(
        report.budget.note.is_some(),
        "ids were withheld, so the reply says so even in the full tier"
    );
}

#[test]
fn every_row_says_how_many_nodes_it_touches_whether_or_not_it_was_cut() {
    let g = design_with_gaps(5);
    let report = g.detect_gaps_within(30_000).unwrap();
    for row in &report.items {
        assert_eq!(
            row.affected_total,
            row.gap.affected_ids.len(),
            "nothing was cut here, so the figure and the list must agree"
        );
        assert!(
            row.affected_withheld.is_none(),
            "a `withheld` that appeared when nothing was withheld would make its absence \
             meaningless"
        );
    }
}

#[test]
fn a_list_too_long_for_even_its_titles_says_how_many_are_missing() {
    let g = design_with_gaps(60);
    let every_gap = g.detect_gaps().unwrap().len();

    // Far below what the titles alone cost: the last resort, where rows are
    // genuinely absent from the reply.
    let report = g.detect_gaps_within(2_000).unwrap();

    assert!(
        report.items.len() < every_gap,
        "this budget cannot hold every title; the test is pointless if it does"
    );
    assert_eq!(
        report.count, every_gap,
        "the count still covers all of them"
    );
    assert_eq!(report.budget.listed, report.items.len());
    assert_eq!(report.budget.of, every_gap);
    assert_eq!(
        report.by_source.values().sum::<usize>(),
        every_gap,
        "the one tier where findings are absent from the list is the tier where the totals \
         matter most"
    );

    let note = report.budget.note.expect("a dropped tail must be stated");
    assert!(note.contains("THE LIST ITSELF IS SHORT"), "{note}");
    assert!(
        note.contains(&format!("{} of {every_gap}", report.items.len())),
        "the note must name both numbers: {note}"
    );
}

#[test]
fn the_worst_gaps_are_the_ones_that_survive_a_dropped_tail() {
    let g = design_with_gaps(60);
    let report = g.detect_gaps_within(2_000).unwrap();
    let severities: Vec<f64> = report.items.iter().map(|r| r.gap.severity).collect();
    assert!(
        severities.windows(2).all(|w| w[0] >= w[1]),
        "a truncated list that dropped an arbitrary tail would be worse than no list: {severities:?}"
    );
}

#[test]
fn a_compacted_row_can_be_handed_straight_back() {
    // `gap_to_prompt` takes a gap object the agent read out of `detect_gaps`.
    // A row from the titles-only tier has to survive that round trip, or the
    // ask half of the loop breaks the moment a design gets big.
    let g = design_with_gaps(60);
    let report = g.detect_gaps_within(20_000).unwrap();
    let row = report.items.first().expect("at least one gap");

    let json = serde_json::to_value(row).unwrap();
    let back: GapCandidate =
        serde_json::from_value(json).expect("a compacted row must deserialize as a GapCandidate");
    assert_eq!(back.id, row.gap.id);
    assert!(back.description.is_empty());
}

#[test]
fn a_scoped_answer_is_budgeted_too() {
    // Scoping is what an unscoped reader is TOLD to do when its answer will not
    // fit. A scoped answer that will not fit either would make that advice a
    // dead end.
    let mut g = design_with_gaps(60);
    for i in 0..60 {
        g.contains("proj:p", node::REQUIREMENT, &format!("req:{i}"))
            .unwrap();
    }

    let scoped = g.detect_gaps_in_scope_within("proj:p", 2, 20_000).unwrap();
    let budget = scoped
        .budget
        .expect("a scoped reply reports its own size too");
    assert_eq!(budget.detail, ReplyDetail::TitlesOnly);
    let note = budget
        .note
        .expect("withholding must be stated here as well");
    assert!(
        note.contains("already scoped"),
        "telling a reader who scoped to scope is advice they cannot follow: {note}"
    );
    assert_eq!(
        scoped.in_scope,
        scoped.items.len(),
        "nothing was dropped from the list at this budget, so the two figures agree"
    );
}

#[test]
fn a_budget_smaller_than_the_empty_reply_is_not_a_panic() {
    // An empty list still serializes to `[]`, so a budget of zero arrives at the
    // last-resort truncation with nothing to truncate. The arithmetic there
    // subtracts what it kept from what it had, and on an empty list that
    // underflows.
    let g = DesignGraph::open_in_memory().unwrap();
    let report = g.detect_gaps_within(0).unwrap();
    assert_eq!(report.count, 0);
    assert!(report.items.is_empty());
}

#[test]
fn a_budgeted_row_replays_into_the_handshake_it_was_meant_for() {
    // THE CONTRACT THE BUDGET WAS BREAKING, reported from the field
    // (hxm_program, 2026-08-26). `gap_to_prompt`'s docs say "REPLAY EACH GAP
    // OBJECT UNCHANGED", and `GapCandidate::description` promises in its own
    // comment that "a compact row handed straight back to `gap_to_prompt` has to
    // deserialize". It could not: `suggested_depth` carried no serde default, so
    // a budgeted reply — which is the ONLY kind a mature design gets — produced
    // rows that were refused with `missing field 'suggested_depth'`.
    //
    // The two mechanisms were fighting: the budget exists so the call can be
    // made at all on a big design, and the handshake exists so the gaps can be
    // asked. On exactly the designs where both matter, they cancelled.
    //
    // This pins the ROUND TRIP rather than the field, because the field is only
    // today's instance: any future field added without a default breaks the same
    // contract, and this test fails when it does.
    let g = design_with_gaps(60);
    let report = g.detect_gaps_within(20_000).unwrap();
    assert_eq!(
        report.budget.detail,
        ReplyDetail::TitlesOnly,
        "60 gaps in 20k budgets down to titles — otherwise this test \
         is not exercising the case it exists for"
    );

    for row in &report.items {
        let wire = serde_json::to_string(&row.gap).expect("a row serializes");
        let back: GapCandidate = serde_json::from_str(&wire)
            .expect("a budgeted row must deserialize — this is the replay contract");
        assert_eq!(back.id, row.gap.id);
        assert!(
            (1..=5).contains(&back.suggested_depth),
            "a replayed row asks for a real depth, got {}",
            back.suggested_depth
        );
    }

    // AND THE HARDER HALF: a row whose optional fields are ABSENT from the JSON
    // entirely, not merely empty — which is what `skip_serializing_if` produces
    // and what a caller reconstructing a row by hand will send.
    let minimal = serde_json::json!({
        "id": report.items[0].gap.id,
        "gap_source": report.items[0].gap.gap_source,
        "scope": report.items[0].gap.scope,
        "severity": report.items[0].gap.severity,
        "title": report.items[0].gap.title,
    });
    let back: GapCandidate = serde_json::from_value(minimal)
        .expect("id, kind, scope, severity and title are enough to replay a gap");
    assert_eq!(
        back.suggested_depth, 2,
        "the default is the ordinary depth, not an invented one"
    );
    assert!(back.affected_ids.is_empty());
}
