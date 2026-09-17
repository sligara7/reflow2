//! `quantity_without_source` and `quantity_check_without_executable_form` —
//! every number that matters says how it was obtained and by what, and the
//! check that judges it has a form a tool can re-run
//! (`req:a-quantity-carries-how-it-was-obtained-and-an-unsourced-one-is-reported`).
//!
//! Anthony, 2026-09-16: "something that rules out that the AI agent just
//! didn't make up a measurement — in general, this is why I created reflow2".
//! No design tool can rule out a made-up number. What it can do is make a
//! number with no source look different from one with a source, count them,
//! and put the count in front of the person — the same discipline it applies
//! to a requirement nobody confirmed. The case that carries the weight is the
//! second: a number the agent proposed, SAID to be the agent's, is allowed.

use std::collections::HashMap;

use reflow2_core::foundation::core::Value;
use reflow2_core::{DesignGraph, GapCandidate, GapSource};

fn pod() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:pod", "bhome pod").unwrap();
    g.add_component("cmp:tank", "Fish tank", "holds the fish", None)
        .unwrap();
    g.add_constraint(
        "con:floor",
        "Interior floor area",
        "Interior floor area stays under 288.9.",
        Some("budget"),
        Some("floor_area"),
        Some(288.9),
        None,
        Some("maximum"),
    )
    .unwrap();
    g.set_constraint_unit("con:floor", "sqft").unwrap();
    g
}

fn find(g: &DesignGraph, s: GapSource) -> Option<GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == s)
}

#[test]
fn a_design_with_no_numeric_limit_raises_nothing() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    g.add_constraint(
        "con:no-combustion",
        "No combustion inside",
        "No combustion inside the envelope.",
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert!(find(&g, GapSource::QuantityWithoutSource).is_none());
    assert!(find(&g, GapSource::QuantityCheckWithoutExecutableForm).is_none());
}

#[test]
fn a_limit_nobody_sourced_is_named_and_the_agents_own_word_is_an_answer() {
    let mut g = pod();
    let gap = find(&g, GapSource::QuantityWithoutSource).expect("288.9 of what, from where?");
    assert!(gap.affected_ids.contains(&"con:floor".to_string()));
    assert!(gap.title.contains("nobody has sourced"), "{}", gap.title);

    // The IFC take-off measured it.
    g.set_constraint_provenance(
        "con:floor",
        Some("measured"),
        Some("art:ifc-shell-model"),
        Some("2026-09-16"),
    )
    .unwrap();
    assert!(find(&g, GapSource::QuantityWithoutSource).is_none());
    let c = g.get_node("Constraint", "con:floor").unwrap().unwrap();
    assert_eq!(
        c.properties.get("limit_source").and_then(Value::as_str),
        Some("art:ifc-shell-model")
    );
    assert_eq!(
        c.properties.get("unit").and_then(Value::as_str),
        Some("sqft"),
        "the other properties are carried"
    );

    // And a number the agent proposed, said to be the agent's, is allowed.
    let mut g2 = pod();
    g2.set_constraint_provenance("con:floor", Some("asserted"), Some("who:claude-code"), None)
        .unwrap();
    assert!(
        find(&g2, GapSource::QuantityWithoutSource).is_none(),
        "asserted by name reads as a proposal, not as a number nobody owns"
    );
}

#[test]
fn a_contribution_with_a_number_and_no_source_is_named_and_the_rollup_lists_it() {
    let mut g = pod();
    g.set_constraint_provenance(
        "con:floor",
        Some("measured"),
        Some("art:ifc-shell-model"),
        None,
    )
    .unwrap();
    g.constrains_in(
        "con:floor",
        "Component",
        "cmp:tank",
        Some(44.0),
        Some("sqft"),
        Some("estimated"),
        None,
        None,
        None,
    )
    .unwrap();
    let gap = find(&g, GapSource::QuantityWithoutSource).expect("44 sqft from where?");
    assert!(gap.affected_ids.contains(&"cmp:tank".to_string()));
    let r = g.budget_report("con:floor").unwrap();
    assert_eq!(r.unsourced, vec!["cmp:tank".to_string()]);
    assert_eq!(r.limit_source.as_deref(), Some("art:ifc-shell-model"));

    g.constrains_in(
        "con:floor",
        "Component",
        "cmp:tank",
        Some(44.0),
        Some("sqft"),
        Some("estimated"),
        Some("who:ajs"),
        None,
        None,
    )
    .unwrap();
    assert!(find(&g, GapSource::QuantityWithoutSource).is_none());
    assert!(g.budget_report("con:floor").unwrap().unsourced.is_empty());
}

#[test]
fn a_source_on_an_edge_with_no_number_is_not_stored() {
    let mut g = pod();
    let e = g
        .constrains_in(
            "con:floor",
            "Component",
            "cmp:tank",
            None,
            None,
            None,
            Some("who:ajs"),
            None,
            None,
        )
        .unwrap();
    assert!(
        !e.properties.contains_key("source"),
        "a source describes a number; with no number it describes nothing: {:?}",
        e.properties
    );
}

#[test]
fn a_check_on_a_limit_with_no_executable_form_is_named_until_the_file_that_is_the_check_is_linked()
{
    let mut g = pod();
    g.set_constraint_provenance(
        "con:floor",
        Some("measured"),
        Some("art:ifc-shell-model"),
        None,
    )
    .unwrap();
    g.add_verification(
        "ver:floor-area-take-off",
        "Floor area take-off",
        Some("measurement"),
        None,
        Some("ifc_quantify on the finish-floor slab."),
    )
    .unwrap();
    g.verifies("ver:floor-area-take-off", "Constraint", "con:floor")
        .unwrap();
    let gap = find(&g, GapSource::QuantityCheckWithoutExecutableForm)
        .expect("a check that exists only as a status is a claim");
    assert_eq!(
        gap.affected_ids,
        vec!["ver:floor-area-take-off".to_string()]
    );
    assert!(
        gap.description.contains("IMPLEMENTS"),
        "{}",
        gap.description
    );

    g.add_artifact(
        "art:take-off-script",
        "take_off.py",
        Some("code"),
        Some("tools/take_off.py"),
    )
    .unwrap();
    let props: HashMap<String, Value> = HashMap::new();
    g.create_edge(
        "IMPLEMENTS",
        "Artifact",
        "art:take-off-script",
        "Verification",
        "ver:floor-area-take-off",
        props,
    )
    .unwrap();
    assert!(
        find(&g, GapSource::QuantityCheckWithoutExecutableForm).is_none(),
        "the file IS the check now"
    );
}
