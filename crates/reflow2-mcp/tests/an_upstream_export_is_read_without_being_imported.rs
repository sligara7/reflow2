//! The READING half of watching an upstream design: what is actually on disk.
//!
//! The judgement lives in the core and is pinned by
//! `an_upstream_design_is_watched_not_imported.rs`. These cover the half that
//! only a filesystem can answer — and the one property the whole feature turns
//! on, which is that reading another design's record must not take it in.

use reflow2_core::{DependencyDeclaration, DesignGraph, UpstreamTarget};
use reflow2_mcp::upstream;

/// House pattern (design_identity.rs, latent_mode.rs, the export tests): a
/// pid-and-name-scoped directory under the system temp dir, so tests do not
/// collide and no dev-dependency is added for a handful of paths.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("reflow2-upstream-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Write a real export of a throwaway design, and return its path.
fn upstream_export(dir: &std::path::Path, graph_id: &str, extra_node: Option<&str>) -> String {
    let mut g = DesignGraph::open_in_memory_as(graph_id).expect("graph");
    g.add_component("cmp:thing", "thing", "a part of the upstream design", None)
        .expect("component");
    if let Some(id) = extra_node {
        g.add_component(id, "later", "work the upstream did afterwards", None)
            .expect("component");
    }
    let doc = g.export_graph().expect("export");
    let path = dir.join(format!("{graph_id}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&doc).expect("json")).expect("write");
    path.to_string_lossy().into_owned()
}

fn target(path: &str, graph_id: &str, baseline: Option<&str>) -> UpstreamTarget {
    UpstreamTarget {
        id: "dep:sim".into(),
        name: "beamline-sim".into(),
        design_export: path.into(),
        graph_id: Some(graph_id.into()),
        design_export_hash: baseline.map(str::to_string),
    }
}

#[test]
fn a_real_export_is_read_and_names_the_design_it_belongs_to() {
    let dir = scratch("read-names-design");
    let path = upstream_export(dir.as_path(), "beamline_sim", None);

    let (observed, not_read) = upstream::observe_upstreams(&[target(&path, "beamline_sim", None)]);
    assert!(not_read.is_none());
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].state, "read");
    assert_eq!(observed[0].graph_id.as_deref(), Some("beamline_sim"));
    assert!(observed[0].content_hash.is_some());
    assert!(observed[0].nodes.unwrap() > 0);
}

/// ⭐ THE PROPERTY THE FEATURE EXISTS FOR. The obvious way to watch a design is
/// to import it — and an import into a store that already holds a design keeps
/// the HOST'S name and upserts the incoming nodes into it, so watching that way
/// swallows the thing being watched. Reading must leave both sides alone.
#[test]
fn reading_an_upstream_does_not_pull_a_single_node_into_this_design() {
    let dir = scratch("no-import");
    let path = upstream_export(dir.as_path(), "beamline_sim", Some("cmp:sim-only"));

    let mut mine = DesignGraph::open_in_memory_as("mine").expect("graph");
    mine.declare_external_dependency(&DependencyDeclaration {
        id: "dep:sim".into(),
        name: "beamline-sim".into(),
        source: path.clone(),
        version: "v2.1.0".into(),
        components: vec![],
        features: vec![],
        declared_in: None,
        graph_id: Some("beamline_sim".into()),
        design_export: Some(path.clone()),
        design_export_hash: upstream::baseline_hash(&path),
        design_export_seen_at: Some("2026-08-26".into()),
        note: None,
    })
    .expect("declare");

    let before = mine.count_all_nodes().expect("count");
    let targets = mine.upstream_targets().expect("targets");
    let (observed, _) = upstream::observe_upstreams(&targets);
    let report = mine.reconcile_upstream(&observed).expect("reconcile");

    assert_eq!(report.findings[0].kind, "unchanged");
    assert_eq!(
        mine.count_all_nodes().expect("count"),
        before,
        "watching an upstream must not take a single node from it"
    );
    assert!(
        mine.get_node("Component", "cmp:sim-only")
            .expect("lookup")
            .is_none(),
        "a node that exists only in the upstream must not appear here"
    );
    // And this design is still itself — the failure an import would produce is
    // subtler than extra nodes: a store that already holds a design keeps its
    // own name, so the give-away is the upstream's content arriving under it.
    assert_eq!(mine.graph_id(), "mine");
}

/// The baseline is taken at declaration time and a read must never move it, so
/// the same movement keeps being reported until somebody acts.
#[test]
fn a_real_change_upstream_is_seen_and_keeps_being_seen() {
    let dir = scratch("keeps-being-seen");
    let path = upstream_export(dir.as_path(), "beamline_sim", None);
    let baseline = upstream::baseline_hash(&path).expect("hash");

    // The upstream exports again, with more in it.
    let _ = upstream_export(dir.as_path(), "beamline_sim", Some("cmp:new-work"));

    let mut mine = DesignGraph::open_in_memory_as("mine").expect("graph");
    mine.declare_external_dependency(&DependencyDeclaration {
        id: "dep:sim".into(),
        name: "beamline-sim".into(),
        source: path.clone(),
        version: "v2.1.0".into(),
        components: vec![],
        features: vec![],
        declared_in: None,
        graph_id: Some("beamline_sim".into()),
        design_export: Some(path.clone()),
        design_export_hash: Some(baseline),
        design_export_seen_at: Some("2026-08-26".into()),
        note: None,
    })
    .expect("declare");

    for _ in 0..2 {
        let targets = mine.upstream_targets().expect("targets");
        let (observed, _) = upstream::observe_upstreams(&targets);
        let report = mine.reconcile_upstream(&observed).expect("reconcile");
        assert_eq!(report.findings[0].kind, "moved");
    }
}

/// ⚠️ COMPUTED, NEVER the hash the document states about itself. A record edited
/// by anything but `export_graph` — a merge, a hand-fix — keeps its old stamp,
/// and trusting it is the defect `sync_debt` already had to fix once. Here that
/// would mean an upstream someone hand-edited reads as unmoved.
#[test]
fn a_stale_self_stamp_does_not_hide_a_real_change() {
    let dir = scratch("stale-stamp");
    let path = upstream_export(dir.as_path(), "beamline_sim", None);
    let baseline = upstream::baseline_hash(&path).expect("hash");

    // Append a node by hand WITHOUT restamping — exactly the shape a merge
    // produces, and the case that fooled the earlier implementation.
    let raw = std::fs::read_to_string(&path).expect("read");
    let mut doc: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    let stamp_before = doc.get("content_hash").cloned();
    doc["nodes"].as_array_mut().expect("nodes").push(
        serde_json::json!({"node_id": "cmp:hand-added", "node_type": "Component", "properties": {}}),
    );
    std::fs::write(&path, serde_json::to_string_pretty(&doc).expect("json")).expect("write");
    assert_eq!(
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|r| serde_json::from_str::<serde_json::Value>(&r).ok())
            .and_then(|d| d.get("content_hash").cloned()),
        stamp_before,
        "the point of this test is that the file's own stamp did NOT move"
    );

    let (observed, _) = upstream::observe_upstreams(&[target(&path, "beamline_sim", None)]);
    assert_ne!(
        observed[0].content_hash.as_deref(),
        Some(baseline.as_str()),
        "the hash must be computed from content, not read off the document"
    );
}

#[test]
fn a_pointer_at_nothing_and_a_pointer_at_rubbish_are_told_apart() {
    let dir = scratch("missing-vs-junk");
    let junk = dir.as_path().join("not-an-export.json");
    std::fs::write(&junk, "this is not JSON at all").expect("write");

    let (observed, _) = upstream::observe_upstreams(&[
        target("/no/such/path/reflow2.json", "beamline_sim", None),
        UpstreamTarget {
            id: "dep:junk".into(),
            ..target(&junk.to_string_lossy(), "beamline_sim", None)
        },
    ]);
    assert_eq!(observed[0].state, "missing");
    assert_eq!(observed[1].state, "unreadable");
}

/// A declaration whose pointer resolves to nothing must still be RECORDABLE —
/// the upstream may not have exported yet, which the hxm_program report measured
/// as the normal state (zero of seven siblings had one). It becomes a finding on
/// the next read rather than a wall at declaration time.
#[test]
fn declaring_a_watch_on_an_export_that_does_not_exist_yet_is_allowed() {
    assert!(upstream::baseline_hash("/no/such/path/reflow2.json").is_none());

    let mut mine = DesignGraph::open_in_memory_as("mine").expect("graph");
    mine.declare_external_dependency(&DependencyDeclaration {
        id: "dep:sim".into(),
        name: "beamline-sim".into(),
        source: "https://github.com/example/beamline-sim.git".into(),
        version: "v2.1.0".into(),
        components: vec![],
        features: vec![],
        declared_in: None,
        graph_id: Some("beamline_sim".into()),
        design_export: Some("/no/such/path/reflow2.json".into()),
        design_export_hash: None,
        design_export_seen_at: None,
        note: None,
    })
    .expect("a watch on an export nobody has written yet must be declarable");

    let targets = mine.upstream_targets().expect("targets");
    let (observed, _) = upstream::observe_upstreams(&targets);
    let report = mine.reconcile_upstream(&observed).expect("reconcile");
    assert_eq!(report.findings[0].kind, "missing");
    assert!(report.findings[0].is_actionable());
}

/// A bounded pass must NAME what it did not open. A pass that quietly checks
/// fewer targets than it knows about reads exactly like one that found
/// everything fine.
#[test]
fn a_bounded_pass_says_what_it_left_unopened() {
    let dir = scratch("bounded");
    let path = upstream_export(dir.as_path(), "beamline_sim", None);
    let many: Vec<UpstreamTarget> = (0..upstream::MAX_UPSTREAMS_READ + 3)
        .map(|i| UpstreamTarget {
            id: format!("dep:{i}"),
            name: format!("sibling-{i}"),
            ..target(&path, "beamline_sim", None)
        })
        .collect();

    let (observed, not_read) = upstream::observe_upstreams(&many);
    assert_eq!(observed.len(), upstream::MAX_UPSTREAMS_READ);
    let skipped = not_read.expect("a bounded pass must say what it skipped");
    assert_eq!(skipped.count, 3);
    assert_eq!(skipped.dependencies.len(), 3);
    assert!(skipped.note.contains("NOT opened"));
}
