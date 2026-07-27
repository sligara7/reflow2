//! `req:design-the-simulator` — telling proof-against-a-model apart from
//! proof-against-reality.
//!
//! The argument for simulating first is that issues are cheap to fix there and
//! expensive in the field. That only holds if somebody can still tell the two
//! apart afterwards — and until 2026-07-27 reflow2 could not: a check run on a
//! rig and the same check run in production were both simply "passing".

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn env(g: &mut DesignGraph, id: &str, env_type: &str) {
    g.create_node(
        node::ENVIRONMENT,
        id,
        Props::new().set("name", id).set("env_type", env_type),
    )
    .unwrap();
}

fn check(g: &mut DesignGraph, id: &str, cap: &str, performed_in: Option<&str>) {
    g.add_verification(id, id, Some("test"), Some("system"))
        .unwrap();
    g.set_verification_status(id, "passing", None).unwrap();
    g.verifies(id, node::CAPABILITY, cap).unwrap();
    if let Some(e) = performed_in {
        g.create_edge(
            edge::PERFORMED_IN,
            node::VERIFICATION,
            id,
            node::ENVIRONMENT,
            e,
            Props::new(),
        )
        .unwrap();
    }
}

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_capability("cap:gear", "Landing gear", "extends", None)
        .unwrap();
    g.add_capability("cap:radio", "Radio", "talks", None)
        .unwrap();
    env(&mut g, "env:rig", "simulation");
    env(&mut g, "env:field", "field");
    g
}

/// The claim worth surfacing: everything that proves this ran against a model.
#[test]
fn a_capability_proven_only_on_a_rig_says_so() {
    let mut g = design();
    check(&mut g, "ver:drop", "cap:gear", Some("env:rig"));
    check(&mut g, "ver:flight", "cap:radio", Some("env:field"));

    let r = g.evidence_report().unwrap();
    assert_eq!(r.simulation_only, 1, "{r:?}");
    let gear = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:gear")
        .unwrap();
    assert!(gear.simulation_only);
    assert_eq!(gear.simulated_environments, vec!["env:rig".to_string()]);

    let radio = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:radio")
        .unwrap();
    assert!(
        !radio.simulation_only,
        "the field check is not a simulation"
    );
}

/// One real check is enough to stop the claim — the point of the progression is
/// that you eventually leave the rig, and reaching reality must show.
#[test]
fn one_real_check_ends_the_simulation_only_claim() {
    let mut g = design();
    check(&mut g, "ver:drop", "cap:gear", Some("env:rig"));
    check(&mut g, "ver:real-drop", "cap:gear", Some("env:field"));

    let r = g.evidence_report().unwrap();
    let gear = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:gear")
        .unwrap();
    assert!(!gear.simulation_only);
    assert_eq!(
        gear.proven_in,
        vec!["env:field".to_string(), "env:rig".to_string()]
    );
    assert_eq!(r.simulation_only, 0);
}

/// **Silence is not evidence of the field.** A check that says nowhere it ran is
/// UNPLACED and counted as such — never assumed real, which would be the
/// flattering reading and the dangerous one.
#[test]
fn a_check_that_names_no_environment_is_unplaced_not_assumed_real() {
    let mut g = design();
    check(&mut g, "ver:somewhere", "cap:gear", None);

    let r = g.evidence_report().unwrap();
    let gear = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:gear")
        .unwrap();
    assert_eq!(gear.unplaced_checks, 1);
    assert!(gear.proven_in.is_empty());
    assert!(
        !gear.simulation_only,
        "unknown is not simulated either — it is unknown"
    );
    assert_eq!(r.with_unplaced_checks, 1);
}

/// A capability nothing has proven yet is the unverified_capability question,
/// not this report's business — it must not appear as "proven nowhere".
#[test]
fn a_capability_with_no_passing_check_is_not_in_the_report() {
    let mut g = design();
    g.add_verification("ver:planned", "planned check", None, None)
        .unwrap();
    g.verifies("ver:planned", node::CAPABILITY, "cap:gear")
        .unwrap();

    let r = g.evidence_report().unwrap();
    assert!(
        r.capabilities.is_empty(),
        "nothing is proven, so there is no evidence to report on: {r:?}"
    );
}
