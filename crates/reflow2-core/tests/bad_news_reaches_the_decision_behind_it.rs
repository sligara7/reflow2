//! Step 4, part two — `cap:revise-trigger`. Bad news walks back along the
//! golden thread to the settled decisions behind it, on Anthony's rulings
//! (`dec:step-4-is-built-on-real-runs-with-no-threshold-and-unscored-good-news`):
//! no threshold, no ranking, good news counted on read and never stored,
//! nothing re-opened.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::verify::{ObservedVerification, VerifyReconcileOptions};

/// A pump design: one requirement, a capability that satisfies it under an
/// accepted choice, two checks on the capability, and an artifact realizing it.
fn world() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Pump").expect("project");
    g.add_requirement("req:flow", "Flow", "Deliver 10 L/min.")
        .expect("req");
    g.add_capability("cap:impeller", "Impeller", "moves water", Some("realized"))
        .expect("cap");
    g.satisfies("cap:impeller", "req:flow").expect("sat");
    for (id, status) in [
        ("dec:centrifugal", "accepted"),
        ("dec:idea-magnetic", "proposed"),
    ] {
        g.create_node(
            node::DECISION,
            id,
            Props::new()
                .set("name", id)
                .set("decision", "x")
                .set("status", status),
        )
        .expect("decision");
    }
    for d in ["dec:centrifugal", "dec:idea-magnetic"] {
        g.create_edge(
            edge::GOVERNED_BY,
            node::CAPABILITY,
            "cap:impeller",
            node::DECISION,
            d,
            Props::new(),
        )
        .expect("governed");
    }
    for v in ["ver:flow-rate", "ver:noise"] {
        g.add_verification(v, v, None, None, None).expect("ver");
        g.verifies(v, "Capability", "cap:impeller")
            .expect("verifies");
        g.set_verification_status(v, "passing", None, None)
            .expect("status");
    }
    g
}

fn run(g: &mut DesignGraph, id: &str, outcome: &str) {
    g.reconcile_verification(
        &[ObservedVerification {
            verification_id: id.into(),
            outcome: outcome.into(),
        }],
        &VerifyReconcileOptions {
            record_events: true,
            exhaustive: false,
            detected_at: Some("2026-09-22".into()),
        },
    )
    .expect("reconcile");
}

#[test]
fn no_bad_news_names_no_decision_and_says_what_that_can_mean() {
    let g = world();
    let r = g.decisions_in_doubt().expect("doubt");
    assert!(r.decisions.is_empty());
    assert!(r.evidence_found.is_empty());
    assert!(r.note.contains("untested design"), "{}", r.note);
}

#[test]
fn a_failed_run_reaches_the_accepted_decision_with_its_path() {
    let mut g = world();
    run(&mut g, "ver:flow-rate", "failed");
    let r = g.decisions_in_doubt().expect("doubt");
    assert_eq!(r.decisions.len(), 1, "{:?}", r.decisions);
    let d = &r.decisions[0];
    assert_eq!(d.decision_id, "dec:centrifugal");
    let e = d
        .evidence
        .iter()
        .find(|e| r.evidence[&e.evidence_id].kind == "failed_run")
        .expect("the failed run is listed");
    assert_eq!(e.evidence_id, "ver:flow-rate");
    assert_eq!(e.via, vec!["ver:flow-rate", "cap:impeller"]);
}

#[test]
fn a_proposed_decision_is_not_a_settled_road_and_is_not_listed() {
    let mut g = world();
    run(&mut g, "ver:flow-rate", "failed");
    let r = g.decisions_in_doubt().expect("doubt");
    assert!(
        r.decisions
            .iter()
            .all(|d| d.decision_id != "dec:idea-magnetic")
    );
}

#[test]
fn good_news_is_counted_beside_the_decision_and_never_stored() {
    let mut g = world();
    run(&mut g, "ver:flow-rate", "failed");
    run(&mut g, "ver:noise", "passed");
    let r = g.decisions_in_doubt().expect("doubt");
    assert_eq!(r.decisions[0].held_up, 1);
    let dec = g
        .get_node(node::DECISION, "dec:centrifugal")
        .expect("read")
        .expect("present");
    assert!(
        dec.properties
            .keys()
            .all(|k| !k.contains("held") && !k.contains("confidence") && !k.contains("score")),
        "nothing about good news may be written onto the decision: {:?}",
        dec.properties.keys().collect::<Vec<_>>()
    );
}

#[test]
fn a_check_recorded_failing_is_listed_apart_from_a_failed_run() {
    let mut g = world();
    g.set_verification_status("ver:noise", "failing", None, None)
        .expect("status");
    let r = g.decisions_in_doubt().expect("doubt");
    let kinds: Vec<_> = r.decisions[0]
        .evidence
        .iter()
        .map(|e| r.evidence[&e.evidence_id].kind)
        .collect();
    assert_eq!(kinds, vec!["recorded_failing"]);
}

#[test]
fn an_open_defect_finding_reaches_its_decision_and_a_closed_one_does_not() {
    let mut g = world();
    for (id, closed) in [("fact:cavitation", false), ("fact:old-leak", true)] {
        g.create_node(
            node::TEMPORAL_FACT,
            id,
            Props::new()
                .set("name", id)
                .set("fact_type", "defect")
                .set("subject_id", "cap:impeller")
                .set_opt("valid_to", closed.then_some("2026-09-01")),
        )
        .expect("fact");
    }
    let r = g.decisions_in_doubt().expect("doubt");
    let ids: Vec<_> = r.decisions[0]
        .evidence
        .iter()
        .map(|e| e.evidence_id.as_str())
        .collect();
    assert_eq!(ids, vec!["fact:cavitation"]);
    assert_eq!(r.evidence_found.get("defect_finding"), Some(&1));
}

#[test]
fn bad_news_under_no_decision_is_named_not_dropped() {
    let mut g = world();
    g.add_verification("ver:orphan", "orphan", None, None, None)
        .expect("ver");
    g.set_verification_status("ver:orphan", "failing", None, None)
        .expect("status");
    let r = g.decisions_in_doubt().expect("doubt");
    assert_eq!(r.evidence_reaching_no_decision, vec!["ver:orphan"]);
}

#[test]
fn decisions_come_back_in_id_order_not_by_how_much_evidence() {
    let mut g = world();
    g.create_node(
        node::DECISION,
        "dec:a-stainless-housing",
        Props::new()
            .set("name", "a")
            .set("decision", "x")
            .set("status", "accepted"),
    )
    .expect("decision");
    g.create_edge(
        edge::GOVERNED_BY,
        node::VERIFICATION,
        "ver:noise",
        node::DECISION,
        "dec:a-stainless-housing",
        Props::new(),
    )
    .expect("governed");
    run(&mut g, "ver:flow-rate", "failed");
    run(&mut g, "ver:noise", "failed");
    let r = g.decisions_in_doubt().expect("doubt");
    let ids: Vec<_> = r.decisions.iter().map(|d| d.decision_id.as_str()).collect();
    // dec:centrifugal has two pieces of evidence, the housing one; id order wins.
    assert_eq!(ids, vec!["dec:a-stainless-housing", "dec:centrifugal"]);
}

#[test]
fn a_piece_of_bad_news_is_stated_once_however_many_decisions_it_reaches() {
    let mut g = world();
    g.create_node(
        node::DECISION,
        "dec:a-stainless-housing",
        Props::new()
            .set("name", "a")
            .set("decision", "x")
            .set("status", "accepted"),
    )
    .expect("decision");
    g.create_edge(
        edge::GOVERNED_BY,
        node::REQUIREMENT,
        "req:flow",
        node::DECISION,
        "dec:a-stainless-housing",
        Props::new(),
    )
    .expect("governed");
    run(&mut g, "ver:flow-rate", "failed");
    let r = g.decisions_in_doubt().expect("doubt");
    assert_eq!(r.decisions.len(), 2, "reaches both decisions");
    for d in &r.decisions {
        assert!(d.evidence.iter().any(|e| e.evidence_id == "ver:flow-rate"));
    }
    // The failed run AND the drift event the recorded run minted — each once.
    let referenced: std::collections::BTreeSet<_> = r
        .decisions
        .iter()
        .flat_map(|d| d.evidence.iter().map(|e| e.evidence_id.clone()))
        .collect();
    assert_eq!(
        r.evidence
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        referenced
    );
    assert_eq!(r.evidence["ver:flow-rate"].kind, "failed_run");
    assert_eq!(r.evidence_found.get("drift"), Some(&1));
}
