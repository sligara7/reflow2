//! Has the design this one DEPENDS ON moved since the declaration was made?
//!
//! The second check `req:design-dependencies-declared` names in its own
//! statement — *"declared-versus-upstream answers has what I depend on moved
//! since"* — and the half that was never built until 2026-08-26.
//!
//! These pin the JUDGEMENT, which lives in the core. What was found on disk is
//! supplied by the caller, exactly as `reconcile_dependencies` takes `observed`,
//! so every case here is expressible without touching a filesystem. The reading
//! half has its own test beside the served tool.

use reflow2_core::{DependencyDeclaration, DesignGraph, ObservedUpstream};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("in-memory graph")
}

/// A dependency that IS another reflow2 design, watched at a named export.
fn watched(hash: Option<&str>) -> DependencyDeclaration {
    DependencyDeclaration {
        id: "dep:sim".into(),
        name: "beamline-sim".into(),
        source: "https://github.com/example/beamline-sim.git".into(),
        version: "v2.1.0".into(),
        components: vec![],
        features: vec![],
        declared_in: Some("Cargo.toml".into()),
        graph_id: Some("beamline_sim".into()),
        design_export: Some("/somewhere/sim/docs/design/reflow2.json".into()),
        design_export_hash: hash.map(str::to_string),
        design_export_seen_at: Some("2026-08-20".into()),
        note: None,
    }
}

fn seen(state: &str, hash: Option<&str>, graph_id: Option<&str>) -> ObservedUpstream {
    ObservedUpstream {
        id: "dep:sim".into(),
        state: state.into(),
        content_hash: hash.map(str::to_string),
        graph_id: graph_id.map(str::to_string),
        nodes: Some(120),
    }
}

fn kinds(g: &DesignGraph, observed: &[ObservedUpstream]) -> Vec<String> {
    g.reconcile_upstream(observed)
        .expect("reconcile")
        .findings
        .iter()
        .map(|f| f.kind.to_string())
        .collect()
}

#[test]
fn an_upstream_that_has_not_moved_says_so_plainly() {
    let mut g = graph();
    g.declare_dependency(&watched(Some("sha256:aaa")))
        .expect("declare");
    let report = g
        .reconcile_upstream(&[seen("read", Some("sha256:aaa"), Some("beamline_sim"))])
        .expect("reconcile");
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, "unchanged");
    assert_eq!(report.watched, 1);
    assert!(!report.findings[0].is_actionable());
    // The date the baseline was taken rides along: an "unchanged" whose age
    // nobody can state is a weaker claim than it looks.
    assert!(report.findings[0].detail.contains("2026-08-20"));
}

#[test]
fn an_upstream_that_moved_is_reported_with_the_pin_it_is_declared_against() {
    let mut g = graph();
    g.declare_dependency(&watched(Some("sha256:aaa")))
        .expect("declare");
    let report = g
        .reconcile_upstream(&[seen("read", Some("sha256:bbb"), Some("beamline_sim"))])
        .expect("reconcile");
    assert_eq!(report.findings[0].kind, "moved");
    assert!(report.findings[0].is_actionable());
    // The version is in the sentence because that is the thing now in doubt:
    // the upstream moved, so whether the pin still describes it is unknown.
    assert!(report.findings[0].detail.contains("v2.1.0"));
    assert!(report.note.contains("1 of 1"));
}

/// 🛑 THE ONE THAT MATTERS MOST. A check that refreshed its own baseline would
/// report `moved` exactly once and then be permanently quiet — worse than no
/// check, because the silence reads as agreement. Nothing in the read path may
/// write, so the same movement is reported every time until somebody
/// re-declares.
#[test]
fn reading_never_updates_the_baseline_so_a_move_keeps_being_reported() {
    let mut g = graph();
    g.declare_dependency(&watched(Some("sha256:aaa")))
        .expect("declare");
    let moved = [seen("read", Some("sha256:bbb"), Some("beamline_sim"))];
    for _ in 0..3 {
        assert_eq!(kinds(&g, &moved), vec!["moved"]);
    }
    // And the stored baseline is untouched, not merely the report.
    let stored = g.declared_dependencies().expect("read back");
    assert_eq!(
        stored[0].design_export_hash.as_deref(),
        Some("sha256:aaa"),
        "a read must not refresh what it compares against"
    );
}

/// Re-declaring IS the acknowledgement — `dec:ask-not-repair` applied here:
/// name the remedy, never take it.
#[test]
fn re_declaring_takes_a_new_baseline_and_the_report_goes_quiet() {
    let mut g = graph();
    g.declare_dependency(&watched(Some("sha256:aaa")))
        .expect("declare");
    let moved = [seen("read", Some("sha256:bbb"), Some("beamline_sim"))];
    assert_eq!(kinds(&g, &moved), vec!["moved"]);

    g.declare_dependency(&watched(Some("sha256:bbb")))
        .expect("re-declare");
    assert_eq!(kinds(&g, &moved), vec!["unchanged"]);
}

#[test]
fn a_readable_export_nobody_has_ever_recorded_is_never_seen_not_unchanged() {
    let mut g = graph();
    g.declare_dependency(&watched(None)).expect("declare");
    let report = g
        .reconcile_upstream(&[seen("read", Some("sha256:aaa"), Some("beamline_sim"))])
        .expect("reconcile");
    assert_eq!(report.findings[0].kind, "never_seen");
    // Not actionable — it says nobody has LOOKED, which is a fact about this
    // design's own record rather than about the upstream.
    assert!(!report.findings[0].is_actionable());
}

#[test]
fn a_pointer_at_nothing_and_a_pointer_at_rubbish_are_different_findings() {
    let mut g = graph();
    g.declare_dependency(&watched(Some("sha256:aaa")))
        .expect("declare");
    assert_eq!(kinds(&g, &[seen("missing", None, None)]), vec!["missing"]);
    assert_eq!(
        kinds(&g, &[seen("unreadable", None, None)]),
        vec!["unreadable"]
    );
}

/// A path pointing at the WRONG design is worth more than a moved hash: left
/// alone, the two would be compared forever and always disagree, so every read
/// would report movement that is really a mis-wiring.
#[test]
fn an_export_belonging_to_a_different_design_is_named_as_that_and_not_as_movement() {
    let mut g = graph();
    g.declare_dependency(&watched(Some("sha256:aaa")))
        .expect("declare");
    let report = g
        .reconcile_upstream(&[seen("read", Some("sha256:bbb"), Some("vision_rig"))])
        .expect("reconcile");
    assert_eq!(report.findings[0].kind, "graph_id_mismatch");
    assert!(report.findings[0].detail.contains("vision_rig"));
    assert!(report.findings[0].detail.contains("beamline_sim"));
}

/// SILENCE IS REPORTED, NEVER ASSUMED. A dependency that names another design
/// and gives nothing to watch would otherwise be indistinguishable from one
/// that was checked and found fine.
#[test]
fn a_declared_design_with_no_export_to_watch_is_reported_not_skipped() {
    let mut g = graph();
    let mut d = watched(None);
    d.design_export = None;
    g.declare_dependency(&d).expect("declare");
    let report = g.reconcile_upstream(&[]).expect("reconcile");
    assert_eq!(report.findings[0].kind, "not_watched");
    assert_eq!(report.designs_declared, 1);
    assert_eq!(report.watched, 0);
    assert!(
        report.note.contains("NONE carries"),
        "a report silent for want of a target must say so: {}",
        report.note
    );
}

/// The same rule from the other end: a target the caller did not open must not
/// come back as agreement.
#[test]
fn a_watched_target_nobody_looked_at_is_not_observed_rather_than_unchanged() {
    let mut g = graph();
    g.declare_dependency(&watched(Some("sha256:aaa")))
        .expect("declare");
    let report = g.reconcile_upstream(&[]).expect("reconcile");
    assert_eq!(report.findings[0].kind, "not_observed");
    assert!(!report.findings[0].is_actionable());
}

/// An ordinary code dependency — no design, no export — says nothing here. The
/// report is about designs, and a crate pin appearing in it as a finding would
/// make the list unreadable on any real project.
#[test]
fn a_plain_build_dependency_produces_no_upstream_finding() {
    let mut g = graph();
    g.declare_dependency(&DependencyDeclaration {
        id: "dep:serde".into(),
        name: "serde".into(),
        source: "crates.io".into(),
        version: "1.0".into(),
        components: vec![],
        features: vec![],
        declared_in: Some("Cargo.toml".into()),
        graph_id: None,
        design_export: None,
        design_export_hash: None,
        design_export_seen_at: None,
        note: None,
    })
    .expect("declare");
    let report = g.reconcile_upstream(&[]).expect("reconcile");
    assert!(report.findings.is_empty());
    assert_eq!(report.designs_declared, 0);
}

#[test]
fn declaring_nothing_says_nobody_has_said_rather_than_nothing_moved() {
    let g = graph();
    let report = g.reconcile_upstream(&[]).expect("reconcile");
    assert!(report.findings.is_empty());
    assert!(
        report.note.contains("nobody has said"),
        "an empty watch list must not read as a clean bill: {}",
        report.note
    );
}

#[test]
fn the_targets_a_caller_should_open_come_from_the_committed_manifest() {
    let mut g = graph();
    g.declare_dependency(&watched(Some("sha256:aaa")))
        .expect("declare");
    let targets = g.upstream_targets().expect("targets");
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].graph_id.as_deref(), Some("beamline_sim"));
    assert_eq!(
        targets[0].design_export,
        "/somewhere/sim/docs/design/reflow2.json"
    );
    assert_eq!(targets[0].design_export_hash.as_deref(), Some("sha256:aaa"));
}

#[test]
fn the_watch_pointer_and_its_baseline_survive_into_the_manifest() {
    let mut g = graph();
    g.declare_dependency(&watched(Some("sha256:aaa")))
        .expect("declare");
    let toml = g.dependency_manifest().expect("manifest");
    assert!(toml.contains("design_export = \"/somewhere/sim/docs/design/reflow2.json\""));
    assert!(toml.contains("design_export_hash = \"sha256:aaa\""));
    assert!(toml.contains("design_export_seen_at = \"2026-08-20\""));
}

/// A manifest that always emitted the fields would make "not watched" and
/// "watched but never looked at" look identical to a person reading the diff.
#[test]
fn an_unwatched_dependency_emits_no_watch_fields_at_all() {
    let mut g = graph();
    let mut d = watched(None);
    d.design_export = None;
    g.declare_dependency(&d).expect("declare");
    let toml = g.dependency_manifest().expect("manifest");
    assert!(!toml.contains("design_export"));
}
