//! An accepted Decision that governs nothing is invisible to every detector.
//!
//! Found 2026-08-01 by running `check-health` and `detect-and-ask` on reflow2's
//! own design, getting a clean bill from all of them, and then counting
//! zero-degree nodes by hand. `dec:sanitize-spof-accepted` — an **accepted**
//! single-point-of-failure disposition — had no edges at all. It is the only
//! one of the five accepted SPOF dispositions not linked to what it disposes,
//! and nothing in reflow2 could see it.
//!
//! `unthreaded_cluster` cannot: it only reports clusters of **≥2**, so a
//! node connected to nothing is never a cluster. `orphan_node` cannot: since
//! [BL-42] it holds exactly one rule, an Artifact realizing nothing.
//!
//! ## Why it matters more than tidiness
//!
//! A Decision with no edges cannot be reached by propagation, so it never
//! appears in any impact analysis — and an *accepted* one claims to shape the
//! design while shaping nothing. For a disposition specifically, it also cannot
//! **expire**: `ver:reviewed-defects` pins that an acceptance lapses when the
//! shape it was accepted about changes, and expiry is computed from the
//! affected set. With no link to that set, the acceptance is permanent — a
//! conditional judgement quietly converted into an off switch.
//!
//! ## The rule is DEGREE-ZERO, and that was decided by measurement
//!
//! The obvious "narrow" rule is *an accepted Decision with no incoming
//! `GOVERNED_BY`*. **Measured on reflow2's own design that fires on SIX, five of
//! which have degree 1–3** — they are connected, just not through that one edge
//! type. That is precisely [BL-42]'s shape, where `orphan_node` reported a
//! well-connected Capability missing one named link and became **20 of 31
//! defects**, the dominant noise source, and had to be cut back.
//!
//! Degree-zero fires on **one**. It is self-limiting in a way an edge-named rule
//! is not: **any** edge at all silences it, so it can never grow into a
//! per-convention nag. The counterweights below pin exactly that.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory, HealSeverity};

/// A minimal coherent design, so nothing else in HEAL fires and the assertions
/// below are about the rule under test rather than about fixture noise.
fn base() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_requirement("req:r", "R", "need r").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_component("cmp:c", "C", "part c", None).unwrap();
    g.satisfies("cap:a", "req:r").unwrap();
    g.allocate("cap:a", "cmp:c").unwrap();
    g
}

fn orphan_ids(g: &DesignGraph) -> Vec<String> {
    g.open_defects()
        .unwrap()
        .into_iter()
        .filter(|i| i.category == HealCategory::OrphanNode)
        .flat_map(|i| i.affected_ids)
        .collect()
}

fn severity_of(g: &DesignGraph, id: &str) -> Option<HealSeverity> {
    g.open_defects()
        .unwrap()
        .into_iter()
        .find(|i| i.category == HealCategory::OrphanNode && i.affected_ids.iter().any(|a| a == id))
        .map(|i| i.severity)
}

/// THE BUG, and it is `dec:sanitize-spof-accepted` reproduced: an accepted
/// Decision wired to nothing. It claims to shape the design, shapes nothing,
/// and — being a disposition — can never expire.
#[test]
fn an_accepted_decision_with_no_edges_is_reported_as_a_warning() {
    let mut g = base();
    g.add_decision(
        "dec:floating",
        "Accepted and wired to nothing",
        "we accept it",
        None,
    )
    .unwrap();
    g.set_decision_status("dec:floating", "accepted").unwrap();

    assert!(
        orphan_ids(&g).contains(&"dec:floating".to_string()),
        "an accepted Decision reachable from nothing must be reported"
    );
    assert_eq!(
        severity_of(&g, "dec:floating"),
        Some(HealSeverity::Warning),
        "an ACCEPTED decision governing nothing is a real defect, not a note"
    );
}

/// The status half of the grading. A `proposed` Decision is a legitimate
/// parking spot — a decision point deliberately not governing anything yet —
/// so it is reported at `info`, never as something to fix.
#[test]
fn a_proposed_decision_with_no_edges_is_only_info() {
    let mut g = base();
    g.add_decision("dec:parked", "OPEN — not settled", "undecided", None)
        .unwrap();

    assert_eq!(
        severity_of(&g, "dec:parked"),
        Some(HealSeverity::Info),
        "a parked decision point is not a defect; reporting it as one is nagging"
    );
}

/// THE COUNTERWEIGHT THAT THE MEASUREMENT DEMANDED, and the most important case
/// here. The rule must key on degree ZERO, not on a missing `GOVERNED_BY`.
///
/// On reflow2's own design the `GOVERNED_BY` form fires on six accepted
/// Decisions, five of which have degree 1–3 — connected, just not through that
/// edge. That is [BL-42] exactly. Here the Decision is reached by a
/// `CONTRADICTS` edge and nothing else: no `GOVERNED_BY` anywhere, and it must
/// still be silent.
#[test]
fn an_accepted_decision_reached_by_any_edge_is_not_reported() {
    let mut g = base();
    g.add_decision("dec:linked", "Accepted and connected", "we accept it", None)
        .unwrap();
    g.set_decision_status("dec:linked", "accepted").unwrap();
    g.add_decision("dec:other", "Another", "also a decision", None)
        .unwrap();
    g.create_edge(
        edge::CONTRADICTS,
        node::DECISION,
        "dec:other",
        node::DECISION,
        "dec:linked",
        Props::new().set("alignment", "supporting"),
    )
    .unwrap();

    assert!(
        !orphan_ids(&g).contains(&"dec:linked".to_string()),
        "ANY edge must silence this — keying on GOVERNED_BY is the BL-42 shape"
    );
}

/// The other counterweight, and it protects a deliberate design choice rather
/// than a convenience. Ids beginning `decision:ack:` are review records, and
/// `structure.rs` excludes them from the design network on purpose — they
/// describe a *judgement about* the design, not how it is structured, which is
/// the `ver:acknowledgement-not-structure` fix. Every one of them is `accepted`
/// by construction, so without this the rule would fire on all twelve of
/// reflow2's own and be nothing but noise.
#[test]
fn a_review_record_with_no_edges_is_not_reported() {
    let mut g = base();
    g.add_decision(
        "decision:ack:heal:deadbeef",
        "Reviewed: heal:deadbeef",
        "Accepted the structural defect heal:deadbeef.",
        Some("out of scope for v1"),
    )
    .unwrap();
    g.set_decision_status("decision:ack:heal:deadbeef", "accepted")
        .unwrap();

    assert!(
        !orphan_ids(&g).contains(&"decision:ack:heal:deadbeef".to_string()),
        "review records are deliberately not structural; firing on them is noise"
    );
}

/// The existing rule must survive untouched. [BL-42] cut `orphan_node` down to
/// exactly one thing — an Artifact realizing nothing — and this change adds
/// beside it rather than reopening what was closed.
#[test]
fn the_artifact_rule_still_fires() {
    let mut g = base();
    g.create_node(
        node::ARTIFACT,
        "art:loose",
        Props::new()
            .set("name", "loose.rs")
            .set("location", "src/loose.rs"),
    )
    .unwrap();

    assert!(
        orphan_ids(&g).contains(&"art:loose".to_string()),
        "the pre-existing orphan_node rule must not be disturbed"
    );
}

/// And the design that is fine must stay silent. A trigger that fires on
/// correct work is the failure BL-42 was filed about, and the whole reason this
/// rule is degree-zero rather than convention-shaped.
#[test]
fn a_well_formed_design_reports_no_orphan_decisions() {
    let mut g = base();
    g.add_decision(
        "dec:real",
        "A governing decision",
        "we do it this way",
        None,
    )
    .unwrap();
    g.set_decision_status("dec:real", "accepted").unwrap();
    g.governed_by(node::CAPABILITY, "cap:a", node::DECISION, "dec:real")
        .unwrap();

    let ids = orphan_ids(&g);
    assert!(
        !ids.contains(&"dec:real".to_string()),
        "a decision that governs something is not an orphan: {ids:?}"
    );
}
