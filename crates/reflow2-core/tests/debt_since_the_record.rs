//! `req:the-loop-can-say-what-this-session-owes`.
//!
//! Triple-corroborated before it was built: Alex asked twice (his §3.3 on
//! 0.24.0 — "always dirty on mature graphs, agents ignore or thrash" — and
//! again in his 2026-08-14 Opportunities), dev_storyflow reached it
//! independently, and it was reproduced first-hand across a whole session where
//! `loop_status` reported the identical 80 gaps and 16 defects from the first
//! call to the last.
//!
//! ⚠️ THE BASELINE IS THE COMMITTED RECORD, NOT A CLOCK, and that was forced
//! rather than chosen: only 2 of this design's 2,367 nodes carry `created_at`,
//! so a time-based session boundary is not computable and never was.

use reflow2_core::DesignGraph;
use std::collections::BTreeSet;

fn baseline(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

/// A design whose requirement is satisfied by nothing — which is a gap, and one
/// anchored to a real node so it can be attributed to ground.
fn with_an_unsatisfied_requirement(id: &str) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_requirement(id, "A need", "must hold").unwrap();
    g
}

/// THE CASE: a node the record does not hold is counted and named.
#[test]
fn a_node_the_record_lacks_is_reported_as_new() {
    let g = with_an_unsatisfied_requirement("req:new");
    let d = g.debt_since(&baseline(&["proj:x"])).unwrap();

    assert!(d.unexported_nodes >= 1);
    assert!(
        d.unexported_sample.contains(&"req:new".to_string()),
        "the new node must be nameable, not just counted: {:?}",
        d.unexported_sample
    );
}

/// COUNTERWEIGHT: a design the record fully holds owes nothing NEW. If this
/// reported the whole design every time, the scoping would be decorative and
/// the mature-graph complaint would be unfixed.
#[test]
fn a_fully_recorded_design_reports_no_new_ground() {
    let g = with_an_unsatisfied_requirement("req:known");
    let mut all: BTreeSet<String> = BTreeSet::new();
    for t in ["Project", "Requirement"] {
        for n in g.scan_nodes(t).unwrap() {
            all.insert(n.node_id);
        }
    }

    let d = g.debt_since(&all).unwrap();
    assert_eq!(d.unexported_nodes, 0);
    assert!(
        d.gaps_on_new_ground.is_empty(),
        "{:?}",
        d.gaps_on_new_ground
    );
}

/// Debt on OLD ground is not attributed to the session. The requirement exists
/// in the record; its gap is real and design-wide, and reporting it here would
/// recreate the "everything is always dirty" problem one level down.
#[test]
fn debt_on_recorded_ground_is_not_called_new() {
    let g = with_an_unsatisfied_requirement("req:known");
    let d = g.debt_since(&baseline(&["proj:x", "req:known"])).unwrap();

    assert!(
        d.gaps_on_new_ground.is_empty(),
        "an old requirement's gap is design-wide, not this session's: {:?}",
        d.gaps_on_new_ground
    );
}

/// ⚠️ THE COUNTERWEIGHT THAT MATTERS MOST: an EMPTY baseline must not read as a
/// clean session. No readable record and a genuinely new design look identical
/// from here, so the answer says which it cannot tell — the same refusal
/// `loop_status` already makes for an unknown contributor.
#[test]
fn an_empty_baseline_says_it_cannot_tell() {
    let g = with_an_unsatisfied_requirement("req:new");
    let d = g.debt_since(&BTreeSet::new()).unwrap();

    assert!(d.unexported_nodes >= 2, "everything counts as new");
    assert!(
        d.note.contains("EVERY node counts as new"),
        "it must say the baseline was empty rather than implying a fresh session: {}",
        d.note
    );
}

/// Every answer says what it measured against, so a reader cannot mistake it
/// for a clock-based one — and is told the baseline moves.
#[test]
fn the_note_always_states_the_baseline_and_that_it_moves() {
    let g = with_an_unsatisfied_requirement("req:new");
    let d = g.debt_since(&baseline(&["proj:x"])).unwrap();

    assert!(d.note.contains("COMMITTED EXPORT"), "{}", d.note);
    assert!(d.note.contains("MOVES"), "{}", d.note);
    assert!(
        d.note.contains("design-wide"),
        "and that the rest of loop_status is NOT scoped: {}",
        d.note
    );
}
