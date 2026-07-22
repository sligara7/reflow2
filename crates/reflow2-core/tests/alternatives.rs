//! Analysis of alternatives (`analyze_alternatives`) — BL-70, branch-by-file.
//!
//! Alternatives are separate exports; this lays them side by side on the same
//! measures and reports each one's divergence from the baseline.

use reflow2_core::{DesignGraph, GraphExport, analyze_alternatives};

fn g() -> DesignGraph {
    DesignGraph::open_in_memory().expect("graph")
}

fn ex(g: &DesignGraph) -> GraphExport {
    g.export_graph().expect("export")
}

/// A small coherent baseline: a project, a satisfied requirement, a capability.
fn baseline() -> DesignGraph {
    let mut g = g();
    g.add_project("proj:x", "X").expect("project");
    g.add_requirement("req:one", "First", "Does the first thing.")
        .expect("req");
    g.set_requirement_status("req:one", "accepted")
        .expect("status");
    g.add_capability("cap:one", "First capability", "Does it.", None)
        .expect("cap");
    g.satisfies("cap:one", "req:one").expect("satisfies");
    g
}

#[test]
fn a_single_alternative_reports_measures_and_no_divergence() {
    let report = analyze_alternatives(&[("only".to_string(), ex(&baseline()))]).expect("aoa");
    assert_eq!(report.baseline, "only");
    assert_eq!(report.branches.len(), 1);
    let b = &report.branches[0];
    assert!(
        b.divergence_from_baseline.is_none(),
        "the baseline has nothing to diverge from"
    );
    assert!(b.design_nodes >= 3, "project + requirement + capability");
    assert_eq!(b.capabilities, 1);
}

#[test]
fn a_variant_carries_its_divergence_from_the_baseline() {
    let base = ex(&baseline());
    let mut variant = baseline();
    variant
        .add_capability("cap:two", "Second capability", "Does another.", None)
        .expect("add");
    let variant = ex(&variant);

    let report = analyze_alternatives(&[
        ("baseline".to_string(), base),
        ("variant".to_string(), variant),
    ])
    .expect("aoa");
    assert_eq!(report.branches.len(), 2);
    assert!(report.branches[0].divergence_from_baseline.is_none());

    let d = report.branches[1]
        .divergence_from_baseline
        .as_ref()
        .expect("the variant diverges from the baseline");
    assert!(d.design_added >= 1, "the variant added a capability");
    assert!(
        report.branches[1].design_nodes > report.branches[0].design_nodes,
        "the variant is a bigger design"
    );
    assert_eq!(report.branches[1].capabilities, 2);
}

#[test]
fn measures_expose_an_alternative_with_more_open_gaps() {
    // A branch whose requirement nobody satisfies carries an extra open gap —
    // exactly the kind of thing an AoA should surface, not bury.
    let mut gappy = g();
    gappy.add_project("proj:x", "X").expect("project");
    gappy
        .add_requirement("req:lonely", "Unsatisfied", "Nobody builds toward this.")
        .expect("req");
    gappy
        .set_requirement_status("req:lonely", "accepted")
        .expect("status");

    let report = analyze_alternatives(&[
        ("baseline".to_string(), ex(&baseline())),
        ("gappy".to_string(), ex(&gappy)),
    ])
    .expect("aoa");
    assert!(
        report.branches[1].open_gaps > 0,
        "an unsatisfied requirement is an open gap the AoA must show"
    );
}

#[test]
fn the_same_alternatives_analyse_identically() {
    let base = ex(&baseline());
    let mut v = baseline();
    v.add_capability("cap:two", "Second", "does", None)
        .expect("add");
    let variant = ex(&v);
    let alts = [("base".to_string(), base), ("v".to_string(), variant)];
    assert_eq!(
        analyze_alternatives(&alts).expect("a"),
        analyze_alternatives(&alts).expect("b"),
    );
}
