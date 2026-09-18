//! `convention_delivered_nowhere` and `agent_instructions_unregistered` — the
//! two absences behind a repository's procedural know-how
//! (`req:a-repos-procedural-know-how-is-delivered-at-the-step-it-bears-on-and-the-design-knows-the-adapter-exists`).
//!
//! Alex, 2026-09-17: after served skills what remains in a repo is "a thin
//! adapter, not a competing playbook." The delivery leg already existed —
//! a DesignRule's `steps` ride get_skill and the tool list — so these two
//! findings are the noticing leg: a convention nobody delivers, and an adapter
//! the design does not know about. Both silent with nothing to run on.

use reflow2_core::foundation::core::Value;
use reflow2_core::nodes::{Props, node};
use reflow2_core::{DesignGraph, GapCandidate, GapSource};

fn find(g: &DesignGraph, s: GapSource) -> Option<GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == s)
}

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g
}

#[test]
fn a_convention_with_no_steps_is_named_and_one_with_steps_is_delivered() {
    let mut g = design();
    assert!(
        find(&g, GapSource::ConventionDeliveredNowhere).is_none(),
        "no conventions, nothing to run on"
    );
    g.add_design_rule(
        "rule:imports-resolve-through-src",
        "Imports resolve through src/",
        "Every import is rooted at src/; the test runner adds it to the path.",
        Some("convention"),
        Some(false),
    )
    .unwrap();
    let gap = find(&g, GapSource::ConventionDeliveredNowhere).expect("findable, delivered nowhere");
    assert_eq!(
        gap.affected_ids,
        vec!["rule:imports-resolve-through-src".to_string()]
    );
    assert!(gap.title.contains("1 of 1"), "{}", gap.title);
    assert!(
        gap.description.contains("add_design_rule"),
        "{}",
        gap.description
    );

    // A methodology rule without steps is not a convention and is not asked.
    g.add_design_rule(
        "rule:m",
        "Method",
        "A way of working.",
        Some("methodology"),
        None,
    )
    .unwrap();
    assert_eq!(
        find(&g, GapSource::ConventionDeliveredNowhere)
            .unwrap()
            .affected_ids
            .len(),
        1
    );

    // Steps set: delivered.
    let stored = g
        .get_node(node::DESIGN_RULE, "rule:imports-resolve-through-src")
        .unwrap()
        .unwrap();
    let mut props = Props::new();
    for (k, v) in &stored.properties {
        props = props.set(k, v.clone());
    }
    props = props.set(
        "steps",
        Value::List(vec![Value::from("adopt"), Value::from("link-artifacts")]),
    );
    g.create_node(node::DESIGN_RULE, "rule:imports-resolve-through-src", props)
        .unwrap();
    assert!(find(&g, GapSource::ConventionDeliveredNowhere).is_none());
}

#[test]
fn an_unregistered_adapter_is_named_once_the_design_is_real_and_a_documents_edge_answers_it() {
    let mut g = design();
    for i in 0..4 {
        g.add_artifact(
            &format!("art:{i}"),
            &format!("{i}.rs"),
            Some("code"),
            Some(&format!("src/{i}.rs")),
        )
        .unwrap();
    }
    assert!(
        find(&g, GapSource::AgentInstructionsUnregistered).is_none(),
        "four artifacts is not yet a design with an adapter to miss"
    );
    g.add_artifact("art:4", "4.rs", Some("code"), Some("src/4.rs"))
        .unwrap();
    let gap = find(&g, GapSource::AgentInstructionsUnregistered).expect("five is");
    assert_eq!(gap.affected_ids, vec!["proj:p".to_string()]);
    assert!(
        gap.description.contains("agent_instructions"),
        "{}",
        gap.description
    );

    // A readme documenting the project is not the adapter.
    g.add_artifact(
        "art:readme",
        "README.md",
        Some("document"),
        Some("README.md"),
    )
    .unwrap();
    g.create_edge(
        edge_documents(),
        node::ARTIFACT,
        "art:readme",
        node::PROJECT,
        "proj:p",
        Props::new().set("doc_kind", "readme"),
    )
    .unwrap();
    assert!(find(&g, GapSource::AgentInstructionsUnregistered).is_some());

    g.add_artifact(
        "art:agents",
        "AGENTS.md",
        Some("document"),
        Some("AGENTS.md"),
    )
    .unwrap();
    g.create_edge(
        edge_documents(),
        node::ARTIFACT,
        "art:agents",
        node::PROJECT,
        "proj:p",
        Props::new().set("doc_kind", "agent_instructions"),
    )
    .unwrap();
    assert!(
        find(&g, GapSource::AgentInstructionsUnregistered).is_none(),
        "the design knows its adapter"
    );
}

fn edge_documents() -> &'static str {
    reflow2_core::nodes::edge::DOCUMENTS
}
