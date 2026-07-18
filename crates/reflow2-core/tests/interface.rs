//! The interface layer — the seam between two parts of a design.
//!
//! Motivated by the failure mode the original Reflow never solved: a change is
//! made on one side of a service boundary and the other side is forgotten.
//! Modelling the contract as a real `Interface` node with `PROVIDES`/`CONSUMES`
//! edges makes the other side reachable by PROPAGATE, and makes an unpaired
//! contract detectable by DETECT.

use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;
use reflow2_core::propagate::PropagateOptions;

/// Two components joined by a contract: `provider` PROVIDES, `consumer` CONSUMES.
fn two_sided_contract() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_component("comp:api", "Score API", "Serves scores")
        .expect("provider");
    g.add_component("comp:ui", "Scoreboard UI", "Displays scores")
        .expect("consumer");
    g.add_interface("iface:scores", "Scores endpoint")
        .expect("interface");
    g.provides("comp:api", "iface:scores").expect("provides");
    g.consumes("comp:ui", "iface:scores").expect("consumes");
    g
}

#[test]
fn changing_one_side_of_an_interface_surfaces_the_other() {
    let g = two_sided_contract();

    let radius = g
        .propagate_from(&["comp:api"], PropagateOptions::default())
        .expect("propagate");

    let reached: Vec<&str> = radius.impacted.iter().map(|n| n.node_id.as_str()).collect();

    assert!(
        reached.contains(&"comp:ui"),
        "the consumer on the far side of the interface must be in the blast radius, got {reached:?}"
    );

    // And the path must be explained through the contract, not inferred.
    let ui = radius
        .impacted
        .iter()
        .find(|n| n.node_id == "comp:ui")
        .expect("consumer impacted");
    let via: Vec<&str> = ui.via.iter().map(|h| h.edge_type.as_str()).collect();
    assert!(
        via.contains(&"PROVIDES") && via.contains(&"CONSUMES"),
        "impact must be carried by the contract edges, got chain {via:?}"
    );
}

#[test]
fn interface_impact_is_symmetric() {
    let g = two_sided_contract();

    let from_consumer = g
        .propagate_from(&["comp:ui"], PropagateOptions::default())
        .expect("propagate");

    assert!(
        from_consumer
            .impacted
            .iter()
            .any(|n| n.node_id == "comp:api"),
        "changing the consumer must also surface the provider"
    );
}

#[test]
fn consumed_but_unprovided_interface_is_a_gap() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_component("comp:ui", "Scoreboard UI", "Displays scores")
        .expect("consumer");
    g.add_interface("iface:scores", "Scores endpoint")
        .expect("interface");
    g.consumes("comp:ui", "iface:scores").expect("consumes");

    let gaps = g.detect_gaps().expect("detect");
    let gap = gaps
        .iter()
        .find(|c| c.gap_source == GapSource::UnprovidedInterface)
        .expect("unprovided interface must be surfaced");

    assert_eq!(gap.affected_ids, vec!["iface:scores"]);
    assert!(
        gap.evidence.contains("0 incoming PROVIDES"),
        "evidence must carry the raw signal, got {}",
        gap.evidence
    );
}

#[test]
fn provided_but_unconsumed_interface_is_a_softer_gap() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_component("comp:api", "Score API", "Serves scores")
        .expect("provider");
    g.add_interface("iface:scores", "Scores endpoint")
        .expect("interface");
    g.provides("comp:api", "iface:scores").expect("provides");

    let gaps = g.detect_gaps().expect("detect");
    let unconsumed = gaps
        .iter()
        .find(|c| c.gap_source == GapSource::UnconsumedInterface)
        .expect("unconsumed interface must be surfaced");
    let unprovided = gaps
        .iter()
        .find(|c| c.gap_source == GapSource::UnprovidedInterface);

    assert!(
        unprovided.is_none(),
        "a provided interface must not also be reported as unprovided"
    );
    assert!(
        unconsumed.severity < 0.5,
        "an unused contract may be deliberate — it must rank below a broken one"
    );
}

#[test]
fn a_fully_paired_interface_reports_no_gap() {
    let g = two_sided_contract();

    let gaps = g.detect_gaps().expect("detect");
    assert!(
        !gaps.iter().any(|c| matches!(
            c.gap_source,
            GapSource::UnprovidedInterface | GapSource::UnconsumedInterface
        )),
        "a contract with both sides present is not a gap"
    );
}

#[test]
fn interface_pairing_is_keyed_on_identity_not_name() {
    // Two interfaces with the *same* name, each paired with its own component.
    // A name-keyed check (reflow's `defaultdict` over interface type strings)
    // would reconcile these against each other and stay silent; identity-keyed
    // detection reports the broken one.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_component("comp:api", "Score API", "Serves scores")
        .expect("provider");
    g.add_component("comp:ui", "Scoreboard UI", "Displays scores")
        .expect("consumer");
    g.add_interface("iface:a", "Scores endpoint").expect("a");
    g.add_interface("iface:b", "Scores endpoint").expect("b");
    g.provides("comp:api", "iface:a").expect("provides a");
    g.consumes("comp:ui", "iface:b").expect("consumes b");

    let gaps = g.detect_gaps().expect("detect");
    let unprovided: Vec<&str> = gaps
        .iter()
        .filter(|c| c.gap_source == GapSource::UnprovidedInterface)
        .flat_map(|c| c.affected_ids.iter().map(String::as_str))
        .collect();

    assert_eq!(
        unprovided,
        vec!["iface:b"],
        "the consumed-but-unprovided contract must be caught despite the shared name"
    );
}
