//! `budget_report` used to sum bare floats and trust that each was typed in
//! the constraint's unit: a contribution of 6.9 (lb) against a limit of 100
//! (kg) rolled up silently. Now the Constraint states its `unit`, each
//! CONSTRAINS edge states the unit its `contribution` is in, and the rollup
//! ADDS ONLY WHAT MATCHES — the rest is reported, never totalled
//! (`req:a-project-declares-its-unit-system-as-a-governed-rule-and-undeclared-or-off-system-units-are-reported`,
//! option B of the Mars Climate Orbiter brainstorm, Anthony 2026-09-13).
//!
//! The honest claim, stated in the brainstorm and pinned here: absence and
//! disagreement are reported. A number typed in the wrong unit with the right
//! label looks identical to a correct one — which is precisely how MCO
//! happened — and nothing here can see that.

use reflow2_core::{BudgetVerdict, DesignGraph};

fn mass_budget() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:lander", "Lander").unwrap();
    g.add_constraint(
        "con:mass",
        "Dry mass",
        "Dry mass stays under 100.",
        Some("budget"),
        Some("mass"),
        Some(100.0),
        None,
        Some("maximum"),
    )
    .unwrap();
    g.set_constraint_unit("con:mass", "kg").unwrap();
    for id in ["cmp:bus", "cmp:tank", "cmp:legs"] {
        g.add_component(id, id, "a part", None).unwrap();
    }
    g
}

#[test]
fn a_contribution_in_another_unit_is_reported_and_left_out_of_the_total() {
    let mut g = mass_budget();
    g.constrains_in(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        Some("kg"),
        None,
        None,
        None,
    )
    .unwrap();
    g.constrains_in(
        "con:mass",
        "Component",
        "cmp:tank",
        Some(30.0),
        Some("kg"),
        None,
        None,
        None,
    )
    .unwrap();
    // 6.9 lb. Typed in good faith, in the unit the supplier's datasheet uses.
    g.constrains_in(
        "con:mass",
        "Component",
        "cmp:legs",
        Some(6.9),
        Some("lb"),
        None,
        None,
        None,
    )
    .unwrap();

    let r = g.budget_report("con:mass").unwrap();
    assert_eq!(r.unit.as_deref(), Some("kg"));
    assert_eq!(r.unit_mismatched, vec!["cmp:legs".to_string()]);
    assert!(
        (r.total - 70.0).abs() < 1e-9,
        "the pound figure is not added to a kilogram total: {}",
        r.total
    );
    assert_eq!(
        r.verdict,
        BudgetVerdict::Incomplete,
        "a number that could not be added leaves the verdict open, like an unstated one"
    );
}

#[test]
fn a_contribution_with_no_unit_is_reported_but_still_totalled() {
    // The line between this and the mismatch above: an unstated unit is a
    // silence, and the old behaviour (trust the number) is kept for it so that
    // every design written before units existed does not turn Incomplete
    // overnight. The silence is REPORTED, which is what was missing.
    let mut g = mass_budget();
    g.constrains_in(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        Some("kg"),
        None,
        None,
        None,
    )
    .unwrap();
    g.constrains(
        "con:mass",
        "Component",
        "cmp:tank",
        Some(30.0),
        None,
        None,
        None,
    )
    .unwrap();
    let r = g.budget_report("con:mass").unwrap();
    assert_eq!(r.unit_unstated, vec!["cmp:tank".to_string()]);
    assert!(r.unit_mismatched.is_empty());
    assert!((r.total - 70.0).abs() < 1e-9);
    assert_eq!(r.verdict, BudgetVerdict::Within);
}

#[test]
fn matching_units_add_up_and_nothing_is_reported() {
    let mut g = mass_budget();
    g.constrains_in(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        Some("kg"),
        None,
        None,
        None,
    )
    .unwrap();
    g.constrains_in(
        "con:mass",
        "Component",
        "cmp:tank",
        Some(30.0),
        Some("kg"),
        None,
        None,
        None,
    )
    .unwrap();
    let r = g.budget_report("con:mass").unwrap();
    assert!(r.unit_mismatched.is_empty() && r.unit_unstated.is_empty());
    assert_eq!(r.verdict, BudgetVerdict::Within);
}

#[test]
fn a_constraint_with_no_unit_cannot_judge_its_contributions_and_says_so() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    g.add_constraint(
        "con:lat",
        "Latency",
        "Under 200.",
        Some("budget"),
        Some("latency"),
        Some(200.0),
        None,
        Some("maximum"),
    )
    .unwrap();
    g.add_component("cmp:a", "a", "a part", None).unwrap();
    g.constrains_in(
        "con:lat",
        "Component",
        "cmp:a",
        Some(50.0),
        Some("ms"),
        None,
        None,
        None,
    )
    .unwrap();
    let r = g.budget_report("con:lat").unwrap();
    assert_eq!(r.unit, None);
    assert!(
        r.unit_mismatched.is_empty(),
        "with no unit on the constraint there is nothing to mismatch against"
    );
    assert!(
        r.unit_unstated.is_empty(),
        "the silence is the CONSTRAINT's, which the unit sweep reports; the edge said `ms`"
    );
}
