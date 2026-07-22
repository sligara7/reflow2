//! Analysis of alternatives (`analyze_alternatives`) — BL-70, branch-by-file.
//!
//! Alternatives are separate exports; this lays them side by side on the same
//! measures and reports each one's divergence from the baseline.

use reflow2_core::{DesignGraph, GapSource, GraphExport, Value, analyze_alternatives};

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

// --- rung 2: the decision point — register alternatives, collapse the fork ---

fn as_str(n: &reflow2_core::DesignGraph, ty: &str, id: &str, key: &str) -> Option<String> {
    n.get_node(ty, id).expect("get_node").and_then(|node| {
        node.properties
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

/// A proposed decision point with nothing registered yet.
fn decision_point() -> DesignGraph {
    let mut g = g();
    g.add_project("proj:x", "X").expect("project");
    g.add_decision("dec:choice", "Which approach", "Undecided.", None)
        .expect("decision");
    g.set_decision_status("dec:choice", "proposed")
        .expect("propose");
    g
}

#[test]
fn alternatives_register_under_a_proposed_decision_and_contradict() {
    let mut g = decision_point();
    g.register_alternative("dec:choice", "alt:a", "Option A", "alt-a.json")
        .expect("a");
    g.register_alternative("dec:choice", "alt:b", "Option B", "alt-b.json")
        .expect("b");

    let alts = g.alternatives_for("dec:choice").expect("list");
    assert_eq!(alts.len(), 2);
    assert_eq!(alts[0].id, "alt:a");
    assert_eq!(alts[0].location.as_deref(), Some("alt-a.json"));

    // The second sibling CONTRADICTS the first.
    let contradicts = g.outgoing("alt:b", Some("CONTRADICTS")).expect("edges");
    assert!(contradicts.iter().any(|e| e.to_id == "alt:a"));
}

#[test]
fn registering_under_a_settled_decision_is_refused() {
    let mut g = g();
    g.add_project("proj:x", "X").expect("project");
    // add_decision creates it accepted (settled), not a proposed decision point.
    g.add_decision("dec:done", "Already chosen", "Settled.", None)
        .expect("decision");

    let err = g
        .register_alternative("dec:done", "alt:x", "X", "x.json")
        .unwrap_err();
    assert!(
        format!("{err}").contains("proposed"),
        "you fork an open choice, not a settled one"
    );
}

#[test]
fn collapsing_accepts_the_decision_supersedes_losers_and_records_the_obituary() {
    let mut g = decision_point();
    g.register_alternative("dec:choice", "alt:a", "Option A", "alt-a.json")
        .expect("a");
    g.register_alternative("dec:choice", "alt:b", "Option B", "alt-b.json")
        .expect("b");

    let report = g
        .collapse_decision("dec:choice", "alt:a", Some("A is simpler"))
        .expect("collapse");
    assert_eq!(report.winner, "alt:a");
    assert_eq!(report.retired, vec!["alt:b".to_string()]);

    // The decision is settled.
    assert_eq!(
        as_str(&g, "Decision", "dec:choice", "status").as_deref(),
        Some("accepted")
    );

    // The outcome is written into the ADR's own alternatives field.
    let obituary =
        as_str(&g, "Decision", "dec:choice", "alternatives").expect("alternatives prose");
    assert!(obituary.contains("chosen"), "the winner is recorded");
    assert!(obituary.contains("retired"), "the loser is recorded");
    assert!(obituary.contains("A is simpler"), "the rationale is kept");

    // The winner supersedes the loser on the record (retired, not deleted).
    let obsoletes = g.outgoing("alt:a", Some("OBSOLETES")).expect("edges");
    assert!(obsoletes.iter().any(|e| e.to_id == "alt:b"));
    assert!(
        g.get_node("Artifact", "alt:b").expect("get").is_some(),
        "the loser is kept, not deleted"
    );
}

#[test]
fn collapsing_to_a_non_alternative_is_refused() {
    let mut g = decision_point();
    g.register_alternative("dec:choice", "alt:a", "Option A", "alt-a.json")
        .expect("a");
    let err = g
        .collapse_decision("dec:choice", "alt:ghost", None)
        .unwrap_err();
    assert!(format!("{err}").contains("not an alternative"));
}

#[test]
fn a_proposed_decision_with_two_alternatives_raises_an_open_fork_gap() {
    let mut g = decision_point();
    g.register_alternative("dec:choice", "alt:a", "Option A", "a.json")
        .expect("a");

    // One road is not a choice — no fork yet.
    let gaps = g.detect_gaps().expect("detect");
    assert!(
        !gaps
            .iter()
            .any(|g| g.gap_source == GapSource::UndecidedDecisionPoint),
        "a single alternative is not an open fork"
    );

    g.register_alternative("dec:choice", "alt:b", "Option B", "b.json")
        .expect("b");

    // Two roads, undecided — the teeth fire.
    let gaps = g.detect_gaps().expect("detect");
    let gap = gaps
        .iter()
        .find(|g| g.gap_source == GapSource::UndecidedDecisionPoint)
        .expect("an undecided decision point is surfaced");
    assert!(gap.affected_ids.contains(&"dec:choice".to_string()));
    assert!(gap.affected_ids.contains(&"alt:a".to_string()));

    // Collapsing settles it — the fork closes, the gap is gone.
    g.collapse_decision("dec:choice", "alt:a", Some("simpler"))
        .expect("collapse");
    let gaps = g.detect_gaps().expect("detect");
    assert!(
        !gaps
            .iter()
            .any(|g| g.gap_source == GapSource::UndecidedDecisionPoint),
        "a settled decision is no longer an open fork"
    );
}
