//! Graph report (SYNTHESIZE) — aggregates the deterministic analyses into one
//! "what should I look at?" artifact.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, Dimension};

fn dep(g: &mut DesignGraph, from: &str, to: &str, w: f64) {
    g.create_edge(
        edge::DEPENDS_ON,
        node::CAPABILITY,
        from,
        node::CAPABILITY,
        to,
        Props::new().set("weight", w),
    )
    .unwrap();
}

#[test]
fn report_aggregates_every_analysis_and_renders_markdown() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // Intent + two capability clusters allocated to two components.
    g.create_node(
        node::REQUIREMENT,
        "req:r",
        Props::new()
            .set("name", "R")
            .set("statement", "s")
            .set("status", "accepted"),
    )
    .unwrap();
    for c in ["cap:a1", "cap:a2", "cap:b1", "cap:b2"] {
        g.add_capability(c, c, "does a thing", None).unwrap();
    }
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();
    g.satisfies("cap:a1", "req:r").unwrap();
    for (c, comp) in [
        ("cap:a1", "cmp:a"),
        ("cap:a2", "cmp:a"),
        ("cap:b1", "cmp:b"),
        ("cap:b2", "cmp:b"),
    ] {
        g.allocate(c, comp).unwrap();
    }
    dep(&mut g, "cap:a1", "cap:a2", 0.9);
    dep(&mut g, "cap:b1", "cap:b2", 0.9);
    dep(&mut g, "cap:a1", "cap:b1", 0.1); // the surprising bridge
    // A declining quality dimension.
    g.add_dimension_observation(
        "o1",
        node::COMPONENT,
        "cmp:a",
        Dimension::Maintainability,
        0.9,
        "e01",
        None,
    )
    .unwrap();
    g.add_dimension_observation(
        "o2",
        node::COMPONENT,
        "cmp:a",
        Dimension::Maintainability,
        0.5,
        "e02",
        None,
    )
    .unwrap();

    let r = g.graph_report().unwrap();

    // Snapshot.
    assert!(r.node_counts.contains(&(node::CAPABILITY, 4)));
    assert!(r.node_counts.contains(&(node::COMPONENT, 2)));
    assert!(r.total_nodes >= 7);

    // Every analysis is represented.
    assert!(r.gap_count > 0 && !r.top_gaps.is_empty());
    let alloc = r.allocation.as_ref().expect("components exist");
    assert_eq!(alloc.component_count, 2);
    assert!(alloc.modularity > 0.9);
    assert_eq!(r.surprising.len(), 1);
    assert_eq!(r.surprising[0].from_id, "cap:a1");
    assert_eq!(r.surprising[0].to_id, "cap:b1");
    assert_eq!(r.declining.len(), 1);
    assert_eq!(r.declining[0].target_id, "cmp:a");
    assert_eq!(r.declining[0].dimension, Dimension::Maintainability);

    // Markdown renders each section.
    let md = r.to_markdown();
    for section in [
        "# Design graph report",
        "## Snapshot",
        "## Top gaps",
        "## Allocation health",
        "## Surprising couplings",
        "## Quality drift",
    ] {
        assert!(md.contains(section), "missing section: {section}");
    }
    assert!(md.contains("cmp:a"));
    assert!(md.contains("maintainability"));
}

#[test]
fn an_empty_graph_reports_empty() {
    let g = DesignGraph::open_in_memory().unwrap();
    let r = g.graph_report().unwrap();
    assert_eq!(r.total_nodes, 0);
    assert_eq!(r.gap_count, 0);
    assert!(r.allocation.is_none());
    assert!(r.to_markdown().contains("Empty graph"));
}

/// BL-43, from the storyflow adopt trial: the import wrote 122 nodes and the
/// report said 109 — the 13 missing were exactly the Fragments, because
/// `total_nodes` summed a hardcoded design-layer list. A count that silently
/// omits a node type is a quiet lie about the size of the design.
#[test]
fn the_total_counts_every_node_including_the_provenance_layer() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:r", "R", "need it").unwrap();
    g.create_node(
        node::FRAGMENT,
        "frag:src",
        Props::new().set("title", "the note it came from"),
    )
    .unwrap();

    let rep = g.graph_report().unwrap();
    assert_eq!(rep.design_nodes, 2, "Project + Requirement");
    assert_eq!(rep.total_nodes, 3, "…and the Fragment is a node too");
    assert_eq!(
        rep.other_counts,
        vec![("Fragment".to_string(), 1)],
        "what the design-layer itemisation does not cover is named, not dropped"
    );

    // And it is visible to a reader, not just to a field.
    let md = rep.to_markdown();
    assert!(md.contains("Fragment 1"), "{md}");
    assert!(md.contains("3 nodes in total"), "{md}");
}

// ---- Loop status: the debt list, computed from state (BL-74) ----------------

#[test]
fn a_design_with_nothing_owed_reads_clean() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();

    let status = g.loop_status().unwrap();

    // A bare project draws phase nudges, and nudges are guidance, not debt.
    assert!(status.clean, "{:?}", status.next);
    assert!(status.next.is_empty());
    assert_eq!(status.unsurfaced_gaps, 0);
}

#[test]
fn captured_intent_owes_a_surface_pass_until_the_question_is_asked() {
    use reflow2_core::AskedQuestion;

    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    // Stated intent plus a capability claiming realized, satisfying nothing,
    // verified by nothing: exactly the state a raw-tools-only capture session
    // leaves behind.
    g.add_requirement("req:r", "R", "Must do x.").unwrap();
    g.add_capability("cap:x", "X", "does x", Some("realized"))
        .unwrap();

    let before = g.loop_status().unwrap();
    assert!(!before.clean);
    assert!(
        before.unsurfaced_gaps > 0,
        "anchored gaps exist and nobody asked"
    );
    assert_eq!(before.unproven_capabilities, 1);
    assert!(
        before.next.iter().any(|l| l.contains("detect-and-ask")),
        "{:?}",
        before.next
    );
    assert!(
        before.next.iter().any(|l| l.contains("no passing check")),
        "{:?}",
        before.next
    );

    // Surfacing every anchored gap moves the debt from "unsurfaced" to
    // "waiting on the user" — the loop advanced one step.
    let gaps: Vec<_> = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|gap| !gap.affected_ids.is_empty())
        .collect();
    for gap in &gaps {
        g.record_asked_question(
            &gap.id,
            &gap.affected_ids,
            "What should happen here?",
            AskedQuestion::default(),
        )
        .unwrap();
    }
    let asked = g.loop_status().unwrap();
    assert_eq!(asked.unsurfaced_gaps, 0);
    assert_eq!(asked.unanswered_questions, gaps.len());
    assert!(
        asked.next.iter().any(|l| l.contains("waiting on the user")),
        "{:?}",
        asked.next
    );

    // An answer that never reaches the design is its own named debt.
    g.answer_question(&gaps[0].id, "It should do x.").unwrap();
    let answered = g.loop_status().unwrap();
    assert_eq!(answered.unwritten_answers, 1);
    assert_eq!(answered.unanswered_questions, gaps.len() - 1);
    assert!(
        answered
            .next
            .iter()
            .any(|l| l.contains("never reached the design")),
        "{:?}",
        answered.next
    );

    // Proving the capability clears the unproven count.
    g.add_verification("ver:x", "x tests", Some("test"), None)
        .unwrap();
    g.verifies("ver:x", node::CAPABILITY, "cap:x").unwrap();
    g.set_verification_status("ver:x", "passing", None).unwrap();
    assert_eq!(g.loop_status().unwrap().unproven_capabilities, 0);
}

#[test]
fn recorded_drift_is_owed_a_disposition_until_accepted() {
    use reflow2_core::drift::{ObservedArtifact, ReconcileOptions};
    use reflow2_core::{DriftDisposition, LinkArtifactOptions};

    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_capability("cap:x", "X", "does x", Some("realized"))
        .unwrap();
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:x".into(),
        name: "x.rs".into(),
        location: Some("src/x.rs".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:x".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:old".into()),
    })
    .unwrap();

    // Built, never reconciled: the ledger calls it unexamined and so do we.
    assert_eq!(g.loop_status().unwrap().unexamined_claims, 1);

    let report = g
        .reconcile_artifacts(
            &[ObservedArtifact {
                artifact_id: "art:x".into(),
                present: true,
                checksum: Some("sha256:new".into()),
            }],
            &ReconcileOptions {
                record_events: true,
                exhaustive: false,
                detected_at: Some("2026-07-21".into()),
            },
        )
        .unwrap();
    assert_eq!(report.findings.len(), 1);

    let drifted = g.loop_status().unwrap();
    assert_eq!(drifted.undispositioned_drift, 1);
    assert!(
        drifted.next.iter().any(|l| l.contains("disposition")),
        "{:?}",
        drifted.next
    );

    g.set_artifact_checksum(
        "art:x",
        "sha256:new",
        DriftDisposition::DesignHolds {
            change_type: reflow2_core::ChangeType::TestFailureFix,
        },
        None,
        Some("2026-07-21"),
    )
    .unwrap();
    let accepted = g.loop_status().unwrap();
    assert_eq!(accepted.undispositioned_drift, 0);
    assert_eq!(accepted.unexamined_claims, 0);
}

// ---- Requirement certainty: derived, never stored (BL-75) -------------------

#[test]
fn certainty_is_derived_from_status_and_provenance() {
    use reflow2_core::RequirementCertainty;

    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();

    // Captured by an agent, awaiting the user: asserted.
    g.add_requirement("req:asserted", "A", "Someone stated it.")
        .unwrap();
    // Recovered from an artifact during adopt, awaiting the user.
    g.add_requirement("req:recovered", "R", "Read out of the code.")
        .unwrap();
    g.set_provenance(node::REQUIREMENT, "req:recovered", "inferred")
        .unwrap();
    // The user said yes — including to a recovered one: provenance keeps
    // saying how it ENTERED, status records their word.
    g.add_requirement("req:confirmed", "C", "The user confirmed it.")
        .unwrap();
    g.set_requirement_status("req:confirmed", "accepted")
        .unwrap();
    g.add_requirement(
        "req:confirmed-recovered",
        "CR",
        "Recovered, then confirmed.",
    )
    .unwrap();
    g.set_provenance(node::REQUIREMENT, "req:confirmed-recovered", "inferred")
        .unwrap();
    g.set_requirement_status("req:confirmed-recovered", "accepted")
        .unwrap();
    // The user decided it out — their word too, not uncertainty.
    g.add_requirement("req:out", "O", "Not in v1.").unwrap();
    g.set_requirement_status("req:out", "dropped").unwrap();

    for (req, expected) in [
        ("req:asserted", RequirementCertainty::Asserted),
        ("req:recovered", RequirementCertainty::Recovered),
        ("req:confirmed", RequirementCertainty::UserConfirmed),
        (
            "req:confirmed-recovered",
            RequirementCertainty::UserConfirmed,
        ),
        ("req:out", RequirementCertainty::SettledOut),
    ] {
        assert_eq!(
            g.requirement_certainty(req).unwrap(),
            expected,
            "{req} should read as {expected:?}"
        );
    }

    let b = g.requirement_certainty_breakdown().unwrap();
    assert_eq!(
        (b.user_confirmed, b.asserted, b.recovered, b.settled_out),
        (2, 1, 1, 1)
    );

    // And the report says it, so no session reconstructs it in prose.
    let md = g.graph_report().unwrap().to_markdown();
    assert!(
        md.contains(
            "Requirement certainty: 2 user-confirmed · 1 asserted, awaiting the user · \
             1 recovered from the artifact, awaiting the user · 1 settled out (deferred/dropped)."
        ),
        "{md}"
    );
}

#[test]
fn a_design_with_no_requirements_makes_no_certainty_claim() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    let r = g.graph_report().unwrap();
    assert!(r.requirement_certainty.is_none());
    assert!(!r.to_markdown().contains("Requirement certainty"));
}
