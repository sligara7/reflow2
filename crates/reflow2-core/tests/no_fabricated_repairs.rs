//! A repair suggestion never proposes fabricating a relationship.
//!
//! `req:a-repair-suggestion-never-proposes-fabrication` (accepted 2026-08-10).
//! Raised by dev_storyflow's storybug Boss and characterised by api-boss as gap
//! D, 2026-08-09, and reproduced in reflow2's own design the same day at five
//! nodes and again at seven.
//!
//! THE CONCRETE CASE. `unthreaded_cluster` used to report a cluster with
//! `suggested_fix_type: generate_bridge` — create edges until it is connected.
//! Where the separation is CORRECT, following that advice fabricates
//! relationships nobody stated in order to silence a warning about a separation
//! that is right. In their words: *"that is manufacturing connectivity, and it
//! is the same act as formatting coverage into existence — with the difference
//! that here the tool proposes it."*
//!
//! ## The line, and it is narrow
//!
//! The requirement does NOT ask the detectors to stop firing, and does NOT ask
//! HEAL to stop proposing repairs. **A suggestion may reorganise or restore; it
//! may never assert a relationship nobody stated.** So the counterweight below
//! matters as much as the three cases above it: `break_cycle`, `merge` and
//! `generate_decision` must keep their suggestions, or this change has quietly
//! turned a narrow rule into a general refusal to help.
//!
//! ## Why it is `None` plus a sentence, rather than a different fix type
//!
//! An empty string or a `no_fix` value would still be a field saying "here is
//! the repair", and the next reader would look for one. `None` says there is no
//! operation, and `repair_is_a_judgement` says in words what to do instead —
//! the same shape as `needs_a_human` in the changelog draft, where the graph
//! holds what moved and never what it costs.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory};

fn issue_of(g: &DesignGraph, cat: HealCategory) -> reflow2_core::HealIssue {
    g.open_defects()
        .unwrap()
        .into_iter()
        .find(|i| i.category == cat)
        .unwrap_or_else(|| panic!("expected a {cat:?} finding in the fixture"))
}

/// THE CASE THAT WAS FILED. An island must not be handed an operation that
/// would invent edges to make it stop being an island.
#[test]
fn a_disconnected_cluster_is_not_offered_a_bridge_to_fabricate() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    // A connected main body...
    g.add_requirement("req:main", "Main", "m").unwrap();
    g.add_capability("cap:main", "Main cap", "m", None).unwrap();
    g.add_component("cmp:main", "Main cmp", "m", None).unwrap();
    g.satisfies("cap:main", "req:main").unwrap();
    g.allocate("cap:main", "cmp:main").unwrap();
    // ...and a genuinely separate pair, coupled to each other and nothing else.
    g.add_requirement("req:far", "Far", "f").unwrap();
    g.add_capability("cap:far", "Far cap", "f", None).unwrap();
    g.satisfies("cap:far", "req:far").unwrap();

    let i = issue_of(&g, HealCategory::UnthreadedCluster);
    assert_eq!(
        i.suggested_fix_type, None,
        "no operation may be offered: connecting the cluster would assert \
         relationships nobody stated"
    );
    let why = i
        .repair_is_a_judgement
        .expect("silence is not enough — the finding must SAY why there is no operation");
    assert!(
        why.contains("judgement"),
        "the sentence must name it as a judgement, not merely decline: {why}"
    );
}

/// A component wired to nothing may be genuinely standalone. Wiring it to
/// something to quiet the finding invents a coupling.
#[test]
fn a_dead_end_component_is_not_offered_a_bridge_either() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_component("cmp:a", "A", "a", None).unwrap();
    g.add_component("cmp:b", "B", "b", None).unwrap();
    g.add_interface("ifc:x", "X").unwrap();
    g.provides("cmp:a", "ifc:x").unwrap();
    g.consumes("cmp:b", "ifc:x").unwrap();
    g.add_component("cmp:lonely", "Lonely", "connected to nothing", None)
        .unwrap();

    let i = issue_of(&g, HealCategory::DeadEnd);
    assert_eq!(i.suggested_fix_type, None);
    assert!(i.repair_is_a_judgement.is_some());
}

/// An orphan used to be offered `generate_owner` — an ownership link nobody
/// drew. A parked thought that correctly governs nothing yet is not a defect
/// with an operation attached.
#[test]
fn an_orphan_is_not_offered_a_link_nobody_drew() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_decision("dec:parked", "Parked", "an open thought", None)
        .unwrap();
    g.set_decision_status("dec:parked", "proposed").unwrap();

    let i = issue_of(&g, HealCategory::OrphanNode);
    assert_eq!(i.suggested_fix_type, None);
    assert!(i.repair_is_a_judgement.is_some());
}

/// THE COUNTERWEIGHT, and it carries as much weight as the three above. The
/// requirement is narrow: reorganising and restoring stay correct. If this ever
/// fails, a rule against INVENTION has become a refusal to help.
#[test]
fn honest_repairs_keep_their_suggestions() {
    // A cycle: breaking it removes an edge that exists. Nothing is invented.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    for id in ["cmp:a", "cmp:b"] {
        g.add_component(id, id, "c", None).unwrap();
    }
    for (a, b) in [("cmp:a", "cmp:b"), ("cmp:b", "cmp:a")] {
        g.create_edge(
            edge::DEPENDS_ON,
            node::COMPONENT,
            a,
            node::COMPONENT,
            b,
            Props::new(),
        )
        .unwrap();
    }
    assert_eq!(
        issue_of(&g, HealCategory::CircularDependency).suggested_fix_type,
        Some("break_cycle"),
        "removing an edge that EXISTS is not fabrication"
    );

    // A contradiction: writing the Decision that resolves it is a real act by a
    // person, not an assertion about what already relates to what.
    let mut h = DesignGraph::open_in_memory().unwrap();
    h.add_project("proj:x", "X").unwrap();
    h.add_requirement("req:a", "A", "a").unwrap();
    h.add_requirement("req:b", "B", "b").unwrap();
    h.create_edge(
        edge::CONTRADICTS,
        node::REQUIREMENT,
        "req:a",
        node::REQUIREMENT,
        "req:b",
        Props::new(),
    )
    .unwrap();
    assert_eq!(
        issue_of(&h, HealCategory::Contradiction).suggested_fix_type,
        Some("generate_decision"),
    );
    assert_eq!(
        issue_of(&h, HealCategory::Contradiction).repair_is_a_judgement,
        None,
        "a finding that HAS an honest operation must not also claim it needs a judgement"
    );
}
