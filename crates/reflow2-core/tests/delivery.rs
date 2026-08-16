//! Derived delivery (BL-104) — "done" is computed from the golden thread.
//!
//! `Requirement.status` carries a `met` value and it is the wrong place to
//! learn delivery from: a hand-set "done" outlives the truth, surviving the
//! capability regressing, the artifact drifting and the check starting to fail.
//! Measured on reflow2's own design the day this was written, ZERO of 28
//! requirements carried `met` while several were plainly shipped — so the field
//! was not merely unreliable, it was unused, and "how much is actually done?"
//! could not be answered from a graph holding every input needed to answer it.
//!
//! The tests that matter here are the NEGATIVE ones. Asserting that a complete
//! thread reads as delivered is easy and proves little; what makes this a
//! derivation rather than a slower assertion is that it goes BACKWARDS when the
//! evidence does.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::node;

/// One requirement, one capability that satisfies it, one artifact realizing
/// that capability, and one check — whose status each test sets for itself.
fn threaded() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:ship", "It ships", "The thing must ship.")
        .unwrap();
    g.set_requirement_status("req:ship", "accepted").unwrap();

    g.add_component("cmp:engine", "Engine", "does the work", None)
        .unwrap();
    g.add_capability("cap:ship", "Ship it", "ships the thing", Some("realized"))
        .unwrap();
    g.satisfies("cap:ship", "req:ship").unwrap();
    g.allocate("cap:ship", "cmp:engine").unwrap();

    g.add_artifact(
        "art:engine",
        "engine.rs",
        Some("code"),
        Some("src/engine.rs"),
    )
    .unwrap();
    g.realizes("art:engine", node::CAPABILITY, "cap:ship", None, None)
        .unwrap();

    g.add_verification("ver:ship", "ship test", Some("test"), None)
        .unwrap();
    g.verifies("ver:ship", node::CAPABILITY, "cap:ship")
        .unwrap();
    g
}

#[test]
fn a_complete_thread_reads_as_delivered_without_anyone_setting_met() {
    let mut g = threaded();
    g.set_verification_status("ver:ship", "passing", None)
        .unwrap();

    let d = g.delivery_coverage().unwrap();
    assert_eq!(d.requirements, 1);
    assert_eq!(d.satisfied, 1);
    assert_eq!(
        d.delivered, 1,
        "satisfied + realized + passing is delivered, with status untouched"
    );

    // The point of the whole exercise: nobody wrote `met` anywhere.
    let req = g.get_node(node::REQUIREMENT, "req:ship").unwrap().unwrap();
    assert_eq!(
        req.properties.get("status").and_then(|v| v.as_str()),
        Some("accepted"),
        "delivery is derived, so the stored status is left alone"
    );
}

#[test]
fn a_failing_check_un_delivers_it() {
    // THE test. A derivation that cannot go backwards is just a slower
    // assertion: if delivery only ever ratchets up, it is a stored claim with
    // extra steps, and it will keep reporting success through a regression.
    let mut g = threaded();
    g.set_verification_status("ver:ship", "passing", None)
        .unwrap();
    assert_eq!(g.delivery_coverage().unwrap().delivered, 1);

    g.set_verification_status("ver:ship", "failing", None)
        .unwrap();
    let d = g.delivery_coverage().unwrap();
    assert_eq!(
        d.delivered, 0,
        "the check now fails, so the requirement is no longer delivered"
    );
    assert_eq!(
        d.satisfied, 1,
        "it is still satisfied — something still claims to deliver it, and \
         conflating the two would hide which half broke"
    );
}

#[test]
fn a_check_that_merely_exists_does_not_deliver() {
    // `dec:passing-is-verified`, applied one level up. A planned test is not a
    // passing one, and counting its existence is the reflow1 failure in
    // miniature: counting test nodes while ignoring test results.
    let g = threaded(); // ver:ship left at its default, `planned`
    assert_eq!(g.delivery_coverage().unwrap().delivered, 0);
}

#[test]
fn unbuilt_does_not_deliver_however_well_tested() {
    // Realization and verification are separate legs of the thread. A capability
    // with a passing check but nothing realizing it is a test of an intention.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:ship", "It ships", "The thing must ship.")
        .unwrap();
    g.add_capability("cap:ship", "Ship it", "ships the thing", None)
        .unwrap();
    g.satisfies("cap:ship", "req:ship").unwrap();
    g.add_verification("ver:ship", "ship test", Some("test"), None)
        .unwrap();
    g.verifies("ver:ship", node::CAPABILITY, "cap:ship")
        .unwrap();
    g.set_verification_status("ver:ship", "passing", None)
        .unwrap();

    let d = g.delivery_coverage().unwrap();
    assert_eq!(d.satisfied, 1);
    assert_eq!(d.delivered, 0, "nothing is built for it");
}

#[test]
fn a_requirement_recovered_by_inference_never_counts_as_delivered() {
    // The brownfield trap the schema names on `Requirement.provenance`: a
    // requirement read back out of the code implementing it is satisfied by
    // construction and can never contradict anything. If these counted, an
    // adopt pass would report itself fully delivered on arrival, having
    // demonstrated nothing — and the number would look best exactly when the
    // design was most speculative.
    let mut g = threaded();
    g.set_verification_status("ver:ship", "passing", None)
        .unwrap();
    let req = g.get_node(node::REQUIREMENT, "req:ship").unwrap().unwrap();
    let mut props = reflow2_core::nodes::Props::new().set("provenance", "inferred");
    for (k, v) in &req.properties {
        if k != "provenance" {
            props = props.set(k, v.clone());
        }
    }
    g.create_node(node::REQUIREMENT, "req:ship", props.build())
        .unwrap();

    let d = g.delivery_coverage().unwrap();
    assert_eq!(
        d.delivered, 0,
        "a thread that closes on itself is not evidence"
    );
    assert_eq!(
        d.inferred_only, 1,
        "and it is counted, not silently dropped — the reader must be able to \
         see how much of the picture rests on inference"
    );
}

#[test]
fn a_dropped_requirement_is_not_counted_as_unfinished() {
    // Abandoning a need is not failing to deliver it. Counting dropped
    // requirements would make the denominator punish good housekeeping.
    let mut g = threaded();
    g.set_requirement_status("req:ship", "dropped").unwrap();
    let d = g.delivery_coverage().unwrap();
    assert_eq!(d.requirements, 0);
    assert_eq!(d.satisfied, 0);
}

#[test]
fn component_granularity_still_delivers() {
    // `dec:component-verified-computed` established that a capability whose
    // component carries a passing suite is genuinely checked, one hop away.
    // Delivery honours that rather than re-litigating it — otherwise a tested
    // brownfield system reads as undelivered for the same reason it once read
    // as unverified.
    let mut g = threaded();
    g.delete_edge("VERIFIES", "ver:ship", "cap:ship").unwrap();
    g.verifies("ver:ship", node::COMPONENT, "cmp:engine")
        .unwrap();
    g.set_verification_status("ver:ship", "passing", None)
        .unwrap();

    assert_eq!(g.delivery_coverage().unwrap().delivered, 1);
}

#[test]
fn the_report_carries_delivery_and_leaves_status_alone() {
    let mut g = threaded();
    g.set_verification_status("ver:ship", "passing", None)
        .unwrap();
    let r = g.graph_report().unwrap();
    let d = r.delivery.as_ref().expect("requirements exist");
    assert_eq!(d.delivered, 1);

    let md = r.to_markdown();
    assert!(
        md.contains("Delivered:"),
        "the rollup must be visible in the read a person actually takes: {md}"
    );
    assert!(
        md.contains("not from a status field"),
        "and it must say where the number comes from, so nobody maintains it by \
         hand alongside: {md}"
    );
}
