//! The tenth agreement axis: the UNIT of each quantity a boundary carries,
//! compared across a published/required pair
//! (`req:a-quantity-that-crosses-a-published-boundary-declares-its-unit-and-the-seam-check-compares-them`).
//!
//! The shape is the Mars Climate Orbiter's exactly: thruster impulse crossed
//! the Lockheed Martin/JPL seam in pound-force seconds and was read as newton
//! seconds; the interface specification said SI; no review compared the two
//! sides. Anthony, 2026-09-13: how can reflow2 insert checks that a design is
//! done in proper units? This is the seam half of the answer.
//!
//! Honest limit, part of the requirement: reflow2 compares DECLARATIONS. It
//! cannot read the unit a program computes in; it puts the two sides'
//! declarations side by side, mechanically, every time.

use std::collections::HashMap;

use reflow2_core::Value;
use reflow2_core::{DesignGraph, GraphExport, Verdict};

fn iface(g: &mut DesignGraph, id: &str, name: &str, units: Option<&[&str]>) {
    let mut p: HashMap<String, Value> = HashMap::new();
    p.insert("name".into(), Value::from(name));
    g.create_node("Interface", id, p).unwrap();
    if let Some(u) = units {
        let owned: Vec<String> = u.iter().map(|s| s.to_string()).collect();
        g.set_interface_units(id, &owned).unwrap();
    }
}

fn provider(units: Option<&[&str]>) -> GraphExport {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:lmt", "Lockheed Martin ground software")
        .unwrap();
    iface(&mut g, "ifc:theirs", "Thruster performance file", units);
    g.set_interface_designation("ifc:theirs", "published")
        .unwrap();
    g.export_graph().unwrap()
}

fn consumer(units: Option<&[&str]>) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:jpl", "JPL navigation").unwrap();
    iface(
        &mut g,
        "ifc:ours",
        "Thruster performance file, as read",
        units,
    );
    g
}

fn pairs() -> Vec<(String, String)> {
    vec![("ifc:ours".to_string(), "ifc:theirs".to_string())]
}

#[test]
fn the_mars_climate_orbiter_seam_is_an_incompatibility() {
    let g = consumer(Some(&["impulse=N·s"]));
    let them = provider(Some(&["impulse=lbf·s"]));
    let r = g.seam_report(&them, &pairs()).unwrap();
    assert_eq!(r.incompatible.len(), 1, "{:?}", r.incompatible);
    let f = &r.incompatible[0];
    assert_eq!(f.verdict, Verdict::Incompatible);
    assert_eq!(f.our_value.as_deref(), Some("impulse=N·s"));
    assert_eq!(f.their_value.as_deref(), Some("impulse=lbf·s"));
    assert!(
        f.detail.contains("impulse") && f.detail.contains("Mars Climate Orbiter"),
        "the finding names the quantity and says why it matters: {}",
        f.detail
    );
}

#[test]
fn the_same_unit_on_both_sides_is_agreement() {
    let g = consumer(Some(&["impulse=N·s", "mass=kg"]));
    let them = provider(Some(&["mass=kg", "impulse=N·s"]));
    let r = g.seam_report(&them, &pairs()).unwrap();
    assert!(r.incompatible.is_empty(), "{:?}", r.incompatible);
    assert!(
        !r.unstated
            .iter()
            .any(|f| f.our_value.is_some() && f.their_value.is_some()),
        "both sides stated units, so the axis is not unstated"
    );
}

#[test]
fn a_silent_side_is_not_agreement() {
    let g = consumer(None);
    let them = provider(Some(&["impulse=lbf·s"]));
    let r = g.seam_report(&them, &pairs()).unwrap();
    assert!(r.incompatible.is_empty());
    assert!(
        r.unstated
            .iter()
            .any(|f| f.their_value.as_deref() == Some("impulse=lbf·s") && f.our_value.is_none()),
        "one side silent on units is reported as unstated, never as agreed: {:?}",
        r.unstated
    );
}

#[test]
fn a_quantity_only_one_side_names_is_unstated_not_agreed() {
    let g = consumer(Some(&["impulse=N·s"]));
    let them = provider(Some(&["impulse=N·s", "mass=kg"]));
    let r = g.seam_report(&them, &pairs()).unwrap();
    assert!(r.incompatible.is_empty());
    assert!(
        r.unstated.iter().any(|f| f.detail.contains("mass")),
        "a quantity the other side carries and we never named is a silence about that \
         quantity, not agreement: {:?}",
        r.unstated
    );
}

#[test]
fn a_boundary_that_carries_no_quantities_says_so_and_that_is_an_answer() {
    let g = consumer(Some(&["none"]));
    let them = provider(Some(&["none"]));
    let r = g.seam_report(&them, &pairs()).unwrap();
    assert!(r.incompatible.is_empty());
    assert!(
        !r.unstated
            .iter()
            .any(|f| f.our_value.as_deref() == Some("none")),
        "`none` on both sides is agreement that nothing numeric crosses here"
    );
}
