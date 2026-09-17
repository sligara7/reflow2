//! `unit_system_undeclared`, `quantity_without_unit` and
//! `unit_outside_declared_system` — the governance half of the units answer
//! (`req:a-project-declares-its-unit-system-as-a-governed-rule-and-undeclared-or-off-system-units-are-reported`).
//!
//! The Mars Climate Orbiter review board found the systems-engineering
//! function "not robust enough" to catch a unit mismatch. This is that review
//! made mechanical: the project declares, per quantity kind, the unit its
//! design is done in — as a DesignRule, so the owner is asked whether breaking
//! it stops the build — and a sweep over every unit-bearing field reports a
//! quantity with no unit and a unit outside the system.
//!
//! The case that keeps this honest is the first: a design with no quantities
//! at all raises NOTHING, because there is nothing to check, and the finding
//! must not read "clean" over an empty population.

use reflow2_core::{DesignGraph, GapCandidate, GapSource};

fn g() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:pod", "bhome pod").unwrap();
    g.add_component("cmp:tank", "Fish tank", "holds the fish", None)
        .unwrap();
    g
}

fn find(g: &DesignGraph, source: GapSource) -> Option<GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == source)
}

fn budget(g: &mut DesignGraph, unit: Option<&str>) {
    g.add_constraint(
        "con:floor",
        "Interior floor area",
        "Interior floor area stays under 280.",
        Some("budget"),
        Some("floor_area"),
        Some(280.0),
        None,
        Some("maximum"),
    )
    .unwrap();
    if let Some(u) = unit {
        g.set_constraint_unit("con:floor", u).unwrap();
    }
}

fn declare_system(g: &mut DesignGraph, units: &[&str]) {
    g.add_design_rule(
        "rule:units",
        "Design units",
        "This design is done in the units listed, per quantity kind.",
        Some("unit_system"),
        Some(false),
    )
    .unwrap();
    let owned: Vec<String> = units.iter().map(|s| s.to_string()).collect();
    g.set_design_rule_units("rule:units", &owned).unwrap();
}

#[test]
fn a_design_with_no_quantities_raises_nothing_rather_than_reading_clean() {
    let g = g();
    for s in [
        GapSource::UnitSystemUndeclared,
        GapSource::QuantityWithoutUnit,
        GapSource::UnitOutsideDeclaredSystem,
    ] {
        assert!(find(&g, s).is_none(), "{s:?} fired with nothing to run on");
    }
}

#[test]
fn a_budget_with_a_limit_and_no_unit_is_named() {
    let mut g = g();
    budget(&mut g, None);
    let gap = find(&g, GapSource::QuantityWithoutUnit).expect("a number with no unit");
    assert!(gap.affected_ids.contains(&"con:floor".to_string()));
    assert!(gap.title.contains("no unit"), "got: {}", gap.title);
}

#[test]
fn a_contribution_with_a_number_and_no_unit_is_named_too() {
    let mut g = g();
    budget(&mut g, Some("sqft"));
    g.constrains(
        "con:floor",
        "Component",
        "cmp:tank",
        Some(44.0),
        None,
        None,
        None,
    )
    .unwrap();
    let gap = find(&g, GapSource::QuantityWithoutUnit).expect("the edge said 44 of what?");
    assert!(
        gap.affected_ids.contains(&"cmp:tank".to_string()),
        "{:?}",
        gap.affected_ids
    );
    assert!(!gap.affected_ids.contains(&"con:floor".to_string()) || gap.affected_ids.len() > 1);
}

#[test]
fn quantities_exist_and_no_unit_system_is_declared_is_asked_once() {
    let mut g = g();
    budget(&mut g, Some("sqft"));
    let gap = find(&g, GapSource::UnitSystemUndeclared).expect("the design carries a quantity");
    assert!(
        gap.affected_ids.is_empty(),
        "one question about the project"
    );
    assert!(
        find(&g, GapSource::UnitOutsideDeclaredSystem).is_none(),
        "nothing to be outside of yet"
    );
    declare_system(&mut g, &["area=sqft", "mass=lb"]);
    assert!(
        find(&g, GapSource::UnitSystemUndeclared).is_none(),
        "declared"
    );
}

#[test]
fn a_unit_outside_the_declared_system_is_named_and_an_enforced_rule_makes_it_louder() {
    let mut g = g();
    budget(&mut g, Some("sqft"));
    declare_system(&mut g, &["area=m2", "mass=kg"]);
    let gap = find(&g, GapSource::UnitOutsideDeclaredSystem).expect("sqft is not m2");
    assert!(gap.affected_ids.contains(&"con:floor".to_string()));
    assert!(gap.title.contains("sqft"), "got: {}", gap.title);
    let advisory = gap.severity;

    g.add_design_rule(
        "rule:units",
        "Design units",
        "This design is done in the units listed, per quantity kind.",
        Some("unit_system"),
        Some(true),
    )
    .unwrap();
    let louder = find(&g, GapSource::UnitOutsideDeclaredSystem).unwrap();
    assert!(
        louder.severity > advisory,
        "an enforced rule broken is a gate finding: {} vs {}",
        louder.severity,
        advisory
    );
}

#[test]
fn a_units_declaration_on_a_boundary_is_swept_as_well() {
    let mut g = g();
    declare_system(&mut g, &["impulse=N·s"]);
    g.add_interface("ifc:thrust", "Thruster file").unwrap();
    g.set_interface_units("ifc:thrust", &["impulse=lbf·s".to_string()])
        .unwrap();
    let gap = find(&g, GapSource::UnitOutsideDeclaredSystem).expect("lbf·s is outside");
    assert!(gap.affected_ids.contains(&"ifc:thrust".to_string()));
    g.set_interface_units("ifc:thrust", &["impulse=N·s".to_string()])
        .unwrap();
    assert!(find(&g, GapSource::UnitOutsideDeclaredSystem).is_none());
}

#[test]
fn everything_in_the_system_raises_nothing() {
    let mut g = g();
    budget(&mut g, Some("sqft"));
    declare_system(&mut g, &["area=sqft"]);
    g.constrains_in(
        "con:floor",
        "Component",
        "cmp:tank",
        Some(44.0),
        Some("sqft"),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    for s in [
        GapSource::UnitSystemUndeclared,
        GapSource::QuantityWithoutUnit,
        GapSource::UnitOutsideDeclaredSystem,
    ] {
        assert!(
            find(&g, s).is_none(),
            "{s:?} fired on a design that answered"
        );
    }
}
