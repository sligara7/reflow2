//! Flows — modelling a *process* (BL-37).
//!
//! `Flow` was fully specified in the schema with no write side, found by
//! modelling reflow2's own coherence loop. These tests pin the three decisions
//! that closed it: a flow is creatable and readable end to end; a process's
//! cycles are **reported, never judged** (`circular_dependency` stays scoped
//! to `DEPENDS_ON` and contracts); and a Flow counts as *structure*, so the
//! phase nudge no longer tells a fully-structured process it has no design.

use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;
use reflow2_core::heal::HealCategory;
use reflow2_core::nodes::{Props, edge, node};

fn process_with(steps: &[(&str, &str, Option<i64>)]) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "The loop").expect("project");
    g.add_flow(
        "flow:loop",
        "The loop",
        Some("How the phases feed each other"),
        Some("process"),
        None,
        None,
    )
    .expect("flow");
    for (id, name, order) in steps {
        g.add_capability(id, name, "a phase", None).expect("cap");
        g.part_of_flow(id, "flow:loop", *order).expect("member");
    }
    g
}

fn triggers(g: &mut DesignGraph, from: &str, to: &str, role: Option<&str>) {
    g.create_edge(
        edge::TRIGGERS,
        node::CAPABILITY,
        from,
        node::CAPABILITY,
        to,
        Props::new().set_opt("role", role),
    )
    .expect("triggers");
}

#[test]
fn a_flow_is_creatable_and_reads_back_in_step_order() {
    let g = process_with(&[
        ("cap:verify", "Verify", Some(2)),
        ("cap:build", "Build", Some(1)),
        ("cap:operate", "Operate", Some(3)),
    ]);
    let rep = g.flow_report("flow:loop").expect("report");
    assert_eq!(rep.flow_name, "The loop");
    assert_eq!(rep.flow_type.as_deref(), Some("process"));
    let ids: Vec<&str> = rep.steps.iter().map(|s| s.capability_id.as_str()).collect();
    assert_eq!(ids, ["cap:build", "cap:verify", "cap:operate"]);
    assert!(
        rep.confessions.is_empty(),
        "a fully-stated flow has nothing to confess: {:?}",
        rep.confessions
    );
}

#[test]
fn steps_without_order_sort_last_and_are_confessed() {
    let g = process_with(&[("cap:b", "B", None), ("cap:a", "A", Some(1))]);
    let rep = g.flow_report("flow:loop").expect("report");
    let ids: Vec<&str> = rep.steps.iter().map(|s| s.capability_id.as_str()).collect();
    assert_eq!(ids, ["cap:a", "cap:b"]);
    assert!(
        rep.confessions.iter().any(|c| c.contains("step_order")),
        "the unstated order must be confessed, not silently invented: {:?}",
        rep.confessions
    );
}

#[test]
fn transitions_carry_roles_and_a_missing_role_is_confessed() {
    let mut g = process_with(&[("cap:a", "A", Some(1)), ("cap:b", "B", Some(2))]);
    triggers(&mut g, "cap:a", "cap:b", Some("feeds"));
    triggers(&mut g, "cap:b", "cap:a", None);
    let rep = g.flow_report("flow:loop").expect("report");
    assert_eq!(rep.transitions.len(), 2);
    let forward = rep
        .transitions
        .iter()
        .find(|t| t.from_id == "cap:a")
        .expect("forward");
    assert_eq!(forward.role.as_deref(), Some("feeds"));
    assert!(
        rep.confessions.iter().any(|c| c.contains("role")),
        "an unroled transition is the load-bearing ambiguity and must be confessed: {:?}",
        rep.confessions
    );
}

/// The decision itself: cycles over TRIGGERS are the process's design —
/// visible in the flow report, absent from the defect list. A DEPENDS_ON
/// cycle stays a defect (pinned in `cycles.rs`); this pins the other half.
#[test]
fn process_cycles_are_reported_never_judged() {
    let mut g = process_with(&[
        ("cap:build", "Build", Some(1)),
        ("cap:verify", "Verify", Some(2)),
    ]);
    triggers(&mut g, "cap:build", "cap:verify", Some("feeds"));
    triggers(&mut g, "cap:verify", "cap:build", Some("forces resync"));

    let rep = g.flow_report("flow:loop").expect("report");
    assert_eq!(rep.cycles.len(), 1);
    assert_eq!(rep.cycles[0].members, ["cap:build", "cap:verify"]);
    assert_eq!(rep.cycles[0].path, ["cap:build", "cap:verify"]);

    let defects = g.detect_defects().expect("defects");
    assert!(
        !defects
            .iter()
            .any(|d| d.category == HealCategory::CircularDependency),
        "a TRIGGERS cycle is a process, not a circular dependency"
    );
}

#[test]
fn a_self_trigger_is_a_degenerate_cycle_not_a_silent_drop() {
    let mut g = process_with(&[("cap:a", "A", Some(1))]);
    triggers(&mut g, "cap:a", "cap:a", Some("retries"));
    let rep = g.flow_report("flow:loop").expect("report");
    assert_eq!(rep.cycles.len(), 1);
    assert_eq!(rep.cycles[0].members, ["cap:a"]);
    assert_eq!(rep.cycles[0].path, ["cap:a"]);
}

#[test]
fn an_entry_point_matching_no_member_is_confessed() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.add_flow("flow:f", "F", None, None, Some("cap:missing"), None)
        .expect("flow");
    g.add_capability("cap:a", "A", "step", None).expect("cap");
    g.part_of_flow("cap:a", "flow:f", Some(1)).expect("member");
    let rep = g.flow_report("flow:f").expect("report");
    assert!(
        rep.confessions
            .iter()
            .any(|c| c.contains("entry_point") && c.contains("cap:missing")),
        "an entry point naming nothing in the flow is a model gap: {:?}",
        rep.confessions
    );
}

#[test]
fn an_entry_point_may_name_the_capability_by_name() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.add_flow("flow:f", "F", None, None, Some("Start here"), None)
        .expect("flow");
    g.add_capability("cap:a", "Start here", "step", None)
        .expect("cap");
    g.part_of_flow("cap:a", "flow:f", Some(1)).expect("member");
    let rep = g.flow_report("flow:f").expect("report");
    assert!(rep.confessions.is_empty(), "{:?}", rep.confessions);
}

/// A Flow counts as structure: the phase nudge that asks "how is this
/// structured into buildable parts?" is answered, for a process, by the flow
/// its capabilities form — a process never grows Components at all.
#[test]
fn a_flow_counts_as_structure_for_the_phase_nudge() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "The loop").expect("project");
    g.add_requirement("req:r", "R", "It must loop")
        .expect("req");
    g.add_capability("cap:a", "A", "a phase", None)
        .expect("cap");

    let fires = |g: &DesignGraph| {
        g.detect_gaps()
            .expect("gaps")
            .iter()
            .any(|c| c.gap_source == GapSource::ConceptWithoutDesign)
    };
    assert!(fires(&g), "no components, no flows: the nudge is right");

    let mut g2 = g;
    g2.add_flow("flow:loop", "The loop", None, None, None, None)
        .expect("flow");
    g2.part_of_flow("cap:a", "flow:loop", Some(1))
        .expect("member");
    assert!(
        !fires(&g2),
        "a process structured as a flow is not 'concept without design'"
    );
}

/// A step of a process is anchored by its Flow, and a capability attached to
/// nothing is not.
///
/// The question moved in BL-42: HEAL used to report this as an `orphan_node`
/// *as well as* DETECT reporting `unallocated_capability`, the same finding
/// twice. It is now asked once, by DETECT — which also means the gate there
/// had to learn that a Flow is structure, or a loose capability on a
/// flow-only graph would have gone silent entirely.
#[test]
fn a_flow_member_has_a_home_and_a_loose_capability_is_asked_about_once() {
    let mut g = process_with(&[("cap:step", "Step", Some(1))]);
    g.add_capability("cap:loose", "Loose", "attached to nothing", None)
        .expect("cap");

    let asked: Vec<String> = g
        .detect_gaps()
        .expect("gaps")
        .into_iter()
        .filter(|c| c.gap_source == GapSource::UnallocatedCapability)
        .flat_map(|c| c.affected_ids)
        .collect();
    assert_eq!(
        asked,
        ["cap:loose"],
        "the flow member has a home; the loose one is asked about"
    );

    let orphans: Vec<String> = g
        .detect_defects()
        .expect("defects")
        .into_iter()
        .filter(|d| d.category == HealCategory::OrphanNode)
        .flat_map(|d| d.affected_ids)
        .collect();
    assert!(
        orphans.is_empty(),
        "and never reported a second time as a defect: {orphans:?}"
    );
}

#[test]
fn a_missing_flow_fails_loud() {
    let g = DesignGraph::open_in_memory().expect("open");
    assert!(g.flow_report("flow:nope").is_err());
}

/// F7, from the storyflow trial: a representative walk can omit the very
/// step that made the process a loop. The cluster is the honest answer to
/// "what is caught in this?", and it is reported separately from the walk.
#[test]
fn a_cycle_reports_every_member_not_just_one_walk_through_it() {
    // a → b → c → a, and a shortcut b → a. The shortcut is a valid 2-step
    // cycle, so a representative walk may return {a, b} and never mention c
    // — which on a real model was the hand-off to the human.
    let mut g = process_with(&[
        ("cap:a", "A", Some(1)),
        ("cap:b", "B", Some(2)),
        ("cap:c", "C", Some(3)),
    ]);
    triggers(&mut g, "cap:a", "cap:b", Some("feeds"));
    triggers(&mut g, "cap:b", "cap:c", Some("feeds"));
    triggers(&mut g, "cap:c", "cap:a", Some("returns"));
    triggers(&mut g, "cap:b", "cap:a", Some("returns early"));

    let rep = g.flow_report("flow:loop").expect("report");
    assert_eq!(rep.cycles.len(), 1, "one cluster");
    assert_eq!(
        rep.cycles[0].members,
        ["cap:a", "cap:b", "cap:c"],
        "every step caught in the loop is named, whatever walk was chosen"
    );
    assert!(
        rep.cycles[0].path.len() >= 2
            && rep.cycles[0]
                .path
                .iter()
                .all(|s| rep.cycles[0].members.contains(s)),
        "the walk is a real walk inside the cluster: {:?}",
        rep.cycles[0].path
    );
}
