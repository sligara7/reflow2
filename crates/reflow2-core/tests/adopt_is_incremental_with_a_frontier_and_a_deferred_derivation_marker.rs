//! The two primitives of an incremental, resumable adopt: a deferred-derivation
//! marker that quiets the intent findings on its subject while the boundary
//! lists it as owed, and a frontier read that says what is captured without
//! intent, what was deferred, what is adjacent and uncaptured, and where to
//! resume
//! (`req:adopt-can-be-incremental-and-resumable-with-a-frontier-and-a-deferred-derivation-marker`).
//!
//! The cases that carry the weight: the marker never makes a question
//! disappear (it moves it to the boundary), closing the marker brings the
//! finding back, and a frontier with no sweep says it does not know what is
//! uncaptured rather than reading clean.

use reflow2_core::coverage::ObservedPath;
use reflow2_core::frontier::DEFERRED_DERIVATION;
use reflow2_core::nodes::{Props, node};
use reflow2_core::{DesignGraph, GapSource};

/// A region pass: one requirement met, one capability nobody asked for, one
/// leaf component nothing is allocated to.
fn region() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:scan", "Scan a sample", "The beamline scans a sample.")
        .unwrap();
    g.set_requirement_status("req:scan", "accepted").unwrap();
    g.add_capability("cap:scan", "Scan", "runs a scan", Some("realized"))
        .unwrap();
    g.satisfies("cap:scan", "req:scan").unwrap();
    g.add_component("cmp:scanner", "Scanner", "does the scanning", None)
        .unwrap();
    g.allocate("cap:scan", "cmp:scanner").unwrap();
    // Captured from the code, intent not yet recovered.
    g.add_capability(
        "cap:archive",
        "Archive",
        "writes runs to the archive",
        Some("realized"),
    )
    .unwrap();
    g.add_component("cmp:archiver", "Archiver", "the archive writer", None)
        .unwrap();
    g
}

fn defer(g: &mut DesignGraph, id: &str, subject: &str, since: &str) {
    g.create_node(
        node::TEMPORAL_FACT,
        id,
        Props::new()
            .set("subject_id", subject)
            .set("fact_type", DEFERRED_DERIVATION)
            .set(
                "statement",
                format!("{subject}: structure captured, intent deferred to the next pass"),
            )
            .set("basis", "measured")
            .set("valid_from", since),
    )
    .unwrap();
}

fn has(g: &DesignGraph, source: GapSource, id: &str) -> bool {
    g.detect_gaps()
        .unwrap()
        .iter()
        .any(|x| x.gap_source == source && x.affected_ids.iter().any(|a| a == id))
}

#[test]
fn a_deferral_quiets_the_intent_finding_on_its_subject_and_the_boundary_lists_it_as_owed() {
    let mut g = region();
    assert!(
        has(&g, GapSource::UnmotivatedCapability, "cap:archive"),
        "before: asked as a gap"
    );
    assert!(has(&g, GapSource::UnallocatedComponent, "cmp:archiver"));

    defer(&mut g, "fact:defer-archive", "cap:archive", "2026-09-17");
    defer(&mut g, "fact:defer-archiver", "cmp:archiver", "2026-09-17");
    assert!(
        !has(&g, GapSource::UnmotivatedCapability, "cap:archive"),
        "deferred on purpose is not missing"
    );
    assert!(!has(&g, GapSource::UnallocatedComponent, "cmp:archiver"));

    let ls = g.loop_status().unwrap();
    assert_eq!(
        ls.deferrals_open, 2,
        "…but it is owed, and the boundary says so"
    );
    assert!(
        ls.next.iter().any(|l| l.contains("deferred")),
        "{:?}",
        ls.next
    );
}

#[test]
fn closing_the_marker_brings_the_finding_back() {
    let mut g = region();
    defer(&mut g, "fact:defer-archive", "cap:archive", "2026-09-17");
    assert!(!has(&g, GapSource::UnmotivatedCapability, "cap:archive"));
    // The pass came back and settled it — or lapsed.
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:defer-archive",
        Props::new()
            .set("subject_id", "cap:archive")
            .set("fact_type", DEFERRED_DERIVATION)
            .set(
                "statement",
                "cap:archive: structure captured, intent deferred to the next pass",
            )
            .set("basis", "measured")
            .set("valid_from", "2026-09-17")
            .set("valid_to", "2026-09-18"),
    )
    .unwrap();
    assert!(
        has(&g, GapSource::UnmotivatedCapability, "cap:archive"),
        "closed marker, open question"
    );
    assert_eq!(g.loop_status().unwrap().deferrals_open, 0);
}

#[test]
fn the_frontier_lists_structure_without_intent_the_deferred_and_the_resume_point() {
    let mut g = region();
    let f = g.frontier(None, &[]).unwrap();
    let ids: Vec<&str> = f
        .structure_without_intent
        .iter()
        .map(|i| i.node_id.as_str())
        .collect();
    assert_eq!(ids, vec!["cap:archive", "cmp:archiver"]);
    assert!(f.deferred.is_empty());
    assert!(f.resume_point.is_none());

    defer(&mut g, "fact:defer-archiver", "cmp:archiver", "2026-09-16");
    defer(&mut g, "fact:defer-archive", "cap:archive", "2026-09-17");
    let f = g.frontier(None, &[]).unwrap();
    assert!(
        f.structure_without_intent.is_empty(),
        "a deferred part is listed once, under deferred"
    );
    assert_eq!(f.deferred.len(), 2);
    assert_eq!(f.deferred[0].subject_id, "cmp:archiver", "oldest first");
    assert_eq!(
        f.resume_point.as_ref().map(|d| d.subject_id.as_str()),
        Some("cap:archive"),
        "you were here: the most recent deferral"
    );
}

#[test]
fn a_frontier_with_no_sweep_says_it_does_not_know_what_is_uncaptured() {
    let g = region();
    let f = g.frontier(None, &[]).unwrap();
    assert!(f.uncaptured.is_none(), "never an empty list");
    assert!(f.note.contains("NOT known"), "{}", f.note);

    let mut g = g;
    g.add_artifact(
        "art:scanner",
        "scanner.py",
        Some("code"),
        Some("src/scanner.py"),
    )
    .unwrap();
    let sweep = vec![
        ObservedPath {
            path: "src/scanner.py".into(),
            mass: 10,
        },
        ObservedPath {
            path: "src/archive/writer.py".into(),
            mass: 30,
        },
        ObservedPath {
            path: "src/archive/index.py".into(),
            mass: 20,
        },
    ];
    let f = g.frontier(Some(&sweep), &[]).unwrap();
    let regions = f.uncaptured.expect("a sweep was handed in");
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].path, "src/archive");
}
