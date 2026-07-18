//! Axis-Y matryoshka detectors — missing intermediate level, level mismatch,
//! orphan level (three-axes.md / chain_reflow / gap-surfacing GS-11).

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, GapSource, HierarchyIssueKind};

/// A Component at an explicit decomposition level.
fn comp(g: &mut DesignGraph, id: &str, level: &str) {
    g.create_node(
        node::COMPONENT,
        id,
        Props::new()
            .set("name", id)
            .set("purpose", "p")
            .set("level", level),
    )
    .unwrap();
}

fn kinds(g: &DesignGraph) -> Vec<HierarchyIssueKind> {
    g.hierarchy_issues()
        .unwrap()
        .iter()
        .map(|i| i.kind)
        .collect()
}

#[test]
fn a_system_containing_a_part_directly_is_a_missing_intermediate() {
    // system ▸ (subsystem skipped) ▸ component  — the carburetor-to-body problem.
    let mut g = DesignGraph::open_in_memory().unwrap();
    comp(&mut g, "cmp:sys", "system");
    comp(&mut g, "cmp:part", "component");
    g.contain_component("cmp:sys", "cmp:part").unwrap();

    let issues = g.hierarchy_issues().unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, HierarchyIssueKind::MissingIntermediateLevel);
    assert_eq!(issues[0].components, ["cmp:sys", "cmp:part"]);
}

#[test]
fn adjacent_levels_contain_cleanly() {
    // subsystem ▸ component — one level apart, no defect.
    let mut g = DesignGraph::open_in_memory().unwrap();
    comp(&mut g, "cmp:sub", "subsystem");
    comp(&mut g, "cmp:part", "component");
    g.contain_component("cmp:sub", "cmp:part").unwrap();

    assert!(g.hierarchy_issues().unwrap().is_empty());
}

#[test]
fn a_component_containing_a_system_is_a_level_mismatch() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    comp(&mut g, "cmp:part", "component");
    comp(&mut g, "cmp:sys", "system");
    g.contain_component("cmp:part", "cmp:sys").unwrap(); // inverted

    assert!(kinds(&g).contains(&HierarchyIssueKind::LevelMismatch));
}

#[test]
fn a_cross_two_level_dependency_is_a_missing_intermediate() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    comp(&mut g, "cmp:sys", "system");
    comp(&mut g, "cmp:part", "component");
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        "cmp:sys",
        node::COMPONENT,
        "cmp:part",
        Props::new(),
    )
    .unwrap();

    assert!(kinds(&g).contains(&HierarchyIssueKind::MissingIntermediateLevel));
}

#[test]
fn a_floating_subsystem_is_an_orphan_level() {
    // A subsystem with nothing above or below it on the spine.
    let mut g = DesignGraph::open_in_memory().unwrap();
    comp(&mut g, "cmp:sub", "subsystem");

    let issues = g.hierarchy_issues().unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, HierarchyIssueKind::OrphanLevel);
    assert_eq!(issues[0].components, ["cmp:sub"]);
}

#[test]
fn a_well_formed_three_level_spine_has_no_issues() {
    // system ▸ subsystem ▸ component, each one level apart.
    let mut g = DesignGraph::open_in_memory().unwrap();
    comp(&mut g, "cmp:sys", "system");
    comp(&mut g, "cmp:sub", "subsystem");
    comp(&mut g, "cmp:part", "component");
    g.contain_component("cmp:sys", "cmp:sub").unwrap();
    g.contain_component("cmp:sub", "cmp:part").unwrap();

    assert!(g.hierarchy_issues().unwrap().is_empty());
}

#[test]
fn hierarchy_defects_surface_as_detect_gaps() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    comp(&mut g, "cmp:sys", "system");
    comp(&mut g, "cmp:part", "component");
    g.contain_component("cmp:sys", "cmp:part").unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(
        gaps.iter()
            .any(|x| x.gap_source == GapSource::MissingIntermediateLevel
                && x.affected_ids == ["cmp:sys", "cmp:part"]),
        "the missing intermediate should surface as a gap"
    );
}

/// BL-24. The shape the tools lead you to — a Project holding a couple of
/// subsystems — used to report one `orphan_level` per subsystem, because the
/// check only recognised a Component parent and the Project carries no level.
/// Modelling reflow2's own design (two crates under one Project) produced two
/// false gaps this way.
#[test]
fn a_subsystem_the_project_contains_is_anchored_not_floating() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    comp(&mut g, "cmp:core", "subsystem");
    comp(&mut g, "cmp:surface", "subsystem");
    g.contains("proj:p", node::COMPONENT, "cmp:core").unwrap();
    g.contains("proj:p", node::COMPONENT, "cmp:surface")
        .unwrap();

    assert!(
        kinds(&g).is_empty(),
        "a Project is the root of the spine — what it contains has a parent, got {:?}",
        g.hierarchy_issues().unwrap()
    );
}

/// The other half: the detector must still catch a genuinely floating part.
/// Anchoring on the Project cannot become "nothing is ever an orphan".
#[test]
fn a_subsystem_nothing_contains_is_still_an_orphan() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    comp(&mut g, "cmp:anchored", "subsystem");
    comp(&mut g, "cmp:floating", "subsystem");
    g.contains("proj:p", node::COMPONENT, "cmp:anchored")
        .unwrap();
    // cmp:floating is contained by nothing at all.

    let issues = g.hierarchy_issues().unwrap();
    assert_eq!(issues.len(), 1, "exactly the floating one, got {issues:?}");
    assert_eq!(issues[0].kind, HierarchyIssueKind::OrphanLevel);
    assert_eq!(issues[0].components, ["cmp:floating"]);
}
