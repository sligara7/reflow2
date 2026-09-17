//! `orphan_level` names the cure, not only the condition.
//!
//! Root cause (xrt-demo F11, measured on its live graph 2026-09-16): a single
//! indivisible top-level part declared `system` raised "not contained by
//! anything above it and contains nothing below it"; declared `component` it
//! raised level_spine_disagreement instead; the reporter concluded "there is
//! no third option". The third option existed — both rules accept a Project
//! parent — and neither message said so, so the agent turned the only knob the
//! message mentioned (`level`) and found both settings wrong
//! (`fact:root-cause-a-childless-root-part-has-no-legal-level-…`).
//!
//! Written before the fix and observed failing on the message assertion.

use reflow2_core::DesignGraph;
use reflow2_core::HierarchyIssueKind;

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:demo", "xrt-demo").unwrap();
    g.add_component(
        "cmp:beamline",
        "The beamline",
        "the designed thing",
        Some("system"),
    )
    .unwrap();
    g.add_component("cmp:dcm", "DCM", "selects the energy", Some("subsystem"))
        .unwrap();
    g.contain_component("cmp:beamline", "cmp:dcm").unwrap();
    g.contains("prj:demo", "Component", "cmp:beamline").unwrap();
    // One indivisible top-level part: no children, and — the reporter's
    // state — not contained by the Project.
    g.add_component(
        "cmp:agent-seam",
        "The agent — the seam between the two servers",
        "holds both MCP servers at once",
        Some("system"),
    )
    .unwrap();
    g
}

#[test]
fn the_message_names_the_project_as_the_thing_that_can_contain_it() {
    let g = design();
    let issues = g.hierarchy_issues().unwrap();
    let orphan = issues
        .iter()
        .find(|i| i.kind == HierarchyIssueKind::OrphanLevel)
        .expect("the uncontained root part is reported");
    assert_eq!(orphan.components, ["cmp:agent-seam"]);
    assert!(
        orphan.message.contains("contain it under the Project"),
        "the finding says what to do, not only what is wrong: {}",
        orphan.message
    );
}

#[test]
fn contained_by_the_project_it_is_a_legal_top_level_part_at_either_level() {
    let mut g = design();
    g.contains("prj:demo", "Component", "cmp:agent-seam")
        .unwrap();
    assert!(
        g.hierarchy_issues().unwrap().is_empty(),
        "a Project parent satisfies both rules: {:?}",
        g.hierarchy_issues().unwrap()
    );
}
