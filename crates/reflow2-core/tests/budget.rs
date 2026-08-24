//! Budget rollups (BL-11): a Constraint with a quantity and a limit, spenders
//! on CONSTRAINS edges, and an honest verdict. Pins the discipline that an
//! unstated contribution is never zero, the path-cumulative half (worst
//! dependency chain), and the cycle refusal.

use reflow2_core::budget::BudgetVerdict;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn mass_budget(limit: Option<f64>) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Sat").expect("project");
    g.add_constraint(
        "con:mass",
        "Total mass",
        "The assembly must not exceed the mass budget.",
        Some("budget"),
        Some("mass_kg"),
        limit,
        None,
        None,
    )
    .expect("constraint");
    for (id, name) in [
        ("cmp:bus", "Bus"),
        ("cmp:payload", "Payload"),
        ("cmp:solar", "Solar"),
    ] {
        g.add_component(id, name, "a part", None).expect("cmp");
    }
    g
}

#[test]
fn a_fully_stated_budget_reaches_a_verdict() {
    let mut g = mass_budget(Some(100.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        Some("measured"),
        None,
        None,
    )
    .expect("edge");
    g.constrains(
        "con:mass",
        "Component",
        "cmp:payload",
        Some(35.0),
        Some("evidence"),
        None,
        None,
    )
    .expect("edge");
    g.constrains(
        "con:mass",
        "Component",
        "cmp:solar",
        Some(20.0),
        None,
        None,
        None,
    )
    .expect("edge");
    let r = g.budget_report("con:mass").expect("report");
    assert_eq!(r.total, 95.0);
    assert_eq!(r.verdict, BudgetVerdict::Within);
    assert_eq!(r.quantity.as_deref(), Some("mass_kg"));
    // Basis coverage is part of the claim's strength, and the default counts
    // as estimated rather than vanishing.
    assert_eq!(r.basis_coverage.get("measured"), Some(&1));
    assert_eq!(r.basis_coverage.get("estimated"), Some(&1));
}

#[test]
fn exceeding_the_limit_is_said_plainly() {
    let mut g = mass_budget(Some(50.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        None,
        None,
        None,
    )
    .expect("edge");
    g.constrains(
        "con:mass",
        "Component",
        "cmp:payload",
        Some(35.0),
        None,
        None,
        None,
    )
    .expect("edge");
    let r = g.budget_report("con:mass").expect("report");
    assert_eq!(r.verdict, BudgetVerdict::Exceeded);
    assert_eq!(r.total, 75.0);
}

/// The discipline the whole module exists for: a contributor with no stated
/// number makes the verdict `incomplete` — a partial sum passed off as a
/// total is how budgets lie.
#[test]
fn an_unstated_contribution_is_reported_never_zeroed() {
    let mut g = mass_budget(Some(100.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        None,
        None,
        None,
    )
    .expect("edge");
    g.constrains(
        "con:mass",
        "Component",
        "cmp:payload",
        None,
        None,
        None,
        None,
    )
    .expect("edge");
    let r = g.budget_report("con:mass").expect("report");
    assert_eq!(r.verdict, BudgetVerdict::Incomplete);
    assert_eq!(r.unstated, vec!["cmp:payload"]);
    assert_eq!(
        r.total, 40.0,
        "the stated part is still summed, labelled partial"
    );
}

#[test]
fn a_budget_with_no_limit_is_ungated_not_passing() {
    let mut g = mass_budget(None);
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        None,
        None,
        None,
    )
    .expect("edge");
    let r = g.budget_report("con:mass").expect("report");
    assert_eq!(r.verdict, BudgetVerdict::Ungated);
}

#[test]
fn a_minimum_budget_gates_the_other_side() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Sat").expect("project");
    g.add_constraint(
        "con:power",
        "Generated power",
        "The arrays must generate at least the bus draw.",
        Some("budget"),
        Some("power_w"),
        Some(500.0),
        None,
        Some("minimum"),
    )
    .expect("constraint");
    g.add_component("cmp:array", "Array", "solar array", None)
        .expect("cmp");
    g.constrains(
        "con:power",
        "Component",
        "cmp:array",
        Some(450.0),
        None,
        None,
        None,
    )
    .expect("edge");
    let r = g.budget_report("con:power").expect("report");
    assert_eq!(
        r.verdict,
        BudgetVerdict::Exceeded,
        "450 under a 500 minimum"
    );
    assert!(r.path_note.contains("minimum"), "{}", r.path_note);
}

/// The path-cumulative half: end-to-end latency along a dependency chain.
/// The worst path is the heaviest chain among contributors, not the total.
#[test]
fn the_worst_dependency_path_is_accumulated() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Pipeline").expect("project");
    g.add_constraint(
        "con:lat",
        "End-to-end latency",
        "A request must complete within the latency budget.",
        Some("budget"),
        Some("latency_ms"),
        Some(200.0),
        None,
        None,
    )
    .expect("constraint");
    // Two chains: a→b→c (60+50+40 = 150) and a→d (60+30 = 90).
    for (id, ms) in [
        ("cmp:a", 60.0),
        ("cmp:b", 50.0),
        ("cmp:c", 40.0),
        ("cmp:d", 30.0),
    ] {
        g.add_component(id, id, "a stage", None).expect("cmp");
        g.constrains("con:lat", "Component", id, Some(ms), None, None, None)
            .expect("edge");
    }
    for (from, to) in [("cmp:a", "cmp:b"), ("cmp:b", "cmp:c"), ("cmp:a", "cmp:d")] {
        g.create_edge(
            edge::DEPENDS_ON,
            node::COMPONENT,
            from,
            node::COMPONENT,
            to,
            Props::new(),
        )
        .expect("dep");
    }
    let r = g.budget_report("con:lat").expect("report");
    assert_eq!(r.worst_path, vec!["cmp:a", "cmp:b", "cmp:c"]);
    assert_eq!(r.worst_path_total, 150.0);
    assert_eq!(
        r.verdict,
        BudgetVerdict::Within,
        "the TOTAL (180) is also within"
    );
}

#[test]
fn contributors_with_no_dependency_edges_get_the_total_only() {
    let mut g = mass_budget(Some(100.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        None,
        None,
        None,
    )
    .expect("edge");
    g.constrains(
        "con:mass",
        "Component",
        "cmp:payload",
        Some(35.0),
        None,
        None,
        None,
    )
    .expect("edge");
    let r = g.budget_report("con:mass").expect("report");
    assert!(r.worst_path.is_empty());
    assert!(
        r.path_note.contains("no dependency path"),
        "{}",
        r.path_note
    );
}

#[test]
fn a_cycle_among_contributors_refuses_a_path_claim() {
    let mut g = mass_budget(Some(100.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        None,
        None,
        None,
    )
    .expect("edge");
    g.constrains(
        "con:mass",
        "Component",
        "cmp:payload",
        Some(35.0),
        None,
        None,
        None,
    )
    .expect("edge");
    for (a, b) in [("cmp:bus", "cmp:payload"), ("cmp:payload", "cmp:bus")] {
        g.create_edge(
            edge::DEPENDS_ON,
            node::COMPONENT,
            a,
            node::COMPONENT,
            b,
            Props::new(),
        )
        .expect("dep");
    }
    let r = g.budget_report("con:mass").expect("report");
    assert!(r.worst_path.is_empty());
    assert!(r.path_note.contains("cycle"), "{}", r.path_note);
    assert_eq!(
        r.verdict,
        BudgetVerdict::Within,
        "the total rollup still stands"
    );
}

#[test]
fn a_missing_constraint_fails_loud() {
    let g = DesignGraph::open_in_memory().expect("open");
    assert!(g.budget_report("con:nope").is_err());
}

#[test]
fn a_provable_overrun_beats_the_incomplete_caveat() {
    // BL-58: unstated spenders can only ADD, so a stated total already over a
    // maximum is provably Exceeded — reporting Incomplete would hide a definite
    // violation behind "we don't know everything."
    let mut g = mass_budget(Some(100.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(120.0),
        Some("measured"),
        None,
        None,
    )
    .unwrap();
    // cmp:payload's contribution is left UNSTATED.
    g.constrains(
        "con:mass",
        "Component",
        "cmp:payload",
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let report = g.budget_report("con:mass").unwrap();
    assert_eq!(
        report.verdict,
        BudgetVerdict::Exceeded,
        "120 > 100 is over budget no matter the unknowns, got {:?}",
        report.verdict
    );
    assert!(
        !report.unstated.is_empty(),
        "the unstated spender is still reported, just not decisive"
    );
}

#[test]
fn a_non_finite_contribution_is_refused_at_the_write_seam() {
    // BL-58: a NaN poisons the total and panics the worst-path max_by.
    let mut g = mass_budget(Some(100.0));
    assert!(
        g.constrains(
            "con:mass",
            "Component",
            "cmp:bus",
            Some(f64::NAN),
            None,
            None,
            None
        )
        .is_err(),
        "a NaN contribution must be refused"
    );
    assert!(
        g.constrains(
            "con:mass",
            "Component",
            "cmp:bus",
            Some(f64::INFINITY),
            None,
            None,
            None,
        )
        .is_err(),
        "an infinite contribution must be refused"
    );
}

// ---- a measurement says when it was taken -----------------------------------
//
// req:evidence-carries-the-date-it-was-taken, from music_graph F22.
//
// `basis: measured` is the strongest evidence claim this schema offers and the
// ONLY one that decays: an estimate stays an estimate, but a measurement is a
// statement about a system at a moment. Their case — a contribution recorded as
// 6.9 fps with basis=measured against a 40 fps minimum, while the true measured
// value was 55.06. Right when taken, wrong for days, and nothing could say so.
//
// The discipline already existed one axis over: ver:evidence-quality's TIME
// axis counts an undated side as UNKNOWN rather than passing it silently, and
// Artifact.last_confirmed_at exists for exactly this. This brings it to the one
// number a KPP verdict rests on.

/// THE CASE: a measurement with no date is NAMED, not assumed fresh.
#[test]
fn an_undated_measurement_is_reported() {
    let mut g = mass_budget(Some(100.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        Some("measured"),
        None,
        None,
    )
    .unwrap();
    g.constrains(
        "con:mass",
        "Component",
        "cmp:payload",
        Some(20.0),
        Some("measured"),
        Some("2026-08-16"),
        None,
    )
    .unwrap();

    let r = g.budget_report("con:mass").unwrap();
    assert_eq!(
        r.undated_measurements,
        vec!["cmp:bus".to_string()],
        "only the one that states no date"
    );
}

/// COUNTERWEIGHT, and the one that keeps this from firing on correct work: an
/// ESTIMATE needs no date. It does not decay, so demanding one would raise a
/// finding on a design that is behaving properly — the
/// signal-trained-to-be-ignored failure this project keeps naming.
#[test]
fn an_undated_estimate_is_not_reported() {
    let mut g = mass_budget(Some(100.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        Some("estimated"),
        None,
        None,
    )
    .unwrap();
    g.constrains(
        "con:mass",
        "Component",
        "cmp:payload",
        Some(20.0),
        None,
        None,
        None,
    )
    .unwrap();

    let r = g.budget_report("con:mass").unwrap();
    assert!(
        r.undated_measurements.is_empty(),
        "an estimate does not go stale: {:?}",
        r.undated_measurements
    );
}

/// COUNTERWEIGHT 2: reporting must not become refusing. An undated measurement
/// is still a STATED number — it counts toward the total and the verdict, and
/// changing that would break every design that has not started dating them.
#[test]
fn an_undated_measurement_still_counts_toward_the_verdict() {
    let mut g = mass_budget(Some(100.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        Some("measured"),
        None,
        None,
    )
    .unwrap();
    g.constrains(
        "con:mass",
        "Component",
        "cmp:payload",
        Some(20.0),
        Some("measured"),
        None,
        None,
    )
    .unwrap();

    let r = g.budget_report("con:mass").unwrap();
    assert_eq!(r.total, 60.0, "the number is stated, so it is counted");
    assert!(r.unstated.is_empty(), "undated is not unstated");
    assert_eq!(r.verdict, BudgetVerdict::Within);
    assert_eq!(r.undated_measurements.len(), 2, "and both are still named");
}

/// The date survives the round trip onto the edge and back out.
#[test]
fn the_date_is_readable_on_the_contributor() {
    let mut g = mass_budget(Some(100.0));
    g.constrains(
        "con:mass",
        "Component",
        "cmp:bus",
        Some(40.0),
        Some("measured"),
        Some("2026-08-16"),
        None,
    )
    .unwrap();

    let r = g.budget_report("con:mass").unwrap();
    let bus = r
        .contributors
        .iter()
        .find(|c| c.node_id == "cmp:bus")
        .unwrap();
    assert_eq!(bus.measured_at.as_deref(), Some("2026-08-16"));
}
