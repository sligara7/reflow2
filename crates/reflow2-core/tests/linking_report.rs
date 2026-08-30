//! Is the linking discipline being followed? — the question the design could
//! not ask about itself.
//!
//! # Why this exists
//!
//! `unreviewed_ideas` computes the ideas nobody has opened, and feeds a
//! detector the owner ACCEPTED on 2026-08-23: an unlinked idea legitimately
//! governs nothing yet, so it is not a defect the loop should press about. That
//! judgement is right and stands. Its side effect was not: the number was
//! computed, wired to something deliberately silent, and reachable from nowhere
//! else.
//!
//! Measured 2026-08-30, that silence hid a real shape — 140 of 207 ideas
//! carried a relation and `no_relation_note` had been used TWICE. So for the 67
//! unlinked ideas, "nobody looked" and "looked and found nothing" could not be
//! told apart, which is the exact distinction the note exists to preserve.
//!
//! This reports and never presses. It raises no gap and moves no detector. What
//! it removes is the inability to see.
//!
//! # The case to break first
//!
//! `a_decision_with_no_kind_is_counted_apart`. The 207 ideas already in the
//! graph carry no `kind`, deliberately — their only evidence of being ideas is
//! an id prefix that was retired in #390, and reading it here would launder it
//! back into a report. If that test ever passes by counting them as ideas, the
//! report has started asserting exactly what the design refused to.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, node};

fn g() -> DesignGraph {
    DesignGraph::open_in_memory().unwrap()
}

fn idea(g: &mut DesignGraph, id: &str, kind: Option<&str>) {
    g.add_decision(id, id, "some text", None).unwrap();
    if let Some(k) = kind {
        g.upsert_node(node::DECISION, id, Props::new().set("kind", k))
            .unwrap();
    }
}

#[test]
fn an_empty_design_reports_nothing_rather_than_a_clean_bill() {
    let r = g().linking_report().unwrap();
    assert_eq!(r.ideas, 0);
    assert_eq!(r.silent, 0);
    assert!(
        !r.not_observed_about.is_empty(),
        "a zero must still carry what it could not see, or it reads as a pass"
    );
}

#[test]
fn a_decision_with_no_kind_is_counted_apart() {
    let mut d = g();
    idea(&mut d, "dec:idea-legacy", None);
    let r = d.linking_report().unwrap();
    assert_eq!(
        r.ideas, 0,
        "an unclassified Decision is NOT an idea, whatever its id says — reading the prefix here \
         would launder back the defect #390 closed"
    );
    assert_eq!(r.kind_unstated, 1, "it is counted apart, not ignored");
    assert_eq!(r.silent, 0);
}

#[test]
fn the_three_states_are_told_apart() {
    let mut d = g();
    idea(&mut d, "dec:linked", Some("exploratory"));
    idea(&mut d, "dec:other", Some("exploratory"));
    idea(&mut d, "dec:noted", Some("exploratory"));
    idea(&mut d, "dec:silent", Some("exploratory"));

    d.review_relations(
        node::DECISION,
        "dec:linked",
        &[reflow2_core::relate::RelationLink {
            relation: "DEPENDS_ON".into(),
            other_type: "Decision".into(),
            other_id: "dec:other".into(),
            evidence: "the first is only worth anything if the second lands".into(),
            incoming: false,
        }],
        None,
    )
    .unwrap();
    d.review_relations(
        node::DECISION,
        "dec:noted",
        &[],
        Some("read dec:other; different subject entirely"),
    )
    .unwrap();

    let r = d.linking_report().unwrap();
    assert_eq!(r.ideas, 4);
    assert!(r.linked >= 1, "dec:linked carries a relation");
    assert_eq!(r.noted, 1, "dec:noted carries the honest-nothing answer");
    assert!(
        r.silent_ids.contains(&"dec:silent".to_string()),
        "the silent ones are NAMED, never just counted: {:?}",
        r.silent_ids
    );
    assert!(
        !r.silent_ids.contains(&"dec:noted".to_string()),
        "a note is a FULL answer — treating it as silence would erase the whole distinction"
    );
}

/// A `choice` carries no linking discipline, so it is counted but never
/// measured against one.
#[test]
fn a_choice_is_not_measured_against_a_discipline_it_does_not_carry() {
    let mut d = g();
    idea(&mut d, "dec:a-choice", Some("choice"));
    let r = d.linking_report().unwrap();
    assert_eq!(r.choice, 1);
    assert_eq!(r.ideas, 0);
    assert_eq!(r.silent, 0);
}
