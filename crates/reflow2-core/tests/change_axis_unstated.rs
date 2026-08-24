//! A design that records changes and never says what KIND of change they were
//! gets asked about it.
//!
//! This is the third leg. A typed tool writes `ChangeEvent.subject` and a
//! served skill says when to use it — and with nothing noticing its ABSENCE,
//! the loop that exists to surface gaps never asks, so the field stays empty
//! in every project that adopts reflow2, forever.
//! `fact:vocabulary-needs-three-legs-and-a-users-project-gets-none-of-it`
//! measured that shape across three cases and named the general form:
//! reflow2's detectors largely check the CONSISTENCY OF WHAT EXISTS, and
//! nothing asks "you have never used this vocabulary at all".
//!
//! ⭐ THE CASE THAT MUST FIRE IS THE ONE AT ZERO USAGE, and it is the case a
//! consistency check structurally cannot see. `decomposition_coverage` is the
//! cautionary sibling: it keys on a Requirement that ALREADY CARRIES a
//! `DECOMPOSES` edge, so a project that never decomposed anything gets zero
//! findings and reads as coherent. All three legs look present and the
//! vocabulary still stays empty. Hence `a_design_that_never_states_the_axis_is_
//! the_loudest_case` below, which is the whole reason this detector exists.

use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;
use reflow2_core::{ChangeSubject, ChangeType};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open")
}

fn event(g: &mut DesignGraph, id: &str, subject: Option<ChangeSubject>) {
    g.add_change_event(
        id,
        "a change",
        ChangeType::Resync,
        subject,
        None,
        None,
        None,
    )
    .expect("event");
}

fn axis_gap(g: &DesignGraph) -> Option<reflow2_core::detect::GapCandidate> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .find(|x| x.gap_source == GapSource::ChangeAxisUnstated)
}

/// The case the detector exists for: changes recorded, the vocabulary never
/// touched. A consistency check reads this as clean.
#[test]
fn a_design_that_never_states_the_axis_is_the_loudest_case() {
    let mut g = graph();
    for i in 0..3 {
        event(&mut g, &format!("chg:{i}"), None);
    }

    let gap = axis_gap(&g).expect(
        "a design with changes and no axis on any of them must be ASKED — this is exactly the \
         case a consistency-shaped check cannot see, and the reason this detector is written as \
         an absence check",
    );
    assert_eq!(gap.affected_ids.len(), 3);
    assert!(
        gap.title.contains("3 of 3"),
        "the finding must carry its denominator: a numerator alone says almost nothing about \
         whether this design is drifting or has simply just started. Got: {}",
        gap.title
    );
}

/// The complement, so the detector cannot pass by always firing.
#[test]
fn a_design_that_states_every_axis_is_not_asked() {
    let mut g = graph();
    event(&mut g, "chg:a", Some(ChangeSubject::System));
    event(&mut g, "chg:b", Some(ChangeSubject::Record));

    assert!(
        axis_gap(&g).is_none(),
        "a design that answered the question must not keep being asked it"
    );
}

/// A design with no changes at all has not failed to state anything.
#[test]
fn a_design_with_no_changes_is_silent() {
    let g = graph();
    assert!(
        axis_gap(&g).is_none(),
        "nothing recorded means nothing unstated — firing here would nag every empty project"
    );
}

/// Partial adoption reports the fraction rather than collapsing to pass/fail.
#[test]
fn a_partly_stated_design_reports_only_the_unstated_ones() {
    let mut g = graph();
    event(&mut g, "chg:stated", Some(ChangeSubject::System));
    event(&mut g, "chg:bare", None);

    let gap = axis_gap(&g).expect("one event still has no axis");
    assert_eq!(gap.affected_ids, vec!["chg:bare".to_string()]);
    assert!(
        gap.title.contains("1 of 2"),
        "the population is every change, not just the unstated ones: {}",
        gap.title
    );
}

/// ⭐ THE AGGREGATE PROPERTY, and the reason this is not per-event. ChangeEvents
/// are the fastest-growing node type in any active design, so a per-event key
/// would expire the user's standing judgement on every single write — the trap
/// `unvalidated_capability` fell into and was re-acknowledged about twenty
/// times for. One acknowledgement must survive the design continuing to move.
#[test]
fn the_gap_id_survives_the_population_growing() {
    let mut g = graph();
    event(&mut g, "chg:1", None);
    let first = axis_gap(&g).expect("gap").id;

    event(&mut g, "chg:2", None);
    let second = axis_gap(&g).expect("gap").id;

    assert_eq!(
        first, second,
        "an aggregate gap keys on its RULE, not its population — otherwise accepting \"we do not \
         track this distinction\" is undone by the next change anybody records"
    );
}

/// ⭐ WHY THE DETECTOR ONLY CHECKS PRESENCE. A blank axis is not a weaker
/// answer than a missing one — it cannot exist at all. `subject` is a schema
/// enum and every write path goes through `create_node`, `import_graph`
/// included, so the store refuses it. This test is what lets the detector stay
/// a plain presence check instead of carrying an emptiness guard that could
/// never fire: if the schema ever loosens, this goes red and the guard comes
/// back with it.
#[test]
fn a_blank_axis_cannot_be_written_at_all() {
    let mut g = graph();
    let err = g
        .create_node(
            reflow2_core::nodes::node::CHANGE_EVENT,
            "chg:blank",
            reflow2_core::nodes::Props::new()
                .set("name", "a change")
                .set("change_type", "resync")
                .set("subject", ""),
        )
        .expect_err("a blank subject must be refused at the schema, not stored as a third value");

    let text = format!("{err:?}");
    assert!(
        text.contains("subject") && text.contains("system") && text.contains("record"),
        "the refusal must name the field and the two legal values: {text}"
    );
}
