//! The as-fielded reconcile (BL-9): does what is *running* match what the
//! design declares? Pins the three divergence kinds, the library-plugin
//! guard (components never produce fielded drift), the persistent-gap loop
//! (event → unresolved_drift → agreement resolves), and the honest edges:
//! unknown ids reported, partial observations never read as absence.

use reflow2_core::detect::GapSource;
use reflow2_core::fielded::{FieldedDriftKind, FieldedOptions, ObservedEnvironment};
use reflow2_core::graph::DesignGraph;

fn deployed_world() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.add_release("rel:v1", "v1", Some("1.0"), None)
        .expect("rel");
    g.add_release("rel:v2", "v2", Some("2.0"), None)
        .expect("rel");
    g.add_environment("env:prod", "Production", Some("production"), None)
        .expect("env");
    g.add_environment("env:lab", "Lab", Some("lab"), None)
        .expect("env");
    g
}

fn obs(env: &str, running: &[&str]) -> ObservedEnvironment {
    ObservedEnvironment {
        environment_id: env.to_string(),
        running: running.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn an_agreeing_deployment_is_not_drift() {
    let mut g = deployed_world();
    g.deploy_to("rel:v1", "env:prod", Some("active"))
        .expect("deploy");
    let r = g
        .reconcile_deployment(&[obs("env:prod", &["rel:v1"])], &FieldedOptions::default())
        .expect("reconcile");
    assert!(r.findings.is_empty(), "{:?}", r.findings);
    assert_eq!(r.agreements, 1);
}

#[test]
fn declared_active_but_not_running_is_deployment_missing() {
    let mut g = deployed_world();
    g.deploy_to("rel:v1", "env:prod", Some("active"))
        .expect("deploy");
    let r = g
        .reconcile_deployment(&[obs("env:prod", &[])], &FieldedOptions::default())
        .expect("reconcile");
    assert_eq!(r.findings.len(), 1);
    assert_eq!(r.findings[0].kind, FieldedDriftKind::DeploymentMissing);
    assert_eq!(r.propagation_seeds, vec!["env:prod", "rel:v1"]);
}

#[test]
fn running_but_never_declared_is_deployment_undeclared() {
    let mut g = deployed_world();
    let r = g
        .reconcile_deployment(&[obs("env:prod", &["rel:v2"])], &FieldedOptions::default())
        .expect("reconcile");
    assert_eq!(r.findings.len(), 1);
    assert_eq!(r.findings[0].kind, FieldedDriftKind::DeploymentUndeclared);
}

#[test]
fn running_while_declared_rolled_back_is_contradicted() {
    let mut g = deployed_world();
    g.deploy_to("rel:v1", "env:prod", Some("rolled_back"))
        .expect("deploy");
    let r = g
        .reconcile_deployment(&[obs("env:prod", &["rel:v1"])], &FieldedOptions::default())
        .expect("reconcile");
    assert_eq!(r.findings.len(), 1);
    assert_eq!(r.findings[0].kind, FieldedDriftKind::DeploymentContradicted);
    assert!(r.findings[0].message.contains("rolled_back"));
}

/// The library-plugin guard, by construction: a design full of components
/// that never "run" produces zero fielded drift — only declarations are
/// compared, so a part shipping *inside* a release is invisible here. The
/// original reflow expected every component to appear as a running thing and
/// manufactured false drift for every library (reflow-audit.md).
#[test]
fn components_and_libraries_never_produce_fielded_drift() {
    let mut g = deployed_world();
    for i in 0..5 {
        g.add_component(
            &format!("cmp:lib{i}"),
            &format!("Library {i}"),
            "a library",
            None,
        )
        .expect("cmp");
    }
    g.deploy_to("rel:v1", "env:prod", Some("active"))
        .expect("deploy");
    let r = g
        .reconcile_deployment(&[obs("env:prod", &["rel:v1"])], &FieldedOptions::default())
        .expect("reconcile");
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn unknown_ids_are_reported_not_skipped() {
    let mut g = deployed_world();
    let r = g
        .reconcile_deployment(
            &[
                obs("env:ghost", &["rel:v1"]),
                obs("env:prod", &["rel:ghost"]),
            ],
            &FieldedOptions::default(),
        )
        .expect("reconcile");
    assert_eq!(r.unknown_ids, vec!["env:ghost", "rel:ghost"]);
}

#[test]
fn a_partial_observation_is_not_evidence_of_absence() {
    let mut g = deployed_world();
    g.deploy_to("rel:v1", "env:prod", Some("active"))
        .expect("deploy");
    // Only the lab was looked at; prod's deployment must not become drift.
    let r = g
        .reconcile_deployment(&[obs("env:lab", &[])], &FieldedOptions::default())
        .expect("reconcile");
    assert!(r.findings.is_empty());
    assert!(
        r.unobserved.is_empty(),
        "not exhaustive: silence about prod"
    );

    let r = g
        .reconcile_deployment(
            &[obs("env:lab", &[])],
            &FieldedOptions {
                exhaustive: true,
                ..Default::default()
            },
        )
        .expect("reconcile");
    assert_eq!(r.unobserved, vec!["rel:v1 in env:prod"]);
}

/// The full loop: divergence → recorded event → persistent gap → the human
/// fixes the declaration → the next observation agrees → the event resolves
/// and the gap closes. The observation is the authority on the fielded side.
#[test]
fn the_divergence_is_a_persistent_gap_until_reality_and_declaration_agree() {
    let mut g = deployed_world();
    g.deploy_to("rel:v1", "env:prod", Some("active"))
        .expect("deploy");
    let r = g
        .reconcile_deployment(
            &[obs("env:prod", &[])],
            &FieldedOptions {
                record_events: true,
                detected_at: Some("2026-07-19T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .expect("reconcile");
    assert_eq!(r.recorded_events.len(), 1);

    let gap = |g: &DesignGraph| {
        g.detect_gaps()
            .expect("gaps")
            .iter()
            .any(|c| c.gap_source == GapSource::UnresolvedDrift)
    };
    assert!(
        gap(&g),
        "a recorded fielded divergence must nag until answered"
    );

    // Re-observing the same divergence is the same event, not a new one.
    let r2 = g
        .reconcile_deployment(
            &[obs("env:prod", &[])],
            &FieldedOptions {
                record_events: true,
                ..Default::default()
            },
        )
        .expect("reconcile");
    assert_eq!(r2.recorded_events, r.recorded_events);

    // The human answers on the design side: the rollout was actually rolled
    // back. Declaration now matches reality; the next reconcile resolves.
    g.deploy_to("rel:v1", "env:prod", Some("rolled_back"))
        .expect("deploy");
    let r3 = g
        .reconcile_deployment(&[obs("env:prod", &[])], &FieldedOptions::default())
        .expect("reconcile");
    assert_eq!(r3.resolved_events, r.recorded_events);
    assert!(!gap(&g), "an answered divergence must stop nagging");
}

#[test]
fn resolution_only_comes_from_an_observation_of_that_environment() {
    let mut g = deployed_world();
    g.deploy_to("rel:v1", "env:prod", Some("active"))
        .expect("deploy");
    g.reconcile_deployment(
        &[obs("env:prod", &[])],
        &FieldedOptions {
            record_events: true,
            ..Default::default()
        },
    )
    .expect("reconcile");
    // Observing only the lab says nothing about prod's open divergence.
    let r = g
        .reconcile_deployment(&[obs("env:lab", &[])], &FieldedOptions::default())
        .expect("reconcile");
    assert!(r.resolved_events.is_empty(), "no evidence, no resolution");
}
