//! Depth axis — dimensional quality rollup + per-epoch drift (graph-analysis
//! step 5), computed with dynograph-vector stats.

use reflow2_core::nodes::{edge, node};
use reflow2_core::{DesignGraph, Dimension, DriftDirection};

/// Record a reading for a component at a given (sortable) epoch.
fn obs(g: &mut DesignGraph, id: &str, target: &str, dim: Dimension, score: f64, at: &str) {
    g.add_dimension_observation(id, node::COMPONENT, target, dim, score, at, None)
        .unwrap();
}

#[test]
fn a_declining_dimension_is_detected_with_a_negative_slope() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:x", "X", "part").unwrap();
    obs(
        &mut g,
        "o1",
        "cmp:x",
        Dimension::Maintainability,
        0.9,
        "e01",
    );
    obs(
        &mut g,
        "o2",
        "cmp:x",
        Dimension::Maintainability,
        0.7,
        "e02",
    );
    obs(
        &mut g,
        "o3",
        "cmp:x",
        Dimension::Maintainability,
        0.5,
        "e03",
    );

    let d = g
        .dimension_drift("cmp:x", Dimension::Maintainability)
        .unwrap()
        .expect("observations exist");
    assert_eq!(d.observation_count, 3);
    assert_eq!(d.first_score, 0.9);
    assert_eq!(d.last_score, 0.5);
    assert!(d.slope < 0.0, "slope should be negative, was {}", d.slope);
    assert_eq!(d.direction, DriftDirection::Declining);
    assert!((d.rollup_score - 0.7).abs() < 1e-9, "mean rollup");
}

#[test]
fn an_improving_dimension_trends_up_and_a_flat_one_is_stable() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:up", "Up", "p").unwrap();
    obs(&mut g, "u1", "cmp:up", Dimension::Reliability, 0.3, "e01");
    obs(&mut g, "u2", "cmp:up", Dimension::Reliability, 0.6, "e02");
    obs(&mut g, "u3", "cmp:up", Dimension::Reliability, 0.9, "e03");
    assert_eq!(
        g.dimension_drift("cmp:up", Dimension::Reliability)
            .unwrap()
            .unwrap()
            .direction,
        DriftDirection::Improving
    );

    let mut g2 = DesignGraph::open_in_memory().unwrap();
    g2.add_component("cmp:flat", "Flat", "p").unwrap();
    obs(&mut g2, "f1", "cmp:flat", Dimension::Security, 0.5, "e01");
    obs(&mut g2, "f2", "cmp:flat", Dimension::Security, 0.5, "e02");
    assert_eq!(
        g2.dimension_drift("cmp:flat", Dimension::Security)
            .unwrap()
            .unwrap()
            .direction,
        DriftDirection::Stable
    );
}

#[test]
fn drift_is_scoped_to_the_named_dimension() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:x", "X", "p").unwrap();
    obs(
        &mut g,
        "m1",
        "cmp:x",
        Dimension::Maintainability,
        0.9,
        "e01",
    );
    obs(
        &mut g,
        "m2",
        "cmp:x",
        Dimension::Maintainability,
        0.5,
        "e02",
    );
    // A different dimension's reading must not leak into the maintainability series.
    obs(&mut g, "s1", "cmp:x", Dimension::Security, 0.1, "e01");

    let d = g
        .dimension_drift("cmp:x", Dimension::Maintainability)
        .unwrap()
        .unwrap();
    assert_eq!(d.observation_count, 2);
}

#[test]
fn drifts_rank_the_most_declining_first() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:sliding", "Sliding", "p").unwrap();
    g.add_component("cmp:rising", "Rising", "p").unwrap();
    obs(
        &mut g,
        "d1",
        "cmp:sliding",
        Dimension::Maintainability,
        0.9,
        "e01",
    );
    obs(
        &mut g,
        "d2",
        "cmp:sliding",
        Dimension::Maintainability,
        0.3,
        "e02",
    );
    obs(
        &mut g,
        "r1",
        "cmp:rising",
        Dimension::Reliability,
        0.3,
        "e01",
    );
    obs(
        &mut g,
        "r2",
        "cmp:rising",
        Dimension::Reliability,
        0.9,
        "e02",
    );

    let drifts = g.dimension_drifts().unwrap();
    assert_eq!(drifts.len(), 2);
    assert_eq!(drifts[0].target_id, "cmp:sliding", "worst decline first");
    assert!(drifts[0].slope < 0.0);
    assert!(drifts[1].slope > 0.0);
}

#[test]
fn rollup_materializes_a_dimension_assessment() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:x", "X", "p").unwrap();
    obs(&mut g, "o1", "cmp:x", Dimension::Testability, 0.8, "e01");
    obs(&mut g, "o2", "cmp:x", Dimension::Testability, 0.6, "e02");

    let assessment = g
        .rollup_assessment("assess:1", node::COMPONENT, "cmp:x", Dimension::Testability)
        .unwrap()
        .expect("something to roll up");
    assert!((assessment.properties["score"].as_f64().unwrap() - 0.7).abs() < 1e-9);
    assert_eq!(assessment.properties["evidence_count"].as_i64(), Some(2));

    // Wired back to the node it assesses.
    let assessed = g.outgoing("assess:1", Some(edge::ASSESSED_ON)).unwrap();
    assert_eq!(assessed.len(), 1);
    assert_eq!(assessed[0].to_id, "cmp:x");
}

#[test]
fn no_observations_yields_no_drift() {
    let g = DesignGraph::open_in_memory().unwrap();
    assert!(
        g.dimension_drift("cmp:absent", Dimension::Maturity)
            .unwrap()
            .is_none()
    );
}
