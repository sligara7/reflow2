//! Component-granularity verification (BL-73) — the third state.
//!
//! The fixture replays the field trial that raised this: a brownfield system
//! with a real per-service test suite, modelled as a passing `Verification`
//! on each *component*, whose capabilities carried no checks of their own.
//! Before BL-73 that read as "0/20 capabilities verified" and cost 21
//! near-identical acknowledges; the honest state is neither `verified` nor
//! unchecked, and it deserves its own word (`dec:component-verified-computed`).

use reflow2_core::nodes::node;
use reflow2_core::{CapabilityVerification, DesignGraph, GapSource};

/// One component with a passing suite carrying three capabilities, one
/// directly-verified capability, and one genuinely unchecked capability on an
/// unverified component.
fn fleet_shaped() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:r", "R", "Must work.").unwrap();

    g.add_component("cmp:auth", "Auth service", "authn/z", None)
        .unwrap();
    g.add_component("cmp:bare", "Bare service", "no suite", None)
        .unwrap();

    for (cap, name) in [
        ("cap:login", "Login"),
        ("cap:logout", "Logout"),
        ("cap:sessions", "Sessions"),
    ] {
        g.add_capability(cap, name, "does it", Some("realized"))
            .unwrap();
        g.satisfies(cap, "req:r").unwrap();
        g.allocate(cap, "cmp:auth").unwrap();
    }
    g.add_capability("cap:direct", "Direct", "has its own test", Some("realized"))
        .unwrap();
    g.satisfies("cap:direct", "req:r").unwrap();
    g.allocate("cap:direct", "cmp:auth").unwrap();
    g.add_capability("cap:naked", "Naked", "nothing checks it", Some("realized"))
        .unwrap();
    g.satisfies("cap:naked", "req:r").unwrap();
    g.allocate("cap:naked", "cmp:bare").unwrap();

    // The real per-service suite, registered where it actually lives.
    g.add_verification("ver:auth-suite", "auth service suite", Some("test"), None)
        .unwrap();
    g.verifies("ver:auth-suite", node::COMPONENT, "cmp:auth")
        .unwrap();
    g.set_verification_status("ver:auth-suite", "passing", None)
        .unwrap();

    // One capability with a direct check of its own.
    g.add_verification("ver:direct", "direct test", Some("test"), None)
        .unwrap();
    g.verifies("ver:direct", node::CAPABILITY, "cap:direct")
        .unwrap();
    g.set_verification_status("ver:direct", "passing", None)
        .unwrap();
    g
}

#[test]
fn the_three_states_are_distinguished() {
    let g = fleet_shaped();
    assert_eq!(
        g.capability_verification("cap:direct").unwrap(),
        CapabilityVerification::Verified
    );
    assert_eq!(
        g.capability_verification("cap:login").unwrap(),
        CapabilityVerification::ComponentVerified
    );
    assert_eq!(
        g.capability_verification("cap:naked").unwrap(),
        CapabilityVerification::Unchecked
    );
}

#[test]
fn a_failing_component_suite_carries_nothing() {
    let mut g = fleet_shaped();
    g.set_verification_status("ver:auth-suite", "failing", None)
        .unwrap();
    assert_eq!(
        g.capability_verification("cap:login").unwrap(),
        CapabilityVerification::Unchecked,
        "verified means a check that passes, at any granularity"
    );
}

#[test]
fn coverage_counts_the_third_state_and_the_report_says_it() {
    let g = fleet_shaped();
    let v = g.verification_coverage().unwrap();
    assert_eq!(v.capabilities, 5);
    assert_eq!(v.capabilities_verified, 1, "direct only");
    assert_eq!(v.capabilities_component_verified, 3);

    let md = g.graph_report().unwrap().to_markdown();
    assert!(
        md.contains("1/5 capability(ies) verified (3 more at component granularity)"),
        "{md}"
    );
}

#[test]
fn one_question_per_component_replaces_n_alarms() {
    let g = fleet_shaped();
    let gaps = g.detect_gaps().unwrap();

    // The three riding capabilities raise no per-capability alarm…
    let unverified: Vec<_> = gaps
        .iter()
        .filter(|gap| gap.gap_source == GapSource::UnverifiedCapability)
        .collect();
    assert_eq!(
        unverified.len(),
        1,
        "only the genuinely unchecked capability is alarmed: {unverified:?}"
    );
    assert_eq!(unverified[0].affected_ids, vec!["cap:naked"]);

    // …one component-granularity question stands in for all of them.
    let granularity: Vec<_> = gaps
        .iter()
        .filter(|gap| gap.gap_source == GapSource::ComponentGranularityVerification)
        .collect();
    assert_eq!(granularity.len(), 1);
    let gap = granularity[0];
    assert!(gap.severity < 0.55, "below the per-capability alarm");
    assert!(gap.affected_ids.contains(&"cmp:auth".to_string()));
    for cap in ["cap:login", "cap:logout", "cap:sessions"] {
        assert!(gap.affected_ids.contains(&cap.to_string()), "{gap:?}");
    }
    assert!(
        !gap.affected_ids.contains(&"cap:direct".to_string()),
        "a directly-verified capability rides nothing"
    );
    assert!(gap.title.contains("component granularity"), "{}", gap.title);
}

#[test]
fn the_granularity_question_is_acknowledgeable_once() {
    let mut g = fleet_shaped();
    let gap = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .find(|gap| gap.gap_source == GapSource::ComponentGranularityVerification)
        .expect("the per-component gap exists");

    g.acknowledge_gap(
        &gap.id,
        &gap.affected_ids,
        "component granularity is enough for v1",
    )
    .unwrap();

    assert!(
        !g.detect_gaps()
            .unwrap()
            .iter()
            .any(|gap| gap.gap_source == GapSource::ComponentGranularityVerification),
        "one acknowledge settles the whole component, not 21"
    );
}

#[test]
fn a_verified_status_claim_backed_by_the_component_suite_is_not_a_contradiction() {
    let mut g = fleet_shaped();
    g.set_capability_status("cap:login", "verified").unwrap();

    let contradictions: Vec<_> = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|gap| {
            gap.gap_source == GapSource::StatusContradiction
                && gap.affected_ids.contains(&"cap:login".to_string())
        })
        .collect();
    assert!(
        contradictions.is_empty(),
        "the component's passing suite backs the claim; the granularity gap asks the \
         depth question instead: {contradictions:?}"
    );
}

#[test]
fn loop_status_counts_component_verified_as_proven() {
    let g = fleet_shaped();
    let status = g.loop_status().unwrap();
    assert_eq!(
        status.unproven_capabilities, 1,
        "only cap:naked owes a check; the riders have one, one hop away"
    );
}
