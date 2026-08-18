//! Which of the design vocabulary a design has never used — figures always,
//! list on request, grouped by domain, and parkable.
//!
//! `dec:idea-how-does-a-users-project-acquire-vocabulary-it-never-uses`, option
//! (b), in the shape a two-arm trial chose for it.
//!
//! # The trial these probes protect
//!
//! Run on reflow2 itself before any of this was built, in the owner's framing —
//! *"a doctor who develops a new medicine, but tries it on himself first"*:
//!
//! | arm | node types | edge types | flat list |
//! |-----|-----------|-----------|-----------|
//! | mature (2535 nodes) | 22/29 | 37/61 | 59 items |
//! | day one (post-genesis, 2 nodes) | 2/29 | 0/61 | **97 items** |
//!
//! The FIGURES passed both arms. The FLAT LIST failed the second outright —
//! longer for the user least able to act on it. Each probe below pins one of
//! the four decisions that came out of that, so a later session cannot quietly
//! undo one without a red test.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, vocabulary_park_decision_id};

/// A design with a little of the golden thread and none of the flow,
/// dimensions or readiness vocabulary — the ordinary shape of a real project.
fn partial_design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_component("cmp:one", "One", "does a thing", None)
        .unwrap();
    g.add_capability("cap:one", "One", "a capability", None)
        .unwrap();
    g.allocate("cap:one", "cmp:one").unwrap();
    g.add_requirement("req:one", "One", "must hold").unwrap();
    g.satisfies("cap:one", "req:one").unwrap();
    g.create_edge(
        edge::CONTAINS,
        node::PROJECT,
        "proj:p",
        node::COMPONENT,
        "cmp:one",
        Props::new(),
    )
    .unwrap();
    // Enough nodes that the "barely started" note stands down.
    for i in 0..8 {
        g.add_requirement(&format!("req:filler{i}"), "filler", "x")
            .unwrap();
    }
    g
}

#[test]
fn the_figures_are_always_there_and_the_flat_list_never_is_unless_asked() {
    // THE DECISION THIS PINS: ship the figures, keep the flat list behind a
    // request. Measured, the list is 97 items on a day-one design and 59 on a
    // mature one — longest exactly where it is least usable.
    let g = partial_design();

    let quiet = g.vocabulary_coverage(false).unwrap();
    assert!(
        quiet.unused.is_none(),
        "the flat list is withheld by default"
    );
    assert!(quiet.node_types.total > 0 && quiet.edge_types.total > 0);
    assert!(
        quiet.edge_types.used < quiet.edge_types.total,
        "this fixture uses only a little of the vocabulary, so there is something to report"
    );

    let asked = g.vocabulary_coverage(true).unwrap();
    let list = asked.unused.expect("asked for, so returned");
    assert!(
        !list.is_empty(),
        "a design using a fraction of the vocabulary has unused vocabulary to name"
    );
    assert_eq!(
        quiet.edge_types.used, asked.edge_types.used,
        "asking for the list must not change the figures"
    );
}

#[test]
fn the_share_is_computed_so_the_reader_is_not_left_dividing() {
    let g = partial_design();
    let c = g.vocabulary_coverage(false).unwrap();
    for cov in [&c.node_types, &c.edge_types, &c.properties_on_used_types] {
        let expected = if cov.total == 0 {
            0.0
        } else {
            cov.used as f64 / cov.total as f64
        };
        assert!(
            (cov.share - expected).abs() < 1e-9,
            "share must equal used/total: {} vs {expected}",
            cov.share
        );
        assert!(cov.used <= cov.total, "used can never exceed total");
    }
}

#[test]
fn unused_vocabulary_is_grouped_by_the_schemas_own_domains() {
    // THE DECISION THIS PINS: group, do not list. The trial found the unused
    // vocabulary clustering into whole subsystems, and the clusters turned out
    // to BE the schema's domains — so the axis is the design's own, never one
    // this code invented.
    let g = partial_design();
    let c = g.vocabulary_coverage(false).unwrap();

    assert!(
        c.domains.len() >= 10,
        "all eleven schema domains are reported, not only the empty ones — a \
         domain missing from the list would read as fully used: {}",
        c.domains.len()
    );
    for d in &c.domains {
        assert!(!d.domain.is_empty());
        assert!(
            d.node_types.total + d.edge_types.total > 0,
            "{} declares nothing",
            d.domain
        );
    }
    // Worst-covered first: the shape of the gap is visible without reading names.
    let scores: Vec<f64> = c
        .domains
        .iter()
        .map(|d| d.node_types.share + d.edge_types.share)
        .collect();
    assert!(
        scores.windows(2).all(|w| w[0] <= w[1] + 1e-9),
        "domains are ordered worst-covered first: {scores:?}"
    );
}

#[test]
fn a_domain_can_be_ruled_deliberately_unused_and_the_remedy_rides_on_the_finding() {
    // THE DECISION THIS PINS: make it parkable. `OWNED_BY` turned up in the
    // trial's unused list and its absence is DELIBERATE
    // (`dec:ownership-reads-claims-before-adding-an-edge`), so a report that
    // could not be told "this one is on purpose" would report a settled
    // decision as a hole forever — `req:a-deliberate-state-is-not-a-defect`.
    let mut g = partial_design();
    let target = g.vocabulary_coverage(false).unwrap().domains[0]
        .domain
        .clone();
    let park_id = vocabulary_park_decision_id(&target);

    let before = g.vocabulary_coverage(false).unwrap();
    assert_eq!(before.parked_domains, 0);
    assert!(
        before.domains.iter().all(|d| !d.parked),
        "nothing is parked until somebody rules it so"
    );
    assert!(
        before.domains.iter().any(|d| d.park_with == park_id),
        "the remedy is stated on the finding itself, whether or not it has been used — \
         a convention that rides only on a skill is one most sessions never load"
    );

    g.add_decision(
        &park_id,
        "The flow vocabulary is deliberately unused here",
        "This project models no processes.",
        Some("Nothing in this design is a workflow, so the flow types would be empty by choice."),
    )
    .unwrap();

    // A PROPOSED ruling must NOT park anything — somebody thinking out loud is
    // not a decision, the same line `ruling: parks` draws.
    let musing = g.vocabulary_coverage(false).unwrap();
    assert_eq!(
        musing.parked_domains, 0,
        "a proposed Decision is somebody thinking out loud and must not silence a finding"
    );

    g.set_decision_status(&park_id, "accepted").unwrap();
    let after = g.vocabulary_coverage(false).unwrap();
    assert_eq!(
        after.parked_domains, 1,
        "an ACCEPTED ruling parks exactly one domain"
    );
    let parked = after.domains.iter().find(|d| d.domain == target).unwrap();
    assert!(parked.parked);
    assert!(
        parked.parked_because.is_some(),
        "and carries WHY, so a later session reads the reason instead of re-litigating"
    );
    assert_eq!(
        after.edge_types.used, before.edge_types.used,
        "parking is a disposition, not a deletion — the figures do not move"
    );
}

#[test]
fn a_design_that_has_barely_started_is_told_so_instead_of_shown_a_wall_of_red() {
    // THE DECISION THIS PINS, and it is the one the trial's second arm forced:
    // straight out of genesis a design holds 2 nodes and every figure is near
    // zero. That is not vocabulary going unused, it is a design that has not
    // begun — and reporting the first as the second is a wall of red on a new
    // user's first read.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:new", "Brand new").unwrap();

    let fresh = g.vocabulary_coverage(false).unwrap();
    let note = fresh
        .note
        .expect("a barely-started design must say so in words");
    assert!(
        note.contains("BARELY STARTED"),
        "the note names the situation rather than describing the numbers: {note}"
    );

    // And it stands down once there is a design to measure.
    assert!(
        partial_design()
            .vocabulary_coverage(false)
            .unwrap()
            .note
            .is_none(),
        "the note's PRESENCE is the signal, so it must disappear on a real design"
    );
}
