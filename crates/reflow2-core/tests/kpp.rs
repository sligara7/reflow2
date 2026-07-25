//! Key performance parameters — inviolable intent, checked rather than remembered.
//!
//! Anthony's acquisitions vocabulary (BL-96). A KPP is a threshold that, if
//! missed, fails the effort regardless of how well everything else went —
//! distinct from a Requirement (a goal you can trade) and from an ordinary
//! Constraint (a limit imposed on you). Modelled as a Constraint with
//! `category: kpp` on his call, because the quantity/limit/direction triple and
//! the budget rollup are exactly what it needs and already existed, unused.
//!
//! The point is the COMPUTATION. His own line: "A KPP that nothing checks is a
//! comment" — and a comment is what gets traded away in the tenth iteration
//! cycle by someone who never read it. So these tests are about violations
//! being *found*, not about the noun being storable.

use reflow2_core::detect::GapSource;
use reflow2_core::nodes::{Props, node};
use reflow2_core::{DesignGraph, Value};

/// A car that must do 500 miles on a tank, with two mass contributors.
fn car() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:car", "Car").unwrap();
    g.add_component("cmp:body", "Body", "the shell", None)
        .unwrap();
    g.add_component("cmp:engine", "Engine", "the motor", None)
        .unwrap();
    g.create_node(
        node::CONSTRAINT,
        "kpp:mass",
        Props::new()
            .set("name", "Curb mass under 3000 lb")
            .set("statement", "The car must weigh under 3000 pounds.")
            .set("category", "kpp")
            .set("quantity", "mass_lb")
            .set("limit", 3000.0)
            .set("direction", "maximum")
            .build(),
    )
    .unwrap();
    g
}

fn constrain(g: &mut DesignGraph, kpp: &str, target: &str, contribution: f64) {
    g.create_edge(
        "CONSTRAINS",
        node::CONSTRAINT,
        kpp,
        node::COMPONENT,
        target,
        Props::new().set("contribution", contribution).build(),
    )
    .unwrap();
}

fn sources(g: &DesignGraph) -> Vec<GapSource> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .map(|x| x.gap_source)
        .collect()
}

#[test]
fn a_kpp_that_binds_nothing_is_reported_as_unbound() {
    // The quietest failure: a parameter asserting something vital, permanently
    // green because it touches nothing that could ever violate it.
    let g = car();
    let gaps = g.detect_gaps().unwrap();
    let unbound = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::KppUnbound)
        .expect("an unbound KPP must be reported");
    assert!(unbound.severity > 0.8, "it must outrank ordinary gaps");
    assert!(unbound.affected_ids.contains(&"kpp:mass".to_string()));
}

#[test]
fn an_unbound_kpp_is_reported_once_not_three_times() {
    // It cannot be breached or contradicted either — both need something bound
    // to reason about. Counting one fault three ways would teach people the
    // KPP findings are noise.
    let g = car();
    let s = sources(&g);
    assert_eq!(s.iter().filter(|x| **x == GapSource::KppUnbound).count(), 1);
    assert!(!s.contains(&GapSource::KppBreached));
    assert!(!s.contains(&GapSource::KppContradicted));
}

#[test]
fn a_kpp_within_its_threshold_says_nothing() {
    let mut g = car();
    constrain(&mut g, "kpp:mass", "cmp:body", 1800.0);
    constrain(&mut g, "kpp:mass", "cmp:engine", 900.0);
    let s = sources(&g);
    assert!(
        !s.contains(&GapSource::KppUnbound) && !s.contains(&GapSource::KppBreached),
        "2700 of 3000 is fine and must stay quiet: {s:?}"
    );
}

#[test]
fn crossing_the_threshold_is_reported_as_a_breach() {
    // THE test. This is the one arithmetic violation, and the whole reason a
    // KPP is worth distinguishing from a requirement nobody adds up.
    let mut g = car();
    constrain(&mut g, "kpp:mass", "cmp:body", 1800.0);
    constrain(&mut g, "kpp:mass", "cmp:engine", 1400.0);

    let gaps = g.detect_gaps().unwrap();
    let breach = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::KppBreached)
        .expect("3200 lb against a 3000 lb threshold must be reported");
    assert!(
        breach.severity >= 0.9,
        "a breached KPP is not a thinness in the design, it is the design failing"
    );
    assert!(
        breach.evidence.contains("3200"),
        "the number must be in the evidence, not just a verdict: {}",
        breach.evidence
    );
}

#[test]
fn an_unstated_contribution_does_not_manufacture_a_breach() {
    // budget_report returns Incomplete when a contribution is missing, and an
    // unknown total must never be read as a breach — asserting failure from
    // absent data is the silent-guess this project forbids.
    let mut g = car();
    constrain(&mut g, "kpp:mass", "cmp:body", 1800.0);
    g.create_edge(
        "CONSTRAINS",
        node::CONSTRAINT,
        "kpp:mass",
        node::COMPONENT,
        "cmp:engine",
        Props::new().build(), // no contribution stated
    )
    .unwrap();

    assert!(
        !sources(&g).contains(&GapSource::KppBreached),
        "an incomplete rollup is not a breach"
    );
}

#[test]
fn an_accepted_decision_over_what_a_kpp_binds_is_surfaced_for_review() {
    let mut g = car();
    constrain(&mut g, "kpp:mass", "cmp:body", 1800.0);
    g.add_decision(
        "dec:steel",
        "Steel body panels",
        "Use steel rather than aluminium.",
        Some("Cheaper tooling."),
    )
    .unwrap();
    g.governed_by(node::COMPONENT, "cmp:body", node::DECISION, "dec:steel")
        .unwrap();

    let gaps = g.detect_gaps().unwrap();
    let flagged = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::KppContradicted)
        .expect("a decision shaping what a KPP binds must be surfaced");
    assert!(flagged.affected_ids.contains(&"dec:steel".to_string()));
    assert!(
        flagged
            .description
            .contains("not a claim that it is broken"),
        "it must read as a prompt to check, never as a verdict: {}",
        flagged.description
    );
}

#[test]
fn a_proposed_decision_is_not_flagged() {
    // An open choice has traded nothing away yet. Flagging it would punish
    // thinking out loud, and the decision-point machinery exists precisely so
    // people record choices before making them.
    let mut g = car();
    constrain(&mut g, "kpp:mass", "cmp:body", 1800.0);
    g.add_decision("dec:maybe", "Maybe aluminium", "Undecided.", None)
        .unwrap();
    g.set_decision_status("dec:maybe", "proposed").unwrap();
    g.governed_by(node::COMPONENT, "cmp:body", node::DECISION, "dec:maybe")
        .unwrap();

    assert!(!sources(&g).contains(&GapSource::KppContradicted));
}

#[test]
fn an_ordinary_constraint_is_not_a_kpp() {
    // The distinction has to be real or the category is decoration: a normal
    // constraint over budget gets the existing treatment, not the KPP one.
    let mut g = car();
    g.create_node(
        node::CONSTRAINT,
        "con:cost",
        Props::new()
            .set("name", "Tooling under $5k")
            .set("statement", "Tooling must cost under $5,000.")
            .set("category", "budget")
            .set("quantity", "cost_usd")
            .set("limit", 5000.0)
            .build(),
    )
    .unwrap();
    constrain(&mut g, "con:cost", "cmp:body", 9000.0);

    let s = sources(&g);
    assert!(
        !s.contains(&GapSource::KppBreached),
        "an over-budget ordinary constraint is not a KPP breach: {s:?}"
    );
}

#[test]
fn the_objective_is_optional_and_never_invented() {
    // Many KPPs state only a threshold. Inventing an objective would be a
    // number the graph asserts on its own, which is the one thing intent
    // vocabulary must never do.
    let g = car();
    let k = g.get_node(node::CONSTRAINT, "kpp:mass").unwrap().unwrap();
    assert!(k.properties.contains_key("limit"));
    assert!(
        !k.properties.contains_key("objective"),
        "unset means unset — no default objective appears from nowhere"
    );
}

// ── The capture half (cap:kpp-proposal) ────────────────────────────────────
//
// The violation half above builds its KPPs with `create_node`, because when it
// was written that was the only way: `add_constraint` had no `objective` and
// its own description never mentioned the category, so the typed helper could
// not record what the user had just confirmed. A capture skill that told an
// agent to record a KPP would have been pointing at a door that was not there.

#[test]
fn a_confirmed_kpp_can_be_recorded_through_the_typed_helper() {
    // The acquisition pair, both halves, in one call: threshold in `limit`,
    // objective beside it — and the detector must read the result as a KPP,
    // not merely store the words.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:car", "Car").unwrap();
    let k = g
        .add_constraint(
            "kpp:range",
            "500 miles on a tank",
            "The car must travel at least 500 miles on one tank.",
            Some("kpp"),
            Some("range_mi"),
            Some(500.0),
            Some(600.0),
            Some("minimum"),
        )
        .unwrap();

    assert_eq!(k.properties["category"], Value::from("kpp"));
    assert_eq!(k.properties["limit"], Value::from(500.0));
    assert_eq!(k.properties["objective"], Value::from(600.0));
    assert!(
        sources(&g).contains(&GapSource::KppUnbound),
        "recorded through the helper it must be a real KPP, not a stored noun"
    );
}

#[test]
fn the_typed_helper_never_defaults_the_objective() {
    // Same rule as the create_node path: many KPPs state only a threshold, and
    // asking the user for an objective they never set would produce a number
    // the design then asserts on their behalf.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:car", "Car").unwrap();
    let k = g
        .add_constraint(
            "kpp:mass",
            "Curb mass under 3000 lb",
            "The car must weigh under 3000 pounds.",
            Some("kpp"),
            Some("mass_lb"),
            Some(3000.0),
            None,
            Some("maximum"),
        )
        .unwrap();

    assert!(k.properties.contains_key("limit"));
    assert!(
        !k.properties.contains_key("objective"),
        "unset means unset, on the typed path too"
    );
}

#[test]
fn the_objective_is_never_mistaken_for_the_threshold() {
    // The slip this pins: reading `objective` as the limit would turn every
    // KPP that is merely short of excellent into a program failure. Missing
    // the objective is disappointing; missing the threshold is fatal, and only
    // the second one is a breach.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:car", "Car").unwrap();
    g.add_component("cmp:body", "Body", "the shell", None)
        .unwrap();
    g.add_constraint(
        "kpp:mass",
        "Curb mass under 3000 lb",
        "The car must weigh under 3000 pounds; 2500 would be excellent.",
        Some("kpp"),
        Some("mass_lb"),
        Some(3000.0),
        Some(2500.0),
        Some("maximum"),
    )
    .unwrap();
    constrain(&mut g, "kpp:mass", "cmp:body", 2700.0);

    assert!(
        !sources(&g).contains(&GapSource::KppBreached),
        "2700 lb misses the 2500 objective but meets the 3000 threshold"
    );
}
