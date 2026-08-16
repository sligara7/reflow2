//! COVERAGE (BL-95) — what the design was never told about.
//!
//! The failure being closed: every other detector reasons about nodes already
//! in the graph, so a design covering 30% of a system reports the same "0 open
//! gaps" as one covering all of it.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;
use reflow2_core::{LinkArtifactOptions, ObservedPath};

fn artifact(g: &mut DesignGraph, id: &str, location: &str) {
    g.link_artifact(LinkArtifactOptions {
        artifact_id: id.into(),
        name: id.into(),
        location: Some(location.into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:a".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })
    .unwrap();
}

fn obs(path: &str, mass: u64) -> ObservedPath {
    ObservedPath {
        path: path.to_string(),
        mass,
    }
}

/// A design that models one file and one opaque directory.
fn designed() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:x", "X").unwrap();
    g.add_capability("cap:a", "A", "does a", None).unwrap();
    artifact(&mut g, "art:one", "src/one.rs");
    g
}

#[test]
fn an_unmodelled_subtree_is_reported_and_a_modelled_file_is_not() {
    let g = designed();
    let r = g
        .coverage_report(
            &[
                obs("src/one.rs", 100),
                obs("src/deep/a.rs", 50),
                obs("src/deep/b.rs", 70),
            ],
            &[],
            Some("2026-07-27"),
        )
        .unwrap();

    assert_eq!(r.claimed, 1);
    assert_eq!(r.unclaimed, 2);
    assert_eq!(r.unclaimed_regions.len(), 1, "{:?}", r.unclaimed_regions);
    let region = &r.unclaimed_regions[0];
    assert_eq!(
        region.path, "src/deep",
        "rolled up to the parent, not per-file"
    );
    assert_eq!(region.paths, 2);
    assert_eq!(region.mass, 120);
    assert_eq!(r.swept_at.as_deref(), Some("2026-07-27"));
}

/// **The trap BL-95 exists to avoid.** A file-count ratio would punish exactly
/// the modelling the `adopt` skill mandates: a vendored mass correctly recorded
/// as ONE opaque artifact would score as 1-of-900 covered and be reported as a
/// failure. Claiming a directory claims what is under it.
#[test]
fn one_opaque_artifact_legitimately_claims_the_mass_beneath_it() {
    let mut g = designed();
    artifact(&mut g, "art:vendor", "third_party");

    let mut sweep = vec![obs("src/one.rs", 10)];
    for i in 0..900 {
        sweep.push(obs(&format!("third_party/lib/f{i}.c"), 1000));
    }
    let r = g.coverage_report(&sweep, &[], None).unwrap();

    assert_eq!(r.unclaimed, 0, "the opaque claim covers its subtree");
    assert!(
        r.unclaimed_regions.is_empty(),
        "correct coarse modelling must not be reported as a hole: {:?}",
        r.unclaimed_regions
    );
    assert_eq!(r.claimed, 901);
}

/// A prefix that is not a directory boundary must not claim: `src/one` must not
/// swallow `src/onetwo.rs`.
#[test]
fn a_partial_name_match_does_not_claim() {
    let g = designed();
    let r = g
        .coverage_report(
            &[obs("src/one.rs", 1), obs("src/one_helper.rs", 1)],
            &[],
            None,
        )
        .unwrap();
    assert_eq!(r.claimed, 1);
    assert_eq!(
        r.unclaimed, 1,
        "a sibling with a shared prefix is NOT covered"
    );
}

/// Rule 6: what was left out is named, never silently dropped. "We ignored the
/// build output" and "the build output is covered" must never look alike.
#[test]
fn exclusions_are_named_not_silently_dropped() {
    let g = designed();
    let r = g
        .coverage_report(
            &[
                obs("src/one.rs", 1),
                obs("target/debug/x", 999),
                obs("src/new.rs", 5),
            ],
            &["target".to_string()],
            None,
        )
        .unwrap();

    assert_eq!(r.excluded.len(), 1);
    assert_eq!(r.excluded[0].path, "target/debug/x");
    assert_eq!(r.excluded[0].excluded_by, "target", "the RULE is named too");
    assert_eq!(r.observed, 2, "excluded paths are not counted as observed");
    assert_eq!(
        r.unclaimed_mass, 5,
        "and an excluded file's mass must not inflate the silence"
    );
}

/// Biggest silence first — a list ordered by path would bury the thing that
/// matters under whatever sorts early.
#[test]
fn regions_rank_by_mass() {
    let g = designed();
    let r = g
        .coverage_report(
            &[
                obs("aaa/small.rs", 1),
                obs("zzz/huge.rs", 10_000),
                obs("mmm/mid.rs", 500),
            ],
            &[],
            None,
        )
        .unwrap();
    let order: Vec<&str> = r
        .unclaimed_regions
        .iter()
        .map(|x| x.path.as_str())
        .collect();
    assert_eq!(order, vec!["zzz", "mmm", "aaa"]);
}

/// A registered artifact the sweep never mentioned is named, so the caller can
/// tell "my sweep was narrower than the design" from "the file is gone" —
/// the second being `reconcile_artifacts`' question, not this one's.
#[test]
fn a_registered_location_the_sweep_missed_is_named() {
    let g = designed();
    let r = g
        .coverage_report(&[obs("src/other.rs", 1)], &[], None)
        .unwrap();
    assert_eq!(r.unobserved_locations, vec!["src/one.rs".to_string()]);
}

/// An empty sweep is a coherent answer, not a crash — and it must not read as
/// full coverage.
#[test]
fn an_empty_sweep_claims_nothing() {
    let g = designed();
    let r = g.coverage_report(&[], &[], None).unwrap();
    assert_eq!(r.observed, 0);
    assert_eq!(r.claimed, 0);
    assert!(r.unclaimed_regions.is_empty());
    assert_eq!(r.unobserved_locations, vec!["src/one.rs".to_string()]);
    assert!(r.swept_at.is_none(), "an undated sweep says so");
}
