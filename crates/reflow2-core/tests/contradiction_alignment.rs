//! A supporting `CONTRADICTS` edge reinforces; it does not contradict.
//!
//! The schema has carried `alignment` from the start — "two decisions /
//! requirements / claims that conflict (**or, with alignment=supporting,
//! reinforce**)" — and the contradiction detector read the edge TYPE and never
//! the property. So every correctly-modelled corroboration came back as a
//! structural defect.
//!
//! Found in use on 2026-07-28, not by review: `dec:commands-are-the-exception`
//! QUALIFIES `dec:skills-served` — same reasoning, narrower scope — and
//! recording that relationship exactly as the schema prescribes turned the graph
//! red. The noise was not the damage. The damage was that the only ways out were
//! to acknowledge a defect that was not one, or to stop recording corroboration
//! at all — and a list that can never reach zero gets skimmed, which is the
//! failure the whole detect-and-ask discipline exists to prevent.

use std::collections::HashMap;

use dynograph_core::Value;
use reflow2_core::DesignGraph;

fn two_decisions() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_project("prj:p", "P").unwrap();
    g.add_decision(
        "dec:broad",
        "Serve everything",
        "Nothing is copied.",
        Some("Copies rot."),
    )
    .unwrap();
    g.add_decision(
        "dec:narrow",
        "Except the pointers",
        "Pointer files are copied.",
        Some("A pointer carries no version-coupled content, so a stale one is still correct."),
    )
    .unwrap();
    g
}

fn contradictions(g: &DesignGraph) -> Vec<String> {
    g.detect_defects()
        .expect("defects")
        .into_iter()
        .filter(|d| format!("{:?}", d.category) == "Contradiction")
        .map(|d| d.message)
        .collect()
}

fn join(g: &mut DesignGraph, alignment: Option<&str>) {
    let mut props: HashMap<String, Value> = HashMap::new();
    if let Some(a) = alignment {
        props.insert("alignment".into(), Value::from(a));
    }
    g.create_edge(
        "CONTRADICTS",
        "Decision",
        "dec:narrow",
        "Decision",
        "dec:broad",
        props,
    )
    .unwrap();
}

#[test]
fn a_supporting_edge_is_not_a_contradiction() {
    let mut g = two_decisions();
    join(&mut g, Some("supporting"));

    assert!(
        contradictions(&g).is_empty(),
        "alignment=supporting means REINFORCE — the schema says so, and a detector that \
         ignores the property makes the property a lie: {:?}",
        contradictions(&g)
    );
}

#[test]
fn an_opposing_edge_is_still_a_contradiction() {
    // The load-bearing counterweight. If the fix silenced the detector outright,
    // real conflicts would vanish — which is far worse than the noise it fixes.
    let mut g = two_decisions();
    join(&mut g, Some("opposing"));

    assert_eq!(
        contradictions(&g).len(),
        1,
        "an opposing edge must still be reported"
    );
}

#[test]
fn an_edge_that_says_nothing_is_still_a_contradiction() {
    // Silence is not a claim of support. `CONTRADICTS` with no alignment means
    // what it has always meant, so no existing design changes behaviour.
    let mut g = two_decisions();
    join(&mut g, None);

    assert_eq!(
        contradictions(&g).len(),
        1,
        "an unlabelled CONTRADICTS edge must keep its historical meaning"
    );
}
