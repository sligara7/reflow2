//! GENESIS — bootstrapping a design graph from a brief (SP-5).

use reflow2_core::nodes::node;
use reflow2_core::{DesignGraph, GENESIS_EPOCH_ID, GenesisOptions};

fn opts(project_id: &str) -> GenesisOptions {
    GenesisOptions {
        project_id: project_id.to_string(),
        name: "Softball Game".to_string(),
        domain: Some("software".to_string()),
        objective: Some("A fun, physics-real softball game.".to_string()),
        mode: Some("rigid".to_string()),
        rescan: false,
    }
}

#[test]
fn genesis_scaffolds_project_and_epoch() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g.genesis(opts("proj:sb")).expect("genesis");

    assert!(report.created);
    assert!(!report.already_initialized);
    assert_eq!(report.project_id, "proj:sb");
    assert_eq!(report.epoch_id, GENESIS_EPOCH_ID);
    assert_eq!(report.project_mode, "rigid", "mode option is applied");
    assert!(
        !report.next_steps.is_empty(),
        "the agent gets a next-steps checklist"
    );

    // Project exists with the given props; genesis Epoch is planted.
    let project = g
        .get_node(node::PROJECT, "proj:sb")
        .unwrap()
        .expect("Project created");
    assert_eq!(project.properties["name"].as_str(), Some("Softball Game"));
    assert_eq!(project.properties["domain"].as_str(), Some("software"));
    assert_eq!(project.properties["mode"].as_str(), Some("rigid"));
    assert!(
        g.get_node(node::DESIGN_EPOCH, GENESIS_EPOCH_ID)
            .unwrap()
            .is_some(),
        "the genesis epoch anchors the timeline"
    );
}

#[test]
fn genesis_is_guarded_and_idempotent() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.genesis(opts("proj:sb")).unwrap();

    // A second genesis without rescan must NOT clobber or duplicate.
    let again = g.genesis(opts("proj:other")).expect("second genesis");
    assert!(again.already_initialized);
    assert!(!again.created);
    assert_eq!(
        g.count_nodes(node::PROJECT).unwrap(),
        1,
        "no duplicate Project — the guard held"
    );
    // The original project's mode is reported back.
    assert_eq!(again.project_mode, "rigid");
}

#[test]
fn genesis_defaults_mode_to_flexible_when_unset() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .genesis(GenesisOptions {
            project_id: "proj:x".to_string(),
            name: "X".to_string(),
            domain: None,
            objective: None,
            mode: None,
            rescan: false,
        })
        .unwrap();
    assert_eq!(
        report.project_mode, "flexible",
        "schema default applies when mode is unset"
    );
}
