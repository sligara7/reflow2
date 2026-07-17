//! DETECT tests — deterministic gap detectors.
//!
//! The two behaviors that matter most: phase-coverage fires at project scope
//! when a whole phase is absent, and per-node traceability fires *only once that
//! phase exists* — so an early-stage graph gets one nudge, not a flood, and a
//! complete thread yields nothing.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, GapScope, GapSource};

fn sources(gaps: &[reflow2_core::GapCandidate]) -> Vec<GapSource> {
    gaps.iter().map(|g| g.gap_source).collect()
}

#[test]
fn early_graph_gets_project_level_phase_nudges_not_per_node_floods() {
    // Only concept exists: one requirement + one capability, nothing downstream.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    g.add_capability("cap:a", "Cap A", "Does A").unwrap();
    g.satisfies("cap:a", "req:a").unwrap();

    let gaps = g.detect_gaps().unwrap();
    let srcs = sources(&gaps);

    // The design phase is absent → one project-level nudge.
    assert!(srcs.contains(&GapSource::ConceptWithoutDesign));
    // Downstream phase-coverage nudges also fire (no verifications either).
    assert!(srcs.contains(&GapSource::BuildWithoutVerification));

    // Crucially: NO per-node traceability gaps, because those phases don't exist
    // yet (no components → unallocated is not asked; no artifacts → unrealized
    // is not asked).
    assert!(!srcs.contains(&GapSource::UnallocatedCapability));
    assert!(!srcs.contains(&GapSource::UnrealizedCapability));
    assert!(!srcs.contains(&GapSource::UnsatisfiedRequirement)); // req:a IS satisfied

    // Phase-coverage gaps are project/phase scoped.
    for gap in &gaps {
        assert_eq!(gap.scope, GapScope::Phase);
    }
}

#[test]
fn traceability_fires_per_node_once_the_phase_exists() {
    // Components exist, so allocation is now expected. cap:b is unallocated.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    g.add_capability("cap:a", "Cap A", "Does A").unwrap();
    g.add_capability("cap:b", "Cap B", "Does B").unwrap();
    g.add_component("cmp:x", "X", "Part X").unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.satisfies("cap:b", "req:a").unwrap();
    g.allocate("cap:a", "cmp:x").unwrap(); // cap:a allocated, cap:b not

    let gaps = g.detect_gaps().unwrap();
    let unallocated: Vec<&str> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::UnallocatedCapability)
        .flat_map(|x| x.affected_ids.iter().map(String::as_str))
        .collect();

    // Exactly cap:b — cap:a is allocated, so it is not flagged.
    assert_eq!(unallocated, ["cap:b"]);
    // The design phase now exists, so concept_without_design no longer fires.
    assert!(!sources(&gaps).contains(&GapSource::ConceptWithoutDesign));
}

#[test]
fn unsatisfied_requirement_ranks_by_priority() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // Two unsatisfied requirements at different priorities; capabilities exist
    // (so the detector is active) but satisfy neither.
    g.create_node(
        node::REQUIREMENT,
        "req:crit",
        Props::new()
            .set("name", "Critical need")
            .set("statement", "must")
            .set("priority", "critical"),
    )
    .unwrap();
    g.create_node(
        node::REQUIREMENT,
        "req:low",
        Props::new()
            .set("name", "Nice to have")
            .set("statement", "maybe")
            .set("priority", "low"),
    )
    .unwrap();
    g.add_capability("cap:x", "X", "does x").unwrap();
    g.add_component("cmp:y", "Y", "part y").unwrap();
    g.allocate("cap:x", "cmp:y").unwrap();

    let gaps = g.detect_gaps().unwrap();
    let unsat: Vec<&reflow2_core::GapCandidate> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::UnsatisfiedRequirement)
        .collect();
    assert_eq!(unsat.len(), 2);
    // Critical outranks low in severity, so it sorts first overall among these.
    let crit = unsat
        .iter()
        .find(|x| x.affected_ids == ["req:crit"])
        .unwrap();
    let low = unsat
        .iter()
        .find(|x| x.affected_ids == ["req:low"])
        .unwrap();
    assert!(crit.severity > low.severity);
}

#[test]
fn dropped_or_met_requirements_are_not_flagged() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.create_node(
        node::REQUIREMENT,
        "req:dropped",
        Props::new()
            .set("name", "Abandoned")
            .set("statement", "no")
            .set("status", "dropped"),
    )
    .unwrap();
    g.add_capability("cap:x", "X", "does x").unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(!sources(&gaps).contains(&GapSource::UnsatisfiedRequirement));
}

#[test]
fn complete_thread_yields_no_traceability_gaps() {
    // A full concept→operate thread: every golden-thread link present.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.create_node(
        node::REQUIREMENT,
        "req:a",
        Props::new()
            .set("name", "A")
            .set("statement", "need")
            .set("status", "accepted"),
    )
    .unwrap();
    g.add_capability("cap:a", "Cap A", "does a").unwrap();
    g.add_component("cmp:a", "Cmp A", "part a").unwrap();
    g.create_node(node::ARTIFACT, "art:a", Props::new().set("name", "a.rs"))
        .unwrap();
    g.create_node(
        node::VERIFICATION,
        "ver:a",
        Props::new().set("name", "test a"),
    )
    .unwrap();
    g.create_node(node::RELEASE, "rel:a", Props::new().set("name", "v1.0"))
        .unwrap();

    g.satisfies("cap:a", "req:a").unwrap();
    g.allocate("cap:a", "cmp:a").unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:a",
        node::CAPABILITY,
        "cap:a",
        Props::new(),
    )
    .unwrap();
    // Verify both the capability and the artifact (each needs its own).
    g.create_edge(
        edge::VERIFIES,
        node::VERIFICATION,
        "ver:a",
        node::CAPABILITY,
        "cap:a",
        Props::new(),
    )
    .unwrap();
    g.create_node(
        node::VERIFICATION,
        "ver:b",
        Props::new().set("name", "test a2"),
    )
    .unwrap();
    g.create_edge(
        edge::VERIFIES,
        node::VERIFICATION,
        "ver:b",
        node::ARTIFACT,
        "art:a",
        Props::new(),
    )
    .unwrap();

    let gaps = g.detect_gaps().unwrap();
    let srcs = sources(&gaps);
    // No traceability gaps at all.
    assert!(!srcs.contains(&GapSource::UnsatisfiedRequirement));
    assert!(!srcs.contains(&GapSource::UnallocatedCapability));
    assert!(!srcs.contains(&GapSource::UnrealizedCapability));
    assert!(!srcs.contains(&GapSource::UnverifiedCapability));
    // And no phase-coverage gaps except deploy/operate (we added a Release, so
    // even that is covered) — expect an empty gap set.
    assert!(gaps.is_empty(), "unexpected gaps: {:?}", srcs);
}

#[test]
fn gap_ids_are_deterministic_across_runs() {
    let build = || {
        let mut g = DesignGraph::open_in_memory().unwrap();
        g.add_requirement("req:a", "A", "need").unwrap();
        g.add_capability("cap:a", "Cap A", "does a").unwrap();
        g.add_component("cmp:a", "Cmp A", "part a").unwrap();
        // cap:a unallocated on purpose.
        g.detect_gaps().unwrap()
    };
    let first = build();
    let second = build();
    let ids1: Vec<&str> = first.iter().map(|g| g.id.as_str()).collect();
    let ids2: Vec<&str> = second.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids1, ids2, "same graph state must yield identical gap ids");
    assert!(first.iter().all(|g| g.id.starts_with("gap:")));
}
