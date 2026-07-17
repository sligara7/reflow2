//! End-to-end coherence loop — the pieces composing on one graph.
//!
//! `CHANGE → PROPAGATE → DETECT → RESOLVE/HEAL → COHERENCE`. The per-step tests
//! prove each stage in isolation; this proves they interoperate on a single
//! evolving design, which is the whole promise of the system.
//!
//! Narrative: a latency requirement is tightened. The change ripples down the
//! golden thread to the verification that no longer proves it; along the way a
//! new unmet need and a redundant capability appear. DETECT surfaces the unmet
//! need for the human; HEAL merges the redundant capability itself.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{
    ChangeAction, ChangeRecord, ChangeType, DesignGraph, EpochType, GapSource, HealCategory,
    HealOp, HealOptions, ImpactDirection, PropagateOptions,
};

/// A fully coherent baseline design at epoch v1: intent → function → structure →
/// build → verify → operate, every golden-thread link present.
fn baseline() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:widget", "Widget").unwrap();

    g.create_node(
        node::REQUIREMENT,
        "req:latency",
        Props::new()
            .set("name", "Latency")
            .set("statement", "Respond within 200ms")
            .set("status", "accepted"),
    )
    .unwrap();
    g.add_capability("cap:fast", "Fast path", "Serve hot reads quickly")
        .unwrap();
    g.add_component("cmp:cache", "Cache", "In-memory cache")
        .unwrap();
    g.create_node(
        node::ARTIFACT,
        "art:cache",
        Props::new().set("name", "cache.rs"),
    )
    .unwrap();
    g.create_node(
        node::VERIFICATION,
        "ver:cap",
        Props::new().set("name", "cap smoke test"),
    )
    .unwrap();
    g.create_node(
        node::VERIFICATION,
        "ver:art",
        Props::new().set("name", "artifact bench"),
    )
    .unwrap();
    g.create_node(node::RELEASE, "rel:v1", Props::new().set("name", "v1.0"))
        .unwrap();

    g.satisfies("cap:fast", "req:latency").unwrap();
    g.allocate("cap:fast", "cmp:cache").unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:cache",
        node::CAPABILITY,
        "cap:fast",
        Props::new(),
    )
    .unwrap();
    g.create_edge(
        edge::VERIFIES,
        node::VERIFICATION,
        "ver:cap",
        node::CAPABILITY,
        "cap:fast",
        Props::new(),
    )
    .unwrap();
    g.create_edge(
        edge::VERIFIES,
        node::VERIFICATION,
        "ver:art",
        node::ARTIFACT,
        "art:cache",
        Props::new(),
    )
    .unwrap();

    g.add_epoch("epoch:v1", "v1 baseline", EpochType::Baseline, 1)
        .unwrap();
    g
}

#[test]
fn full_coherence_loop() {
    let mut g = baseline();

    // ---- COHERENCE (start): a complete thread has nothing to flag. ----
    assert!(
        g.detect_gaps().unwrap().is_empty(),
        "baseline should have no gaps"
    );
    assert!(
        g.detect_defects().unwrap().is_empty(),
        "baseline should have no structural defects"
    );

    // ---- CHANGE: tighten the latency requirement at a new epoch. ----
    g.add_epoch("epoch:v2", "v2 hardening", EpochType::Revision, 2)
        .unwrap();
    g.precedes("epoch:v1", "epoch:v2").unwrap();
    let (snapshot, change_event) = g
        .record_change(ChangeRecord {
            epoch_id: "epoch:v2",
            change_event_id: "chg:tighten",
            name: "Tighten latency 200ms -> 100ms",
            change_type: ChangeType::RequirementCreep,
            target_type: node::REQUIREMENT,
            target_id: "req:latency",
            action: ChangeAction::Modified,
        })
        .unwrap();
    // Now actually apply the edit (create-or-replace; edges are preserved).
    g.create_node(
        node::REQUIREMENT,
        "req:latency",
        Props::new()
            .set("name", "Latency")
            .set("statement", "Respond within 100ms")
            .set("status", "accepted"),
    )
    .unwrap();

    // The past is remembered: snapshot holds the old bound, live holds the new.
    let snapshot = snapshot.expect("a Modified change snapshots the prior state");
    let old = reflow2_core::parse_snapshot_state(&snapshot).unwrap();
    assert_eq!(old["statement"].as_str(), Some("Respond within 200ms"));
    let live = g
        .get_node(node::REQUIREMENT, "req:latency")
        .unwrap()
        .unwrap();
    assert_eq!(
        live.properties["statement"].as_str(),
        Some("Respond within 100ms")
    );

    // ---- PROPAGATE: the change ripples down the golden thread. ----
    let radius = g
        .propagate_change(&change_event.node_id, PropagateOptions::default())
        .unwrap();
    let reached = |id: &str| radius.impacted.iter().find(|n| n.node_id == id);

    // The capability that satisfies it is directly impacted, downstream.
    let cap = reached("cap:fast").expect("capability is in the blast radius");
    assert_eq!(cap.distance, 1);
    assert_eq!(cap.direction, ImpactDirection::Downstream);
    // The ripple carries all the way to the verifications — the tightened
    // requirement may mean the tests no longer prove the claim.
    assert!(
        reached("ver:cap").is_some(),
        "the capability's verification is in the blast radius"
    );
    assert!(reached("art:cache").is_some());
    assert!(!radius.was_truncated());

    // ---- The change also spawns follow-on work in the graph. ----
    // A new, still-unmet need surfaced by the tightening...
    g.create_node(
        node::REQUIREMENT,
        "req:offline",
        Props::new()
            .set("name", "Offline")
            .set("statement", "Serve from cache when offline")
            .set("status", "accepted"),
    )
    .unwrap();
    // ...and a redundant capability someone added in parallel.
    g.add_capability("cap:fast-dup", "Fast path (dup)", "Serve hot reads quickly")
        .unwrap();
    g.create_edge(
        edge::DUPLICATES,
        node::CAPABILITY,
        "cap:fast",
        node::CAPABILITY,
        "cap:fast-dup",
        Props::new(),
    )
    .unwrap();

    // ---- DETECT: surface the unmet need for the human to resolve. ----
    let gaps = g.detect_gaps().unwrap();
    assert!(
        gaps.iter()
            .any(|gp| gp.gap_source == GapSource::UnsatisfiedRequirement
                && gp.affected_ids == ["req:offline"]),
        "DETECT should surface the new unsatisfied requirement"
    );

    // ---- HEAL: the redundant capability is a structural defect HEAL fixes. ----
    let defects = g.detect_defects().unwrap();
    assert!(
        defects
            .iter()
            .any(|d| d.category == HealCategory::Duplicate),
        "HEAL should detect the duplicate"
    );

    let proposal = g.propose_heal(HealOptions::default()).unwrap();
    // The duplicate becomes a merge keeping the well-connected original.
    let merge = proposal
        .operations
        .iter()
        .find(|o| matches!(&o.op, HealOp::Merge { .. }))
        .expect("a merge operation for the duplicate");
    match &merge.op {
        HealOp::Merge {
            keep_id, remove_id, ..
        } => {
            assert_eq!(keep_id, "cap:fast");
            assert_eq!(remove_id, "cap:fast-dup");
        }
        _ => unreachable!(),
    }
    // The unmet-need orphan is generative → the proposal is gated for review.
    assert!(proposal.requires_human_review);

    let report = g.apply_heal(&proposal).unwrap();
    assert!(report.applied);
    assert!(report.operations_applied >= 1);
    assert!(
        report.verified,
        "post-repair verification confirms the merged defect is gone"
    );

    // ---- COHERENCE (restored, structurally): duplicate resolved. ----
    assert!(
        g.get_node(node::CAPABILITY, "cap:fast-dup")
            .unwrap()
            .is_none()
    );
    assert!(g.get_node(node::CAPABILITY, "cap:fast").unwrap().is_some());
    assert!(
        !g.detect_defects()
            .unwrap()
            .iter()
            .any(|d| d.category == HealCategory::Duplicate),
        "no duplicate remains after HEAL"
    );

    // The unmet need is still open — HEAL doesn't invent intent; that's the
    // human's call via the DETECT prompt. The loop surfaced it honestly.
    assert!(
        g.detect_gaps()
            .unwrap()
            .iter()
            .any(|gp| gp.affected_ids == ["req:offline"]),
        "the unmet requirement remains for the human to resolve"
    );
}
