//! What a defect sweep COULD have found, as distinct from what it did.
//!
//! `SweepScope` already refused to let an empty result stand for a whole empty
//! graph. These pin the two finer cases, and the second is the one that cost
//! real time.
//!
//! 🛑 THE MEASURED FAILURE (reflow2's own design, 2026-08-20).
//! `circular_dependency` walked 182 dependency pairs and found nothing — a
//! large, healthy-looking population. **Zero of those pairs joined two
//! subsystem-level components.** So "no circular dependencies" was true of the
//! flattened network and said nothing whatever about the subsystems, while
//! reading exactly like a clean bill of health for them. Two real subsystem
//! cycles were found with a hand-written Python script, not with reflow2.
//!
//! ⭐ A PER-RULE POPULATION WOULD NOT HAVE CAUGHT IT, which is why there are two
//! mechanisms here and not one. The population was fine. What was empty was the
//! SUB-POPULATION the reader cared about. That is the general shape for any
//! project that declares a hierarchy: the answer is computed over a flattened
//! graph, and the reader assumes it holds at the level they asked about.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn graph_with_two_subsystems() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    for (id, name) in [("sys:a", "A"), ("sys:b", "B")] {
        g.add_component(id, name, "a subsystem", Some("subsystem"))
            .expect("subsystem");
    }
    for (id, name) in [("cmp:x", "X"), ("cmp:y", "Y")] {
        g.add_component(id, name, "a part", Some("component"))
            .expect("component");
    }
    g.contain_component("sys:a", "cmp:x").expect("nest");
    g.contain_component("sys:b", "cmp:y").expect("nest");
    g
}

fn depends(g: &mut DesignGraph, from: &str, to: &str) {
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        from,
        node::COMPONENT,
        to,
        Props::new(),
    )
    .expect("depends_on");
}

#[test]
fn a_level_with_components_but_no_coupling_is_named_as_silent_not_clean() {
    // THE CASE THAT COST TWO DAYS. The components below are coupled, so the
    // topology rules have a real population and report cleanly — while the
    // subsystem level they sit under has no coupling at all.
    let mut g = graph_with_two_subsystems();
    depends(&mut g, "cmp:x", "cmp:y");

    let sweep = g.detect_defects().expect("sweep");

    let sub = sweep
        .swept
        .coupling_by_level
        .iter()
        .find(|l| l.level == "subsystem")
        .expect("the subsystem level is reported");
    assert_eq!(sub.components, 2);
    assert_eq!(sub.coupled_pairs, 0);

    let note = sweep
        .swept
        .coverage_note
        .expect("a level with components and no coupling must be named");
    assert!(note.contains("subsystem"), "{note}");
    // The wording is the point: SILENT about that level, not CLEAN about it.
    assert!(note.contains("silent about that level"), "{note}");
}

#[test]
fn a_level_that_is_actually_coupled_is_not_named() {
    let mut g = graph_with_two_subsystems();
    depends(&mut g, "cmp:x", "cmp:y");
    depends(&mut g, "sys:a", "sys:b");

    let sweep = g.detect_defects().expect("sweep");

    let sub = sweep
        .swept
        .coupling_by_level
        .iter()
        .find(|l| l.level == "subsystem")
        .expect("present");
    assert_eq!(sub.coupled_pairs, 1);
    let note = sweep.swept.coverage_note.unwrap_or_default();
    assert!(
        !note.contains("subsystem level has"),
        "a coupled level must not be reported as silent: {note}"
    );
}

#[test]
fn a_level_holding_one_component_is_never_called_blind() {
    // A level with a single component CANNOT have a pair joining two components
    // at it. That is arithmetic, not a modelling gap, and naming it would be
    // the false positive that teaches a reader to skip this line — which is how
    // a signal dies.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.add_component("sys:only", "Only", "the one system", Some("system"))
        .expect("system");
    g.add_component("cmp:a", "A", "a part", Some("component"))
        .expect("component");
    g.add_component("cmp:b", "B", "another", Some("component"))
        .expect("component");
    depends(&mut g, "cmp:a", "cmp:b");

    let sweep = g.detect_defects().expect("sweep");

    let system = sweep
        .swept
        .coupling_by_level
        .iter()
        .find(|l| l.level == "system")
        .expect("present");
    assert_eq!(system.components, 1);
    assert_eq!(system.coupled_pairs, 0);
    let note = sweep.swept.coverage_note.unwrap_or_default();
    assert!(!note.contains("system level has"), "{note}");
}

#[test]
fn a_rule_that_walked_nothing_is_named_and_a_rule_with_a_population_is_not() {
    // The simpler half: a rule whose population is empty reports clean for the
    // same reason an empty graph does.
    let mut g = graph_with_two_subsystems();
    depends(&mut g, "cmp:x", "cmp:y");
    depends(&mut g, "sys:a", "sys:b");

    let sweep = g.detect_defects().expect("sweep");
    let pop = |rule: &str| {
        sweep
            .swept
            .rule_populations
            .iter()
            .find(|p| p.rule == rule)
            .unwrap_or_else(|| panic!("{rule} is reported"))
    };

    // Nothing DUPLICATES anything here, so that rule had nothing to walk.
    assert_eq!(pop("duplicate").examined, 0);
    // While the cycle rule has real pairs to walk.
    assert!(pop("circular_dependency").examined > 0);

    let note = sweep
        .swept
        .coverage_note
        .expect("the starved rule is named");
    assert!(note.contains("duplicate"), "{note}");
    assert!(note.contains("NOTHING TO EXAMINE"), "{note}");
    assert!(
        !note.contains("circular_dependency"),
        "a rule with a population must not be named: {note}"
    );
}

#[test]
fn a_population_is_the_same_number_the_rule_walks() {
    // Deriving these independently would be a second implementation of one
    // number, able to disagree with the first. Adding a real edge must move the
    // reported population, or the report is decorative.
    let mut g = graph_with_two_subsystems();
    let before = g
        .detect_defects()
        .expect("sweep")
        .swept
        .rule_populations
        .iter()
        .find(|p| p.rule == "circular_dependency")
        .expect("present")
        .examined;

    depends(&mut g, "cmp:x", "cmp:y");

    let after = g
        .detect_defects()
        .expect("sweep")
        .swept
        .rule_populations
        .iter()
        .find(|p| p.rule == "circular_dependency")
        .expect("present")
        .examined;

    assert_eq!(
        after,
        before + 1,
        "the reported population must track reality"
    );
}

#[test]
fn every_rule_is_reported_so_none_can_go_silently_uncounted() {
    // A rule missing from this list would be a rule whose coverage nobody can
    // see — the exact absence the whole sweep-scope idea exists to end.
    let g = graph_with_two_subsystems();
    let sweep = g.detect_defects().expect("sweep");
    let reported: Vec<&str> = sweep
        .swept
        .rule_populations
        .iter()
        .map(|p| p.rule)
        .collect();
    for rule in &sweep.swept.rules {
        assert!(
            reported.contains(rule),
            "rule '{rule}' runs but its population is not reported"
        );
    }
    assert_eq!(reported.len(), sweep.swept.rules.len());
}
